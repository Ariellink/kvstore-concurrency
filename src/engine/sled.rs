
use sled::Db;
use std::path::PathBuf;
use crate::{KvsEngine,KVStoreError,Result};

pub struct SledKvStore {
    inner: sled::Db,
}

impl SledKvStore {
    pub fn open (open_path: impl Into<PathBuf>) -> Result<SledKvStore> {
        let inner_sleddb = sled::open(open_path.into())?;
        
        Ok(SledKvStore {
            inner: inner_sleddb,
        })
    }
}

impl KvsEngine for SledKvStore {
    fn set(&mut self, key: String, value: String) -> Result<()> {
        self.inner.insert(key, value.into_bytes())?; //into_bytes return the vec
        self.inner.flush()?; 
        Ok(())
    }

    fn get(&mut self, key: String) -> Result<Option<String>> {
        let val = self
        .inner
        .get(key)?
        .map(|vec|vec.to_vec())
        .map(String::from_utf8)
        .transpose()?; //utf8 errors !!!
        Ok(val) //String和IVec
    }
    
    fn remove(&mut self, key: String) -> Result<()> {
        // Db::remove only returns if it existed.
        self.inner.remove(key)?.ok_or(KVStoreError::KeyNotFound)?;
        self.inner.flush()?; 
        Ok(())
    } 
}