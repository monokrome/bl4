#!/usr/bin/env bash
#
# build-book.sh - Build the BL4 Reverse Engineering Guide in multiple formats
#
# Usage:
#   ./bin/build-book.sh [options] [output_dir]
#
# Options:
#   --pdf        Build PDF only
#   --epub       Build EPUB only
#   --mobi       Build MOBI only
#   --all        Build all formats (default)
#   --clean      Clean build directory before building
#
# Requirements:
#   PDF:  pandoc, texlive-xetex, texlive-fonts-recommended,
#         texlive-fonts-extra, texlive-latex-extra, librsvg2-bin
#   EPUB: pandoc
#   MOBI: pandoc, calibre (for ebook-convert)
#
# On Ubuntu/Debian:
#   sudo apt-get install pandoc texlive-xetex texlive-fonts-recommended \
#     texlive-fonts-extra texlive-latex-extra librsvg2-bin calibre

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DOCS_DIR="$PROJECT_ROOT/docs/guide"

# Book metadata
BOOK_TITLE="Borderlands 4 Reverse Engineering Guide"
BOOK_SUBTITLE="Zero to Hero"
BOOK_AUTHOR="The bl4 Project Contributors"
BOOK_LANGUAGE="en-US"
BOOK_PUBLISHER="bl4 Project"
BOOK_RIGHTS="BSD-2-Clause License"
BOOK_URL="https://bl4.monokro.me"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build flags
BUILD_PDF=false
BUILD_EPUB=false
BUILD_MOBI=false
BUILD_ALL=true
CLEAN=false
OUTPUT_DIR=""

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

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
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
        --mobi)
            BUILD_MOBI=true
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
        -h|--help)
            echo "Usage: $0 [options] [output_dir]"
            echo ""
            echo "Options:"
            echo "  --pdf        Build PDF only"
            echo "  --epub       Build EPUB only"
            echo "  --mobi       Build MOBI only"
            echo "  --all        Build all formats (default)"
            echo "  --clean      Clean build directory before building"
            echo "  -h, --help   Show this help message"
            exit 0
            ;;
        *)
            OUTPUT_DIR="$1"
            shift
            ;;
    esac
done

# Set default output directory
OUTPUT_DIR="${OUTPUT_DIR:-$PROJECT_ROOT/build}"

# If building all, set all flags
if $BUILD_ALL; then
    BUILD_PDF=true
    BUILD_EPUB=true
    BUILD_MOBI=true
fi

# Check dependencies
check_deps() {
    local missing=()

    command -v pandoc >/dev/null 2>&1 || missing+=("pandoc")

    if $BUILD_PDF; then
        command -v xelatex >/dev/null 2>&1 || missing+=("texlive-xetex")
    fi

    if $BUILD_MOBI; then
        command -v ebook-convert >/dev/null 2>&1 || missing+=("calibre (for ebook-convert)")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing dependencies: ${missing[*]}"
    fi
}

# Clean build directory
clean_build() {
    if [ -d "$OUTPUT_DIR" ]; then
        info "Cleaning build directory..."
        rm -rf "$OUTPUT_DIR"
    fi
}

# Create build directory
setup_build() {
    mkdir -p "$OUTPUT_DIR"
}

