use crate::{thread_pool::ThreadPool,Result};
use rayon;

pub struct RayonThreadPool {
    raypool: rayon::ThreadPool
}

impl ThreadPool for RayonThreadPool {
    fn new(thread: u32) -> Result<Self>
        where Self: Sized,
    {
        let raypool = rayon::ThreadPoolBuilder::new().num_threads(thread.try_into().unwrap()).build().unwrap();
        Ok(RayonThreadPool { raypool })
    }

    fn spawn<F>(&self, job: F)
    where F: FnOnce() + Send + 'static,
    {
        self.raypool.spawn(job);
    }
}