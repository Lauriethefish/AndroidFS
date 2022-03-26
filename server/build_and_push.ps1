cargo ndk -t arm64-v8a build --release
adb push ./target/aarch64-linux-android/release/server /data/local/tmp/server
adb shell chmod 555 /data/local/tmp/server