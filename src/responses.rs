use crate::serde::{Serialize, Deserialize};
use crate::models::*;

pub type Result<T> = std::result::Result<T, Error>;

pub type ListFiles = Vec<FileInfo>;
pub type StatFile = FileInfo;
pub type ReadFile = u32;

#[derive(Serialize, Deserialize)]
pub struct FreeSpace {
    pub total_bytes: u64,
    pub free_bytes: u64
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Error {
    FileNotFound,
    NoSuchHandle,
    FileExists,
    PermissionDenied,
    CouldNotFindDisk,
    Other
}