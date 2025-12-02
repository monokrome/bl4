# CLAUDE.md - Project Guide for Claude Code

## Project Overview

**bl4 (bl4)** - A Borderlands 4 save file editor and item serial decoder.

- **Repository**: https://github.com/monokrome/bl4
- **Version**: 0.2.0
- **License**: BSD-2-Clause

## Architecture

### Workspace Structure

```
bl4/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── bl4/                # Core library (Rust + WASM)
│   │   ├── src/
│   │   │   ├── lib.rs      # Library exports
│   │   │   ├── crypto.rs   # AES-256 encryption/decryption
│   │   │   ├── save.rs     # YAML parsing & querying
│   │   │   ├── serial.rs   # Item serial decoding
│   │   │   ├── backup.rs   # Smart backup management
│   │   │   └── wasm.rs     # WebAssembly bindings
│   │   ├── build-wasm.sh   # WASM build script
│   │   └── package.json    # NPM package config
│   │
│   └── bl4-cli/            # CLI tool
│       └── src/
│           ├── main.rs     # CLI commands
│           └── config.rs   # Configuration management
│
└── docs/
    └── data_structures.md  # Serial format, game structures, RE findings
```

### Design Principles

1. **All BL4 logic in `bl4` crate** - The library handles all mutations, encoding/decoding, and game-specific logic
2. **WASM-first for editors** - The library compiles to WebAssembly for browser-based save editors
3. **Handles over raw data** - Editor uses handles to reference save data; never touches raw data directly except for file I/O
4. **File I/O exception** - Browser editors load/save file contents directly (browsers can't access filesystem the same way)

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `crypto.rs` | AES-256-ECB encryption with Steam ID-derived keys |
| `save.rs` | YAML parsing, path-based queries, convenience methods |
| `serial.rs` | Item serial decode/encode (Base85 + bit-packed tokens) |
| `backup.rs` | SHA-256 hash-based backup tracking |
| `wasm.rs` | JavaScript bindings via wasm-bindgen |

## Development Commands

```bash
# Build
cargo build --release -p bl4-cli

# Test
cargo test

# Build WASM
cd crates/bl4 && ./build-wasm.sh

# Lint
cargo clippy
```

## Key Technical Details

### Encryption Flow
```
.sav → AES-256-ECB decrypt → zlib decompress → YAML
YAML → zlib compress → AES-256-ECB encrypt → .sav
```

### Key Derivation
1. Extract digits from Steam ID string
2. Parse as u64 and convert to 8-byte little-endian
3. XOR first 8 bytes of BASE_KEY with Steam ID bytes
4. Use the resulting 32 bytes directly as the AES key (no hashing)

### Item Serial Format
```
@Ug<type><base85_data>
```
- Prefix: `@Ug` (constant)
- Type: Single char (r=weapon, e=equipment, u=utility, etc.)
- Data: Custom Base85 encoded, bit-mirrored, token-based bitstream

## Current Work: Serial Decoding

### What's Working
- Base85 decoding with custom alphabet
- Bit mirroring
- Token parsing (VarInt, VarBit, Part, String, Separator)
- Formatted token output (`bl4 decode` command)
- Level extraction (appears after first separator)
- Part indices extraction (appear after `||` double separator)

### What's Needed
- [ ] Map part indices to actual game parts/stats
- [ ] WASM bindings for `ItemSerial` (not currently exposed to JS)
- [ ] Serial encoding (create/modify items)
- [ ] Inventory manipulation API

### Reverse Engineering Approach
1. Correlate known items with decoded tokens
2. Use memory dumps to find item data structures
3. Use radare2/Ghidra for static binary analysis
4. Diff similar items to isolate token changes

## Testing

Sample serials for testing:
- Weapon: `@Ugr$ZCm/&tH!t{KgK/Shxu>k`
- Equipment: `@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_`
- Utility: `@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1`
