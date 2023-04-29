### Threadpools

- `NaiveThreadPool`: this implementation is not going to reuse threads between jobs. -> testcase: `thread_pool::naive_thread_pool_*`
- `SharedQueueThreadPool`:  Instead of creating a new thread for every multithreaded job to be performed, a thread pool maintains a "pool" of threads, and reuses those threads instead of creating a new one.  -> testcase: `shared_queue_thread_pool_*`
    - how to deal with panicking jobs? spawned function panics:  if a thread in your pool panics you need to make sure that the total number of threads doesn't decrease. You have at least two options:   
        - **Let the thread die and spawn another, or catch the panic and keep the existing thread running. [`catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)**  
        `pub fn catch_unwind<F: FnOnce() -> R + UnwindSafe, R>(f: F) -> Result<R>`  
            这可以运行任意 Rust 代码，捕获恐慌并允许优雅地处理错误。

            1. 如果闭包没有panic, return Ok(closure result)。
            2. 如果闭包panic, return Err(casue) 最初调用 panic 的对象。  
        - What are the tradeoffs? You've got to pick one, but leave a comment in your code explaining your choice.

```rust


//由于单元测试中传入的闭包可能会 panic 但不想看到线程池中的线程减少，一种方案是检测到线程 panic 退出之后新增新的线程，另一种方式则是捕获可能得 panic。
//例如在 Java 中可以使用 try catch 捕捉一个 throwable 的错误，在 go 中可以 defer recover 一个 panic。
//在 rust 中类似的语法是 catch_unwind，因而在执行真正的 job 闭包时，会使用 panic::catch_unwind(AssertUnwindSafe(job)) 的方式来确保该线程不会由于执行闭包而 panic。

impl Worker {
    
     pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Result<Self> {
      
        let handle = thread::Builder::new().spawn(move || loop{
    
        let message = receiver.lock().expect(&format!("mutex poisoned in thread {}", id)).recv(); 

        match message {
            Ok(Message::NewJob(message)) => {
                println!("Worker {id} got a job; executing.");
                // execute the closure in unwind(F)
                if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(message)) {
                    eprint!("{} executes a job with error {:?}", id, err);
                }
                    
            }
            Ok(Message::Terminate) => {
                println!("Worker {id} was told to terminate; shutting down.");
                break;
            } 
            Err(e) => {
                println!("Got a receiver error: {:?}",e);
                break;
            }
               ...
```
        
- `RayonThreadPool`: [rayon::ThreadPool](https://docs.rs/rayon/latest/rayon/struct.ThreadPool.html)  
背景： Rayon 使用工作窃取调度程序。关键思想是每个线程都有自己的任务双端队列。每当生成新任务时（无论是通过join()、Scope::spawn()还是其他方式），新任务都会被推送到线程的本地双端队列中。工作线程优先执行自己的任务；但是，如果他们用完了任务，他们将尝试从其他线程“窃取”任务。因此，此函数与其他活动的工作线程存在固有的竞争，这些工作线程可能正在从本地双端队列中删除项目。

    ```rust
    pub fn spawn <OP>(&self, op:OP)其中 OP: FnOnce () + Send + 'static,

    //在此线程池中生成一个异步任务。此任务将在隐式全局范围内运行，这意味着它可能比当前堆栈帧持续时间更长——因此，它无法将任何引用捕获到堆栈上（您可能需要一个闭包）move。
    ```
    感觉用[pub fn install<OP, R>(&self, op: OP) -> R](https://docs.rs/rayon/latest/rayon/struct.ThreadPool.html) 好像也可以。