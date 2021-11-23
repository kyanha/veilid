#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

if [ ! "$(uname)" == "Darwin" ]; then
    echo Not running on MacOS
    exit 1
fi

# install android targets
rustup target add aarch64-apple-darwin aarch64-apple-ios x86_64-apple-darwin x86_64-apple-ios

echo Manual Step:
echo   install +ios-arm64-nightly-YYYY-MM-DD toolchain for bitcode from https://github.com/getditto/rust-bitcode/releases/latest and unzip
echo   xattr -d -r com.apple.quarantine .
echo   ./install.sh

# Ensure brew is installed
if command -v brew &> /dev/null; then 
    echo '[X] brew is available in the path'
else
    echo 'brew is not available in the path'
    exit 1
fi

# Ensure xcode is installed
if command -v xcode-select &> /dev/null; then 
    echo '[X] XCode is available in the path'
else
    echo 'XCode is not available in the path'
    exit 1
fi

# Ensure we have command line tools
xcode-select --install

# Ensure packages are installed
brew install capnp

