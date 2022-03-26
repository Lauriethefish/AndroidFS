#![feature(fn_traits)]
extern crate dokan;
extern crate widestring;
extern crate winapi;
extern crate androidfs_shared;
extern crate bincode;
extern crate byteorder;
extern crate linked_hash_map;
extern crate serde;
extern crate log;
extern crate env_logger;

mod adb;
mod client;
mod cache;
mod file_system;
use dokan::{Drive, MountFlags};
use file_system::*;
use widestring::U16CString;
use log::*;
use std::collections::HashSet;
use std::net::{SocketAddrV4, Ipv4Addr};

use std::{time::Duration, sync::{Arc, Mutex}, net::TcpStream};
use adb::Invokeable;

use client::Client;

const BASE_PORT: u16 = 15000;
const MAX_PORT: u16 = 16000;

// List the mount points in the order we prefer to use them
const MOUNT_POINTS: &[&'static str] = &[
	"Q:",
	"R:",
	"S:",
	"T:",
	"U:",
	"V:",
	"W:",
	"X:",
	"Y:",
	"Z:",
	"D:",
	"E:",
	"F:",
	"G:",
	"H:",
	"I:",
	"J:",
	"K:",
	"L:",
	"M:",
	"N:",
	"O:",
	"P:"
];

enum SetupError {
	NoAvailablePort,
	NoAvailableDriveLetter,
	AdbError(adb::Error),
	DaemonUnreachable
}

impl From<adb::Error> for SetupError {
    fn from(err: adb::Error) -> Self {
        Self::AdbError(err)
    }
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::AdbError(err) => f.write_fmt(format_args!("Invoking ADB failed: {}", err)),
			Self::NoAvailablePort => f.write_str("No port available to forward the daemon to"),
			Self::NoAvailableDriveLetter => f.write_str("No drive letter available"),
            Self::DaemonUnreachable => f.write_str("Failed to connect to the daemon"),
		}?;
		Ok(())
    }
}

fn setup(device: adb::Device, drive_map: Arc<Mutex<HashSet<String>>>) -> Result<String, SetupError> {
	info!("Attempting to mount {}", device.serial_number);

	// To allow us to access our daemon, we need to search for available ports for adb forward
	debug!("Searching for available port in the range {} to {}", BASE_PORT, MAX_PORT);
	let mut chosen_port: Option<u16> = None;
	for port in BASE_PORT..=MAX_PORT {
		debug!("Attempting forward to {}", port);
		match device.invoke_result(vec!["forward".to_string(), format!("tcp:{}", port), "tcp:12345".to_string()]) {
			Ok(_) => {
				chosen_port = Some(port);
				break;
			},
			Err(err) => {
				trace!("Port failed: {}", err)
			}
		}
	}

	let port = match chosen_port {
		Some(port) => port,
		None => {
			return Err(SetupError::NoAvailablePort)
		}
	};
	debug!("Forwarded to {}", port);

	let mount_point = match MOUNT_POINTS
	.iter()
	.filter(|p| !std::path::Path::new(&format!("{}\\", p)).exists())
	.next() {
		Some(mount_point) => mount_point,
		None => return Err(SetupError::NoAvailableDriveLetter)
	};

	// Push the daemon and make it executable
	device.invoke_result(vec!["push".to_string(), "./androidfs_server".to_string(), "/data/local/tmp".to_string()])?;
	device.invoke_shell_command_result(vec!["chmod".to_string(), "555".to_string(), "/data/local/tmp/androidfs_server".to_string()])?;
	
	{
		let drive_map = drive_map.clone();
		let device = device.clone();
		std::thread::spawn(move || {
			debug!("Hello from daemon thread");
			let device = device;
			match device.invoke_shell_command_result(vec!["./data/local/tmp/androidfs_server".to_string()]) {
				Ok(_) => {},
				Err(err) => {
					error!("Invoking daemon failed: {}", err)
				}
			};

			// The daemon process exiting indicates that the device has been unplugged

			// Make sure to only unmount if the mount thread has not exited for some other reason
			// e.g. an error during startup
			// If we unmounted in that case it could lead us to unmount another device
			if drive_map.lock().unwrap().contains(&device.serial_number) {
				if !dokan::unmount(U16CString::from_str(mount_point).unwrap()) {
					warn!("Failed to unmount {}", device.serial_number)
				}
		
				info!("Device {} disconnected", device.serial_number);
			}
		});
	}

	// Make sure that the daemon has had time to start up
	std::thread::sleep(Duration::from_millis(500));

	let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
	let tcp_stream = match TcpStream::connect(address) {
		Ok(tcp_stream) => tcp_stream,
		Err(_) => return Err(SetupError::DaemonUnreachable)
	};

	let volume_name = U16CString::from_str(device.serial_number.clone()).unwrap();
	let flags = MountFlags::CASE_SENSITIVE;

	{
		let drive_map = drive_map.clone();
		let device = device.clone();

		// Start a new thread which mounts the drive (once the drive is mounted, the thread is blocked)
		std::thread::spawn(move || {
			match Drive::new()
			.mount_point(&U16CString::from_str(mount_point).unwrap())
			.flags(flags)
			.thread_count(0)
			.mount(&QuestFsHandler::new(Client::new(tcp_stream), volume_name.clone())) {
				Ok(_) => debug!("Mount thread exited"),
				Err(err) => {
					error!("Mount error: {}", err);
				}
			};
			drive_map.lock().unwrap().remove(&device.serial_number);
		});
	}

	drive_map.lock().unwrap().insert(device.serial_number.clone()); // Avoid this device getting added again
	Ok(mount_point.to_string())
}

fn main() {
	match std::env::var("RUST_LOG") {
		Ok(_) => env_logger::init(), // Use the given log level if set
		// Otherwise, default to DEBUG for now
		Err(_) => env_logger::Builder::from_default_env().filter(None, LevelFilter::Debug).init()
	}

	let adb = adb::DebugBridge {
		adb_path: r"adb.exe".to_string()
	};
	
	let mounted_devices: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
	loop {
		let devices = adb.get_devices().unwrap();

		let devices_guard = mounted_devices.lock().unwrap();
		let mut to_mount: Vec<adb::Device> = Vec::new();
		for device in devices {
			if !devices_guard.contains(&device.serial_number) {
				to_mount.push(device.clone())
			}
		}
		drop(devices_guard);

		for device in to_mount {
			match setup(device.clone(), mounted_devices.clone()) {
				Ok(drive) => {
					info!("Mounted {} as {}", device.serial_number, drive);
				},
				Err(err) => {
					error!("Failed to mount: {}", err)
				}
			}
		}

		std::thread::sleep(Duration::from_millis(100));
	}
}
