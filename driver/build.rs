
use std::path::Path;
use std::env;
use std::io;

const SERVER_NAME: &str = "androidfs_server";
const SERVER_TARGET_PATH: &str = "../server/target/aarch64-linux-android/release/";

fn main() {
    let build_type = env::var("PROFILE").unwrap();
    let root_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let orig_path = Path::new(root_dir.as_str()).join(SERVER_TARGET_PATH).join(SERVER_NAME);
    let dest_path = Path::new(root_dir.as_str()).join("target").join(build_type).join(SERVER_NAME);

    match std::fs::copy(orig_path.clone(), dest_path) {
        Ok(_) => {},
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => panic!("Please compile the server project first {:?}", orig_path.to_str()),
            _ => panic!("Could not copy androidfs_server: {}", err)
        }
    }
}