# Create cover image for EPUB/MOBI
create_cover_image() {
    info "Creating cover image..."

    # Create a simple SVG cover
    cat > "$OUTPUT_DIR/cover.svg" << 'COVER_SVG'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 800">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#1a1a2e"/>
      <stop offset="50%" style="stop-color:#16213e"/>
      <stop offset="100%" style="stop-color:#0f3460"/>
    </linearGradient>
    <linearGradient id="accent" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" style="stop-color:#ff6b35"/>
      <stop offset="100%" style="stop-color:#f7c59f"/>
    </linearGradient>
  </defs>

  <!-- Background -->
  <rect width="600" height="800" fill="url(#bg)"/>

  <!-- Decorative lines -->
  <rect x="50" y="150" width="500" height="4" fill="url(#accent)" opacity="0.8"/>
  <rect x="50" y="650" width="500" height="4" fill="url(#accent)" opacity="0.8"/>

  <!-- Binary decoration -->
  <text x="300" y="100" font-family="monospace" font-size="14" fill="#ffffff" opacity="0.15" text-anchor="middle">
    01001000 01100101 01111000 00100000 01000101 01100100 01101001 01110100
  </text>

  <!-- Title -->
  <text x="300" y="280" font-family="Arial, sans-serif" font-size="48" font-weight="bold" fill="#ffffff" text-anchor="middle">
    BORDERLANDS 4
  </text>

  <!-- Subtitle -->
  <text x="300" y="350" font-family="Arial, sans-serif" font-size="28" fill="url(#accent)" text-anchor="middle">
    Reverse Engineering Guide
  </text>

  <!-- Tagline -->
  <text x="300" y="420" font-family="Arial, sans-serif" font-size="22" font-style="italic" fill="#cccccc" text-anchor="middle">
    Zero to Hero
  </text>

  <!-- Decorative hex -->
  <text x="300" y="520" font-family="monospace" font-size="16" fill="#ff6b35" opacity="0.6" text-anchor="middle">
    0x42 0x4C 0x34
  </text>

  <!-- Author -->
  <text x="300" y="720" font-family="Arial, sans-serif" font-size="18" fill="#888888" text-anchor="middle">
    The bl4 Project Contributors
  </text>

  <!-- URL -->
  <text x="300" y="760" font-family="monospace" font-size="14" fill="#666666" text-anchor="middle">
    bl4.monokro.me
  </text>
</svg>
COVER_SVG

    # Convert SVG to PNG if rsvg-convert is available
    if command -v rsvg-convert >/dev/null 2>&1; then
        rsvg-convert -w 600 -h 800 "$OUTPUT_DIR/cover.svg" > "$OUTPUT_DIR/cover.png"
        info "Cover image created: cover.png"
    else
        warn "rsvg-convert not found, using SVG cover (EPUB only)"
    fi
}

# Create metadata file for EPUB
create_epub_metadata() {
    info "Creating EPUB metadata..."

    cat > "$OUTPUT_DIR/metadata.xml" << EOF
<dc:title>$BOOK_TITLE</dc:title>
<dc:creator>$BOOK_AUTHOR</dc:creator>
<dc:language>$BOOK_LANGUAGE</dc:language>
<dc:publisher>$BOOK_PUBLISHER</dc:publisher>
<dc:rights>$BOOK_RIGHTS</dc:rights>
<dc:description>A comprehensive guide to understanding game internals, reverse engineering techniques, and using the bl4 tooling to analyze and modify Borderlands 4. From binary basics to advanced memory analysis.</dc:description>
<dc:subject>Reverse Engineering</dc:subject>
<dc:subject>Game Hacking</dc:subject>
<dc:subject>Unreal Engine</dc:subject>
<dc:subject>Borderlands</dc:subject>
EOF
}

# Create custom CSS for EPUB
create_epub_css() {
    info "Creating EPUB stylesheet..."

    cat > "$OUTPUT_DIR/epub.css" << 'EOF'
/* Base typography */
body {
    font-family: Georgia, "Times New Roman", serif;
    line-height: 1.6;
    margin: 1em;
}

/* Headings */
h1, h2, h3, h4, h5, h6 {
    font-family: "Helvetica Neue", Arial, sans-serif;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    page-break-after: avoid;
}

h1 {
    font-size: 2em;
    border-bottom: 2px solid #ff6b35;
    padding-bottom: 0.3em;
}

h2 {
    font-size: 1.5em;
    color: #333;
}

h3 {
    font-size: 1.25em;
    color: #444;
}

/* Code blocks */
pre, code {
    font-family: "Courier New", Consolas, monospace;
    font-size: 0.9em;
    background-color: #f4f4f4;
    border-radius: 3px;
}

pre {
    padding: 1em;
    overflow-x: auto;
    border-left: 3px solid #ff6b35;
    margin: 1em 0;
}

code {
    padding: 0.2em 0.4em;
}

pre code {
    padding: 0;
    background: none;
}

/* Tables */
table {
    border-collapse: collapse;
    width: 100%;
    margin: 1em 0;
    font-size: 0.9em;
}

th, td {
    border: 1px solid #ddd;
    padding: 0.5em;
    text-align: left;
}

th {
    background-color: #f8f8f8;
    font-weight: bold;
}

tr:nth-child(even) {
    background-color: #fafafa;
}

/* Blockquotes (for notes/tips) */
blockquote {
    margin: 1em 0;
    padding: 0.5em 1em;
    border-left: 4px solid #ff6b35;
    background-color: #fff8f0;
}

blockquote p {
    margin: 0.5em 0;
}

/* Definition lists (glossary) */
dl {
    margin: 1em 0;
}

dt {
    font-weight: bold;
    color: #333;
    margin-top: 1em;
}

dd {
    margin-left: 1.5em;
    margin-bottom: 0.5em;
}

/* Links */
a {
    color: #0066cc;
    text-decoration: none;
}

a:hover {
    text-decoration: underline;
}

/* Prevent page breaks inside elements */
pre, blockquote, table, figure {
    page-break-inside: avoid;
}

/* Chapter titles */
.chapter-title {
    page-break-before: always;
}

/* Cover page styling */
.cover {
    text-align: center;
    page-break-after: always;
}
EOF
}

