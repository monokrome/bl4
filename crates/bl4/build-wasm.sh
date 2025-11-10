#!/bin/bash
# Build bl4.js WebAssembly package for NPM (bundler target)

set -e

echo "Building bl4.js for WebAssembly (bundler target)..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed"
    echo ""
    echo "Install it with:"
    echo "  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    echo ""
    echo "Or with cargo:"
    echo "  cargo install wasm-pack"
    exit 1
fi

wasm-pack build --target bundler --features wasm

echo ""
echo "Build complete - output in pkg/"
echo "Ready for: npm publish"
