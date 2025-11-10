#!/bin/bash
# Build bl4.js WebAssembly package

set -e

echo "Building bl4.js for WebAssembly..."

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

# Accept target from argument or prompt
if [ -n "$1" ]; then
    choice=$1
else
    # Build for different targets
    echo "Choose target:"
    echo "  1) web       - For use in browsers via <script type=module>"
    echo "  2) bundler   - For webpack/rollup/parcel"
    echo "  3) nodejs    - For Node.js"
    echo "  4) all       - Build all targets"
    read -p "Select (1-4): " choice
fi

build_target() {
    local target=$1
    local outdir=${2:-pkg/$target}
    echo ""
    echo "Building for $target..."
    wasm-pack build --target $target --features wasm --out-dir $outdir
    echo "Built to $outdir/"
}

case $choice in
    1)
        build_target web pkg
        ;;
    2)
        build_target bundler pkg
        ;;
    3)
        build_target nodejs pkg
        ;;
    4)
        build_target web pkg/web
        build_target bundler pkg/bundler
        build_target nodejs pkg/nodejs
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "Build complete"
echo ""
echo "See README-WASM.md for usage instructions"
