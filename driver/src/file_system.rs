use std::time::Duration;

use shared::*;
use crate::client;
use crate::cache::Cache;

use dokan::*;
use shared::models::FileHandle;
use widestring::{U16CStr, U16CString};
use winapi::um::winnt::{self, PSECURITY_DESCRIPTOR, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_RECALL_ON_OPEN};
use winapi::shared::ntstatus::*;
use crate::log::*;

pub struct QuestFsHandler {
    volume_name: U16CString,
	client: client::Client,
	directory_cache: Cache<String, Result<Vec<shared::models::FileInfo>, OperationError>>,
	stat_cache: Cache<String, Result<shared::models::FileInfo, OperationError>>
}


// Converts winnnt to *nix file names (\ to /)
fn convert_file_name(win_file_name: &U16CStr) -> String {
	win_file_name.to_string_lossy().replace("\\", "/")
}

// Approximates the linux file mode using winnt file attributes
fn convert_attributes(file: &shared::models::FileInfo) -> u32 {
	// Tell windows that our files take longer to access
	// Unfortunately it doesn't seem as though this actually changes the way it treats them (it will still try to open all the images in a folder to preview them, for example)
	let mut attributes = FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_RECALL_ON_OPEN;

	if file.mode & 0xa000 == 0xa000 {
		// Symlinks
		attributes |= FILE_ATTRIBUTE_DIRECTORY | /* TODO: Explorer does not recognise this and leaves the icon as is */ FILE_ATTRIBUTE_REPARSE_POINT;
	}
	if file.mode & 0x4000 == 0x4000 {
		attributes |= FILE_ATTRIBUTE_DIRECTORY;
	}

	attributes
}

impl QuestFsHandler {
    pub fn new(client: client::Client, volume_name: U16CString) -> Self {
        QuestFsHandler { 
            volume_name: volume_name,
            client: client,
			// TODO: Test these values more and see what is reasonable in terms of accuracy and speed
            directory_cache: Cache::new(Duration::from_millis(1000), 50),
			stat_cache: Cache::new(Duration::from_secs(1000), 1000)
        }
    }

	// Stats or returns the cached stat of file_name
	fn stat_file(&self, file_name: String) -> Result<models::FileInfo, OperationError> {
		match self.stat_cache.try_get(&file_name) {
			Some(cached) => cached,
			None => {
				let stat_result = client::convert_response(self.client.stat_file(&file_name));

				self.stat_cache.put(file_name, stat_result.clone());
				stat_result
			}
		}
	}
}

