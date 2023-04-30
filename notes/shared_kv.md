

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

