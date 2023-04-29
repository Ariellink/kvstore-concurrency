use crate::Result;

pub trait ThreadPool {
    //creates a new thread pool, immediately spawning the specified number of threads.
    //returns an error if any thread fails to spawn. All previously-spawned threads are terminated.
    fn new(thread: u32) -> Result<Self>
        //size of Self should be known during compilation time, further restricting `Self`
        where Self: Sized;
    //Spawn a function into the threadpool.
    //Spawning always succeeds, but if the function panics the threadpool continues to operate with the same number of threads — the thread count is not reduced nor is the thread pool destroyed, corrupted or invalidated.
    fn spawn<F>(&self, job: F)
    where F: FnOnce() + Send + 'static;
}