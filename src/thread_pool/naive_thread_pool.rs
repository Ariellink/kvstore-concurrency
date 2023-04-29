use crate::{thread_pool::ThreadPool,Result};
use std::thread;
//this implementation is not going to reuse threads between jobs.

pub struct NaiveThreadPool;

impl ThreadPool for NaiveThreadPool {
    fn new(_: u32) -> Result<Self>
        //size of Self should be known during compilation time, further restricting `Self`
    where Self: Sized, 
    {
        Ok(NaiveThreadPool)
    }

    //Spawn a function into the threadpool.
    //Spawning always succeeds, but if the function panics the threadpool continues to operate with the same number of threads — the thread count is not reduced nor is the thread pool destroyed, corrupted or invalidated.
    fn spawn<F>(&self, job: F)
    where F: FnOnce() + Send + 'static,
    {
        thread::spawn(job);
    }
}