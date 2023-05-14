use std::net::TcpStream;
use std::{env, process};
use clap::{arg, command, Command, ArgMatches};
use kvs::{Result};
use serde::Deserialize;
//use serde::Deserializer;
use std::io::{BufReader,BufWriter,Write};
use kvs::{Request,Response};

//build the Command instance
fn main() -> Result<()> {
    let matches = command!() // requires `cargo` feature
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
                Command::new("get")
                .about("get a vaule from a key: get <key>")
                .arg(arg!(<KEY>).help("A String key").required(true))
                .arg(arg!(-a --addr <ipport> "example: 127.0.0.1:4000").required(true).default_value("127.0.0.1:4000"))
        )
        .subcommand(
            Command::new("set")
                .about("set a key/vaule pair: set <key> <vaule>")
                .arg(arg!(<KEY>).help("A String key").required(true))
                .arg(arg!(<VALUE>).help("A String vaule").required(true))
                .arg(arg!(-a --addr <ipport> "example: 127.0.0.1:4000").required(true).default_value("127.0.0.1:4000"))
        )
        .subcommand(
            Command::new("rm")
                .about("remove the a key/vaule pair: rm <key>")
                .arg(arg!(<KEY>).help("A String key").required(true))
                .arg(arg!(-a --addr <ipport> "example: 127.0.0.1:4000").required(true).default_value("127.0.0.1:4000"))
        )
        .get_matches(); //get the command struct

        if let Err(err) = send_request(matches) {
            eprintln!("{:?}", err);
            process::exit(-1);
        }

        Ok(())
}
    //errors to stderr to stderr

    //fn send_request()
    //再对command本身进行模式匹配，根据不同的命令进行后续操作
    //比如get command
    fn send_request(matches:ArgMatches) -> Result<()> {
        match matches.subcommand() {
            Some(("get", _matches)) => {
                let key = _matches.get_one::<String>("KEY").unwrap();
                let addr = _matches.get_one::<String>("addr").unwrap();
                //拿到了server ip和要查询的key
                //需要建立连接
                let mut client = Client::new(&addr)?;
                match client.request(&Request::GET(key.to_owned()))? {
                    Some(val) => println!("{}", val),
                    None =>println!("Key not found"),
                };
            },
            Some(("set", _matches)) => {
                let key = _matches.get_one::<String>("KEY").unwrap();
                let value = _matches.get_one::<String>("VALUE").unwrap();
                let addr = _matches.get_one::<String>("addr").unwrap();
                let mut client = Client::new(&addr)?;
                client.request(&Request::SET(key.to_owned(), value.to_owned()))?;  
            },
            Some(("rm", _matches)) => {
                let key = _matches.get_one::<String>("KEY").unwrap();
                let addr = _matches.get_one::<String>("addr").unwrap();
                let mut client = Client::new(&addr)?;
                client.request(&Request::RM(key.to_owned()))?;
            },
            _ => process::exit(-1),
        }
        Ok(())    
    }

struct Client {
    //for response
    reader: serde_json::Deserializer<serde_json::de::IoRead<BufReader<TcpStream>>>,
    //for request
    writer: BufWriter<TcpStream>,
}

impl Client {
    fn new(addr: &str) -> Result<Client> {
        let stream = TcpStream::connect(addr)?; 
        Ok(Client {
            reader: serde_json::Deserializer::from_reader(BufReader::new(stream.try_clone()?)), //try clone的错是啥？
            writer: BufWriter::new(stream), //client往里写request
        })
    }

    fn request(&mut self, request: &Request) -> Result<Option<String>> {
        // 把request序列化为JSON, 然后放进Client::writer (or IO stream)
        serde_json::to_writer(&mut self.writer, request)?;
        //flush this output stream to server
        self.writer.flush()?; //flush cannot be detected.

        //client处理server发过来的respone
        //发response的逻辑在server.rs
        match Response::deserialize(&mut self.reader)? {
            Response::Ok(val) => Ok(val),
            Response::Err(err) => Err(kvs::KVStoreError::ServerError(err)),
        } 
    }
}
