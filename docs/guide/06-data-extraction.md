# Chapter 6: Data Extraction

To build a save editor, we need game data: weapon stats, part definitions, manufacturer info. This chapter covers extracting that data from BL4's pak files.

---

## Game File Overview

BL4's data is stored in Unreal Engine pak files:

```
Borderlands 4/OakGame/Content/Paks/
├── pakchunk0-Windows_0_P.utoc    ← Main game assets
├── pakchunk0-Windows_0_P.ucas
├── pakchunk2-Windows_0_P.utoc    ← Audio (Wwise)
├── pakchunk2-Windows_0_P.ucas
├── pakchunk3-Windows_0_P.utoc    ← Localized audio
├── global.utoc                   ← Shared engine data
└── ...
```

### File Types

| Extension | Format | Purpose |
|-----------|--------|---------|
| `.utoc` | Table of Contents | Asset index for IoStore |
| `.ucas` | Container Archive | Compressed asset data |
| `.pak` | Legacy PAK | Some configs/audio |
| `.uasset` | Extracted Asset | Object definitions |
| `.uexp` | Export Data | Bulk data (textures, meshes) |

!!! note
    BL4 uses **IoStore** (UE5's new format), not the older PAK-only format. Tools must support both.

---

## Extraction Tools

### retoc — IoStore Extraction

Extracts assets from `.utoc/.ucas` containers.

**Install**:
```bash
cargo install --git https://github.com/trumank/retoc retoc_cli
```

**Usage**:
```bash
# List assets
retoc list /path/to/pakchunk0-Windows_0_P.utoc

# Extract all assets (Zen format)
retoc unpack /path/to/pakchunk0-Windows_0_P.utoc ./output/

# Convert to legacy format (for other tools)
retoc to-legacy /path/to/Paks/ ./output/ --no-script-objects
```

!!! warning
    For `to-legacy`, point at the **Paks directory**, not a single file. It needs `global.utoc` for ScriptObjects.

### repak — Legacy PAK Files

For older-style PAK files:

```bash
cargo install repak

repak list /path/to/file.pak
repak unpack /path/to/file.pak ./output/
```

### uextract — Our Custom Tool

The bl4 project includes `uextract` for parsing extracted assets:

```bash
cargo build --release -p uextract

# List all assets
./target/release/uextract /path/to/Paks --list

# Extract with filtering
./target/release/uextract /path/to/Paks -o ./output --ifilter "BalanceData"

# Use usmap for property resolution
./target/release/uextract /path/to/Paks -o ./output --usmap share/borderlands.usmap
```

---

## Asset Hierarchy

Extracted assets follow Unreal's content structure:

```
OakGame/Content/
├── Gear/
│   ├── Weapons/
│   │   ├── _Shared/           # Shared weapon data
│   │   │   ├── BalanceData/   # Stat templates
│   │   │   └── NamingStrategies/
│   │   ├── AssaultRifles/
│   │   ├── Pistols/
│   │   ├── Shotguns/
│   │   ├── SMGs/
│   │   ├── SniperRifles/
│   │   └── HeavyWeapons/
│   ├── Shields/
│   └── GrenadeGadgets/
├── PlayerCharacters/
│   ├── DarkSiren/
│   ├── Paladin/
│   ├── Gravitar/
│   └── ExoSoldier/
└── GameData/
    └── Loot/                  # Drop pools, rarity tables
```

---

## Understanding .uasset Files

### Zen Package Format (UE5)

UE5 uses "Zen" packages with unversioned serialization:

```
┌─────────────────────────────┐
│       Package Header        │
├─────────────────────────────┤
│        Name Map             │  ← Local FName table
├─────────────────────────────┤
│       Import Map            │  ← External dependencies
├─────────────────────────────┤
│       Export Map            │  ← Objects in this package
├─────────────────────────────┤
│      Export Data            │  ← Serialized properties
└─────────────────────────────┘
```

### Unversioned Serialization

Properties are serialized without field names:

```
Versioned (old):   "Damage" : 50.0, "Level" : 10
Unversioned (new): 0x42480000 0x0000000A
                   └── Just values, no names
```

To parse unversioned data, you need a **usmap** file that provides the schema.

### Fragment Headers

UE5 uses fragment-based serialization:

```
Fragment: [SkipNum:7][HasZeroes:1][IsLast:1][ValueCount:7]

Meaning: "Skip N properties, then serialize M properties"
```

This allows sparse serialization (only non-default values stored).

---

## The Usmap File

A usmap contains all class/struct definitions needed to parse assets.

### Why You Need It

Without usmap:
```
Raw bytes: 00 00 48 42 00 00 00 0A 00 00 80 3F
Result: ???
```

With usmap:
```
Struct: FWeaponStats
  +0x00 Damage (f32) = 50.0
  +0x04 Level (u32) = 10
  +0x08 Scale (f32) = 1.0
```

### Generating Usmap

We generate usmap from memory dumps:

```bash
bl4 memory --dump share/dumps/game.raw dump-usmap

# Output: BL4.usmap
# Names: 64917, Enums: 2986, Structs: 16849, Properties: 58793
```

### Pre-Generated Usmap

The project includes a pre-generated usmap:

```
share/manifest/mappings.usmap
```

Use it with extraction tools:

```bash
./target/release/uextract /path/to/Paks -o ./output --usmap share/manifest/mappings.usmap
```

---

## Key Asset Types

### Balance Data

Weapon/gear stat templates:

```
OakGame/Content/Gear/Weapons/_Shared/BalanceData/
├── WeaponStats/
│   └── Struct_Weapon_Barrel_Init.uasset
├── Rarity/
│   └── Struct_Weapon_RarityInit.uasset
└── Elemental/
    └── Struct_Weapon_Elemental_Init.uasset
```

### Naming Strategies

How weapons get their prefixes:

```
OakGame/Content/Gear/Weapons/_Shared/NamingStrategies/
└── WeaponNamingStruct.uasset
```

Maps stat types to prefix names:
- Damage → "Tortuous", "Agonizing"
- ReloadSpeed → "Frenetic", "Manic"
- etc.

### Part Data

Individual weapon parts:

```
OakGame/Content/Gear/Weapons/Pistols/JAK/Parts/
├── Barrel/
│   ├── JAK_PS_Barrel_01.uasset
│   └── JAK_PS_Barrel_02.uasset
├── Grip/
└── Scope/
```

### Loot Pools

Drop tables and rarity weights:

```
OakGame/Content/GameData/Loot/
├── ItemPools/
│   ├── ItemPoolDef_Boss.uasset
│   └── ItemPoolDef_Legendary.uasset
└── RarityData/
```

---

## Using bl4-research

The bl4 project includes research tools for organizing extracted data.

### Generate Manifest

```bash
cargo build --release -p bl4-research

# Generate pak manifest
bl4-research pak-manifest -e share/manifest/extracted -o share/manifest

# Generate items database
bl4-research items-db -m share/manifest
```

### Output Files

```
share/manifest/
├── index.json           # Manifest metadata
├── pak_manifest.json    # 81,097 indexed assets
├── pak_summary.json     # Statistics
├── manufacturers.json   # 10 manufacturers
├── weapons_breakdown.json
├── items_database.json  # Drop pools, stats
├── mappings.usmap       # Reflection data
└── ...
```

---

## Practical: Extracting Weapon Data

### Step 1: Extract Assets

```bash
# Extract everything from main pak
retoc unpack ~/.steam/steam/steamapps/common/"Borderlands 4"/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc ./bl4_assets/
```

### Step 2: Find Weapon Balance Data

```bash
find ./bl4_assets -path "*BalanceData*" -name "*.uasset" | head -20
```

### Step 3: Parse with Usmap

```bash
./target/release/uextract ./bl4_assets -o ./parsed --usmap share/manifest/mappings.usmap --ifilter "Struct_Weapon"
```

### Step 4: Examine Output

```bash
cat ./parsed/Struct_Weapon_Barrel_Init.json
```

```json
{
  "asset_path": "OakGame/Content/Gear/Weapons/_Shared/BalanceData/WeaponStats/Struct_Weapon_Barrel_Init",
  "exports": [
    {
      "class": "ScriptStruct",
      "properties": {
        "Damage_Scale": 1.0,
        "FireRate_Scale": 1.0,
        "Accuracy_Scale": 1.0,
        ...
      }
    }
  ]
}
```

---

## Finding Specific Data

### Search by Asset Name

```bash
# Find all legendary items
find ./bl4_assets -name "*legendary*" -type f

# Find manufacturer data
find ./bl4_assets -iname "*manufacturer*"

# Find class mods
find ./bl4_assets -path "*ClassMod*"
```

### Search by Content

```bash
# Find assets mentioning "Linebacker"
grep -r "Linebacker" ./bl4_assets --include="*.uasset" -l

# Find specific stat types
strings ./bl4_assets/**/*.uasset | grep "BaseDamage"
```

### Using uextract Filters

```bash
# Only extract weapon-related assets
./target/release/uextract /path/to/Paks --list --ifilter "Weapon"

# Only extract specific manufacturers
./target/release/uextract /path/to/Paks --list --ifilter "MAL_"  # Maliwan
./target/release/uextract /path/to/Paks --list --ifilter "JAK_"  # Jakobs
```

---

## Asset Property Patterns

### Stat Properties

Stats follow the pattern: `StatName_ModifierType_Index_GUID`

```
Damage_Scale_14_4D6E5A8840F57DBD840197B3CB05686D
CritDamage_Add_50_740BF8EA43AFEE45A6A954B40FD8101E
FireRate_Value_36_67DA482B483B02CAC87864955A611952
```

| Modifier | Meaning |
|----------|---------|
| `Scale` | Multiplier (×) |
| `Add` | Flat addition (+) |
| `Value` | Absolute override |
| `Percent` | Percentage bonus |

### Common Stats

| Stat | Description |
|------|-------------|
| `Damage` | Base damage per shot |
| `CritDamage` | Critical hit multiplier |
| `FireRate` | Shots per second |
| `ReloadTime` | Reload duration |
| `MagSize` | Magazine capacity |
| `Accuracy` | Base accuracy |
| `ProjectilesPerShot` | Pellets (shotguns) |
| `StatusChance` | Elemental proc chance |

---

## Compression: Oodle

BL4 uses Oodle compression for pak files.

### What Is Oodle?

Oodle is a commercial compression library by RAD Game Tools. It's fast and achieves high ratios.

### Using Oodle

The `retoc` tool handles Oodle automatically by loading the library from the game:

```bash
# Oodle library location
~/.steam/steam/steamapps/common/"Borderlands 4"/Engine/Binaries/ThirdParty/Oodle/
└── oo2core_9_win64.dll
```

On Linux/Proton, Wine loads this DLL when retoc runs.

!!! tip
    If extraction fails with Oodle errors, ensure the game is installed and the DLL path is accessible.

---

## Building a Data Pipeline

### Automated Extraction Script

```bash
#!/bin/bash
# extract_bl4_data.sh

GAME_DIR="$HOME/.steam/steam/steamapps/common/Borderlands 4"
OUTPUT_DIR="./bl4_data"
USMAP="./share/manifest/mappings.usmap"

# Step 1: Extract pak files
echo "Extracting pak files..."
retoc unpack "$GAME_DIR/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc" "$OUTPUT_DIR/raw"

# Step 2: Parse with usmap
echo "Parsing assets..."
./target/release/uextract "$OUTPUT_DIR/raw" -o "$OUTPUT_DIR/parsed" --usmap "$USMAP"

# Step 3: Generate manifest
echo "Generating manifest..."
bl4-research pak-manifest -e "$OUTPUT_DIR/parsed" -o "$OUTPUT_DIR/manifest"

echo "Done! Output in $OUTPUT_DIR"
```

### Manifest Structure

The generated manifest contains:

```json
{
  "version": "1.2",
  "source": "BL4 Pak Files + Memory Dump",
  "files": {
    "pak_manifest": "pak_manifest.json",
    "items_database": "items_database.json",
    "manufacturers": "manufacturers.json"
  },
  "mappings": {
    "names": 64917,
    "enums": 2986,
    "structs": 16849,
    "properties": 58793
  }
}
```

---

## Exercises

### Exercise 1: Find Your Favorite Weapon

1. Extract the main pak
2. Search for a legendary weapon you know (e.g., "Linebacker")
3. Find its balance data asset
4. Examine its base stats

### Exercise 2: Compare Manufacturers

1. Find all assets for two manufacturers (e.g., Jakobs vs Maliwan)
2. Compare their shared weapon types
3. Note any obvious stat pattern differences

### Exercise 3: Map a Drop Pool

1. Find `ItemPoolDef_Boss.uasset` or similar
2. Extract and parse it
3. Document what items can drop from bosses

---

## Troubleshooting

### "Oodle decompression failed"

- Ensure game is installed
- Check that `oo2core_9_win64.dll` exists
- On Linux, verify Wine can access the path

### "Unknown property type"

- Usmap might be outdated (game patched)
- Re-generate usmap from fresh memory dump

### "Asset not found"

- Asset might be in different pakchunk
- Try extracting from all pakchunks
- Check for DLC content in separate files

---

## Key Takeaways

1. **IoStore is UE5's format** — Use retoc, not old pak tools
2. **Usmap is essential** — Unversioned data needs schema
3. **Assets follow patterns** — Learn the directory structure
4. **Automate extraction** — Scripts save time for updates

---

## Next Chapter

Now that we have data, let's put it all together with the bl4 command-line tools.

**Next: [Chapter 7: Using bl4 Tools](07-bl4-tools.md)**
