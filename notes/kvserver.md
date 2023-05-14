# 多线程服务端

KvServer初始化的过程中需要加入线程池，这样可以使得并发的请求能够在多核cpu的服务端并行执行。  
-  `is_stop` 的原子变量: 当前线程结束阻塞等待进而退出
阻塞的原因是因为tcplistener的incoming（）函数是阻塞的，一旦进入serve函数当前线程就阻塞了。is_stop是异步的写法。

```rust
pub struct KvServer <E,P>
where
    E: KvsEngine, 
    P: ThreadPool,
{
    engine: E,
    pool: P,
    is_stop: Arc<AtomicBool>
}
```
### 初始化KvServer
- 初始化/open一个KvStore   
- 初始化一个线程池 

```rust
impl <E: KvsEngine, P: ThreadPool> KvServer<E,P> {
    // construct
    pub fn new(engine: E, pool: P,is_stop: Arc<AtomicBool>) -> Self {
        KvServer { 
            engine,
            pool,
            is_stop: is_stop, 
        }
    }
```
这里将handle_connection方法从KvServer中单独拿出来了。  

serve方法先clone engine, 然后放入新线程中运行。用log crate打印错误。

```rust
pub fn serve(&mut self, addr: &String) -> Result<()> {
    let listener = TcpListener::bind(addr)?;
    info!("serving request and listening on [{}]", addr);
    for stream in listener.incoming() { 
        if self.is_stop.load(Ordering::SeqCst) {
            break;
        }
        //clone the egine
        let engine = self.engine.clone();
        //spawn a new thread in threadpool
        self.pool.spawn(move || match stream {
            Ok(stream) => {
                if let Err(e) = handle_connection(engine, stream) {
                    error!("Unexpected error occours when serving request: {:?}", e);
                }}
            Err(e) => {
                error!("Unexpected error occours when serving request: {:?}", e);
                }
        });
    }
    Ok(())
} 
```