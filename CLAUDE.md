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
│   │   │   ├── parts.rs    # Part database and lookups
│   │   │   ├── backup.rs   # Smart backup management
│   │   │   └── wasm.rs     # WebAssembly bindings
│   │   ├── build-wasm.sh   # WASM build script
│   │   └── package.json    # NPM package config
│   │
│   ├── bl4-cli/            # CLI tool
│   │   └── src/
│   │       ├── main.rs     # CLI commands
│   │       ├── config.rs   # Configuration management
│   │       └── memory.rs   # Memory analysis for UE5 structures
│   │
│   └── bl4-research/       # Data extraction and manifest generation
│       └── src/
│           ├── main.rs     # Research CLI commands
│           └── manifest.rs # Pak manifest and items database generation
│
├── share/
│   └── manifest/           # Extracted game data (git-lfs)
│       ├── index.json      # Manifest index with metadata
│       ├── pak_manifest.json    # Full asset manifest (81,097 items)
│       ├── pak_summary.json     # Summary statistics
│       ├── items_database.json  # Item pools and stat modifiers
│       ├── manufacturers.json   # Manufacturer data (10 manufacturers)
│       ├── weapons_breakdown.json
│       ├── mappings.usmap       # UE5 reflection data (16,849 structs)
│       └── ...                  # Additional extracted data
│
└── docs/                   # Documentation
    ├── data_structures.md  # Serial format, game structures, RE findings
    ├── extraction.md       # Game data extraction guide
    ├── usmap.md            # Usmap generation details
    └── ...

Important files for troubleshooting:

- **share/dumps** contains full memory dumps of the running game
- **share/saves** contains real working save files for various characters

```

### Design Principles

1. **All BL4 logic in `bl4` crate** - The library handles all mutations, encoding/decoding, and game-specific logic
2. **WASM-first for editors** - The library compiles to WebAssembly for browser-based save editors
3. **Handles over raw data** - Editor uses handles to reference save data; never touches raw data directly except for file I/O
4. **File I/O exception** - Browser editors load/save file contents directly (browsers can't access filesystem the same way), so all file I/O must be in clients (not the library)

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `crypto.rs` | AES-256-ECB encryption with Steam ID-derived keys |
| `save.rs` | YAML parsing, path-based queries, convenience methods |
| `serial.rs` | Item serial decode/encode (Base85 + bit-packed tokens) |
| `parts.rs` | Part database and lookups from extracted manifest data |
| `backup.rs` | SHA-256 hash-based backup tracking |
| `wasm.rs` | JavaScript bindings via wasm-bindgen |
| `memory.rs` | UE5 runtime structure discovery (GNames, GUObjectArray) |
| `manifest.rs` | Pak manifest generation and items database extraction |

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

## Memory Analysis CLI Commands

```bash
# Build parts database from memory dump (works offline with dump file)
bl4 memory build-parts-db share/dumps/Borderlands4.dmp

# Generate object map JSON for fast subsequent lookups
bl4 memory generate-object-map share/dumps/Borderlands4.dmp -o object_map.json

# Find objects matching a name pattern
bl4 memory find-objects-by-pattern share/dumps/Borderlands4.dmp ".part_" --limit 20

