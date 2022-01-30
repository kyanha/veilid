#!/bin/bash

SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
CARGO_MANIFEST_PATH=$(python -c "import os; print(os.path.realpath(\"$SCRIPTDIR/Cargo.toml\"))")
# echo CARGO_MANIFEST_PATH: $CARGO_MANIFEST_PATH 

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
    elif [ "$arch" == "x86_64" ]; then
        echo x86_64
        CARGO_TARGET=x86_64-apple-ios
        CARGO_TOOLCHAIN=
    else
        echo Unsupported ARCH: $arch
        continue
    fi
    FLUTTER_DIR=$(dirname `which flutter`)
    HOMEBREW_DIR=$(dirname `which brew`)
    CARGO_DIR=$(dirname `which cargo`)
    env -i PATH=/usr/bin:/bin:/usr/local/bin:$HOMEBREW_DIR:$FLUTTER_DIR:$CARGO_DIR HOME="$HOME" USER="$USER" cargo $CARGO_TOOLCHAIN build $EXTRA_CARGO_OPTIONS --target $CARGO_TARGET --manifest-path $CARGO_MANIFEST_PATH
done

