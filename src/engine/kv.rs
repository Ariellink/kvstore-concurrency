use std::fs::{OpenOptions, remove_file, self};
use std::path::PathBuf;
use std::{collections::HashMap,fs::File};
use std::io::{BufReader,Write, BufWriter, Seek, SeekFrom, self, Read};
use serde_json;
use crate::KvsEngine;
struct CommandPos {
    offset: u64,
    length: u64,
    file_id: u64,
}

pub struct KvStore {
    // key：String， vaule_metadata: CommandPos
    index: HashMap<String, CommandPos>,
    current_reader: HashMap<u64,BufReader<File>>,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    dir_path: PathBuf,
    size_for_compaction: u64,
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
    fn sorted_file_ids(path: &PathBuf) -> Result<Vec<u64>> {
        //get the every filepath and dir in the directory
        let pathbuf_list = fs::read_dir(path)?
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
        let dir_path = open_path.into();
        
        fs::create_dir_all(&dir_path)?;

        let mut index = HashMap::new();
        let mut current_reader = HashMap::new();
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
            current_reader.insert(id, reader);
            
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
                        size_for_compaction += val_length;
                    }
                    Command::RM(key) => {
                        //set cmd length
                        let size_pre_setcmd = index.remove(&key).map(|p|p.length).unwrap_or(0);
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
                current_reader.insert(
                    current_file_id,
                    BufReader::new(File::open(&current_file_path)?),
                );
            }

        let mut store = KvStore{
            index,
            current_reader,
            current_writer,
            current_file_id,
            dir_path,
            size_for_compaction,
        };

        if store.size_for_compaction > MAX_COMPACTION_SIZE {
            store.compact()?;
        }

        Ok(store)
    
    }

    fn compact(& mut self) -> Result<()> {
        self.create_new_file()?;
        //traverse the hashmap 
        let mut before_offset = 0;
        for each_cmdpos in self.index.values_mut() {
            //get the index entry into reader
            let buf_reader = self.current_reader.get_mut(&each_cmdpos.file_id).expect("can not find key in the memory...");
            
            buf_reader.seek(SeekFrom::Start(each_cmdpos.offset))?;
            let mut takebuf = buf_reader.take(each_cmdpos.length);
            //copy the reader to writer
            //let offset0_in_writer = self.current_writer.position;
            io::copy(&mut takebuf, &mut self.current_writer)?;
            
            let ofset1_in_writer = self.current_writer.position;
            
            //update the index: key -> value, as value pos has been changed
            *each_cmdpos = CommandPos {
                //offset : offset0_in_writer,
                offset : before_offset,
                length : ofset1_in_writer - before_offset,
                file_id : self.current_file_id,
            };  
            before_offset = ofset1_in_writer; 
        }
        self.current_writer.flush()?;
        
        let file_arr: Vec<u64> = self.current_reader.keys().filter(|&&k| k < self.current_file_id).cloned().collect();
        for file_id in file_arr  {
                self.current_reader.remove(&file_id);
                let file_path = self.dir_path.join(format!("data_{}.txt",file_id));
                remove_file(file_path)?;
        }
        self.size_for_compaction = 0;
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
        self.current_reader.insert(self.current_file_id, BufReader::new(File::open(&new_file_path)?));

        Ok(())
    }
}

impl KvsEngine for KvStore {    
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

        /* another completion Q -(2) */
        // if let Command::Set(_key,_) = this_command {
        //     let increased_length = self.index.insert(_key, 
        //         CommandPos { 
        //             offset: offset0, 
        //             length: length, 
        //             file_id: self.current_file_id, 
        //         }
        //     ).unwrap().length;
        //     self.size_for_compaction += increased_length;
        // } 

        if self.size_for_compaction > MAX_COMPACTION_SIZE {
            self.compact()?;
        }

        Ok(())
    }

    fn get(&mut self, key: String) -> Result<Option<String>> {
        //find the commandPos in the index
        //pattern match release the value from option<vaule> returned by hashmap.get
        if let Some(cmdpos) = self.index.get(&key) {
            let file_id = cmdpos.file_id;
            //open file 
            //update current_reader to a new file
            let bufreader_in_file = self.current_reader.get_mut(&file_id).expect("cannot find the key in in-memory index");
            
            //locate the cursor to the offset + length by Seek
            //locate the self.bufreader's cursor into the value pos
            bufreader_in_file.seek(SeekFrom::Start(cmdpos.offset))?;
            let data_read = bufreader_in_file.take(cmdpos.length);

            //desearlize 
            //get the value from Command struct
            if let Command::SET(_,value) = serde_json::from_reader(data_read)? {
                Ok(Some(value))
            } else{
                Err(KVStoreError::UnknownCommandType) }
        } else {
            Ok(None) 
        }
    }

    fn remove(&mut self, key: String) -> Result<()> {
    //hashmap get() returns an Option
    if self.index.get(&key).is_some() {
        //update the index
        //
        let setcod_len_tobe_destoryed = self.index.remove(&key).
            map(|p|p.length).unwrap_or(0);
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
            self.compact()?;
        }
            Ok(())
        } else {
            Err(KVStoreError::KeyNotFound)
        }
    }
}