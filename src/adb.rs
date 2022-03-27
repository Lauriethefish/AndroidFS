use std::{process::{Command, Output}, fmt::Display, os::windows::process::CommandExt};
use crate::log::*;

#[derive(Clone, Debug)]
pub struct DebugBridge {
    pub adb_path: String
}

#[derive(Clone, Debug)]
pub struct Device {
    debug_bridge: DebugBridge,
    pub serial_number: String
}

#[derive(Clone, Debug)]
pub struct Error {
    pub output: Output,
    pub kind: ErrorKind
}

#[derive(Clone, Debug)]
pub enum ErrorKind {
    NonSuccessExitCode,
    ParseFailure
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("ADB returned exit code {}", self.output.status))
    }
}


impl Invokeable for Device {
    fn invoke(&self, mut command: Vec<String>) -> Output {
        command.insert(0, "-s".to_string());
        command.insert(1, self.serial_number.clone());

        self.debug_bridge.invoke(command)
    }
}

pub trait Invokeable {
    fn invoke(&self, command: Vec<String>) -> Output;
    
    fn invoke_result(&self, command: Vec<String>) -> Result<Output, Error> {
        let result = self.invoke(command);
        if result.status.success() {
            Ok(result)  
        }   else {
            Err(Error {
                output: result,
                kind: ErrorKind::NonSuccessExitCode
            })
        }
    }

    fn invoke_shell_command_result(&self, mut command: Vec<String>) -> Result<Output, Error> {
        command.insert(0, "shell".to_string());
        self.invoke_result(command)
    }
}

impl DebugBridge {
    pub fn get_devices(&self) -> Result<Vec<Device>, Error> {
        let output = self.invoke_result(vec!["devices".to_string()])?;
        let std_out = String::from_utf8_lossy(&output.stdout);

        let mut result: Vec<Device> = Vec::new();
        for line in std_out.lines() {
            if !line.ends_with("device") {
                continue;
            }
            let serial = match line.split_whitespace().next() {
                Some(serial) => serial,
                None => return Err(Error {
                    kind: ErrorKind::ParseFailure,
                    output: output
                })
            };
            result.push(Device {
                debug_bridge: self.clone(),
                serial_number: serial.to_string()
            })
        }

        Ok(result)
    }
}

impl Invokeable for DebugBridge {
    fn invoke(&self, command: Vec<String>) -> Output {
        trace!("Invoking ADB: {:?}", command);
        Command::new(self.adb_path.clone())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .args(command)
            .output()
            .expect("Invoking ADB failed")
    }
}