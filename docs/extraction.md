# Borderlands 4 Game Data Extraction

This document describes how to extract game data from Borderlands 4 using Rust-based tools.

## Overview

BL4 uses Unreal Engine 5's IoStore container format for game assets:

| File Type | Description |
|-----------|-------------|
| `.utoc` | Table of contents - indexes all assets |
| `.ucas` | Container archive - compressed asset data (Oodle) |
| `.pak` | Legacy format - configs, textures, audio |
| `.uasset` | Extracted asset files |
| `.uexp` | Bulk data for split assets |

## Required Tools

### retoc (IoStore extraction)

Extracts `.ucas/.utoc` containers.

**Installation:**
```bash
cargo install --git https://github.com/trumank/retoc retoc_cli
```

**Commands:**
```bash
# Show container info
retoc info <path>.utoc

# List all assets
retoc list <path>.utoc

# Extract all assets (Zen format)
retoc unpack <path>.utoc <output_dir>

# Convert Zen format to Legacy format
retoc to-legacy <path>.utoc <output_dir> --no-script-objects
```

**Known Limitations:**
- **IMPORTANT**: Must point at the entire `Paks/` directory, NOT individual `.utoc` files. The `global.utoc` contains the `ScriptObjects` chunk required for conversion.
- Legacy files are still UE5.5 format - the `unreal_asset` crate (max UE5.2) cannot parse them
- Use `strings` command on legacy assets for basic data extraction

**Working extraction:**
```bash
# Convert ALL paks to legacy format (required for ScriptObjects)
retoc to-legacy "/path/to/Borderlands 4/OakGame/Content/Paks/" ./output/ --no-shaders
```

### repak (PAK files)

Handles legacy `.pak` files.

**Installation:**
```bash
cargo install repak
```

**Commands:**
```bash
# List files in PAK
repak list <path>.pak

# Extract PAK contents
repak unpack <path>.pak <output_dir>
```

### unreal_asset (Asset parsing)

Parses `.uasset` files. Available as a Rust crate.

**Cargo.toml:**
```toml
[dependencies]
unreal_asset = { version = "0.3", features = ["oodle"] }
```

### uextract (Primary extraction tool)

Custom Rust tool for extracting BL4 pak files with property value parsing.

**Build:**
```bash
cargo build --release -p uextract
# Binary at: ./target/release/uextract
```

**Commands:**
```bash
# List all assets in pak directory
./target/release/uextract "/path/to/Paks" --list

# Extract all assets to JSON
./target/release/uextract "/path/to/Paks" -o /tmp/output

# Extract with usmap schema (for full DataTable parsing)
./target/release/uextract "/path/to/Paks" -o /tmp/output --usmap share/borderlands.usmap

# Filter by pattern
./target/release/uextract "/path/to/Paks" --list --ifilter "BalanceData"
```

**Capabilities:**
- Parses UE5.5 IoStore format with Zen packages
- Extracts unversioned properties using FUnversionedHeader fragment parsing
- Outputs JSON with asset metadata and parsed property values
- Supports usmap schema files (16,731 structs, 2,979 enums) for property resolution

**Usmap Schema:**

A `.usmap` file at `share/borderlands.usmap` contains the property schema for BL4's unversioned assets. The usmap contains engine and game class definitions but **not** user-defined DataTable row structs.

**Unversioned Property Parsing:**

UE5 uses a fragment-based header for unversioned serialization:
- Each fragment is 16-bits: `SkipNum(7) | HasZeroes(1) | IsLast(1) | ValueCount(7)`
- Fragments encode "skip N properties, then serialize M properties"
- A zero mask follows if any fragment has `HasZeroes` set
- Property values are serialized after the header in schema order

**Current Limitations:**
- DataTable row structs (e.g., `Struct_EnemyDrops`) are user-defined and not in usmap
- Complex nested types require additional parsing not yet implemented

### bl4 memory (Memory analysis)

Part of the main bl4-cli. Analyzes game memory from dumps or live process.

**Build:**
```bash
cargo build --release -p bl4-cli
```

**Commands:**
```bash
# Discover UE5 global structures (GNames, GUObjectArray)
bl4 memory discover gnames
bl4 memory discover guobjectarray

# Read raw memory at address
bl4 memory read 0x15125e310 --size 128

# Dump usmap from memory dump
bl4 memory --dump ./share/dumps/vex.raw dump-usmap
```

**Requirements:**
- Memory dump file (MDMP format from procdump) for offline analysis
- Or running BL4 process for live analysis (works with Wine/Proton on Linux)