# Create LaTeX cover for PDF
create_pdf_cover() {
    info "Creating PDF cover page..."

    cat > "$OUTPUT_DIR/cover.md" << 'COVER'
\thispagestyle{empty}
\begin{center}
\vspace*{3cm}

{\Huge\bfseries Borderlands 4}

\vspace{0.5cm}

{\LARGE Reverse Engineering Guide}

\vspace{1cm}

{\Large\itshape Zero to Hero}

\vspace{4cm}

{\large The bl4 Project Contributors}

\vfill

{\small https://bl4.monokro.me}

\end{center}
\newpage
COVER
}

# Create acknowledgments page
create_acknowledgments() {
    info "Creating acknowledgments page..."

    cat > "$OUTPUT_DIR/acknowledgments.md" << 'ACK'
\thispagestyle{empty}

\vspace*{2cm}

\section*{Acknowledgments}

This guide and the bl4 tooling would not be possible without the incredible work of the reverse engineering and modding community. Special thanks to:

\begin{itemize}
\item \textbf{FromDarkHell} and the \textbf{Borderlands modding community} — For pioneering save editing techniques in earlier Borderlands titles
\item \textbf{trumank} — Creator of \texttt{retoc}, the IoStore extraction tool that makes pak file analysis possible
\item \textbf{atenfyr} — Creator of \texttt{UAssetAPI} and related Unreal asset parsing tools
\item \textbf{FabianFG} — For \texttt{CUE4Parse} and Unreal Engine research
\item \textbf{TheNaeem} — For \texttt{UnrealMappingsDumper} and usmap generation techniques
\item \textbf{UE4SS Team} — For the RE-UE4SS modding framework and documentation
\item \textbf{RAD Game Tools} — For Oodle compression (used in UE5)
\item \textbf{glacierpiece} — For early BL4 save file research
\end{itemize}

\vspace{1cm}

Thanks also to everyone who has contributed to understanding Unreal Engine internals and sharing their knowledge openly.

\vfill

\begin{center}
\rule{0.5\textwidth}{0.4pt}

{\small For updates to this guide, please refer to:}

{\small\texttt{https://bl4.monokro.me}}
\end{center}

\newpage
ACK
}

# Create blank page for PDF
create_blank_page() {
    cat > "$OUTPUT_DIR/blank.md" << 'BLANK'
\thispagestyle{empty}
\mbox{}
\newpage
BLANK
}

# Create acknowledgments for EPUB (plain markdown)
create_epub_acknowledgments() {
    info "Creating EPUB acknowledgments..."

    cat > "$OUTPUT_DIR/acknowledgments-epub.md" << 'ACK'
# Acknowledgments

This guide and the bl4 tooling would not be possible without the incredible work of the reverse engineering and modding community. Special thanks to:

- **FromDarkHell** and the **Borderlands modding community** — For pioneering save editing techniques in earlier Borderlands titles
- **trumank** — Creator of `retoc`, the IoStore extraction tool that makes pak file analysis possible
- **atenfyr** — Creator of `UAssetAPI` and related Unreal asset parsing tools
- **FabianFG** — For `CUE4Parse` and Unreal Engine research
- **TheNaeem** — For `UnrealMappingsDumper` and usmap generation techniques
- **UE4SS Team** — For the RE-UE4SS modding framework and documentation
- **RAD Game Tools** — For Oodle compression (used in UE5)
- **glacierpiece** — For early BL4 save file research

Thanks also to everyone who has contributed to understanding Unreal Engine internals and sharing their knowledge openly.

---

*For updates to this guide, please refer to: https://bl4.monokro.me*

ACK
}

# Define chapter files in order
CHAPTERS=(
    "00-introduction.md"
    "01-binary-basics.md"
    "02-unreal-architecture.md"
    "03-memory-analysis.md"
    "04-save-files.md"
    "05-item-serials.md"
    "06-data-extraction.md"
    "07-bl4-tools.md"
    "appendix-a-sdk-layouts.md"
    "appendix-b-weapon-parts.md"
    "appendix-c-loot-system.md"
    "appendix-d-game-files.md"
    "glossary.md"
)

# Build combined markdown for PDF
build_pdf_markdown() {
    info "Creating PDF markdown..."

    cat > "$OUTPUT_DIR/book-pdf.md" << FRONTMATTER
---
documentclass: report
geometry: margin=1in
toc: true
toc-depth: 3
numbersections: true
colorlinks: true
linkcolor: blue
urlcolor: blue
header-includes: |
  \\usepackage{fancyhdr}
  \\pagestyle{fancy}
  \\fancyhead[L]{BL4 Reverse Engineering Guide}
  \\fancyhead[R]{\\thepage}
  \\fancyfoot[C]{}
include-before:
  - cover.md
  - acknowledgments.md
  - blank.md
---

FRONTMATTER

    # Append each chapter
    for file in "${CHAPTERS[@]}"; do
        if [ -f "$DOCS_DIR/$file" ]; then
            echo "" >> "$OUTPUT_DIR/book-pdf.md"

            # Convert MkDocs admonitions to LaTeX
            sed -e 's/^!!! note$/\\begin{quote}\\textbf{Note:}/g' \
                -e 's/^!!! tip$/\\begin{quote}\\textbf{Tip:}/g' \
                -e 's/^!!! warning$/\\begin{quote}\\textbf{Warning:}/g' \
                "$DOCS_DIR/$file" >> "$OUTPUT_DIR/book-pdf.md"

            echo "" >> "$OUTPUT_DIR/book-pdf.md"
            echo "\\newpage" >> "$OUTPUT_DIR/book-pdf.md"
        else
            warn "Chapter not found: $file"
        fi
    done
}

# Build combined markdown for EPUB
build_epub_markdown() {
    info "Creating EPUB markdown..."

    # Start with title
    cat > "$OUTPUT_DIR/book-epub.md" << EOF
---
title: "$BOOK_TITLE"
subtitle: "$BOOK_SUBTITLE"
author: "$BOOK_AUTHOR"
lang: $BOOK_LANGUAGE
---

EOF

    # Add acknowledgments
    cat "$OUTPUT_DIR/acknowledgments-epub.md" >> "$OUTPUT_DIR/book-epub.md"
    echo "" >> "$OUTPUT_DIR/book-epub.md"

    # Append each chapter
    for file in "${CHAPTERS[@]}"; do
        if [ -f "$DOCS_DIR/$file" ]; then
            echo "" >> "$OUTPUT_DIR/book-epub.md"

            # Convert MkDocs admonitions to blockquotes for EPUB
            sed -e 's/^!!! note$/> **Note:**/g' \
                -e 's/^!!! tip$/> **Tip:**/g' \
                -e 's/^!!! warning$/> **Warning:**/g' \
                -e 's/^    \(.*\)$/> \1/g' \
                "$DOCS_DIR/$file" >> "$OUTPUT_DIR/book-epub.md"

            echo "" >> "$OUTPUT_DIR/book-epub.md"
        fi
    done
}

# Build PDF
build_pdf() {
    header "Building PDF"

    create_pdf_cover
    create_acknowledgments
    create_blank_page
    build_pdf_markdown

    info "Running pandoc for PDF..."
    cd "$OUTPUT_DIR"
    pandoc book-pdf.md \
        -o bl4-guide.pdf \
        --pdf-engine=xelatex \
        --toc \
        --number-sections \
        -V geometry:margin=1in \
        -V documentclass=report \
        -V colorlinks=true \
        -V linkcolor=blue \
        -V urlcolor=blue \
        --highlight-style=tango

    if [ -f "$OUTPUT_DIR/bl4-guide.pdf" ]; then
        SIZE=$(du -h "$OUTPUT_DIR/bl4-guide.pdf" | cut -f1)
        info "PDF created: bl4-guide.pdf ($SIZE)"
    else
        error "PDF creation failed"
    fi
}

# Build EPUB
build_epub() {
    header "Building EPUB"

    create_cover_image
    create_epub_metadata
    create_epub_css
    create_epub_acknowledgments
    build_epub_markdown

    info "Running pandoc for EPUB..."

    local cover_option=""
    if [ -f "$OUTPUT_DIR/cover.png" ]; then
        cover_option="--epub-cover-image=$OUTPUT_DIR/cover.png"
    elif [ -f "$OUTPUT_DIR/cover.svg" ]; then
        cover_option="--epub-cover-image=$OUTPUT_DIR/cover.svg"
    fi

    cd "$OUTPUT_DIR"
    pandoc book-epub.md \
        -o bl4-guide.epub \
        --toc \
        --toc-depth=3 \
        --epub-metadata=metadata.xml \
        --css=epub.css \
        $cover_option \
        --highlight-style=tango

    if [ -f "$OUTPUT_DIR/bl4-guide.epub" ]; then
        SIZE=$(du -h "$OUTPUT_DIR/bl4-guide.epub" | cut -f1)
        info "EPUB created: bl4-guide.epub ($SIZE)"
    else
        error "EPUB creation failed"
    fi
}

# Build MOBI
build_mobi() {
    header "Building MOBI"

    # MOBI requires EPUB first
    if [ ! -f "$OUTPUT_DIR/bl4-guide.epub" ]; then
        warn "EPUB not found, building EPUB first..."
        build_epub
    fi

    info "Converting EPUB to MOBI with Calibre..."

    cd "$OUTPUT_DIR"
    ebook-convert bl4-guide.epub bl4-guide.mobi \
        --output-profile kindle_pw3 \
        --mobi-file-type new \
        --no-inline-toc \
        --cover cover.png 2>/dev/null || \
    ebook-convert bl4-guide.epub bl4-guide.mobi \
        --output-profile kindle_pw3 \
        --mobi-file-type new \
        --no-inline-toc

    if [ -f "$OUTPUT_DIR/bl4-guide.mobi" ]; then
        SIZE=$(du -h "$OUTPUT_DIR/bl4-guide.mobi" | cut -f1)
        info "MOBI created: bl4-guide.mobi ($SIZE)"
    else
        error "MOBI creation failed"
    fi
}

# Main execution
main() {
    header "BL4 Reverse Engineering Guide Builder"

    echo ""
    info "Output directory: $OUTPUT_DIR"
    info "Building formats: $(
        formats=""
        $BUILD_PDF && formats="${formats}PDF "
        $BUILD_EPUB && formats="${formats}EPUB "
        $BUILD_MOBI && formats="${formats}MOBI "
        echo "$formats"
    )"
    echo ""

    check_deps

    if $CLEAN; then
        clean_build
    fi

    setup_build

    # Build requested formats
    $BUILD_PDF && build_pdf
    $BUILD_EPUB && build_epub
    $BUILD_MOBI && build_mobi

    # Summary
    echo ""
    header "Build Complete"
    echo ""

    if [ -f "$OUTPUT_DIR/bl4-guide.pdf" ]; then
        echo -e "  ${GREEN}✓${NC} PDF:  $OUTPUT_DIR/bl4-guide.pdf"
    fi
    if [ -f "$OUTPUT_DIR/bl4-guide.epub" ]; then
        echo -e "  ${GREEN}✓${NC} EPUB: $OUTPUT_DIR/bl4-guide.epub"
    fi
    if [ -f "$OUTPUT_DIR/bl4-guide.mobi" ]; then
        echo -e "  ${GREEN}✓${NC} MOBI: $OUTPUT_DIR/bl4-guide.mobi"
    fi

    echo ""
}

main
