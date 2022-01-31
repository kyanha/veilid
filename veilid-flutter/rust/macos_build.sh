#!/bin/bash

# Setup varaiables
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
FLUTTER_DIR=$(dirname `which flutter`)
HOMEBREW_DIR=$(dirname `which brew`)
CARGO_DIR=$(dirname `which cargo`)
CARGO_MANIFEST_PATH=$(python -c "import os; print(os.path.realpath(\"$SCRIPTDIR/Cargo.toml\"))")
TARGET_DIR=$(dirname `cargo locate-project --message-format plain`)/target

# Configure outputs
OUTPUT_FILENAME=libveilid_flutter.dylib
OUTPUT_DIR=$TARGET_DIR/macos_lib

# Get Rust configurations from xcode configurations
if [ "$CONFIGURATION" == "Debug" ]; then 
    EXTRA_CARGO_OPTIONS="$@"
    RUST_CONFIGURATION="debug"
else
    EXTRA_CARGO_OPTIONS="$@ --release"
    RUST_CONFIGURATION="release"
fi

# Build all the matching architectures for the xcode configurations
ARCHS=${ARCHS:=x86_64}
echo ARCHS: $ARCHS
LIPO_LIST=""
for arch in $ARCHS
do
    if [ "$arch" == "arm64" ]; then
        echo arm64
        CARGO_TARGET=aarch64-apple-darwin
        CARGO_TOOLCHAIN=
    elif [ "$arch" == "x86_64" ]; then
        echo x86_64
        CARGO_TARGET=x86_64-apple-darwin
        CARGO_TOOLCHAIN=
    else
        echo Unsupported ARCH: $arch
        continue
    fi

    # Cargo build
    env -i PATH=/usr/bin:/bin:/usr/local/bin:$HOMEBREW_DIR:$FLUTTER_DIR:$CARGO_DIR HOME="$HOME" USER="$USER" cargo $CARGO_TOOLCHAIN build $EXTRA_CARGO_OPTIONS --target $CARGO_TARGET --manifest-path $CARGO_MANIFEST_PATH

    # Add output to lipo list
    LIPO_LIST="$LIPO_LIST $TARGET_DIR/$CARGO_TARGET/$RUST_CONFIGURATION/$OUTPUT_FILENAME"
done

# Lipo the architectures together
mkdir -p $OUTPUT_DIR
lipo -output "$OUTPUT_DIR/$OUTPUT_FILENAME" -create $LIPO_LIST
