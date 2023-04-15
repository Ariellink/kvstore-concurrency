// The command which is also an entry to write on disks
// command struct supports serial and deserial

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]

pub enum Command {
    SET(String, String),
    RM(String),
}