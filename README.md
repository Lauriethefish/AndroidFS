# AndroidFS

A proof of concept for mounting an ADB device as a drive in windows.

## Compilation
1. Install [dokan libary v1.5](https://github.com/dokan-dev/dokany/releases/tag/v1.5.1.1000).
2. Install [rustup](https://rustup.rs/).
3. Install the rust Android target: `rustup target add aarch64-linux-android`.
4. Download the [Android NDK](https://developer.android.com/ndk/downloads), and set `ANDROID_NDK_HOME` to point to its path on your computer.
5. Compile the server by navigating to `./server` and running `cargo ndk -t arm64-v8a --release`.
6. Compile by navigating to `./driver` and running `cargo build --release`.

## Usage

NOTE: Prebuilt binaries are not currently available.

Run `./android_fs.exe`.
Devices will be automatically added/removed as drives when connected/disconnected.

The default drive letter is `Q:`

## Architecture

AndroidFS works with a "server" executable pushed to devices automatically upon connection.
The driver then communicates with this via sockets and `adb forward`.
This approach avoids us having to pull files to temporary locations each time they are opened/edited.

Performance is a major concern with this driver, navigating needs to be as snappy as possible.
To improve this, file stats and directory listings are heavily cached, which is a major speedup, especially since Windows often makes multiple requests per second for the same directory listing and stats when opening a folder in explorer.