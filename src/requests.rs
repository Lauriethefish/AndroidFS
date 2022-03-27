use crate::serde::{Serialize, Deserialize};
use crate::models::*;

#[derive(Serialize, Deserialize)]
pub enum Request {
    List(ListFiles),
    Stat(StatFile),
    Delete(DeleteFile),
    CreateFile(CreateFile),
    CreateDirectory(CreateDirectory),
    Open(OpenFile),
    Close(CloseFile),
    Move(MoveFile),
    GetFreeSpace,
    Read(ReadFile),
    Write(WriteFile),
    SetEndOfFile(SetEndOfFile)
}

#[derive(Serialize, Deserialize)]
pub struct ReadFile {
    pub handle: FileHandle,
    pub offset: u64,
    pub len: u64
}

#[derive(Serialize, Deserialize)]
pub struct WriteFile {
    pub handle: FileHandle,
    pub offset: u64,
    pub len: u64
}

#[derive(Serialize, Deserialize)]
pub struct SetEndOfFile {
    pub handle: FileHandle,
    pub len: u64
}

pub type CloseFile = FileHandle;

pub type ListFiles = String;

pub type StatFile = String;

pub type DeleteFile = String;

#[derive(Serialize, Deserialize)]
pub struct CreateFile {
    path: String,
    // TODO: Additional details
}

#[derive(Serialize, Deserialize)]
pub struct CreateDirectory {
    path: String,
    // TODO: Additional details
}

#[derive(Serialize, Deserialize)]
pub struct OpenFile {
    pub path: String,
    // TODO: Additional details
}

#[derive(Serialize, Deserialize)]
pub struct MoveFile {
    pub from: String,
    pub to: String,
    pub replace_if_exists: bool
}