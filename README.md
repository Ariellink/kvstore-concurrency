# kvstore-concurrency

## Introduction
A multi-threaded, persistent key/value store server and client with synchronous networking over a custom protocol.

## Points
- Synchronous networing
- Respond to multiple requests using increasingly sophisticated concurrent implementations, to reduce latency of individual request.

## Design
- [ ] In-memory index -> concurrent data structure, and shared by all threads
- [ ] Compaction will be done on a dedicate thread
- [ ] `KvsEngine` and `KvStore` methods should take &self instead of &mut self, and impl `Clone`
- [ ] `Trait ThreadPool`

    `ThreadPool::new(threads: u32) -> Result<ThreadPool>`
    - Creates a new thread pool, immediately spawning the specified number of threads.
    - Returns an error if any thread fails to spawn. All previously-spawned threads are terminated.

    `ThreadPool::spawn<F>(&self, job: F) where F: FnOnce() + Send + 'static`
    - Spawn a function into the threadpool.
    - Spawning always succeeds, but if the function panics the threadpool continues to operate with the same number of threads — the thread count is not reduced nor is the thread pool destroyed, corrupted or invalidated.
