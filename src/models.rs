use std::time::SystemTime;
use crate::serde::{Serialize, Deserialize};

pub type FileHandle = u32;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub creation_time: SystemTime,
    pub last_modified: SystemTime,
    pub last_accessed: SystemTime,
    pub name: String,
    pub size: u64,
    pub mode: u32,
    pub ino: u64
}