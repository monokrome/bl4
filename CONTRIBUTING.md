# Contributing to BL4

## Development Setup

```bash
# Clone the repository
git clone https://github.com/monokrome/bl4
cd bl4

# Build all packages
cargo build --release

# Run tests
cargo test
```

## Project Structure

- `src/bl4` - Core library (serial encoding/decoding, save parsing)
- `src/bl4-cli` - Command-line interface
- `src/bl4-idb` - Items database library
- `src/bl4-ncs` - NCS file parser
- `src/bl4-community` - Community server (items.bl4.dev)
- `src/uextract` - UE5 asset extractor
- `src/linewise` - Line-based text processing utility

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Keep functions under 50 lines where practical
- Use conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `docs:`

## Releasing

Releases are automated via GitHub Actions when a version tag is pushed.

### Release Process

1. **Update version** in `Cargo.toml` (workspace root):
   ```toml
   [workspace.package]
   version = "X.Y.Z"
   ```

2. **Commit the version bump**:
   ```bash
   git add Cargo.toml
   git commit -m "chore: bump version to X.Y.Z"
   ```

3. **Create and push the tag**:
   ```bash
   git tag vX.Y.Z
   git push origin main --tags
   ```

### What the Release Does

The `release.yml` workflow triggers on `v*` tags and:

1. **Publishes to crates.io** (in order):
   - bl4-ncs
   - bl4
   - bl4-idb
   - bl4-cli

2. **Builds binaries** for:
   - Linux x86_64
   - Linux aarch64
   - macOS x86_64
   - macOS aarch64

3. **Builds Docker image**:
   - `monokrome/bl4-community:latest`
   - `monokrome/bl4-community:X.Y.Z`

4. **Builds WASM package**:
   - Publishes to npm as `@monokrome/bl4`

5. **Generates documentation**:
   - PDF and EPUB guide books

6. **Creates GitHub Release** with all artifacts

### Version Scheme

We use semantic versioning:
- **MAJOR**: Breaking API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

## Testing

```bash
# Run all tests
cargo test

# Run tests for a specific package
cargo test -p bl4

# Run with output
cargo test -- --nocapture
```

## Pull Requests

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes with conventional commits
4. Push and open a PR against `main`
