use std::cell::RefCell;
use std::fs::{OpenOptions, remove_file, self};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64,Ordering};
use std::sync::{Arc,Mutex};
use std::time::SystemTime;
use dashmap::DashMap;
use log::{info,warn};
use std::{collections::HashMap, collections::hash_map, fs::File};
use std::io::{BufReader,Write, BufWriter, Seek, SeekFrom, self, Read, Take};
use serde_json;
use crate::KvsEngine;


struct CommandPos {
    offset: u64,
    length: u64,
    file_id: u64,
}

#[derive(Clone)]
pub struct KvStore {
    // key：String， vaule_metadata: CommandPos
    index: Arc<DashMap<String, CommandPos>>,
    current_readers: Reader,
    current_writer: Arc<Mutex<Writer>>,    
}

pub struct Writer {
    dir_path: Arc<PathBuf>,
    current_readers: Reader,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    size_for_compaction: u64,
    index: Arc<DashMap<String, CommandPos>>,
}

pub struct Reader {
    dir_path: Arc<PathBuf>,
    compaction_number: Arc<AtomicU64>,
    readers: RefCell<HashMap<u64, BufReader<File>>>,
}

impl Clone for Reader {
    fn clone(&self) -> Self {
        Reader {
            dir_path: Arc::clone(&self.dir_path),
            compaction_number: Arc::clone(&self.compaction_number),
            readers: RefCell::new(HashMap::new()),
        }
    }
}

const MAX_COMPACTION_SIZE: u64 = 1024; 
// BufWriterWithPos is a bufWriter and Position
// design for getting the write offset quickly instead of using seek()
// complete the Write trait for BufWriterwith Postion and write function as original write does not provide offset position
struct BufWriterWithPos<T>
where
    T : Write + Seek
{
    bufwriter: BufWriter<T>,
    position: u64,
}


use crate::KVStoreError;
use crate::Result;
//construction and locate func definition 
impl <T: Write + Seek> BufWriterWithPos<T> {
    //inherate the KVStoreError
    fn new(mut inner: T) -> Result<Self> {
        Ok(
            BufWriterWithPos {  
                //move the cursor 0 byte from the end of file
                //return the cursor postions Result<u64>
                position: inner.seek(SeekFrom::End(0))?,
                //create the writer buffer using T
                bufwriter: BufWriter::new(inner), 
            }
        )
    }

    fn get_position(&self) -> u64 {
        self.position
    }
}

//impl Writer trait for BufWriterWithPos so that it can use Writer's methods defined in std::io and fs lib
use crate::Command;
impl <T: Write + Seek> Write for BufWriterWithPos<T> {
    
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = self.bufwriter.write(buf)?; // return how many bytes written
        self.position += len as u64; //usize to u64
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.bufwriter.flush()
    }
}


impl KvStore {
    //read all validate the files in current dir to get the vector of sorted file_ids
    fn sorted_file_ids(path: &Arc<PathBuf>) -> Result<Vec<u64>> {
        //get the every filepath and dir in the directory
        let pathbuf_list = fs::read_dir(path.as_path())?
            .map(|res|res.map(|e|e.path()))
            .flatten();
        //filter filepath of all txt files
        //get the filenames
        //get the file_id from the filename
        let mut id_iter : Vec<u64> = pathbuf_list
            .filter(|path|path.is_file() && path.extension() == Some("txt".as_ref()))
            .flat_map(|pathbuf| {
                pathbuf.file_name()
                .and_then(|filename|filename.to_str())
                .map(
                    // remove the header and the end in data_{file_id}.txt
                    |filename| {
                        filename.trim_start_matches("data_")
                                .trim_end_matches(".txt")
                    }
                )
                .map(str::parse::<u64>) 
            }).flatten().collect();
            id_iter.sort();
            Ok(id_iter)
    }
    
    
    //main() calls open(env::current_dir()?) directly
    //env::current_dir()? -> PathBuf
    //open(parameter)：impl Into<PathBuf> trait, which means that para in open func must be transferred to PathBuf
   
