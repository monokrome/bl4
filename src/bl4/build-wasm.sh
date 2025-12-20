#!/bin/bash
set -e

if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed" >&2
    echo "Install: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh" >&2
    echo "    or: cargo install wasm-pack" >&2
    exit 1
fi

wasm-pack build --target bundler --features wasm
