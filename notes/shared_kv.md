

## Creating a shared `KvsEngine` trait
This time, our KvsEngine takes self as &self instead of &mut self as previously, It also implements Clone, which must be done explicitly for each implementation, as well as Send + 'static, implicit properties of the definition of each implementation. 

*before*
```rust
pub trait KvsEngine {
    fn set(&mut self, key: String, value: String) -> Result<()>;
    fn get(&mut self, key: String) -> Result<Option<String>>;
    fn remove(&mut self, key: String) -> Result<()>;
  }
```

*now*
```rust
pub trait KvsEngine: Clone + Send + 'static {
    fn set(& self, key: String, value: String) -> Result<()>;
    fn get(& self, key: String) -> Result<Option<String>>;
    fn remove(& self, key: String) -> Result<()>;
  }

```
> ### Questions  

1. Think about why the engine needs to implement `Clone` when we have a multithreaded implementation. Consider the design of other concurrent data types in Rust, like `Arc`.
2. Now think about why that makes us use `&self` instead of `&mut self`. What do you know about **shared mutable state**?  

    This is what Rust is all about.


实现 KvsEngine trait 的 object is shared between threads. Object需要在heap上，并且因为共享状态shared state不能被mutable, 需要被同步primitive所保护。

So, move the data inside your implementation of `KvsEngine`, `KvStore` onto the heap using a thread-safe shared pointer type and protect it behind a lock of your choosing.



## How to make single-threaded `KvStore` thread-safe?

### Locks

single-thread KvStore
```rust
pub struct KvStore {
    // key：String， vaule_metadata: CommandPos
    index: HashMap<String, CommandPos>,
    current_reader: HashMap<u64,BufReader<File>>,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    dir_path: PathBuf,
    size_for_compaction: u64,
}
```

simple multithreaded version, protecting everything with a lock
```rust
#[derive(Clone)]
pub struct KvStore(Arc<Mutex<SharedKvStore>>)

#[derive(Clone)]
pub struct SharedKvStore {
    // key：String， vaule_metadata: CommandPos
    index: HashMap<String, CommandPos>,
    current_reader: HashMap<u64,BufReader<File>>,
    current_writer: BufWriterWithPos<File>,
    current_file_id: u64,
    dir_path: PathBuf,
    size_for_compaction: u64,
}
```
> `Arc<Mutex<T>>`
- `Arc`: 把数据放在heap上，so that 这个数据就能在线程间被共享。并且Arc提供的clone的方法为每个线程都创建了一个handle(类似引用的抽象)。  
- `Mutex`: 提供在不使用 `&mut` 可变引用的前提下，提供一种方法获得T的重写权限。  

In this case, Muetx不仅限制了`SharedKvStore`的write access, 也限制了read access。实际上，读并发并不会造成数据竞争。因此这样任何线程需要和`KvStore`交互时，都必须要等待Mutex被其他线程unlock，处理请求的时间并没有加快，而且还加上了线程切换的负荷。

>`RwLock`
允许任意数量的readers （& pointers）, 或者一个writer (a &mut pointer)。在kvstore中，所有的read requests可以满足并发，但是单个写请求进来，所有在系统中的其他请求都停止。我们只要将mutex换成rwlock。

```rust
    thread
           +  +--------+
      T1   |  |   R1   |
           |  +--------+
      T2   |  |   R2   |
           |  +-----------------+
      T3   |           |   W1   |
           |           +-----------------+
      T4   |                    |   W2   |
           +                    +--------+
              --> read/write reqs over time -->
```

### Lock-free readers

removing the locks from readers

we need to consider how each one of the fields of SharedKvStore is used:

```rust
pub struct KvStore {
    /// Directory for the log and other data
    path: PathBuf,
    /// The logid, and log reader
    reader: HashMap<u64,BufReader<File>>,,
    /// The log writer
    writer: BufWriterWithPos<File>,
    /// The in-memory index from key to log pointer
    index: BTreeMap<String, CommandPos>,
    /// The number of bytes representing "stale" commands that could be
    /// deleted during a compaction
    uncompacted: u64,
}
```
- `path: PathBuf`: logs存放的位置，never changes. immutable类型。并且immutable 类型在rust是`Sync`。因此它根本不需要任何保护。每个线程都可以通过一个 shared reference来读取它。   
> **How immutable values can be shared trivially between theads (they are Sync)？**   
 Immutable values are the best for concurrency — just throw them behind an `Arc` and don't think about them again. In our example, again PathBuf is easily clonable, and immutable.
- `reader: HashMap<u64,BufReader<File>>`: read handle to the current log file.当compaction后，需要更改file。
> **how to share access to files across threads？**  
  `File`其实并不是文件，只是磁盘中物理资源的handle。因此可以为同一个实际的文件打开多个句柄。  
  但是File的API没有实现Clone trait, 虽然它有一个try_clone的方法，但是他的语义对于多线程应用来说有着复杂的影响。

> **the differences between Files from File::open and try_clone?**   
 `File::open` 方法打开同一个文件两次，它们使用的是同一个文件描述符。  
 通过 `File::open` 和 `File::try_clone` 方法打开同一个文件，并使用 `BufReader` 对象读取文件内容，并输出到控制台。可以看到，输出结果中 reader1 和 reader2 输出的内容也是完全一样的，但这里的 reader1 和 reader2 使用的是不同的文件描述符，可以在多个线程之间共享文件的读写权限。  
 总之，如果你只需要在**同一个线程**中访问文件，那么直接使用 File::open 方法打开同一个文件两次即可。如果需要在**多个线程**之间共享文件的读写权限，那么需要使用 `File::open` 方法打开文件，并使用 `File::try_clone` 方法克隆文件描述符。
    
- `writer: BufWriterWithPos<File>`: write handle to the current log file. 任何写操作都需要获取writer的write access。并且在compaction后，也要将writer中的current opened file修改。
- `index: BTreeMap<String, CommandPos>`: in-memory index。每个read请求都要读，每个写请求都要改。并且进行compaction之后，index中间包的`*each_cmdpos`也会要改。  
*`BtreeMap`平衡二叉搜索树，是按照key排序的map,增删查都是O(logN)。`HashMap`是乱序的，增删查都是最查O(logN),最好O(1)。
- uncompacted: 简单计算所有"stale"commands in logs, 来确定何时进行compact()。

在我们的用例中，我们有两个明确的角色：reader和writer（也许第三个角色是压缩器）。 在 Rust 中，将reader和writer逻辑分离到它们自己的并发类型中是很常见的。 Readers有自己的数据集，Writer   有自己的数据集，这就提供了很好的封装机会，所有的读操作都是一种类型，所有的写操作都是另一种类型。   
     
## Understand and maintain sequential consistency 

 > your code will break entirely unless you tell the compiler via synchronized types and operations that it must not allow reordering.  
 Any operation that must occur before or after another must be exlicitly arranged to do so with synchronized types or operations, whether they be locks, atomics or otherwise.

如果更新index发生在文件写入之前怎么办？


*Reading*  
[Atomic vs. Non-Atomic Operations](https://preshing.com/20130618/atomic-vs-non-atomic-operations/)

