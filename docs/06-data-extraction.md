# Chapter 6: Data Extraction

A save editor needs game data: weapon stats, part definitions, manufacturer information. You might assume this data lives neatly in game files, waiting to be extracted. The reality is more complicated—and more interesting.

This chapter explores what data we can extract, what we can't, and why. Along the way, we'll document our investigation into authoritative category mappings, including the binary analysis that revealed why some data simply doesn't exist in extractable form.

---

## The Game File Landscape

BL4's data lives in Unreal Engine pak files, stored in IoStore format:

```text
Borderlands 4/OakGame/Content/Paks/
├── pakchunk0-Windows_0_P.utoc    ← Main game assets
├── pakchunk0-Windows_0_P.ucas    ← Compressed data
├── pakchunk2-Windows_0_P.utoc    ← Audio (Wwise)
├── pakchunk3-Windows_0_P.utoc    ← Localized audio
├── global.utoc                   ← Shared engine data
└── ...
```

**IoStore** is UE5's container format, splitting asset indices (`.utoc`) from compressed data (`.ucas`). This differs from older PAK-only formats and requires specialized tools.

!!! note
    BL4 uses IoStore (UE5's format), not legacy PAK. Tools like `repak` won't work on `.utoc/.ucas` files. You need `retoc` or similar IoStore-aware extractors.

---

## What We Can Extract

Some game data extracts cleanly from pak files:

**Balance data**: Stat templates and modifiers for weapons, shields, and gear. These define base damage, fire rate, accuracy scales.

**Naming strategies**: How weapons get their prefix names. "Damage → Tortuous" mappings live in extractable assets.

**Body definitions**: Weapon body assets that reference parts and mesh fragments.

**Loot pools**: Drop tables and rarity weights for different sources.

**Gestalt meshes**: Visual mesh fragments that parts reference.

These assets follow Unreal's content structure:

```text
OakGame/Content/
├── Gear/
│   ├── Weapons/
│   │   ├── _Shared/BalanceData/
│   │   ├── Pistols/JAK/Parts/
│   │   └── ...
│   └── Shields/
├── PlayerCharacters/
│   ├── DarkSiren/
│   └── ...
└── GameData/Loot/
```

---

## What We Can't Extract

Here's where it gets interesting. The mappings between serial tokens and actual game parts—the heart of what makes serial decoding work—don't exist as extractable pak file assets.

We wanted authoritative category mappings. Serial token `{4}` on a Vladof SMG should mean a specific part, and we wanted the game's own data to tell us which one. So we investigated.

### The Investigation: Binary Analysis

We used Rizin (a radare2 fork) to analyze the Borderlands4.exe binary directly:

```bash
rz-bin -S Borderlands4.exe
```

Results:
- Total size: 715 MB
- .sdata section: 157 MB (code)
- .rodata section: 313 MB (read-only data)

We searched for part prefix strings like "DAD_PS.part_" and "VLA_SM.part_barrel". Nothing. The prefixes don't exist as literal strings in the binary.

We searched for category value sequences. Serial decoding uses Part Group IDs like 2, 3, 4, 5, 6, 7 (consecutive integers stored as i64). We found one promising sequence at offset 0x02367554:

```python
# Found sequence 2,3,4,5,6,7 as consecutive i64 values at 0x02367554
```

But examining the context revealed it was near crypto code—specifically "Poly1305 for x86_64, CRYPTOGAMS". Those consecutive integers were coincidental, not category definitions.

!!! warning "False Positives"
    When searching binaries for numeric patterns, verify the context. Small consecutive integers appear in many places: crypto code, lookup tables, version numbers. Always examine surrounding bytes.

### UE5 Metadata: What We Know

From usmap analysis, we confirmed the exact structure linking parts to serials:

```text
GbxSerialNumberIndex (12 bytes)
├── Category (Int64): Part Group ID
├── scope (Byte): EGbxSerialNumberIndexScope (Root=1, Sub=2)
├── status (Byte): EGbxSerialNumberIndexStatus
└── Index (Int16): Position in category
```

Every `InventoryPartDef` contains this structure. The `Category` field maps to Part Group IDs (2=Daedalus Pistol, 22=Vladof SMG, etc.). The `Index` field determines which part token decodes to this part.

But here's the problem: we found **zero** `InventoryPartDef` assets in pak files.

```bash
uextract /path/to/Paks find-by-class InventoryPartDef
# Result: 0 assets found
```

### Where Parts Actually Live

Parts aren't stored as individual pak file assets. They're:

1. **Runtime UObjects** — Created when the game initializes
2. **Code-defined** — Registrations happen in native code
3. **Self-describing** — Each part carries its own index internally

### The Key Insight: Self-Describing Parts

Here's the crucial design pattern we discovered: **there is no separate mapping file because each part stores its own index**.

Every part UObject contains a `GbxSerialNumberIndex` structure at offset +0x28:

```text
UObject + 0x28: GbxSerialNumberIndex (4 bytes)
├── Scope (1 byte)   ← EGbxSerialNumberIndexScope (Root=1, Sub=2)
├── Status (1 byte)  ← Reserved/state flags
└── Index (2 bytes)  ← THE serial index for this part
```

This is a "reverse mapping" architecture:

- **Traditional approach**: Separate lookup table maps `index → part_name`
- **BL4's approach**: Each part stores its own index; the "mapping" IS the parts themselves

**Why this design makes sense:**

| Benefit | Explanation |
|---------|-------------|
| No central registry | Adding DLC parts doesn't require updating a mapping file |
| Self-contained | Each part is fully self-describing |
| Stable indices | A part's index never changes because it's intrinsic to that part |
| No sync issues | Impossible for mapping to drift from actual parts |

**Practical implication**: When we extract parts from memory, we're not building a mapping from separate data—we're reading the authoritative index directly from each part. The memory dump contains the complete, correct mapping because that mapping IS the parts.

!!! note "Why Memory Dumps Are Essential"
    Since each part carries its own index internally, and parts only exist as runtime UObjects (not pak file assets), memory dumps are the only way to capture this data. The game's binary contains the code to create parts, but the actual GbxSerialNumberIndex values are set during initialization.

---

## Memory Extraction: The Breakthrough

Through systematic memory analysis, we discovered **authoritative part-to-index mappings can be extracted** from memory dumps. Here's the structure:

### The Part Registration Structure

When the game loads, it creates UObjects for each part and registers them in an internal array. This array has a discoverable pattern:

```text
Part Array Entry (24 bytes):
├── FName Index (4 bytes)     ← References the part name in FNamePool
├── Padding (4 bytes)         ← Always zero
├── Pointer (8 bytes)         ← Address of the part's UObject
├── Marker (4 bytes)          ← 0xFFFFFFFF sentinel value
└── Priority (4 bytes)        ← Selection priority (not the serial index!)
```

The serial **Index** is stored **inside the pointed UObject**, at offset +0x28:

```text
UObject at Pointer (offset +0x28):
├── Scope (1 byte)            ← EGbxSerialNumberIndexScope (always 2 for parts)
├── Reserved (1 byte)         ← Usually 0
└── Index (2 bytes, Int16)    ← THE SERIAL INDEX we need!
```

!!! important "Category Derivation"
    The Part Group ID (category) is **not** stored in the UObject at a fixed offset. Instead, derive it from the part name prefix (e.g., `DAD_PS` → category 2, `VLA_AR` → category 17). The bl4 tool includes a complete prefix-to-category mapping.

### Verified Example

Searching for FName `DAD_PS.part_barrel_01` (FName index 0x736a0a):

1. **Find the array entry**: FName appears in the part array with pointer 0x7ff4ca7d75d0
2. **Read offset +0x28**: At 0x7ff4ca7d75f8 we find `02 00 07 00`
3. **Parse**: Scope=2, Reserved=0, **Index=7**
4. **Derive category**: `DAD_PS` prefix → category 2
5. **Verify**: Reference data confirms DAD_PS.part_barrel_01 has index 7 ✓

Additional verified mappings:
- `DAD_PS.part_barrel_02` → Index 8 ✓
- `DAD_PS.part_barrel_01_Zipgun` → Index 1 ✓
- `DAD_PS.part_barrel_02_rangefinder` → Index 78 ✓

### Extraction Algorithm

```python
# Pseudocode for extracting all part mappings
def extract_parts(memory_dump):
    # Step 1: Build FName lookup table
    # Scan FNamePool for all names containing ".part_"
    fname_table = {}  # fname_idx -> name
    for block in fnamepool.blocks:
        for entry in block:
            if ".part_" in entry.name.lower():
                fname_table[entry.index] = entry.name

    parts = []

    # Step 2: Scan memory for 0xFFFFFFFF markers
    for marker_addr in scan(memory_dump, "ff ff ff ff"):
        # Read the 24-byte entry (marker is at offset 16)
        entry = read(marker_addr - 16, 24)

        fname_idx = entry[0:4]      # FName index
        pointer = entry[8:16]       # UObject pointer

        # Validate: known FName, padding=0, valid pointer
        if fname_idx not in fname_table:
            continue
        if entry[4:8] != 0 or not is_valid_pointer(pointer):
            continue

        # Read serial index from pointed UObject at offset +0x28
        uobject = read(pointer, 0x2C)
        scope = uobject[0x28]
        index = uobject[0x2A:0x2C]  # Int16 at bytes 2-3

        # Derive category from part name prefix
        name = fname_table[fname_idx]
        category = get_category_from_prefix(name)

        if category is not None:
            parts.append({
                'name': name,
                'category': category,
                'index': index
            })

    return parts

def get_category_from_prefix(name):
    prefix = name.split(".part_")[0].lower()
    # Pistols
    if prefix == "dad_ps": return 2
    if prefix == "jak_ps": return 3
    # ... (complete mapping in bl4 source)
    return None
```

### Why This Works

The game registers parts at startup into internal arrays. Each entry links:
- **FName reference** → The part's name (e.g., "VLA_SM.part_barrel_01")
- **UObject pointer** → The full part definition, including serial index

By scanning for the 0xFFFFFFFF sentinel pattern that marks entry boundaries, we can walk these arrays and extract every part mapping the game knows about.

!!! tip "Practical Implication"
    Memory dumps contain **authoritative** part-to-index mappings. Extract them directly—no empirical testing required for known parts. Empirical validation is only needed for new parts added in patches.

### Extraction Results and Limitations

Running the extraction on a Dec 2025 memory dump yields:

| Metric | Value |
|--------|-------|
| Total parts extracted | ~1,041 |
| Categories covered | 47 |
| Expected parts (estimated) | ~2,600+ |

!!! warning "Incomplete Coverage"
    Memory extraction only captures parts that were **instantiated in memory** at the time of the dump. This means:

    - **Significant gaps exist** in index sequences (e.g., category 2 has indices 2, 3, 5-8, 10-11... but missing 0, 1, 4, 9, etc.)
    - **Only ~40% of parts** have authoritative indices
    - Parts not spawned during the capture session are missing
    - Static part pool definitions aren't available through this method

**What we have:**
- Accurate indices for parts that WERE instantiated
- Complete category-to-prefix mappings
- Manufacturer associations

**What we're missing:**
- Complete part lists per category
- Parts from DLC/content not loaded during capture
- Static part pool definitions (which would give complete part sets)

### Data Pipeline: Current State

The part mapping workflow uses multiple data sources:

**1. Part Names (Complete)**

`share/manifest/part_pools.json` contains the complete list of 2,566 part names, organized by category. Source: memory dump FName extraction.

```bash
# Part names are complete - we know ALL parts that exist
bl4 memory --dump game.dmp dump-parts -o parts_dump.json
```

**2. Part Indices (Incomplete)**

`share/manifest/parts_database.json` contains ~1,041 parts with authoritative indices (40% coverage). Source: memory dump UObject scanning.

```bash
# Extract indices from memory - only gets instantiated parts
bl4 memory --dump game.dmp extract-parts -o parts_with_categories.json
```

**The Gap**: We have all part names but only 40% of indices. Missing indices are for parts not instantiated during the memory capture.

### What NCS/UASSET Data Provides

Current NCS extraction yields 806 files with 232 unique types:

| NCS Type | Contents | Part Data |
|----------|----------|-----------|
| `inv.bin` | 13,860 inventory entries | Part attributes, stats, **serial indices** |
| `inv_name_part.bin` | 946 part naming entries | Display names |
| `GbxActorPart.bin` | 2,167 actor parts | Cosmetic/mesh parts with indices |
| `itempool.bin` | Item pool definitions | Loot tables |

### NCS Serial Index Discovery

**Breakthrough**: NCS `inv.bin` DOES contain serial indices for weapon parts! The indices are stored in the binary section entries.

**Format**: Part names in NCS use a different format than memory:

| NCS Format | Memory Format | Index |
|------------|---------------|-------|
| `BOR_SG_Grip_01` | `BOR_SG.part_grip_01` | 42 |
| `BOR_SG_Foregrip_02` | `BOR_SG.part_foregrip_02` | 81 |
| `BOR_SG_Barrel_02_B` | `BOR_SG.part_barrel_02_b` | 71 |

**Verified matches** between NCS indices and memory-extracted indices:

- `BOR_SG_Grip_01` = 42 ✓
- `BOR_SG_Grip_02` = 43 ✓
- `BOR_SG_Foregrip_01` = 50 ✓
- `BOR_SG_Foregrip_02` = 81 ✓
- `BOR_SG_Barrel_02_B` = 71 ✓
- `BOR_SG_Barrel_02_D` = 73 ✓

**Important caveats**:

1. Not all records with `value_0` are indices (some are attribute values)
2. Only records matching weapon part naming patterns (e.g., `XXX_YY_Part_Type`) contain indices
3. Some mismatches exist (e.g., `BOR_SG_Barrel_01` shows 4 in NCS but 7 in memory)
4. NCS may contain MORE parts than memory extraction captures

**Extraction approach**: Filter for records where the name matches weapon part patterns, then extract `value_0` as the serial index. Cross-reference with memory-extracted indices where available.

### Strategies for Complete Index Coverage

1. **NCS extraction (preferred)** — Extract indices from `inv.bin` `value_0` fields. This is static game data that doesn't require memory dumps.
2. **Memory dumps (validation)** — Use memory-extracted indices to validate NCS data and capture parts with mismatched formats.
3. **Multiple memory dumps** — Capture game state with different loadouts to instantiate more parts.
4. **Empirical validation** — Verify unknown indices by testing serials in-game.

**Recommended workflow**:

1. Extract all potential indices from NCS `inv.bin`
2. Cross-reference with memory-extracted indices
3. For matches, trust the data
4. For mismatches, investigate (NCS format may differ from memory format)
5. For NCS-only parts, treat indices as provisional until validated

### Category Derivation from NCS (Jan 2026 Discovery)

**Finding**: NCS files do NOT directly store category IDs. However, categories can be derived from part name prefixes:

```rust
// Prefix-to-category mapping
"BOR_SG" → Category 12  (Ripper Shotgun)
"JAK_SG" → Category 9   (Jakobs Shotgun)
"DAD_PS" → Category 2   (Daedalus Pistol)
"VLA_AR" → Category 17  (Vladof Assault Rifle)
// ... etc
```

**Implementation**: The `bl4-ncs` library includes `category_from_prefix()` function that extracts the manufacturer-weapon prefix and maps it to the corresponding category ID.

**Limitations**:

1. **Only works for prefixed parts**: Parts like `BOR_SG_Barrel_01_A` derive categories successfully
2. **Non-prefixed parts fail**: Generic parts (`comp_01_common`, `part_firmware_*`, `part_ra_*`) have no prefix, so category cannot be determined from NCS alone
3. **Same part name, different categories**: Parts like `comp_01_common` exist in many categories with different indices. Without category context, these cannot be uniquely identified.

**Result**: NCS extraction using BinaryParserV2 yields:
- **875 part names** extracted from `inv*.bin` files
- **38 parts with categories** (only manufacturer-prefixed parts like BOR_SG, ORD_AR)
- **837 parts without categories** (generic parts like comp_*, part_firmware_*)

**Conclusion**: NCS provides part indices but NOT categories. For complete part database:
- Use NCS for manufacturer-specific weapon parts (BOR_SG, JAK_PS, etc.)
- Requires memory dumps or other sources for non-prefixed parts
- Categories must be derived from prefixes, not extracted from NCS data

---

## Empirical Validation (Fallback)

For edge cases or when memory extraction isn't possible, empirical validation remains an option:

1. Collect serials from real game items
2. Decode the Part Group ID and part tokens
3. Record which weapon/part combinations the tokens represent
4. Validate by injecting serials into saves and checking in-game

The `parts_database.json` file combines memory-extracted mappings with empirically-verified data for comprehensive coverage.

---

## Extraction Tools

### retoc — IoStore Extraction

The essential tool for BL4's pak format:

```bash
cargo install --git https://github.com/trumank/retoc retoc_cli

# List assets in a container
retoc list /path/to/pakchunk0-Windows_0_P.utoc

# Extract all assets
retoc unpack /path/to/pakchunk0-Windows_0_P.utoc ./output/
```

!!! warning
    For converting to legacy format, point at the **Paks directory**, not a single file. The tool needs access to `global.utoc` for ScriptObjects:
    ```bash
    retoc to-legacy /path/to/Paks/ ./output/ --no-script-objects
    ```

### uextract — Project Tool

The bl4 project's custom extraction tool:

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

## The Usmap Requirement

UE5 uses "unversioned" serialization. Properties are stored without field names:

```text
Versioned (old):   "Damage": 50.0, "Level": 10
Unversioned (new): 0x42480000 0x0000000A
                   └── Just values, no names
```

To parse unversioned data, you need a usmap file containing the schema—all class definitions, property names, types, and offsets.

We generate usmap from memory dumps:

```bash
bl4 memory --dump share/dumps/game.dmp dump-usmap

# Output: mappings.usmap
# Names: 64917, Enums: 2986, Structs: 16849, Properties: 58793
```

The project includes a pre-generated usmap at `share/manifest/mappings.usmap`.

---

## Extracting Parts from Memory

Since parts only exist at runtime, memory extraction is the path forward.

### Step 1: Create Memory Dump

Follow Chapter 3's instructions to capture game memory while playing.

### Step 2: Extract Part Names

```bash
bl4 memory --dump share/dumps/game.dmp dump-parts \
    -o share/manifest/parts_dump.json
```

This scans for strings matching `XXX_YY.part_*` patterns:

```json
{
  "DAD_AR": [
    "DAD_AR.part_barrel_01",
    "DAD_AR.part_barrel_01_a",
    "DAD_AR.part_body"
  ],
  "VLA_SM": [
    "VLA_SM.part_barrel_01"
  ]
}
```

### Step 3: Build Parts Database

```bash
bl4 memory --dump share/dumps/game.dmp build-parts-db \
    -i share/manifest/parts_dump.json \
    -o share/manifest/parts_database.json
```

The result maps parts to categories and indices:

```json
{
  "parts": [
    {"category": 2, "index": 0, "name": "DAD_PS.part_barrel_01"},
    {"category": 22, "index": 5, "name": "VLA_SM.part_body_a"}
  ],
  "categories": {
    "2": {"count": 74, "name": "Daedalus Pistol"},
    "22": {"count": 84, "name": "Vladof SMG"}
  }
}
```

!!! important "Index Ordering"
    Part indices from memory dumps reflect the game's internal registration order—not alphabetical. Parts typically register in this order: unique variants, bodies, barrels, shields, magazines, scopes, grips, licensed parts. Alphabetical sorting produces wrong indices.

---

## Working with Extracted Assets

### Asset Structure

Extracted `.uasset` files follow the Zen package format:

```text
Package
├── Header
├── Name Map (local FNames)
├── Import Map (external dependencies)
├── Export Map (objects defined here)
└── Export Data (serialized properties)
```

With usmap, these parse into readable JSON:

```json
{
  "asset_path": "OakGame/Content/Gear/Weapons/_Shared/BalanceData/WeaponStats/Struct_Weapon_Barrel_Init",
  "exports": [
    {
      "class": "ScriptStruct",
      "properties": {
        "Damage_Scale": 1.0,
        "FireRate_Scale": 1.0,
        "Accuracy_Scale": 1.0
      }
    }
  ]
}
```

### Finding Specific Data

```bash
# Find legendary items
find ./bl4_assets -name "*legendary*" -type f

# Find manufacturer data
find ./bl4_assets -iname "*manufacturer*"

# Search asset contents
grep -r "Linebacker" ./bl4_assets --include="*.uasset" -l
```

### Stat Patterns

Stats follow naming conventions: `StatName_ModifierType_Index_GUID`

| Modifier | Meaning |
|----------|---------|
| `Scale` | Multiplier (×) |
| `Add` | Flat addition (+) |
| `Value` | Absolute override |
| `Percent` | Percentage bonus |

---

## Oodle Compression

BL4 uses Oodle compression (RAD Game Tools). The `retoc` tool handles decompression automatically by loading the game's DLL:

```text
~/.steam/steam/steamapps/common/"Borderlands 4"/Engine/Binaries/ThirdParty/Oodle/
└── oo2core_9_win64.dll
```

!!! tip
    If extraction fails with Oodle errors, verify the game is installed and the DLL path is accessible. On Linux, Wine must be able to load the DLL.

---

## Building a Data Pipeline

An automated extraction script saves time when the game updates:

```bash
#!/bin/bash
GAME_DIR="$HOME/.steam/steam/steamapps/common/Borderlands 4"
OUTPUT_DIR="./bl4_data"
USMAP="./share/manifest/mappings.usmap"

# Extract pak files
retoc unpack "$GAME_DIR/OakGame/Content/Paks/pakchunk0-Windows_0_P.utoc" "$OUTPUT_DIR/raw"

# Parse with usmap
./target/release/uextract "$OUTPUT_DIR/raw" -o "$OUTPUT_DIR/parsed" --usmap "$USMAP"
```

---

## Summary: Data Sources

| Data | Source | Extractable? | Status |
|------|--------|--------------|--------|
| Balance/stats | Pak files | Yes | Complete |
| Naming strategies | Pak files / NCS | Yes | Complete |
| Loot pools | Pak files / NCS | Yes | Complete |
| Body definitions | Pak files | Yes | Complete |
| Part names | NCS (`inv.bin`) | Yes | Complete (~13,860 entries) |
| Part serial indices | Memory dump | Partial | ~40% coverage (1,041/~2,600) |
| Category mappings | Code analysis | Yes | Complete |

!!! important "Current State"
    **Part names** are available from NCS data (complete list).
    **Part indices** are only available from memory dumps (incomplete).

    Serial decoding works for the ~1,041 parts we have indices for. Unknown indices display as `[category:index]` placeholders until we can extract complete mappings from static game data or multiple memory dumps.

---

## Exercises

**Exercise 1: Extract and Explore**

Extract the main pak file. Find balance data for a weapon type you use. What stats does the base template define?

**Exercise 2: Search for Part References**

Search extracted assets for references to specific parts (like "JAK_PS.part_barrel"). Where do they appear? What references them?

**Exercise 3: Compare Manufacturers**

Extract assets for two manufacturers (Jakobs vs Maliwan). Compare directory structures. What patterns emerge?

---

## What's Next

We've covered the full data extraction story—what works, what doesn't, and why. The bl4 project wraps all these techniques into command-line tools.

Next, we'll tour those tools: how to decode serials, edit saves, extract data, and more, all from the command line.

**Next: [Chapter 7: Using bl4 Tools](07-bl4-tools.md)**