impl<'a, 'b: 'a> FileSystemHandler<'a, 'b> for QuestFsHandler {
    type Context = FileHandle;

    fn create_file(
		&'b self,
		win_file_name: &U16CStr,
		_security_context: &DOKAN_IO_SECURITY_CONTEXT,
		_desired_access: winnt::ACCESS_MASK,
		_file_attributes: u32,
		_share_access: u32,
		create_disposition: u32,
		_create_options: u32,
		_info: &mut OperationInfo<'a, 'b, Self>,
	) -> Result<CreateFileInfo<Self::Context>, OperationError> {
		// TODO: Finish this method (use memfs as reference)

		let file_name = convert_file_name(win_file_name);

		// TODO: Define constants for these
		if create_disposition == 1 || create_disposition == 3 { // OPEN/OPEN-IF
			let stat = self.stat_file(file_name.clone())?;

			if convert_attributes(&stat) & FILE_ATTRIBUTE_DIRECTORY == FILE_ATTRIBUTE_DIRECTORY {
				return Ok(CreateFileInfo {
					context: 0,
					is_dir: true,
					new_file_created: true
				})
			}

			let handle = client::convert_response(self.client.open_file(file_name))?;

			Ok(CreateFileInfo {
				context: handle,
				is_dir: false,
				new_file_created: false
			})
		}	else	{
			Ok(CreateFileInfo {
				context: 0,
				is_dir: true,
				new_file_created: true
			})
		}
	}

    fn close_file(
		&'b self,
		_file_name: &U16CStr,
		_info: &OperationInfo<'a, 'b, Self>,
		context: &'a Self::Context,
	) {
		// Windows randomly calls this method with context 0 for seemingly no reason
		if *context == 0 {
			return;
		}

		match self.client.close_file(*context) {
			Ok(_) => {},
			Err(_) => error!("Warning: Could not close handle {} - did not exist", context),
		}
	}

    fn read_file(
		&'b self,
		_: &U16CStr,
		offset: i64,
		buffer: &mut [u8],
		_info: &OperationInfo<'a, 'b, Self>,
		context: &'a Self::Context,
	) -> Result<u32, OperationError> {
		client::convert_response(self.client.read_file(*context, offset as u64, buffer))
	}

    fn write_file(
		&'b self,
		_: &U16CStr,
		offset: i64,
		buffer: &[u8],
		_info: &OperationInfo<'a, 'b, Self>,
		context: &'a Self::Context,
	) -> Result<u32, OperationError> {
		client::convert_response(self.client.write_file(*context, offset as u64, buffer))?;

		Ok(buffer.len() as u32)
	}

    fn flush_file_buffers(
		&'b self,
		_file_name: &U16CStr,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO: Keep some written data in a buffer and flush here?
		Ok(())
	}

    fn get_file_information(
		&'b self,
		win_file_name: &U16CStr,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<FileInfo, OperationError> {
		let file_name = convert_file_name(win_file_name);
		let file_info = self.stat_file(file_name)?;

		Ok(FileInfo {
			attributes: convert_attributes(&file_info),
			creation_time: file_info.creation_time,
			last_access_time: file_info.last_accessed,
			last_write_time: file_info.last_modified,
			file_size: file_info.size,
			number_of_links: 0, // TODO: Find a way to fetch this data?
			file_index: file_info.ino
		})
	}

    fn find_files(
		&'b self,
		win_file_name: &U16CStr,
		mut fill_find_data: impl FnMut(&FindData) -> Result<(), FillDataError>,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		let file_name = convert_file_name(win_file_name);

		let files: Vec<models::FileInfo> = match self.directory_cache.try_get(&file_name) {
			Some(files) => files,
			None => {
				let result = client::convert_response(self.client.list_files(file_name.as_str()));
				self.directory_cache.put(file_name.clone(), result.clone());
				result
			}
		}?;

		for file in &files {
			fill_find_data.call_mut((&FindData {
				attributes: convert_attributes(file),
				creation_time: file.creation_time,
				last_access_time: file.last_accessed,
				last_write_time: file.last_modified,
				file_size: file.size,
				file_name: U16CString::from_str(file.name.as_str()).unwrap()
			},))?;
		}

		Ok(())
	}

    fn set_file_attributes(
		&'b self,
		_file_name: &U16CStr,
		_file_attributes: u32,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		Err(OperationError::NtStatus(STATUS_NOT_IMPLEMENTED))
	}

    fn set_file_time(
		&'b self,
		_file_name: &U16CStr,
		_creation_time: FileTimeInfo,
		_last_access_time: FileTimeInfo,
		_last_write_time: FileTimeInfo,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO?: Is there even any point to this
		Ok(())
	}

    fn delete_file(
		&'b self,
		win_file_name: &U16CStr,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO: Never called, likely due to incomplete create_file

		let file_name = convert_file_name(win_file_name);
		client::convert_response(self.client.delete_file(file_name))
	}

    fn delete_directory(
		&'b self,
		win_file_name: &U16CStr,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO: Never called, likely due to incomplete open_file

		let file_name = convert_file_name(win_file_name);
		client::convert_response(self.client.delete_file(file_name))
	}

    fn move_file(
		&'b self,
		_file_name: &U16CStr,
		_new_file_name: &U16CStr,
		_replace_if_existing: bool,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO
		Err(OperationError::NtStatus(STATUS_NOT_IMPLEMENTED))
	}

    fn set_end_of_file(
		&'b self,
		_file_name: &U16CStr,
		offset: i64,
		_info: &OperationInfo<'a, 'b, Self>,
		context: &'a Self::Context,
	) -> Result<(), OperationError> {
		client::convert_response(self.client.set_end_of_file(*context, offset as u64))
	}

    fn set_allocation_size(
		&'b self,
		_file_name: &U16CStr,
		_alloc_size: i64,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		Ok(())
	}

    fn get_disk_free_space(
		&'b self,
		_info: &OperationInfo<'a, 'b, Self>,
	) -> Result<DiskSpaceInfo, OperationError> {
		let free_space = client::convert_response(self.client.get_free_space())?;

		Ok(DiskSpaceInfo {
			byte_count: free_space.total_bytes,
			free_byte_count: free_space.free_bytes,
			available_byte_count: free_space.free_bytes
		})
	}

    fn get_volume_information(
		&'b self,
		_info: &OperationInfo<'a, 'b, Self>,
	) -> Result<VolumeInfo, OperationError> {
		Ok(VolumeInfo {
                name: self.volume_name.clone(),
                serial_number: 0,
                max_component_length: 4095, // Path length limit on Android
                fs_flags: winnt::FILE_CASE_PRESERVED_NAMES | winnt::FILE_CASE_SENSITIVE_SEARCH | winnt::FILE_UNICODE_ON_DISK
				| winnt::FILE_PERSISTENT_ACLS | winnt::FILE_NAMED_STREAMS,
                fs_name: U16CString::from_str("NTFS").unwrap() // Windows will recognise NTFS and turn on all features
        })
	}

    fn get_file_security(
		&'b self,
		_file_name: &U16CStr,
		_security_information: u32,
		_security_descriptor: PSECURITY_DESCRIPTOR,
		_buffer_length: u32,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<u32, OperationError> {
		// TODO?: Is this necessary
		Err(OperationError::NtStatus(STATUS_NOT_IMPLEMENTED))
	}

    fn set_file_security(
		&'b self,
		_file_name: &U16CStr,
		_security_information: u32,
		_security_descriptor: PSECURITY_DESCRIPTOR,
		_buffer_length: u32,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		// TODO?: Is this necessary
		Err(OperationError::NtStatus(STATUS_NOT_IMPLEMENTED))
	}

    fn find_streams(
		&'b self,
		_file_name: &U16CStr,
		_fill_find_stream_data: impl FnMut(&FindStreamData) -> Result<(), FillDataError>,
		_info: &OperationInfo<'a, 'b, Self>,
		_context: &'a Self::Context,
	) -> Result<(), OperationError> {
		Err(OperationError::NtStatus(STATUS_NOT_IMPLEMENTED))
	}
}