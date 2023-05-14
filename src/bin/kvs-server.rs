use kvs::{KVStoreError, EngineType, KvsEngine,KvServer,Result, KvStore,SledKvStore};
use kvs::thread_pool::{ThreadPool,SharedQueueThreadPool,NaiveThreadPool,RayonThreadPool};
use clap::{arg,command, ArgMatches};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::{env};
use log::{info, LevelFilter};

fn main() -> Result<()> {
    //logger 
    env_logger::builder().filter_level(LevelFilter::Info).init();
    //get the args
    let matches = command!()
    .arg(
        //specify the server listening port
        arg!(-a --addr <ipport> "example: 127.0.0.1:4000")
        .required(false)
        .default_value("127.0.0.1:4000")
    )
    //存在不指定engine_type的情况
    .arg(
        arg!(-e --engine <engine_name> "sled or kvs")
        .required(false)
        .value_parser(["kvs", "sled"]),
    )
    .get_matches();
    if let Err(err) = init(matches) {
        eprint!("{:?}", err);
        std::process::exit(-1);
    }
    Ok(())
}

//parse matches
fn init(matches: ArgMatches) -> Result<()> {

    let addr = matches.get_one::<String>
    ("addr").unwrap();
    let engine_type_userspecified = matches.get_one::<String>
    ("engine");

    //logger
    info!("Version: {}",env!("CARGO_PKG_VERSION"));
    info!("Addr: [{}]", addr);
    info!("EngineTypeSpecifiedByUser: [{}]", engine_type_userspecified.unwrap());

    let engine_type = judge_engine(engine_type_userspecified.cloned())?;
    info!("engine_type: [{}]", engine_type);
    
    match engine_type {
        EngineType::KvStore => {
            run_server(KvStore::open(env::current_dir()?.join(EngineType::KvStore.to_string()))?, addr)
        },
        EngineType::SledKvStore => {
            run_server(SledKvStore::open(env::current_dir()?.join(EngineType::SledKvStore.to_string()))?, addr)
        },
    }
}

//根据当前engine是否在当前路径已经初始化来决定enginetype和返回错误
//当前engine是否在当前路径已经初始化，不允许更改engineType, 使用open()初始化
fn judge_engine(engine_type: Option<String>) -> Result<EngineType> {
    let curr_dir = env::current_dir()?;
    match engine_type {
        None => {
            //impl Display trait for enum EngineType in server.rs
            if curr_dir.join(EngineType::SledKvStore.to_string()).exists() {
                return Ok(EngineType::SledKvStore);
            }
            return Ok(EngineType::KvStore)
        }
        Some(eg) => {
            if eg == EngineType::SledKvStore.to_string() {
                if curr_dir.join(EngineType::KvStore.to_string()).exists() {
                    return Err(KVStoreError::ChangeEngineError);
                }
                Ok(EngineType::SledKvStore)
            } else {
                if curr_dir.join(EngineType::SledKvStore.to_string()).exists() {
                    return Err(KVStoreError::ChangeEngineError);
                }
                return Ok(EngineType::KvStore)
            }
        }
    }
}

//构造并运行KvsServer实例并监听处理stream在server()函数
//engine: 是KvStore实例或者是SledKvStore实例
fn run_server<E>(engine: E,addr: &String,) -> Result<()> 
where E: KvsEngine
{   
    info!("running server with engine_type");
    let mut server = KvServer::new(
        engine,
        SharedQueueThreadPool::new(num_cpus::get())?,
        Arc::new(AtomicBool::new(false)),
    );
    server.serve(addr)?;
    Ok(())
}


