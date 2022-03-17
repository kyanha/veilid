#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

if [ ! "$(uname)" == "Darwin" ]; then
    echo Not running on MacOS
    exit 1
fi

# install targets
rustup target add aarch64-apple-darwin aarch64-apple-ios x86_64-apple-darwin x86_64-apple-ios

# install cargo packages
cargo install wasm-bindgen-cli

# install bitcode compatible ios toolchain
echo Manual Step:
echo   install +ios-arm64-1.57.0 toolchain for bitcode from https://github.com/getditto/rust-bitcode/releases/latest and unzip
echo   xattr -d -r com.apple.quarantine .
echo   ./install.sh

# ensure brew is installed
if command -v brew &> /dev/null; then 
    echo '[X] brew is available in the path'
else
    echo 'brew is not available in the path'
    exit 1
fi

# ensure xcode is installed
if command -v xcode-select &> /dev/null; then 
    echo '[X] XCode is available in the path'
else
    echo 'XCode is not available in the path'
    exit 1
fi

# ensure we have command line tools
xcode-select --install

# ensure packages are installed
if [ "$BREW_USER" == "" ]; then
    BREW_USER=`ls -lad /opt/homebrew/. | cut -d\  -f4`
    echo "Must sudo to homebrew user \"$BREW_USER\" to install capnp package:"
fi
sudo -H -u $BREW_USER brew install capnp

