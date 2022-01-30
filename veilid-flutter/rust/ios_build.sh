#!/bin/bash

SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

CARGO_MANIFEST_PATH=$SCRIPTDIR/Cargo.toml

if [ "$CONFIGURATION" == "Debug" ]; then 
    EXTRA_CARGO_OPTIONS="$@"
else
    EXTRA_CARGO_OPTIONS="$@ --release"
fi
ARCHS=${ARCHS:=arm64}
for arch in $ARCHS
do
    if [ "$arch" == "arm64" ]; then
        echo arm64
        CARGO_TARGET=aarch64-apple-ios
        CARGO_TOOLCHAIN=+ios-arm64-1.57.0
        #CARGO_TOOLCHAIN=
    elif [ "$arch" == "x86_64" ]; then
        echo x86_64
        CARGO_TARGET=x86_64-apple-ios
        CARGO_TOOLCHAIN=
    else
        echo Unsupported ARCH: $arch
        continue
    fi
    HOMEBREW_DIR=$(dirname `which brew`)
    FLUTTER_DIR=$(dirname `which flutter`)
    env -i PATH=/usr/bin:/bin:/usr/local/bin:$HOMEBREW_DIR:$FLUTTER_DIR ~/.cargo/bin/cargo $CARGO_TOOLCHAIN build $EXTRA_CARGO_OPTIONS --target $CARGO_TARGET --manifest-path $CARGO_MANIFEST_PATH
done

