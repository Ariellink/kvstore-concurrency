# Cpmaction workflow

compact始终持有writer的锁。

```rust
impl Writer {  
    fn compact(& mut self) -> Result<()> {}
}
```

TODO:  
1. 创建一个空文件
2. read index 第一个k:v
3. 根据v (`CommmandPos`), 在writer维护的current_reader.readers (`RefCell<HashMap<u64, BufReader<File>>>`)中找到 k = CommandPos.file_id对应的bufreader  
    3.1 因为 writer中单独维护的readers很可能是空的。因为没有打开CommandPos.file_id的 bufreader。（WHY? 因为Writer中更新reader的方法只有在compact的过程中，所以第一个文件是未打开的）  
    3.2 我们要先更新writer中的bufreader
------
首先看下`Reader::read_add()`方法的设计：  
    1. 传入参数 commandpos, 和一个闭包  
    2. 检查commandpos.file_id在current_reader.readers (`RefCell<HashMap<u64, BufReader<File>>>`)是否打开, 没打开就打开  
    3. 拿到command所在文件的readerbuf  
    4. 用seek和take取出这个command的readerbuf  
    5. 使用f处理这个readerbuf    
    
```rust
    fn read_add<F,R>(&self, postion: &CommandPos, f: F) -> Result<R> 
    where
        F: FnOnce(Take<&mut BufReader<File>>) -> Result<R>
    {
        self.try_to_remove_stale_readers_in_reader();

        let mut readers = self.readers.borrow_mut();
        //check if reader exists, if not, open it
        if let Entry::Vacant(entry) = readers.entry(postion.file_id) {
            let new_reader = BufReader::new(File::open(
                &self
                        .dir_path
                        .join(format!("data_{}.txt", postion.file_id)),
            )?);
            entry.insert(new_reader);
        }
        //locates the Bufreader
        let source_reader = readers.get_mut(&postion.file_id).expect("can not find key in files in opened readers during locating commandpos");
        //locates the commandpos start position
        source_reader.seek(SeekFrom::Start(postion.offset))?;
        //get the readerbuf of this command by taken to its length
        let data_reader = source_reader.take(postion.length as u64);
        //handle this command reader buffer
        f(data_reader)
    }
    ```
5. 拿到这个command的readbuf后要将它拷贝到writer中
```rust
fn copy_data_to_writer(
        &self,
        position: &CommandPos,
        writer: &mut BufWriterWithPos<File>,
    ) -> Result<()> {
        self.read_add(position, |mut data_reader| {
            io::copy(&mut data_reader, writer)?;
            Ok(())
        })
    }
```

## 修改后的compact
```rust
fn compact(& mut self) -> Result<()> {
        self.create_new_file()?;
        //traverse the hashmap 
        let mut before_offset = 0;
        for mut entry in self.index.iter_mut() {
            //get the index entry into reader
            let position = entry.value_mut();

            //self.current_readers.copy_data_to_writer(position, &mut self.current_writer)?;
            self.current_readers.read_add(position, |mut databuf| {
                io::copy(&mut databuf, &mut self.current_writer)?;
                Ok(())
            })?;

            let offset1_in_writer = self.current_writer.position;
            //update the index: key -> value, as value pos has been changed
            *position = CommandPos {
                //offset : offset0_in_writer,
                offset : before_offset,
                length : offset1_in_writer - before_offset,
                file_id : self.current_file_id,
            };  
            before_offset = offset1_in_writer; 
        }
        self.current_writer.flush()?;
                
        //Writer中的current_readers.compaction_number存入当前最新file id
        self.current_readers.compaction_number.store(self.current_file_id, Ordering::SeqCst);
        //删除Writer中的current_readers中旧的<file, BufReader<File>>
        self.current_readers.remove_useless_reader_in_writer(self.current_file_id)?;
        self.size_for_compaction = 0;
        //Writer创建一个新文件
        self.create_new_file()?; 
        Ok(())
    }
```
