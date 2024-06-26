# kvstore-concurrency

## Introduction
A multi-threaded, persistent key/value store server and client with synchronous networking over a custom protocol.

## Points
- Synchronous networing
- Respond to multiple requests using increasingly sophisticated concurrent implementations, to reduce latency of individual request.

## Design
- [x] In-memory index -> concurrent data structure, and shared by all threads
- [x] Compaction will be done on a dedicate thread
- [x] `KvsEngine` and `KvStore` methods should take &self instead of &mut self, and impl `Clone`
- [x] `Trait ThreadPool`

    `ThreadPool::new(threads: u32) -> Result<ThreadPool>`
    - Creates a new thread pool, immediately spawning the specified number of threads.
    - Returns an error if any thread fails to spawn. All previously-spawned threads are terminated.

    `ThreadPool::spawn<F>(&self, job: F) where F: FnOnce() + Send + 'static`
    - Spawn a function into the threadpool.
    - Spawning always succeeds, but if the function panics the threadpool continues to operate with the same number of threads — the thread count is not reduced nor is the thread pool destroyed, corrupted or invalidated.

### Threadpool

#### 3 Types thread pools
  
- `NaiveThreadPool`: this implementation is not going to reuse threads between jobs. 
- `SharedQueueThreadPool`:  Instead of creating a new thread for every multithreaded job to be performed, a thread pool maintains a "pool" of threads, and reuses those threads instead of creating a new one. If a thread in your pool panics, the thread dies and spawn another, panic will be catched and keep the existing thread running. 
- `RayonThreadPool`: another threadpool implementation built by [rayon::ThreadPool](https://docs.rs/rayon/latest/rayon/struct.ThreadPool.html). 
