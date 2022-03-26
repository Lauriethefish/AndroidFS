extern crate byteorder;
extern crate bincode;
extern crate sysinfo;
extern crate rand;
extern crate serde;

mod requests;
mod responses;
mod models;
use models::*;

use rand::Rng;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use sysinfo::{SystemExt, DiskExt};

use std::convert::TryInto;
use std::net::TcpStream;
use std::{net::TcpListener, io::Read};
use std::io::{Write, Seek};
use std::fs;
use std::time::SystemTime;
use std::os::unix::fs::MetadataExt;
use std::collections::HashMap;

use serde::Serialize;

type FileHandleMap = HashMap<FileHandle, fs::File>;

fn main() {
    println!("Starting up server");
    let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
    let (mut client, _) = listener.accept().unwrap();

    let mut file_handles = FileHandleMap::new();

    loop {
        // Read request length
        let length = match client.read_u64::<BigEndian>() {
            Ok(length) => length,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    println!("Stopping server");
                    return;
                }
                panic!("{}", err)
            }
        };

        // Read request into a local buffer
        let mut buf = vec![0u8; length as usize];
        client.read_exact(&mut buf[..]).unwrap();

        // Deserialize the request
        let request = bincode::deserialize::<requests::Request>(&buf[..]).unwrap();
        match request {
            requests::Request::List(req) => write_response(&mut client, handle_list_files(req)),
            requests::Request::CreateFile(_) => todo!(),
            requests::Request::CreateDirectory(_) => todo!(),
            requests::Request::Open(req) => write_response(&mut client, handle_open(req, &mut file_handles)),
            requests::Request::Delete(req) => write_response(&mut client, handle_delete_file(req)),
            requests::Request::Move(_) => todo!(),
            requests::Request::GetFreeSpace => write_response(&mut client, handle_get_free_space()),
            requests::Request::Stat(req) => write_response(&mut client, handle_stat_file(req)),
            requests::Request::Read(req) => handle_read_file(req, &mut file_handles, &mut client),
            requests::Request::Write(req) => handle_write_file(req, &mut file_handles, &mut client),
            requests::Request::Close(req) => write_response(&mut client, handle_close(req, &mut file_handles)),
            requests::Request::SetEndOfFile(req) => write_response(&mut client, handle_set_end_of_file(req, &mut file_handles))
        };
    }
}

fn write_response<T: Serialize>(client: &mut TcpStream, response: responses::Result<T>) {
    let encoded_response = bincode::serialize(&response).unwrap();
    client.write_u64::<BigEndian>(encoded_response.len().try_into().unwrap()).unwrap();
    client.write_all(&encoded_response[..]).unwrap();
}



fn unwrap_or_epoch(result: std::io::Result<SystemTime>) -> SystemTime {
    match result {
        Ok(time) => time,
        Err(_) => SystemTime::UNIX_EPOCH
    }
}

fn handle_close(request: requests::CloseFile, file_handles: &mut FileHandleMap) -> responses::Result<()> {
    match file_handles.remove(&request) {
        Some(_) => Ok(()),
        None => Err(responses::Error::NoSuchHandle)
    }
}

fn handle_set_end_of_file(request: requests::SetEndOfFile, file_handles: &mut FileHandleMap) -> responses::Result<()> {
    let file = match file_handles.get(&request.handle) {
        Some(file) => file,
        None => return Err(responses::Error::NoSuchHandle)
    };
    
    match file.set_len(request.len) {
        Ok(_) => Ok(()),
        Err(err) => Err(to_response_error(err))
    }
}

fn handle_open(request: requests::OpenFile, file_handles: &mut FileHandleMap) -> responses::Result<FileHandle> {
    match std::fs::OpenOptions::new()
        .create(false)
        .write(true)
        .read(true)
        .open(request.path) {
        Ok(file) => {
            let mut rng = rand::thread_rng();
            let mut handle_id: u32;
            loop {
                handle_id = rng.gen_range(1..u32::MAX);
                if !file_handles.contains_key(&handle_id) {
                    break;
                }
            }

            file_handles.insert(handle_id, file);
            Ok(handle_id)
        },
        Err(err) => Err(to_response_error(err))
    }
}

