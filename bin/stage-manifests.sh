#!/usr/bin/env bash
set -euo pipefail

# Stage manifest files for a release
# Usage: ./bin/stage-manifests.sh <version>
# Example: ./bin/stage-manifests.sh v0.3.0

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 v0.3.0"
    exit 1
fi

# Ensure version starts with 'v'
if [[ ! "$VERSION" =~ ^v ]]; then
    VERSION="v$VERSION"
fi

# Get version from workspace Cargo.toml
CRATE_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
EXPECTED_TAG="v$CRATE_VERSION"

if [[ "$VERSION" != "$EXPECTED_TAG" ]]; then
    echo "Error: Version mismatch"
    echo "  Requested: $VERSION"
    echo "  bl4 crate: $EXPECTED_TAG"
    echo ""
    echo "Update the version in crates/bl4/Cargo.toml first, or use: $0 $EXPECTED_TAG"
    exit 1
fi

MANIFEST_DIR="share/manifest"
MANIFEST_FILES=(
    "$MANIFEST_DIR/parts_database.json"
    "$MANIFEST_DIR/part_pools.json"
    "$MANIFEST_DIR/category_names.json"
    "$MANIFEST_DIR/BL4.usmap"
)

# Verify all manifest files exist
echo "Checking manifest files..."
for file in "${MANIFEST_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        echo "Error: Missing manifest file: $file"
        exit 1
    fi
    echo "  Found: $file ($(du -h "$file" | cut -f1))"
done

# Check if release already exists
if gh release view "$VERSION" &>/dev/null; then
    echo ""
    echo "Release $VERSION already exists."
    read -p "Upload manifests to existing release? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Uploading manifests to existing release..."
        gh release upload "$VERSION" "${MANIFEST_FILES[@]}" --clobber
    else
        echo "Aborted."
        exit 1
    fi
else
    echo ""
    echo "Creating draft release $VERSION..."
    gh release create "$VERSION" \
        "${MANIFEST_FILES[@]}" \
        --title "$VERSION" \
        --notes "Manifest data for $VERSION. Binaries will be added when tag is pushed." \
        --draft
fi

echo ""
echo "Release staged: $VERSION"
echo ""
echo "Next steps:"
echo "  1. Review the release: gh release view $VERSION"
echo "  2. If draft, publish it: gh release edit $VERSION --draft=false"
echo "  3. Push the tag to trigger build:"
echo "     git tag $VERSION"
echo "     git push origin $VERSION"
echo ""
echo "The workflow will download these manifests and attach built binaries."
