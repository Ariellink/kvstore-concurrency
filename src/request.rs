use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize,Deserialize,Debug)]
pub enum Request {
    SET(String,String),
    RM(String),
    GET(String),
}