fn handle_read_file(request: requests::ReadFile, file_handles: &mut FileHandleMap, client: &mut TcpStream) {
    let file = match file_handles.get_mut(&request.handle) {
        Some(file) => file,
        None => {
            write_response::<responses::ReadFile>(client, Err(responses::Error::NoSuchHandle));
            return;
        }
    };
    let file_length = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(err) => {
            write_response::<responses::ReadFile>(client, Err(to_response_error(err)));
            return;
        }
    };
    match file.seek(std::io::SeekFrom::Start(request.offset)) {
        Ok(_) => {},
        Err(err) => {
            write_response::<responses::ReadFile>(client, Err(to_response_error(err)));
            return;
        }
    };


    let length_readable = std::cmp::min(file_length - request.offset, request.len);

    write_response::<responses::ReadFile>(client, Ok(length_readable as u32));

    let mut limited = file.take(length_readable);

    std::io::copy(&mut limited, client).unwrap();
}


fn handle_write_file(request: requests::WriteFile, file_handles: &mut FileHandleMap, client: &mut TcpStream) {
    let file = match file_handles.get_mut(&request.handle) {
        Some(file) => file,
        None => {
            write_response::<responses::ReadFile>(client, Err(responses::Error::NoSuchHandle));
            return;
        }
    };
    match file.seek(std::io::SeekFrom::Start(request.offset)) {
        Ok(_) => {},
        Err(err) => {
            write_response::<responses::ReadFile>(client, Err(to_response_error(err)));
            return;
        }
    };

    write_response::<()>(client, Ok(()));

    std::io::copy(&mut client.take(request.len), file).unwrap();
}

fn handle_delete_file(request: requests::DeleteFile) -> responses::Result<()> {
    let metadata = match fs::metadata(request.clone()) {
        Ok(metadata) => metadata,
        Err(err) => return Err(to_response_error(err))
    };

    println!("Deleting {}", request);
    match if metadata.is_dir() { std::fs::remove_dir_all(request) } else { std::fs::remove_file(request) } {
        Ok(_) => Ok(()),
        Err(err) => Err(to_response_error(err))
    }
}


fn handle_get_free_space() -> responses::Result<responses::FreeSpace> {
    let mut system = sysinfo::System::default();
    system.refresh_disks_list();
    system.refresh_disks();

    for disk in system.disks() {
        if disk.mount_point().to_string_lossy() == "/storage/emulated" {
            return Ok(responses::FreeSpace {
                total_bytes: disk.total_space(),
                free_bytes: disk.available_space()
            });
        }
    }

    Err(responses::Error::CouldNotFindDisk)
}

fn handle_list_files(request: requests::ListFiles) -> responses::Result<responses::ListFiles> {
    match fs::read_dir(request) {
        Ok(files) => {
            let files = files.filter_map(|f| { 
                let entry = match f {
                    Ok(file) => file,
                    Err(_) => return None
                };

                let metadata = match fs::metadata(entry.path()) {
                    Ok(metadata) => metadata,
                    Err(_) => return None,
                };

                Some(metadata_to_file_info(entry.file_name().to_string_lossy().to_string(), metadata))
            }).collect();

            Ok(files)
        },
        Err(err) => Err(to_response_error(err))
    }
}

fn handle_stat_file(request: requests::StatFile) -> responses::Result<responses::StatFile> {
    match fs::metadata(&request) {
        Ok(metadata) => Ok(metadata_to_file_info(request, metadata)),
        Err(err) => {
            Err(to_response_error(err))
        }
    }
}

fn metadata_to_file_info(file_name: String, metadata: fs::Metadata) -> FileInfo {
    FileInfo 
    {
        name: file_name,
        size: metadata.len(),
        last_accessed: unwrap_or_epoch(metadata.accessed()),
        last_modified: unwrap_or_epoch(metadata.modified()),
        creation_time: unwrap_or_epoch(metadata.created()),
        mode: metadata.mode(),
        ino: metadata.ino()
    }
}

fn to_response_error(err: std::io::Error) -> responses::Error {
    match err.raw_os_error() {
        Some(os_err) => {
            match os_err {
                2 => responses::Error::FileNotFound,
                13 => responses::Error::PermissionDenied ,
                _ => responses::Error::Other
            }
        },
        _ => responses::Error::Other,
    }
}