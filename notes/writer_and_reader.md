
# Writer and Reader

### DashMap内部实现了Arc可以在线程之间不加Arc进行传递
```rust
use dashmap::DashMap;

fn main() {
    let map = DashMap::new();
    let mut handles = vec![];
    for i in 0..10 {
        let handle = std::thread::spawn(move || {
            map.insert(i, i * i);
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join().unwrap();
    }
    println!("{:?}", map);
}
```
`DashMap` 内部使用了 `Arc` 进行引用计数，因此在使用 `DashMap` 时不需要手动传递 `Arc`。`DashMap` 的实现方式是将哈希表分成多个片段（shard），每个片段都有一个读写锁，不同片段之间互不干扰，因此可以并发地访问。每个片段内部使用 `RwLock` 进行读写控制，因此在同一片段内进行读写时，需要先获得写锁才能进行写操作，读锁和写锁之间不会互相阻塞。因为在内部实现中使用了 `Arc` 和多个 `RwLock`，所以 `DashMap` 可以在多个线程之间安全地共享。

在上述示例代码中，由于 `map` 是 `main` 函数中的局部变量，所以当它被传递到 `thread::spawn` 中时，发生了所有权的转移，即 `map` 的所有权被转移给了新创建的线程。由于 `map` 是 `DashMap` 类型，它的内部实现使用了 `Arc`，所以在转移所有权的过程中，`Arc` 的引用计数被增加了。在新线程中，由于 `map` 的类型是 `Arc<DashMap<K, V>>`，所以可以直接进行读写操作，而无需再次传递 `Arc`。这是因为 `Arc` 实现了多线程间的所有权共享，因此可以在多个线程之间安全地共享一个对象的所有权。


```rust
pub struct KvStore {
    // key：String， vaule_metadata: CommandPos
    index: DashMap<String, CommandPos>,
    current_readers: Reader,
    current_writer: Arc<Mutex<Writer>>,    
}

pub struct Writer {
    dir_path: Arc<PathBuf>,//有必要,因为是和Reader共享的
    current_readers: Reader,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    size_for_compaction: Arc<AtomicU64>,//有必要,因为是和Reader共享的??也？
    index: DashMap<String, CommandPos>,
}

struct Reader {
    dir_path: Arc<PathBuf>,
    compaction_number: Arc<AtomicU64>,
    readers: RefCell<HashMap<u64, BufReader<File>>>,
}
```

初始化
```rust

let current_readers = Reader {
    dir_path: Arc::clone(&dir_path),
    compaction_number: Arc::new(AtomicU64::new(0)), 
    readers: RefCell::new(readers),
} 

let current_writer = Arc::new(Mutex::new(
    Writer {
        dir_path,
        current_readers: current_readers.clone(),
        current_writer,
        current_file_id,
        size_for_compaction,
        index,
    }
));

let mut store = KvStore{
    index,
    current_readers,
    current_writer,
};

// if store.size_for_compaction > MAX_COMPACTION_SIZE {
//     store.compact()?;
// }

Ok(store)
```
open时不要compaction了？见下

## Reader

- `RefCell`提供安全的内部可变性机制，它允许在不可变引用中修改其值。以确保在任何时候只有一个可变引用或任意数量的不可变引用，以便多个线程在不共享所有权的情况下对同一数据进行读写操作。`RefCell`对象则允许在不可变引用的前提下获取对对象内部的可变引用，这是通过`borrow_mut()`方法来实现的。这意味着 `RefCell`可以用于在多个线程之间共享对象的可变性，但需要程序员保证线程安全。
- `Arc`用于共享`不可变对象`的所有权.通过 Arc 引用获取对对象内部的可变引用需要使用 Mutex 或 RwLock 等线程安全类型来包装对象。
- 内部的readers在clone时并不是拷贝refcell, 而是初始化一个新的map。因为当多线程读同一个文件时，会创建多个reader, 这些readers可以在应用层对同一个文件执行并发IO读请求。RefCell来获取内部可变性。
> 在 Clone 的实现中，首先使用 Arc::clone 方法对 dir_path 和 compaction_number 字段进行克隆，以便新创建的 Reader 对象与原始对象共享相同的目录路径和压缩版本号。

> 总之，这段代码的作用是为 Reader 结构体实现一个可克隆的副本方法，以便在需要复制 Reader 对象时，可以共享目录路径和压缩版本号,但是每个线程Writer都

```rust
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
```

## impl Writer
```rust
pub struct Writer {
    dir_path: Arc<PathBuf>,//有必要,因为是和Reader共享的
    current_readers: Reader,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    size_for_compaction: u64,
    index: DashMap<String, CommandPos>,
}
Impl Writer {
    fn set
    fn rm
    fn compact
    fn create_new_file
}
```
## Writer compact

