use std::net::{TcpListener,TcpStream};
use std::sync::atomic::AtomicBool;
use crate::thread_pool::ThreadPool;
use crate::{Result,KvsEngine,Request,Response};
//use serde::Deserialize;
use std::io::BufReader;
use std::fmt;
use log::{info,error,debug};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

pub enum EngineType {
    KvStore,
    SledKvStore,
}

//for to_string() can be used on enum EngineType when combine the current dir in kvs_server.rs
impl fmt::Display for EngineType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EngineType::KvStore => write!(f,"kvs"),
            EngineType::SledKvStore => write!(f,"sled"),
        }
    }
}


pub struct KvServer <E,P> 
where 
    E: KvsEngine,
    P: ThreadPool, // KvStore & SledKvStore
{
    engine: E,
    pool: P,
}

impl <E: KvsEngine, P: ThreadPool> KvServer<E,P> {
    // construct
    pub fn new(engine: E, pool: P) -> Self {
        KvServer { 
            engine,
            pool,
        }
    }

    //serve and listen at addr
    //循环处理每一个stream
    pub fn serve(&mut self, addr: &String) -> Result<()> {
        let listener = TcpListener::bind(addr)?;
        info!("serving request and listening on [{}]", addr);
        for stream in listener.incoming() { 
            if self.is_stop.load(Ordering::SeqCst) {
                break;
            }
            //clone the egine
            let engine = self.engine.clone();
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
}

// deserialize the stream to data gram strcut
// call from struct
fn handle_connection<E: KvsEngine> (engine: E, mut stream: TcpStream) -> Result<()> {
    let request = Request::deserialize(&mut serde_json::Deserializer::from_reader(BufReader::new(&mut stream)))?;
    info!("tcpstream: {:?}", &stream);
    let bufreader = BufReader::new(&mut stream);
    info!("bufreader: {:?}",&bufreader);

    let now = SystemTime::now();
    debug!("Request: {:?}", &request);

    let response;
    match request {
       Request::GET(key) => {
           match engine.get(key) {
               Ok(value) => response = Response::Ok(value),
               Err(err) => response = Response::Err(err.to_string()),
           }
       }
       Request::SET(key, val) => {
           match engine.set(key, val) {
               Ok(()) => response = Response::Ok(None),
               Err(err) => response = Response::Err(err.to_string()),
           }
       }
       Request::RM(key) => {
           match engine.remove(key) {
               Ok(()) => response = Response::Ok(None),
               Err(err) => response = Response::Err(err.to_string()),
           }
       }
    }
   
   debug!("Response: {:?},spent time: {:?}", &response, now.elapsed());

   serde_json::to_writer(stream, &response)?;
   
   Ok(())
}
