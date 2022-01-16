#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

OS="unknown"
if [ "$(uname)" == "Linux" ]; then
    if [ ! "$(grep -Ei 'debian|buntu|mint' /etc/*release)" ]; then
        echo Not a supported Linux
        exit 1
    fi
    OS="linux"
elif [ "$(uname)" == "Darwin" ]; then
    OS="macos"
fi
if [ "$OS" == "unknown" ]; then
    echo "Not a supported operating system for this script"
    exit 1
fi

# ensure flutter is installed
if command -v flutter &> /dev/null; then 
    echo '[X] flutter is available in the path'
else
    echo 'flutter is not available in the path, install flutter from here: https://docs.flutter.dev/get-started/install'
    exit 1
fi

# ensure dart is installed
if command -v dart &> /dev/null; then 
    echo '[X] dart is available in the path'
else
    echo 'dart is not available in the path, check your environment variables and that Flutter was installed correctly'
    exit 1
fi

# ensure cargo is installed
if command -v cargo &> /dev/null; then 
    echo '[X] cargo is available in the path'
else
    echo 'cargo is not available in the path, ensure Rust is installed correctly'
    exit 1
fi

# install cargo cbindgen
cargo install cbindgen

# install dart ffigen
dart pub global activate ffigen

# install flutter_rust_bridge_codegen
cargo install flutter_rust_bridge_codegen

# Ensure packages are installed
if [ "$OS" == "linux" ]; then
    sudo apt-get install libclang-dev
elif [ "$OS" == "macos" ]; then
    brew install llvm
fi