    pub fn open(open_path: impl Into<PathBuf>) -> Result<KvStore> {
        let dir_path = Arc::new(open_path.into());
        
        fs::create_dir_all(&dir_path.as_path())?;

        let index =Arc::new(DashMap::new());
        let mut readers = HashMap::new();
        // how to get current_file_id and current compaction_size
        // Update index and current_reader，as they have file_id mapping
        // Traverse all existing logfiles
        let file_ids = KvStore::sorted_file_ids(&dir_path)?;
        let mut current_file_id= 0;
        
        if let Some(id) = file_ids.last() {
            current_file_id = *id;
        }
        
        let mut size_for_compaction = 0;
        /*
        * 1.recreate the currennt reader: file_id, bufreader
        * 2.recreate the index: key id, Cmdpos - offset + length + file_id
        */

        //1) When no logs on the disk，current_file_id is 0.
        //2) and now current_file_id is 0,file_ids vec is empty，this following block will be passed
        for id in file_ids {
            let file_path = dir_path.join(format!("data_{}.txt", id));
            //open the each file into bufreader
            let reader = BufReader::new(File::open(&file_path)?);
            //1.Update the reader list
            readers.insert(id, reader);
            
            //deserliaze the files on disk
            //split the command: into_iter to convert the deserialized commands to iter
            let mut des_iter = serde_json::Deserializer::from_reader(BufReader::new(File::open(&file_path)?)).into_iter::<Command>();
            
            let mut offset0 = des_iter.byte_offset() as u64;//bytes which have been deserialized
            
            while let Some(command) = des_iter.next() {
                let offset1 = des_iter.byte_offset() as u64;
                //length of each command
                let val_length = offset1 - offset0;
                
                match command? { 
                    Command::SET(key,_ ) => {
                        index.insert(key, 
                            CommandPos{
                                offset: offset0,
                                length: val_length,
                                file_id: id,
                            }
                        );
                        //TODO：if the key exist, + compaction size, if not NOT +
                        size_for_compaction += val_length;
                    }
                    Command::RM(key) => {
                        //set cmd length
                        let size_pre_setcmd = index.remove(&key).map(|(_,p)|p.length).unwrap_or(0);
                        size_for_compaction += size_pre_setcmd; 
                        
                        //rm cmd length
                        size_for_compaction += val_length;

                    }
                };
                offset0 = offset1;
            }
        }
   
        //To initialize current_writer, need to get the current_file_id firstly
        //writer must be opened using openoption append
        let current_file_path = dir_path.join(format!("data_{}.txt",current_file_id));

        let current_writer = BufWriterWithPos::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&current_file_path)?,
            )?;
        
            //3) once the loop has been passed, and current_writer has created a file named data_0.txt
            //4) update log file whose file_id == 0 in reader
            if current_file_id == 0 {
                    readers.insert(
                    current_file_id,
                    BufReader::new(File::open(&current_file_path)?),
                );
            }
        let current_readers = Reader {
            dir_path: Arc::clone(&dir_path),
            compaction_number: Arc::new(AtomicU64::new(0)),
            readers: RefCell::new(readers),
        };  

        let current_writer = Arc::new(Mutex::new(
            Writer {
                dir_path,
                current_readers: current_readers.clone(),
                current_writer,
                current_file_id,
                size_for_compaction,
                index:Arc::clone(&index),
            }
        ));
        
        let store = KvStore{
            index,
            current_readers,
            current_writer,
        };

        Ok(store)
    }
}

impl KvsEngine for KvStore {
    fn set(& self, key: String, value: String) -> Result<()> {
      self.current_writer.lock().unwrap().set(key, value)?;
      Ok(())
    }

    fn get(& self, key: String) -> Result<Option<String>> {
        if let Some (entry) = self.index.get(&key) {
            self.current_readers.read_command(entry.value())
        } else {
            Ok(None)
        }
    }
    fn remove(& self, key: String) -> Result<()> {
        self.current_writer.lock().unwrap().remove(key)?;
        Ok(())
    }
}

impl Writer {    
    fn set(&mut self, key: String, value: String) -> Result<()> {
        let this_command = Command::SET(key.clone(), value);
        //to vec as write_all receives a [u8] buf
        let serialized_command = serde_json::to_vec(&this_command)?; 
        //store the previous offset
        let offset0 = self.current_writer.get_position(); 
        //write to which file? -(1)
        //initialize the struct current writer
        self.current_writer.write_all(&serialized_command)?;
        self.current_writer.flush()?;

        // get the new offset
        let offset1 = self.current_writer.get_position();
        let length = offset1 - offset0;

        //update the index
        //key was supposed to have been moved
        self.index.insert(key, 
            CommandPos { 
                offset: offset0, 
                length: length, 
                file_id: self.current_file_id, 
            }
        );
        self.size_for_compaction += length;

        if self.size_for_compaction > MAX_COMPACTION_SIZE {
            let now = SystemTime::now();
            info!("Compaction starts");
            self.compact()?;
            info!("Compaction finished, costed {:?}", now.elapsed());
        }

        Ok(())
    }
   
    fn remove(&mut self, key: String) -> Result<()> {
    //hashmap get() returns an Option
    if self.index.get(&key).is_some() {
        //update the index
        //
        let setcod_len_tobe_destoryed = self.index.remove(&key).
            map(|(_,p)|p.length).unwrap_or(0);
        self.size_for_compaction += setcod_len_tobe_destoryed;
        
        //initialize the command Rm()
        let command = Command::RM(key);
        //get the current writer offset
        let offset0 = self.current_writer.get_position();
        //serialize the command
        let serialized_command = serde_json::to_vec(&command)?;
        //update the writer
        self.current_writer.write_all(&serialized_command)?;
        self.current_writer.flush()?;
        //pattern matching get the key
        
        self.size_for_compaction += self.current_writer.get_position() - offset0;
        
        if self.size_for_compaction > MAX_COMPACTION_SIZE {
            let now = SystemTime::now();
            info!("Compaction starts");
            self.compact()?;
            info!("Compaction finished, costed {:?}", now.elapsed());
        }

        Ok(())
        } else {
            Err(KVStoreError::KeyNotFound)
        }
    }