# Extract part definitions with category/index (requires parts in GUObjectArray)
bl4 memory extract-parts share/dumps/Borderlands4.dmp -o parts_with_categories.json
```

**Note:** The `extract-parts` command was designed to read SerialIndex.Category directly from InventoryPartDef UObjects, but investigation revealed that part definitions are not present in the GUObjectArray. Part names exist in the FName pool but their UObject instances are not registered at runtime.

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

## Current Status

### Completed Features

**Serial Decoding** (fully working):
- Base85 decoding with custom alphabet
- Bit mirroring
- Token parsing (VarInt, VarBit, Part, String, Separator)
- Formatted token output (`bl4 decode` command)
- Level extraction (appears after first separator)
- Part indices extraction (appear after `||` double separator)
- Manufacturer ID mapping

**Data Extraction** (fully working):
- Pak manifest generation (81,097 assets cataloged)
- Usmap generation from memory dumps (16,849 structs, 58,793 properties)
- Items database with drop pools (62 pools) and stat modifiers
- Manufacturer data (10 manufacturers: BOR, COV, DAD, DPL, JAK, MAL, ORD, TED, TOR, VLA)
- Weapon type breakdown (AR, Heavy, Pistol, Shotgun, SMG, Sniper)
- Gear types (ClassMod, Enhancement, Firmware, Gadget, Grenade, RepairKit, Shield)

**Memory Analysis** (working):
- GNames pool discovery and reading (358 FName blocks)
- FName resolution with multi-block support via `FNameReader`
- UObject layout verified (standard UE5)
- SDK data pointers identified (updated for Nov 2025 patch)
- GUObjectArray iteration (469,504 objects)
- Object map generation for fast lookups

**Parts Database** (partially validated):
- Part names extracted from memory dump (2,615 unique parts)
- Category/Index mappings for 52 part groups
- `dump-parts` and `build-parts-db` CLI commands
- Embedded database in core library via `CategoryPartsDatabase`
- Serial validation: ~75% weapon parts found, ~16% equipment parts found
- Added MAL_SG (cat 19) and bor_sr (cat 25) mappings
- Missing equipment categories: 44, 55, 97, 140, 151, 289 (class mods, firmware, etc.)

**Key Discovery: InventoryPartDef objects are NOT in GUObjectArray**
- Part names exist in FName pool as strings
- But actual UObject instances with SerialIndex data are not present
- Parts appear to be compiled into game code as static data, not runtime UObjects
- UObject-based extraction approach will not work for category mapping

### What's Needed
- [ ] WASM bindings for `ItemSerial` (not currently exposed to JS)
- [ ] Serial encoding (create/modify items)
- [ ] Inventory manipulation API
- [ ] Complete equipment category mappings (class mods, firmware)
- [ ] Alternative extraction approach (pak file parsing or manual mapping)

## Extracted Data Summary

The `share/manifest/` directory contains extracted game data (stored via git-lfs):

| File | Contents |
|------|----------|
| `pak_manifest.json` | 81,097 game assets indexed |
| `mappings.usmap` | 16,849 structs, 2,986 enums, 58,793 properties |
| `items_database.json` | 62 item pools, 26 items with stats, 73 stat types |
| `manufacturers.json` | 10 manufacturers with paths |
| `weapons_breakdown.json` | Weapon counts by type/manufacturer |
| `parts_dump.json` | 2,615 part names grouped by prefix |
| `parts_database.json` | Parts with category/index mappings (53 categories) |

## Part Group IDs

Parts are organized by Category (Part Group ID):

| Range | Type | Examples |
|-------|------|----------|
| 2-7 | Pistols | DAD_PS, JAK_PS, TED_PS, TOR_PS, ORD_PS, VLA_PS |
| 8-12 | Shotguns | DAD_SG, JAK_SG, TED_SG, TOR_SG, BOR_SG |
| 13-18 | Assault Rifles | DAD_AR, JAK_AR, TED_AR, TOR_AR, VLA_AR, ORD_AR |
| 20-23 | SMGs | DAD_SM, BOR_SM, VLA_SM, MAL_SM |
| 26-29 | Snipers | JAK_SR, VLA_SR, ORD_SR, MAL_SR |
| 244-247 | Heavy Weapons | VLA_HW, TOR_HW, BOR_HW, MAL_HW |
| 279-288 | Shields | Energy, Armor, manufacturer variants |
| 300-330 | Gadgets | Grenade, Turret, Repair Kit, Terminal |
| 400-409 | Enhancements | Manufacturer-specific enhancements |

## Testing

Sample serials for testing:
- Weapon: `@Ugr$ZCm/&tH!t{KgK/Shxu>k`
- Equipment: `@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_`
- Utility: `@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1`
