use std::io::Read;
use std::{io::Write, convert::TryInto};
use std::net::TcpStream;
use std::sync::{Mutex, MutexGuard};
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use dokan::OperationError;
use winapi::shared::ntstatus::*;
use serde::de::DeserializeOwned;

use crate::models::*;
use crate::requests;
use crate::responses;

pub enum Error {
    IOFailed(std::io::Error),
    ReceivedInvalidData(bincode::Error),
    RequestFailed(responses::Error)
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IOFailed(err)
    }
}

impl From<responses::Error> for Error {
    fn from(err: responses::Error) -> Self {
        Self::RequestFailed(err)
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Self::ReceivedInvalidData(err)
    }
}


pub type Result<T> = std::result::Result<T, Error>;

pub fn convert_response<T>(resp: Result<T>) -> std::result::Result<T, OperationError> {
    match resp {
        Ok(data) => Ok(data),
        Err(err) => Err(convert_error(err))
    }
}

pub fn convert_error(err: Error) -> OperationError {
    let nt_status = match err {
        Error::IOFailed(_)  => STATUS_INTERNAL_ERROR,
        Error::ReceivedInvalidData(_) => STATUS_INTERNAL_ERROR,
        Error::RequestFailed(err) => match err {
            responses::Error::FileNotFound => STATUS_INVALID_DEVICE_REQUEST,
            responses::Error::NoSuchHandle => STATUS_INVALID_DEVICE_REQUEST,
            responses::Error::FileExists => STATUS_INVALID_DEVICE_REQUEST,
            responses::Error::PermissionDenied => STATUS_ACCESS_DENIED,
            responses::Error::Other => STATUS_INTERNAL_ERROR,
            responses::Error::CouldNotFindDisk => STATUS_NOT_IMPLEMENTED
        }
    };

    OperationError::NtStatus(nt_status)
}

pub struct Client {
    connection: Mutex<TcpStream>
}

impl Client {
    pub fn new(tcp_stream: TcpStream) -> Client {
        Client {
            connection: Mutex::new(tcp_stream)
        }
    }

    pub fn send_keep_connection<T: DeserializeOwned>(&self, request: requests::Request) -> Result<(T, MutexGuard<TcpStream>)> {
        let mut connection = self.connection.lock().unwrap();
        
        let encoded = bincode::serialize(&request).unwrap();
        connection.write_u64::<BigEndian>(encoded.len().try_into().unwrap()).unwrap();
        connection.write(&encoded[..]).unwrap();

        let length = connection.read_u64::<BigEndian>().unwrap();
        let mut buffer = vec![0u8; length as usize];

        connection.read_exact(&mut buffer[..]).unwrap();

        let deserialized: responses::Result<T> = bincode::deserialize(&buffer[..])?;
        let data = deserialized?;
        Ok((data, connection))
    }
    
    pub fn send<'a, T: DeserializeOwned>(&self, request: requests::Request) -> Result<T> {
        Ok(self.send_keep_connection::<T>(request)?.0)
    }

    pub fn list_files(&self, path: &str) -> Result<responses::ListFiles> {
        self.send(requests::Request::List(path.to_string()))
    }

    pub fn get_free_space(&self) -> Result<responses::FreeSpace> {
        self.send(requests::Request::GetFreeSpace)
    }

    pub fn close_file(&self, handle: FileHandle) -> Result<()> {
        self.send(requests::Request::Close(handle))
    }

    pub fn delete_file(&self, path: String) -> Result<()> {
        self.send(requests::Request::Delete(path))
    }

    pub fn open_file(&self, path: String) -> Result<FileHandle> {
        self.send(requests::Request::Open(requests::OpenFile {
            path: path
        }))
    }

    pub fn read_file(&self, handle: FileHandle, offset: u64, buffer: &mut [u8]) -> Result<u32> {
        let req = requests::Request::Read(requests::ReadFile {
            handle: handle,
            offset: offset,
            len: buffer.len() as u64
        });

        // Send the read request first to receive the length read
        let (receive_result, mut connection) = self.send_keep_connection::<responses::ReadFile>(req)?;
        let length_read = receive_result as usize;

        connection.read_exact(&mut buffer[0..length_read])?;
        Ok(length_read as u32)
    }

    pub fn write_file(&self, handle: FileHandle, offset: u64, data: &[u8]) -> Result<()> {
        let req = requests::Request::Write(requests::WriteFile {
            handle: handle,
            offset: offset,
            len: data.len() as u64
        });
        let (_, mut connection) = self.send_keep_connection::<()>(req)?;
        connection.write_all(data)?;
        Ok(())
    }

    pub fn set_end_of_file(&self, handle: FileHandle, len: u64) -> Result<()> {
        let req = requests::Request::SetEndOfFile(requests::SetEndOfFile {
            handle: handle,
            len: len
        });

        self.send(req)
    }

    pub fn stat_file(&self, path: &str) -> Result<responses::StatFile> {
        self.send(requests::Request::Stat(path.to_string()))
    }
}