1. 创建一个新log file，把current_writer更新到这个log file, current_file_id也更新。
2. 读writer.index, 遍历key, 打开key对应的file,根据起始位置和结束位置，找到value.
    - 把readerbuf里的value拷贝到writerbuf里，写入
    - 更新index的value, comdpos到新写入的位置上。
3. Writer中的current_readers.compaction_number存入当前最新file id (原子操作)
4. 由于Writer中的reader是自己新开的，自己维护的，所以读操作去读这部分。
5. **删除**  
    - map到readers中的所有<compaction_number的key,即file_id  
    - 删除，删除的是writer中的打开的<compaction_number的readers键值对,kvstore::reader中的bufreader要另外再去删除  
    - 真实地删除底层文件 `fs::remove_file(&file_path)`

> 另外 compact()方法调用放在了每次set和remove后，
```rust
impl writer {
    fn set() {
        ...
        if self.useless_size > MAX_USELESS_SIZE {
        let now = SystemTime::now();
        info!("Compaction starts");
        self.compact()?;
        info!("Compaction finished, cost {:?}", now.elapsed());
        }
    }
}
```

## impl Reader  
compact 流程会始终持有 writer 的写锁，因而此时并不存在并发安全问题，其在结束后会尝试删除掉过时的文件。不过该删除并不会影响其他读线程的 reader 句柄继续读去文件。
6. 对于reader中维护的<file_id, Bufreader> hashmap,在 compaction 中其执行的索引尽管可能文件已经被删除了，但由于其持有句柄因而始终能够读到数据。  
7. 对于reader中维护这组句柄，无法在writer compaction中清除，不加以清理，会有很多已经compact掉但是仍存在的句柄，需要进行句柄删除。这部分操作我们在每次查询钱进行一次。

```rust
impl Reader {
    //反序列化
    fn read_command(&self, postion: &CommandPos) -> Result<Option<String>> {
        self.read_add(postion, |data_reader| {
            if let Command::SET(_, value) = serde_json::from_reader(data_reader)? {
                Ok(Some(value))
            } else {
                Err(KVStoreError::UnknownCommandType)
            }
        })
    }
    //通过position定位到entry所在的bufreader
    fn read_add<F,R>(&self, postion: &CommandPos, f: F) -> Result<R> 
    where
        F: FnOnce(Take<&mut BufReader<File>>) -> Result<R>
    {
        //!!! 
        self.try_to_remove_stale_readers();

        let mut readers = self.readers.borrow_mut();

        //check the if the file handle exists
        //因为在compaction以及create_new_file实在writer中完成的，新建的文件句柄没有更新到reader维护的bufreader中，所以需要检查句柄是否存在，不存在的话要另外打开
        if let hash_map::Entry::Vacant(entry) = readers.entry(postion.file_id) {
            let new_reader = BufReader::new(File::open(self.dir_path.join(format!("data_{}.txt", postion.file_id))?
            ));
            entry.insert(new_reader);
        }
        //locate the commad position
        let source_reader = readers.get_mut(&postion.file_id).expect("Can not find key in files but it is in memory");

        source_reader.seek(SeekFrom::Start(postion.offset))?;
        let data_reader = source_reader.take(postion.length as u64);
        
        f(data_reader)
    }
    
    fn remove_useless_reader_in_writer(&mut self, file_id: u64) -> Result<()> {}
    //删除reader中被compact掉的hashmap中的文件句柄
    fn try_to_remove_stale_readers_in_reader(&self) {
        let compaction_number = self.compaction_number.load(Ordering::SeqCst);
        let mut readers = self.readers.borrow_mut();
        //delete the readers older than compaction_number
        readers.retain(|&k, _|k >= compaction_number);
    }

     fn copy_data_to_writer(
        &self,
        position: &CommandPosition,
        writer: &mut BufWriterWithPosition<File>,
    ) -> Result<()> {
        self.read_add(position, |mut data_reader| {
            io::copy(&mut data_reader, writer)?;
            Ok(())
        })
    }

```

### 原子类型 `compaction_number`
Writer中的是compaction size, 但是reader中保存的是最新的compacion file id  
让reader单独去删除readers中stale data  

在实现无锁读之后，reader 的清理便不再能够串行起来了，因而需要一个多线程共享的原子变量来记录最新 compaction 之后的 file_number，小于这个 file_number 的文件和对应的 reader 便都可以删除了。


```rust
pub struct Reader {
    dir_path: Arc<PathBuf>,
    compaction_number: Arc<AtomicU64>,
    readers: RefCell<HashMap<u64, BufReader<File>>>,
}
```