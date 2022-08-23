#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

if [ "$(lsb_release -d | grep -qEi 'debian|buntu|mint')" ]; then
    echo Not a supported Linux
    exit 1
fi

# ensure ANDROID_SDK_ROOT is defined and exists
if [ -d "$ANDROID_SDK_ROOT" ]; then
    echo '[X] $ANDROID_SDK_ROOT is defined and exists' 
else
    echo '$ANDROID_SDK_ROOT is not defined or does not exist'
    exit 1
fi

# ensure ANDROID_NDK_HOME is defined and exists
if [ -d "$ANDROID_NDK_HOME" ]; then
    echo '[X] $ANDROID_NDK_HOME is defined and exists' 
else
    echo '$ANDROID_NDK_HOME is not defined or does not exist'
    exit 1
fi

# ensure ndk is installed
if [ -f "$ANDROID_NDK_HOME/ndk-build" ]; then
    echo '[X] Android NDK is installed at the location $ANDROID_NDK_HOME' 
else
    echo 'Android NDK is not installed at the location $ANDROID_NDK_HOME'
    exit 1
fi

# ensure cmake is installed
if [ -d "$ANDROID_SDK_ROOT/cmake" ]; then
    echo '[X] Android SDK CMake is installed' 
else
    echo 'Android SDK CMake is not installed'
    exit 1
fi

# ensure emulator is installed
if [ -d "$ANDROID_SDK_ROOT/emulator" ]; then
    echo '[X] Android SDK emulator is installed' 
else
    echo 'Android SDK emulator is not installed'
    exit 1
fi

# ensure adb is installed
if command -v adb &> /dev/null; then 
    echo '[X] adb is available in the path'
else
    echo 'adb is not available in the path'
    exit 1
fi

# ensure rustup is installed
if command -v rustup &> /dev/null; then 
    echo '[X] rustup is available in the path'
else
    echo 'rustup is not available in the path'
    exit 1
fi

# ensure cargo is installed
if command -v cargo &> /dev/null; then 
    echo '[X] cargo is available in the path'
else
    echo 'cargo is not available in the path'
    exit 1
fi

# install targets
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android wasm32-unknown-unknown

# install cargo packages
cargo install wasm-bindgen-cli

# Ensure packages are installed
sudo apt-get install libc6-dev-i386 libc6:i386 libncurses5:i386 libstdc++6:i386 lib32z1 libbz2-1.0:i386 openjdk-11-jdk llvm wabt capnproto
