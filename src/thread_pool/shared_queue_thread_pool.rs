
use std::sync::mpsc;
use std::sync::{Arc,Mutex};
use std::thread;
use crate::thread_pool::ThreadPool;
use crate::Result;
use std::panic;

pub struct SharedQueueThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Message>>
   }

pub enum Message {
    NewJob(Job),
    Terminate,
}

pub type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool for SharedQueueThreadPool {
    fn new(thread_num: u32)-> Result<Self> 
        where Self: Sized,
    {
        //if num of thread was specified less than 1, invoke panic
        assert!(thread_num > 0);

        let mut workers = Vec::with_capacity(thread_num.try_into().unwrap());

        let (sender, receiver) = mpsc::channel();
        
        let receiver = Arc::new(Mutex::new(receiver));
        
        for id in 0..thread_num {
            //To avoid value being moved in previous iteration of loop
            //use smart pointers to wrap the receiver: Arc<T>
            //std::sync::mpsc::Receiver<Job>  cannot be shared between threads safely
            //wrap it as Mutex, Mutex will ensure that only one worker gets a job from the receiver at a time.
            //In SharedQueueThreadPool::new, we put the receiver in an Arc and a Mutex. For each new worker, we clone the Arc to bump the reference count so the workers can share ownership of the receiver.
            let worker = Worker::new(id.try_into().unwrap(), Arc::clone(&receiver))?;
            workers.push(worker);
        }

        Ok(
            SharedQueueThreadPool { 
                workers, 
                sender: Some(sender), //when sender was used as member of the SharedQueueThreadPool, Sender<T> inferred to be Sender<Job>
            }
        )
    }

    fn spawn<F>(&self, job: F)
    where
        F: FnOnce() + 'static + Send,
    {
        // warp the closure as Box pointer
        let message = Message::NewJob(Box::new(job));
        //sender of the SharedQueueThreadPool sends the job, and 
        self.sender.as_ref().unwrap().send(message).unwrap();
    }
}

//impl Drop for SharedQueueThreadPool to have graceful shutdown

impl Drop for SharedQueueThreadPool {
    fn drop(&mut self) {
        println!("Sending terminate message to all workers.");

        for _ in &mut self.workers {
             if let Some(m) = &self.sender {
                m.send(Message::Terminate).unwrap()
             }
             //self.sender.as_ref().unwrap().send(Message::Terminate).unwrap();
        }
        
        println!("Shutting down all workers.");

        for worker in &mut self.workers {
            //explictly drop the sender before waiting for threads to finish
            //drop(self.sender.take()); //then all calls to recv() in the loop with return an error
            //-> change the worker loop to handle the errors
            println!("Shutting down worker {}", worker.worker_id);
            //here is only one mutable borrow of each worker
            //join(self),the self here is JoinHandle<()>, join() takes its arguments' ownership

            // if let Some(m) = &self.sender {
            //     m.send(Message::Terminate).unwrap()
            // }
            
            println!("Shutted down worker {}", worker.worker_id);
            //need to move the thread out of the Worker instance that owns it
            //thread: Option<thread::JoinHandle<()>>, Option.take()to move the value out of he some variant, leave None in its place
             //worker.handle.join(); //error!
            if let Some(_handle) = worker.handle.take() {
                 _handle.join().unwrap();
            }
            println!("Joined worker {}", worker.worker_id);
        }
    }
}


pub struct Worker {
    pub worker_id: usize,
    //Option.take() move the ownership of the worker, so that join() can consume the thread
    pub handle: Option<thread::JoinHandle<()>>,
}

impl Worker {
    //1. worker spawns a thread which contains a rx 
    //2. construct the Worker with id provided and the waiting rx
   //pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Self, PoolCreationError> {
     pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Result<Self> {
        //Loop: closure to loop forever, asking the receiving end of the channel for a job and running the job when it gets one. 
        let handle = thread::Builder::new().spawn(move || loop{
            //Blocking: blocks this thread and waiting availale job received
            let message = receiver.lock().expect(&format!("mutex poisoned in thread {}", id)).recv();
            
            match message {
               Ok(Message::NewJob(message)) => {
                    println!("Worker {id} got a job; executing.");
                    //catch the panic closure using catch_unwind(F), and let the thread drop the error and back to the loop
                    if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(message)) {
                        eprint!("{} executes a job with error {:?}", id, err);
                    }
                     // execute the closure
               }
               Ok(Message::Terminate) => {
                    println!("Worker {id} was told to terminate; shutting down.");
                    break;
               } 
               Err(e) => {
                    println!("Got a receiver error: {:?}",e);
                    break;
               }
            }

        })?;
        Ok(
            Worker {
                worker_id: id,
                handle: Some(handle), //for Option<thread::JoinHandle<()>>
            }
        )
    }
}