    fn compact(& mut self) -> Result<()> {
        self.create_new_file()?;
        //traverse the hashmap 
        let mut before_offset = 0;
        for mut entry in self.index.iter_mut() {
            //get the index entry into reader
            let position = entry.value_mut();
            let mut readmap = self.current_readers.readers.borrow_mut();
            let buf_reader = readmap.get_mut(&position.file_id).expect("can not find key in the memory...");

            buf_reader.seek(SeekFrom::Start(position.offset))?;
            let mut takebuf = buf_reader.take(position.length);
            //copy the reader to writer
            //let offset0_in_writer = self.current_writer.position;
            io::copy(&mut takebuf, &mut self.current_writer)?;
            
            let ofset1_in_writer = self.current_writer.position;
            
            //update the index: key -> value, as value pos has been changed
            *position = CommandPos {
                //offset : offset0_in_writer,
                offset : before_offset,
                length : ofset1_in_writer - before_offset,
                file_id : self.current_file_id,
            };  
            before_offset = ofset1_in_writer; 
        }
        self.current_writer.flush()?;
        
        // let file_arr: Vec<u64> = self.current_reader.keys().filter(|&&k| k < self.current_file_id).cloned().collect();
        // for file_id in file_arr  {
        //         self.current_reader.remove(&file_id);
        //         let file_path = self.dir_path.join(format!("data_{}.txt",file_id));
        //         remove_file(file_path)?;
        // }
        
        //Writer中的current_readers.compaction_number存入当前最新file id
        self.current_readers.compaction_number.store(self.current_file_id, Ordering::SeqCst);
        //删除Writer中的current_readers中旧的<file, BufReader<File>>
        self.current_readers.remove_useless_reader_in_writer(self.current_file_id)?;
        self.size_for_compaction = 0;
        //Writer创建一个新文件
        self.create_new_file()?; 
        Ok(())
    }

    fn create_new_file(& mut self) -> Result<()> {

        self.current_file_id += 1;
        //dir_path is the current execution path to be joined to create the absolute path
        //build the new file path based on dir_path and current file id
        let new_file_path = self.dir_path.join(format!("data_{}.txt", self.current_file_id));

        //OpenOptions for opening the new file
        let new_file = OpenOptions::new()
        .create(true) //(2) create_new(true)当存在就失败
        //.write(true)
        .append(true) //if the file exists, then append data to the file
        .open(&new_file_path)?;

        //update the current_writer with the newest file handle
        self.current_writer = BufWriterWithPos::new(new_file)?;

        //update the current_reader by inserting <the newest file_id, Bufreader> 
        self.current_readers.readers.borrow_mut().insert(self.current_file_id, BufReader::new(File::open(&new_file_path)?));

        Ok(())
    }
}

impl Reader {
    fn read_command(&self, postion: &CommandPos) -> Result<Option<String>> {
        self.read_add(postion, |data_reader| {
            if let Command::SET(_, value) = serde_json::from_reader(data_reader)? {
                Ok(Some(value))
            } else {
                Err(KVStoreError::UnknownCommandType)
            }
        })
    }

    fn read_add<F,R>(&self, postion: &CommandPos, f: F) -> Result<R> 
    where
        F: FnOnce(Take<&mut BufReader<File>>) -> Result<R>
    {
        self.try_to_remove_stale_readers_in_reader();

        let mut readers = self.readers.borrow_mut();

        //check the if the file handle exists
        if let hash_map::Entry::Vacant(entry) = readers.entry(postion.file_id) {
            let new_reader = BufReader::new(File::open(self.dir_path.join(format!("data_{}.txt", postion.file_id))
            )?);
            entry.insert(new_reader);
        }
        //locate the commad position
        let source_reader = readers.get_mut(&postion.file_id).expect("Can not find key in files but it is in memory");

        source_reader.seek(SeekFrom::Start(postion.offset))?;
        let data_reader = source_reader.take(postion.length as u64);
        
        f(data_reader)
    }

    //删除小于file_number的所有文件在writer中
    fn remove_useless_reader_in_writer(&mut self, file_number: u64) -> Result<()> {
        let mut readers = self.readers.borrow_mut();
        
        let deleted_file_numbers: Vec<u64> = readers
            .iter()
            .map(|(key,_)|*key)
            .filter(|key|*key < file_number)
            .collect();
        
        for number in deleted_file_numbers {
            
            //remove the readers <HashMap<file_id, bufread>> maintained in Writers 
            readers.remove(&number);
            //delete those files older than compaction_number
            let file_path = self.dir_path.join(format!("data_{}.txt", number));
            if let Err(e) = remove_file(&file_path) {
                warn!("can not delete file {:?} because {}", file_path, e)
            }
        }
        Ok(())
    }

    fn try_to_remove_stale_readers_in_reader(&self) {
        let compaction_number = self.compaction_number.load(Ordering::SeqCst);
        let mut readers = self.readers.borrow_mut();
        //delete the readers older than compaction_number
        readers.retain(|&k, _|k >= compaction_number);
    }
}