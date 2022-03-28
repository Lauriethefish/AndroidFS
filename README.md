# AndroidFS

A proof of concept for mounting an ADB device as a drive in windows.

## Compilation
1. Install [dokan libary v1.5](https://github.com/dokan-dev/dokany/releases/tag/v1.5.1.1000).
2. Install [rustup](https://rustup.rs/).
3. Run `rustup install nightly` to install nightly rust.
4. Install the rust Android target: `rustup target add aarch64-linux-android`.
5. Download the [Android NDK](https://developer.android.com/ndk/downloads), and set `ANDROID_NDK_HOME` to point to its path on your computer.
6. Run `./build.ps1`.

## Usage

NOTE: AndroidFS is currently in an alpha state, be careful.

1. Download the installer from [actions](https://github.com/Lauriethefish/AndroidFS/actions)
2. Run the installer.
3. Restart your PC.
Devices will be automatically added/removed as drives when connected/disconnected.
The default drive letter is `Q:`

If you wish to enable the console window for debugging, set the `ANDROIDFS_CONSOLE` environment variable to `1`, and set `RUST_LOG` to `DEBUG`. (this requires a restart to take effect)

## Architecture

AndroidFS works with a "server" executable pushed to devices automatically upon connection.
The driver then communicates with this via sockets and `adb forward`.
This approach avoids us having to pull files to temporary locations each time they are opened/edited.

Performance is a major concern with this driver, navigating needs to be as snappy as possible.
To improve this, file stats and directory listings are heavily cached, which is a major speedup, especially since Windows often makes multiple requests per second for the same directory listing and stats when opening a folder in explorer.