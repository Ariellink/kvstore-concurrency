use failure::Fail;
//use std::io::string;
//use sled::Error;

pub type Result<T> = std::result::Result<T,KVStoreError>;

#[derive(Fail, Debug)]
pub enum KVStoreError {
    //(1) merge Error from std::io
    #[fail(display = "{}", _0)]
    IoError(#[cause] std::io::Error),

    //(2) merge Error from serde_json
    #[fail(display = "{}", _0)]
    SerdeError(#[cause] serde_json::Error),

    #[fail(display = "Unknown command type")]
    UnknownCommandType,

    #[fail(display = "Key not found")]
    KeyNotFound,

    #[fail(display = "{}", _0)]
    ServerError(String),

    //(4) merge Error from sled::Error
    #[fail(display = "{}", _0)]
    SledError(#[cause] sled::Error),

    #[fail(display = "Changing engine is not allowed after initilization in current dir")]
    ChangeEngineError,

    #[fail(display = "{}", _0)]
    Utf8Error(#[cause]std::string::FromUtf8Error)
}

impl From<std::io::Error> for KVStoreError {
    fn from(err: std::io::Error) -> Self {
        KVStoreError::IoError(err)
    }

}

impl From<serde_json::Error> for KVStoreError {
    fn from(err: serde_json::Error) -> Self {
        KVStoreError::SerdeError(err)
    }
}

impl From<sled::Error> for KVStoreError {
    fn from(err: sled::Error) -> Self {
        KVStoreError::SledError(err)
    }
}

impl From<std::string::FromUtf8Error> for KVStoreError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        KVStoreError::Utf8Error(err)
    }
}