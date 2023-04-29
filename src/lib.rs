//#![deny(missing_docs)]
/*!
The KvStore store key/value pairs.
*/

mod errors;
mod engine;
mod request;
mod response;
mod server;
mod thread_pool;

pub use errors::{KVStoreError, Result};
pub use engine::KvsEngine;
pub use engine::Command;
pub use engine::KvStore;
pub use engine::SledKvStore;
pub use request::Request;
pub use response::Response;
pub use server::{EngineType,KvServer};