**Current Status:**
- GNames discovery: Working
- GUObjectArray discovery: Working
- Usmap generation: Complete (64,917 names, 16,849 structs, 58,793 properties)

See [Data Structures - UE5 Runtime Structure Discovery](data_structures.md#ue5-runtime-structure-discovery) for technical details.

### bl4-research (Manifest generation)

Part of this project. Organizes extracted data into JSON manifest files.

**Build:**
```bash
cargo build --release -p bl4-research
```

**Commands:**
```bash
# Generate manifest from extracted files
bl4-research manifest

# Build pak manifest from uextract output
bl4-research pak-manifest -e share/manifest/extracted -o share/manifest

# Generate items database with drop pools and stats
bl4-research items-db -m share/manifest

# Alternative commands (if extraction already done)
bl4-research manufacturers
bl4-research weapons
```

**Output:** JSON files in `share/manifest/`:
- `index.json` - Manifest index with metadata
- `pak_manifest.json` - Full asset manifest (81,097 items)
- `pak_summary.json` - Summary statistics
- `weapons_breakdown.json` - Weapon counts by type/manufacturer
- `items_database.json` - Item drop pools and stat modifiers
- `mappings.usmap` - UE5 reflection data for asset parsing

## Game File Locations

### Steam (Linux)
```
~/.steam/steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/
```

### Steam (Windows)
```
C:\Program Files (x86)\Steam\steamapps\common\Borderlands 4\OakGame\Content\Paks\
```

### Key PAK Chunks

| Chunk | Contents |
|-------|----------|
| `pakchunk0-Windows_0_P` | Core game assets, weapons, gear |
| `pakchunk2-Windows_0_P` | Audio (Wwise .bnk) |
| `pakchunk3-Windows_0_P` | Localized audio |
| `pakchunk10-Windows_0_P` | Large assets |

## Extraction Examples

### Extract All Assets from Main Container

```bash
# Create output directory
mkdir -p /tmp/bl4_extract

# Extract pakchunk0 (main game data)
retoc unpack \
  "~/.steam/steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc" \
  /tmp/bl4_extract
```

### Find Specific Asset Types

```bash
# Weapon balance data
find /tmp/bl4_extract -path "*BalanceData*" -name "*.uasset"

# Weapon naming tables
find /tmp/bl4_extract -iname "*naming*" -name "*.uasset"

# Manufacturer data
find /tmp/bl4_extract -iname "*manufacturer*" -name "*.uasset"

# Part definitions
find /tmp/bl4_extract -path "*Parts*" -name "*.uasset"
```

### Extract Strings from Assets

```bash
# Quick inspection of asset contents
strings /tmp/bl4_extract/OakGame/Content/Gear/Weapons/_Shared/BalanceData/WeaponStats/Struct_WeaponStats.uasset
```

## Asset Path Mapping

Game asset paths use `/Game/` prefix which maps to extracted paths:

| Game Path | Extracted Path |
|-----------|----------------|
| `/Game/Gear/Weapons/...` | `OakGame/Content/Gear/Weapons/...` |
| `/Script/OakGame.ClassName` | Engine script class (not extractable) |

## Key Asset Locations

### Weapon Data

```
OakGame/Content/Gear/Weapons/
├── _Shared/
│   ├── BalanceData/
│   │   ├── WeaponStats/Struct_WeaponStats.uasset
│   │   ├── Rarity/Struct_Weapon_RarityInit.uasset
│   │   ├── Elemental/Struct_Weapon_Elemental_Init.uasset
│   │   └── UnderbarrelData/Struct_Weapon_*.uasset
│   └── NamingStrategies/
│       └── WeaponNamingStruct.uasset
├── AssaultRifles/
├── Pistols/
├── Shotguns/
├── SMGs/
├── SniperRifles/
└── HeavyWeapons/
```

### Gear Data

```
OakGame/Content/Gear/
├── shields/BalanceData/
├── GrenadeGadgets/_Shared/BalanceData/
├── Gadgets/
│   ├── HeavyWeapons/_Shared/BalanceData/
│   └── Turrets/_Shared/BalanceData/
└── RepairKits/_Shared/BalanceData/
```

## Asset Contents

### Stat Property Pattern

Stat properties in .uasset files follow the pattern: `StatName_ModifierType_Index_GUID`

**Modifier Types:**
| Type | Description |
|------|-------------|
| `Scale` | Multiplier applied to base value |
| `Add` | Flat value added to stat |
| `Value` | Absolute value replacement |
| `Percent` | Percentage modifier |

**Example from barrel data:**
```
Damage_Scale_14_4D6E5A8840F57DBD840197B3CB05686D
CritDamage_Add_50_740BF8EA43AFEE45A6A954B40FD8101E
FireRate_Value_36_67DA482B483B02CAC87864955A611952
```

### Known Stat Types

| Stat | Description |
|------|-------------|
| `Damage` | Base damage |
| `CritDamage` | Critical hit damage |
| `FireRate` | Firing rate |
| `ReloadTime` | Reload time |
| `MagSize` | Magazine capacity |
| `Accuracy` | Base accuracy |
| `AccImpulse` | Accuracy impulse (recoil recovery) |
| `AccRegen` | Accuracy regeneration |
| `AccDelay` | Accuracy delay |
| `Spread` | Projectile spread |
| `Recoil` | Weapon recoil |
| `Sway` | Weapon sway |
| `ProjectilesPerShot` | Pellets per shot |
| `AmmoCost` | Ammo consumption |
| `StatusChance` | Status effect chance |
| `StatusDamage` | Status effect damage |
| `EquipTime` | Weapon equip time |
| `PutDownTime` | Weapon holster time |
| `ZoomDuration` | ADS zoom time |
| `ElementalPower` | Elemental damage bonus |
| `DamageRadius` | Splash damage radius |

### Naming Structure

Extracted from `WeaponNamingStruct.uasset`:

| Field | Index | GUID | Description |
|-------|-------|------|-------------|
| `Damage` | 2 | `9DFA8E9A4AF1B3A1...` | Damage prefix names |
| `CritDamage` | 9 | `C4432C8C40CA15F0...` | Crit damage prefixes |
| `FireRate` | 10 | `459C49044DE26DE5...` | Fire rate prefixes |
| `ReloadSpeed` | 11 | `61FAACA14D48B609...` | Reload prefixes |
| `MagSize` | 12 | `C735EA434D50CD82...` | Mag size prefixes |
| `Accuracy` | 13 | `5B35CC194CB71AE4...` | Accuracy prefixes |
| `ElementalPower` | 14 | `842A58234E5D5D79...` | Elemental prefixes |
| `ADSProficiency` | 16 | `02D519604FE47BA5...` | ADS proficiency prefixes |
| `Single` | 18 | `240AB1EB411BED6B...` | Single-modifier prefixes |
| `DamageRadius` | 21 | `EE89495D493F3450...` | Splash damage prefixes |

The naming indices correspond to part slots in weapon naming tables.

## Programmatic Extraction

### Using retoc as a Library

The `retoc` crate can be used as a library (not just CLI):

```rust
// Note: retoc exposes internal crates that may be usable
// Check https://github.com/trumank/retoc for library usage
```

### Using unreal_asset

```rust
use unreal_asset::Asset;
use std::fs::File;
use std::io::BufReader;

fn parse_uasset(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Parse the asset
    let asset = Asset::new(&mut reader, None)?;

    // Access exports, imports, names, etc.
    for export in asset.exports.iter() {
        println!("Export: {:?}", export);
    }

    Ok(())
}
```

## Compression Notes

BL4 uses **Oodle** compression for IoStore containers. The `retoc` tool handles decompression automatically by loading the Oodle library from the game installation.

If extraction fails with compression errors:
1. Ensure the game is installed
2. Check that `oo2core_9_win64.dll` (or equivalent) exists in the game directory
3. On Linux/Proton, the library may need to be accessible via Wine/Proton

## Batch Processing

### Extract All Weapon Data

```bash
#!/bin/bash
GAME_DIR="$HOME/.steam/steam/steamapps/common/Borderlands 4"
OUT_DIR="/tmp/bl4_weapons"

mkdir -p "$OUT_DIR"

# Extract main container
retoc unpack "$GAME_DIR/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc" "$OUT_DIR"

# Find all weapon-related assets
find "$OUT_DIR" -path "*Gear/Weapons*" -name "*.uasset" > weapon_assets.txt

echo "Found $(wc -l < weapon_assets.txt) weapon assets"
```

### Generate Asset Index

```bash
# Create searchable index of all assets
retoc list "$GAME_DIR/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc" > asset_index.txt
```

## Usmap Generation

For parsing extracted .uasset files, you need a mappings file (`.usmap`) that describes all UE5 class/struct layouts. See [usmap.md](usmap.md) for details.

**Quick generation:**
```bash
# Requires a memory dump from the running game
bl4 memory --dump ./share/dumps/your_dump.raw dump-usmap

# Output in share/manifest/mappings.usmap
```

## Related Documentation

- [Data Structures](data_structures.md) - Item serial format and game structures
- [Weapons](weapons.md) - Weapon mechanics and part system
- [Usmap](usmap.md) - Usmap generation from memory dumps
