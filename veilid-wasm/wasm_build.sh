#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

if [[ "$1" == "release" ]]; then 
    RELEASE=-r
    GENERATE_SOURCE_MAP=
else 
    RELEASE=
    RUSTFLAGS="-O -g"
    GENERATE_SOURCE_MAP="./wasm-sourcemap.py ../target/wasm32-unknown-unknown/debug/veilid_wasm.wasm -o ../target/wasm32-unknown-unknown/debug/veilid_wasm.wasm.map --dwarfdump `which llvm-dwarfdump`"
fi

pushd $SCRIPTDIR 2> /dev/null
cargo build --target wasm32-unknown-unknown $RELEASE
$GENERATE_SOURCE_MAP
popd 2> /dev/null