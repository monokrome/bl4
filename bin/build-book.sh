#!/usr/bin/env bash
#
# build-book.sh - Build the BL4 Reverse Engineering Guide using Quarto
#
# Usage:
#   ./bin/build-book.sh [options]
#
# Options:
#   --html       Build HTML only
#   --pdf        Build PDF only
#   --epub       Build EPUB only
#   --all        Build all formats (default)
#   --clean      Clean output directory before building
#   --preview    Start Quarto preview server
#
# Requirements:
#   - quarto (https://quarto.org)
#   - For PDF: tinytex or system TeX (quarto will prompt to install tinytex)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
QUARTO_DIR="$PROJECT_ROOT/docs/quarto"
OUTPUT_DIR="$PROJECT_ROOT/docs/rendered/quarto"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Build flags
BUILD_HTML=false
BUILD_PDF=false
BUILD_EPUB=false
BUILD_ALL=true
CLEAN=false
PREVIEW=false

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

header() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

usage() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --html       Build HTML only"
    echo "  --pdf        Build PDF only"
    echo "  --epub       Build EPUB only"
    echo "  --all        Build all formats (default)"
    echo "  --clean      Clean output directory before building"
    echo "  --preview    Start Quarto preview server"
    echo "  -h, --help   Show this help message"
    exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --html)
            BUILD_HTML=true
            BUILD_ALL=false
            shift
            ;;
        --pdf)
            BUILD_PDF=true
            BUILD_ALL=false
            shift
            ;;
        --epub)
            BUILD_EPUB=true
            BUILD_ALL=false
            shift
            ;;
        --all)
            BUILD_ALL=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        --preview)
            PREVIEW=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Check for quarto
check_deps() {
    if ! command -v quarto >/dev/null 2>&1; then
        error "quarto not found. Install from https://quarto.org/docs/get-started/"
    fi
    info "Using $(quarto --version)"
}

# Clean output directory
clean_build() {
    if [ -d "$OUTPUT_DIR" ]; then
        info "Cleaning output directory..."
        rm -rf "$OUTPUT_DIR"
    fi
}

# Build with quarto
build() {
    local formats=()

    if $BUILD_ALL; then
        formats=("html" "pdf" "epub")
    else
        $BUILD_HTML && formats+=("html")
        $BUILD_PDF && formats+=("pdf")
        $BUILD_EPUB && formats+=("epub")
    fi

    for format in "${formats[@]}"; do
        header "Building $format"
        info "Running: quarto render $QUARTO_DIR --to $format"

        if quarto render "$QUARTO_DIR" --to "$format"; then
            info "$format build complete"
        else
            error "$format build failed"
        fi
    done
}

# Start preview server
preview() {
    header "Starting Quarto Preview Server"
    info "Press Ctrl+C to stop"
    quarto preview "$QUARTO_DIR"
}

# Show build results
show_results() {
    echo ""
    header "Build Complete"
    echo ""

    if [ -d "$OUTPUT_DIR" ]; then
        if [ -f "$OUTPUT_DIR/index.html" ]; then
            echo -e "  ${GREEN}✓${NC} HTML: $OUTPUT_DIR/index.html"
        fi

        # Find PDF files
        for pdf in "$OUTPUT_DIR"/*.pdf; do
            [ -f "$pdf" ] && echo -e "  ${GREEN}✓${NC} PDF:  $pdf"
        done

        # Find EPUB files
        for epub in "$OUTPUT_DIR"/*.epub; do
            [ -f "$epub" ] && echo -e "  ${GREEN}✓${NC} EPUB: $epub"
        done
    fi

    echo ""
}

# Main execution
main() {
    header "BL4 Reverse Engineering Guide Builder (Quarto)"
    echo ""

    check_deps

    if $CLEAN; then
        clean_build
    fi

    if $PREVIEW; then
        preview
    else
        build
        show_results
    fi
}

main
