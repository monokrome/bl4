# Chapter 8: NCS Format (Nexus Config Store)

This chapter provides a comprehensive technical reference for Gearbox's NCS (Nexus Config Store) binary format used in Borderlands 4.

---

## Overview

NCS is Gearbox's custom format for storing game configuration data that doesn't fit into standard Unreal Engine assets. It contains:

- Item pool definitions (what can drop)
- Loot configuration (drop rates, weights)
- Achievement definitions
- Manufacturer data
- Part preferences
- DataTable serializations

### Why NCS Matters

Standard PAK extraction returns no results for critical game data classes:

```text
ItemPoolDef      - Item pool definitions
ItemPoolListDef  - Item pool lists
loot_config      - Loot configuration
```

These are stored in NCS format embedded within `.pak` files, not as standard `.uasset` files.

---

## File Structure

NCS data exists at two levels:
1. **NCS Chunks** - Oodle-compressed blocks within pak files
2. **Decompressed Content** - The actual typed configuration data

### NCS Chunk Format (Compressed)

#### Outer Header (16 bytes)

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    1     u8     Version (always 0x01)
0x01    3     bytes  Magic: "NCS" (0x4e 0x43 0x53)
0x04    4     u32    Compression flag (0 = raw, non-zero = Oodle)
0x08    4     u32    Decompressed size (little-endian)
0x0c    4     u32    Compressed size (little-endian)
```

#### Inner Header (16+ bytes)

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    4     u32    Oodle magic: 0xb7756362 (big-endian)
0x04    4     bytes  Hash/checksum
0x08    4     u32    Format flags (big-endian)
0x0c    4     u32    Block count (big-endian)
```

#### Format Flags

| Flags | Format | Description |
|-------|--------|-------------|
| `0x00000000` | Single-block | Small files, one Oodle block |
| `0x03030812` | Multi-block | Large files, 256KB blocks |

---

## Decompressed Content Format

After decompression, NCS content follows a structured binary format:

### Overall Layout

```
[Header (8 bytes)]
[Type Name (null-terminated)]
[Format Section (7 bytes)]
[Entry Section (variable)]
[String Table (variable)]
[Control Section (4 bytes)]
[Category Names (variable)]
[Binary Data Section (variable)]
```

### Header (8 bytes)

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    6     bytes  Zeros (reserved)
0x06    2     u16    Checksum/identifier (little-endian)
```

### Type Name

The NCS content type as a null-terminated ASCII string:

- `achievement`
- `ainodefollowsettings`
- `itempool`
- `itempoollist`
- `loot_config`
- `preferredparts`
- etc.

### Format Section (7 bytes)

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    3     bytes  Format prefix (determines structure variant)
0x03    4     ascii  Format code (e.g., "abjx", "abij")
```

#### Format Prefix

| Prefix | Structure |
|--------|-----------|
| `03 03 00` | Compact format (no GUID) |
| `04 xx 00` | Extended format (with 4-byte GUID) |

#### Format Codes

| Code | Description | Examples |
|------|-------------|----------|
| `abjx` | Extended entries with dependents | achievement, preferredparts |
| `abij` | Indexed entries | itempoollist, aim_assist |
| `abjl` | Labeled entries | inv_name_part |
| `abhj` | Hash-indexed entries | - |
| `abpe` | Property-based entries | audio_event |

### Entry Section

Starts with an entry marker byte (`0x01`) followed by field information:

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    1     u8     Entry marker (always 0x01)
0x01    1     u8     First field value (often first ID digit)
0x02    1     u8     Field count marker (0xc0 | count)
```

The field count is encoded as `0xc0 | field_count`:
- `0xc2` = 2 fields per entry (0xc0 | 0x02)
- `0xc3` = 3 fields per entry (0xc0 | 0x03)

### String Table

Contains null-terminated strings for entry names and values, interleaved:

```
[Entry 1 Name\0][Value 1\0][Value 2\0]...
[Entry 2 Name\0][Value 1\0][Value 2\0]...
```

#### Differential Encoding

To save space, entry names use differential encoding:

| Entry | Encoded String | Reconstructed |
|-------|----------------|---------------|
| 1 | `ID_A_10_worldevents_colosseum` | `ID_Achievement_10_worldevents_colosseum` |
| 2 | `1airship` | `ID_Achievement_11_worldevents_airship` |
| 3 | `2meteor` | `ID_Achievement_12_worldevents_meteor` |
| 4 | `1224_missions_side` | `ID_Achievement_24_missions_side` |
| 5 | `9main` | `ID_Achievement_29_missions_main` |

**Algorithm:**
1. First entry is the base (with abbreviation expansion like `ID_A_` → `ID_Achievement_`)
2. Subsequent entries encode only changed portions
3. Leading digit(s) indicate the new suffix characters after the common prefix
4. Remaining text replaces the final segment

#### Packed String Values

When the field count marker (e.g., `0xc2` = 2 fields) indicates multiple fields per entry,
field values are stored consecutively in the string table. However, values can be **packed**
together to save space:

**String Table for Achievement (2 fields per entry):**
```
[0] 'ID_A_10_worldevents_colosseum'  → Entry 0 name
[1] '10'                              → Entry 0 achievementid
[2] '1airship'                        → Entry 1 name (diff)
[3] '11'                              → Entry 1 achievementid
[4] '2meteor'                         → Entry 2 name (diff)
[5] '1224_missions_side'              → PACKED: Entry 2 achievementid + Entry 3 name
[6] '24'                              → Entry 3 achievementid
[7] '9main'                           → Entry 4 name (diff)
[8] '29'                              → Entry 4 achievementid
```

The string `'1224_missions_side'` is a **packed value** containing:
- `'12'` - achievementid for Entry 2 (matches expected 2-digit pattern)
- `'24_missions_side'` - differential name for Entry 3

**Parsing Logic:**
1. After reading Entry 2's differential name (`'2meteor'`), expect an achievementid
2. Read the next string: `'1224_missions_side'`
3. Extract the numeric prefix matching the expected ID length (2 digits): `'12'`
4. Use `'12'` as Entry 2's achievementid
5. Remaining `'24_missions_side'` becomes Entry 3's differential name
6. Read `'24'` as Entry 3's achievementid

This packed encoding occurs when the achievementid and next entry name share numeric prefixes,
allowing concatenation without an explicit separator.

### Value Packing Patterns

NCS files use aggressive value packing to minimize storage. Common patterns:

**1. Numeric Prefix + String Suffix:**
```
"1airship"   -> (1, "airship")     # achievement ID + name
"9main"      -> (9, "main")        # achievement ID + name
"5true"      -> (5, true)          # count + boolean
```

**2. Float + String:**
```
"0.175128Session" -> (0.175128, "Session")  # float value + identifier
```

**3. Multiple Numeric Values:**
```
"1224_missions_side" -> (12, "24_missions_side")  # ID + remaining string
```

**Unpacking Rules:**
- Identify packed strings by: digit prefix + alpha/special suffix
- Split at first non-numeric character (or after float pattern)
- Field abbreviations in binary section indicate expected types
- Context from field definitions determines interpretation

### Control Section (4 bytes)

Marks the transition from string table to category/field definitions:

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    1     u8     Marker (always 0x01)
0x01    1     u8     Separator (always 0x00)
0x02    1     u8     Entry/index count
0x03    1     u8     Type/mode byte
```

#### Type/Mode Byte

| Value | ASCII | Interpretation |
|-------|-------|----------------|
| `0x62` | 'b' | Text-based format |
| `0xe9` | - | Encoded format (0xe0 | 0x09) |
| `0x06` | - | Simple format |

### Category Names

DLC/content pack identifiers as null-terminated strings:

```
none\0
base\0
basegame\0
```

These indicate which DLC or content pack an entry belongs to.

### Binary Data Section

Contains structured data for value lookup. The format varies by content type.

#### abjx Format Binary Section Structure

For compact abjx format files (e.g., `achievement`):

```
Offset  Size    Description
------  ----    -----------
0x00    12      ASCII field abbreviations (e.g., 'corid_aid.a!')
0x0c    4       u32 offset/count value
0x10    4       u32 secondary value
0x14    var     Hash table or lookup data
...     var     Entry metadata (indices, flags)
...     var     Tail section with packed values
-4      4       Checksum (FNV-1a or CRC)
```

**ASCII Field Abbreviations:**

The first bytes encode field names in a compact ASCII format:

| Type | Abbreviation | Decoded Fields |
|------|--------------|----------------|
| achievement | `corid_aid.a!` | `cor`, `id`, `_aid`, `.a`, `!` |
| ainodefollowsettings | `cortldmlwalk` | Travel, Distance, Walk related |

**Tail Section Pattern:**

The tail section (before checksum) contains packed entry metadata:

```
... 13 14 14 14 12 25 08 08 28 13 0a 19 28 13 28 13 28 12 28 28 00 00 ...
```

- `0x28` (40) appears as a separator or marker byte
- Values like `13`, `14`, `12` (19, 20, 18) may encode string lengths or indices
- Final `00 00` marks the end of entry data

#### abij Format Binary Section Structure

For extended abij format files (e.g., `itempoollist`):

```
Offset  Size    Description
------  ----    -----------
0x00    var     Category names (none, base, basegame, ...)
var     var     Field names as null-terminated strings
var     4       u32 start marker (e.g., 0x00 0x1c 0x02 0x00)
...     var     Hash/lookup tables
-4      4       Checksum
```

**Field Names Section:**

Unlike abjx, abij format uses full field names:

```
none\0
base\0
basegame\0
pad\0
cor_dbinstance\0
structtype\0
hand\0
items\0
rowname\0
columnvalue\0
pstbms\0
```

These field names describe the structure of each entry record.

---

## Format Variations by Type

NCS files have different internal structures depending on their format code and type.

### Simple Format (abjx with compact prefix)

Used by: `achievement`, `ainodefollowsettings`

**Control section:** `01 00 [count] [mode]`

**Field definition:** ASCII abbreviations concatenated in binary section:
- `corid_aid.a!` for achievement (2 fields)
- Abbreviations encode field names compactly

### Extended Format (abij with extended prefix)

Used by: `itempoollist`, `damage_filter`

**Control section:** `[count_hi] [count_lo] 01` (different marker!)

**Field definition:** Full field names as null-terminated strings:
```
none\0
base\0
basegame\0
pad\0
cor_dbinstance\0
structtype\0
hand\0
items\0
rowname\0
columnvalue\0
pstbms\0
```

### Hash-Indexed Format (abhj)

Used by: `inv_params`

**Entry marker:** `0xb0` (not `0x01`)

**Field definition:** Long ASCII prefix with concatenated abbreviations:
```
indexbitsizetypeftdwdbrandomizedf
```

For 31 fields, this encodes ~1 char per field on average.

### Summary Table

| Format | Entry Marker | Control Pattern | Field Names |
|--------|--------------|-----------------|-------------|
| `abjx` compact | `0x01` | `01 00 xx yy` | ASCII abbreviations in binary |
| `abij` extended | `0xb0` | `xx yy 01` | Full names as strings |
| `abhj` | `0xb0` | varies | Long ASCII abbreviation |

---

## Type Prefixes

Some values in the string table have single-letter type prefixes:

| Prefix | Type | Example |
|--------|------|---------|
| `T` | Text/String | `Tnone` = string "none" |
| `b` | Base/Boolean | (context-dependent) |
| `F` | Float | (context-dependent) |

---

## Worked Example: achievement.bin

### File Layout (278 bytes)

```
0x000-0x008 (  8 bytes): Header
0x009-0x015 ( 12 bytes): Type name: 'achievement'
0x015-0x01c (  7 bytes): Format: 03 03 00 abjx
0x01c-0x01f (  3 bytes): Entry marker + field info: 01 0a c2
0x01f-0x073 ( 84 bytes): String table (entries + values)
0x073-0x077 (  4 bytes): Control section: 01 00 0b e9
0x077-0x08a ( 19 bytes): Category names: none, base, basegame
0x08a-0x116 (140 bytes): Binary data section
```

### String Table Contents

| Offset | String | Purpose |
|--------|--------|---------|
| 0x1f | `ID_A_10_worldevents_colosseum` | Entry 1 name |
| 0x3d | `10` | Entry 1 achievementid |
| 0x40 | `1airship` | Entry 2 differential |
| 0x49 | `11` | Entry 2 achievementid |
| 0x4c | `2meteor` | Entry 3 differential |
| 0x54 | `1224_missions_side` | Entry 3+4 encoding |
| 0x67 | `24` | Entry 4 achievementid |
| 0x6a | `9main` | Entry 5 differential |
| 0x70 | `29` | Entry 5 achievementid |

### Expected Output (JSON)

```json
{
  "achievement": {
    "records": [{
      "entries": [
        {
          "id_achievement_10_worldevents_colosseum": {
            "achievement": "ID_Achievement_10_worldevents_colosseum",
            "achievementid": "10"
          }
        },
        {
          "id_achievement_11_worldevents_airship": {
            "achievement": "ID_Achievement_11_worldevents_airship",
            "achievementid": "11"
          }
        },
        {
          "id_achievement_12_worldevents_meteor": {
            "achievement": "ID_Achievement_12_worldevents_meteor",
            "achievementid": "12"
          }
        },
        {
          "id_achievement_24_missions_side": {
            "achievement": "ID_Achievement_24_missions_side",
            "achievementid": "24"
          }
        },
        {
          "id_achievement_29_missions_main": {
            "achievement": "ID_Achievement_29_missions_main",
            "achievementid": "29"
          }
        }
      ]
    }]
  }
}
```

---

## DataTable Relationship

NCS files contain serialized DataTable rows that reference schemas in `.uasset` files.

### Schema to NCS Mapping

**Schema File:** `Struct_DedicatedDropProbability.uasset`
```json
{
  "name": "Primary_2_A7EABE6349CCFEA454C199BC8C113D94",
  "value_type": "Double",
  "float_value": 0.0
}
```

**NCS Reference:**
```
Table_DedicatedDropProbability
Prim2_A7EABE6349CCFEA454C199BC8C113D94
```

The GUID portion matches, allowing NCS values to be mapped to their schema types.

### Numeric Values as Strings

Numeric values (weights, probabilities) are stored as strings:
- `"0.200000"` - weight value
- `"1.500000"` - probability

The binary section contains indices into this string table.

---

## Hash Function

Field names are hashed using **FNV-1a 64-bit**:

```rust
const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;

fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash = OFFSET_BASIS;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
```

---

## NCS Manifest

Each pak file contains an NCS manifest at the `_NCS/` path:

```
Offset  Size  Type   Description
------  ----  ----   -----------
0x00    5     bytes  Magic: "_NCS/" (0x5f 0x4e 0x43 0x53 0x2f)
0x05    1     u8     Null terminator
0x06    2     u16    Entry count (little-endian)
0x08    2     u16    Unknown (typically 0x0000)
0x0a    var   Entry  Entry records
```

Each manifest entry:
```
length (u32) | filename (length-1 bytes) | null (u8) | index (u32)
```

Sorting by index gives the correct order matching NCS chunk offsets.

---

## Known File Types

| Type | Description | Count |
|------|-------------|-------|
| `achievement` | Achievement definitions | 1 |
| `aim_assist_parameters` | Aim assist config | 1 |
| `ainodefollowsettings` | AI follow settings | 1 |
| `ainodeLeadsettings` | AI lead settings | 1 |
| `attribute` | Game attributes | ~10 |
| `audio_event` | Audio event mappings | 1 |
| `coordinated_effect_filter` | Effect filters | 1 |
| `gbx_ue_data_table` | Gearbox data tables | many |
| `inv` | Inventory part definitions | 1 |
| `itempool` | Item pool definitions | many |
| `itempoollist` | Item pool lists (boss drops) | many |
| `loot_config` | Loot configuration | many |
| `Mission` | Mission data | many |
| `preferredparts` | Part preferences | 1 |
| `trait_pool` | Trait pool definitions | many |
| `vending_machine` | Vending inventory | many |

### Key Files for Loot Analysis

| File | Purpose |
|------|---------|
| `itempoollist.bin` | Boss → legendary item mappings. Contains `ItemPoolList_<BossName>` records with dedicated drops. |
| `itempool.bin` | General item pool definitions including rarity weights, world drops, Black Market items. |
| `loot_config.bin` | Global loot configuration parameters. |
| `preferredparts.bin` | Part preferences for weapon/gear generation. |
| `inv.bin` | Inventory part definitions with manufacturer prefixes and categories. |

### Extracting Drop Information

Use the `bl4 drops` command to extract and query drop data:

```bash
# Generate drops manifest from NCS data
bl4 drops generate "/path/to/ncs_native" -o share/manifest/drops.json

# Find where an item drops
bl4 drops find hellwalker

# List drops from a specific boss
bl4 drops source Timekeeper
```

See [Appendix C: Loot System Internals](appendix-c-loot-system.md) for detailed documentation.

---

## Entity Display Name Mappings (NameData)

NCS files contain `NameData_*` entries that map internal entity type names to their in-game display names. These are stored as string records in the binary data.

### NameData Format

Each NameData entry follows this pattern:

```
NameData_<InternalType>, <UUID>, <DisplayName>
```

Where:
- **InternalType**: The internal category/type (e.g., `Meathead`, `Thresher`, `Pangolin`)
- **UUID**: A unique identifier (GUID) for the specific variant
- **DisplayName**: The human-readable name shown in-game

### Boss Name Examples

| Internal Type | UUID | Display Name |
|---------------|------|--------------|
| `NameData_Meathead` | `D342D6EE47173677CE1C068BADA88F69` | Saddleback |
| `NameData_Meathead` | `B8EAFB724DAB6362B39A5592718B54B0` | The Immortal Boneface |
| `NameData_Meathead` | `33D8546645185A85D4575F984C7DC44B` | Saddleback & Boneface |
| `NameData_Meathead` | `B632671F4B8F70E97E878BA8CFEEC00B` | Big Encore Saddleback |

### Enemy Variant Examples

Enemies have elemental and rank variants with unique UUIDs:

```
NameData_Thresher, 35D7CBFD4E844BB1624140B84DE69546, Vile Thresher
NameData_Thresher, 7505A0A34FC98F3916DABBA70974675F, Badass Thresher
NameData_Thresher, 4829F4F643423CB6F1F144B4F5A2F2CB, Burning Badass Thresher
NameData_Thresher, 2A0DEEE34653F2E0BD3C8BABC4D1353D, Boreal Badass Thresher

NameData_Bat, 160168F945945478127AD496A3BB0673, Badass Kratch
NameData_Bat, 52A45B3F42B1A9480C36188401A6C801, Vile Kratch
NameData_Bat, 05E8994641C36AF561B109ACD197D81D, Airstrike Kratch
```

### Boss Replay Table

The `Table_BossReplay_Costs` entries also reference boss display names:

```
Table_BossReplay_Costs, 2DCA8E674F8F83E700B52B959C65C2D2, Meathead Riders: Saddleback, The Immortal Boneface
```

### Challenge Text References

UVH (Ultra Vault Hunter) challenge strings contain boss names with location context:

```
UVH_Rankup_2_Challenges, ..., Kill Bramblesong in UVH 1 (Abandoned Auger Mine, Stoneblood Forest, Terminus Range).
UVH_Rankup_2_Challenges, ..., Kill Bio-Thresher Omega in UVH 1 (Fades District, Dominion).
UVH_Rankup_4_Challenges, ..., Kill Mimicron in UVH 3 (Order Bunker, Idolator's Noose, Fadefields).
UVH_Rankup_4_Challenges, ..., Kill Skull Orchid in UVH 3 (Abandoned Auger Mine, Grindstone of the Worthy, Carcadia Burn).
```

### Extracting NameData

Use `strings` to extract NameData mappings from NCS binaries:

```bash
# Get all NameData entries
strings /path/to/ncs_native/*/*.bin | grep "^NameData_" | sort -u

# Get specific boss type mappings
strings /path/to/ncs_native/*/*.bin | grep "^NameData_Meathead"

# Get challenge text with boss names
strings /path/to/ncs_native/*/*.bin | grep "UVH.*Kill"
```

### Internal to Display Name Mapping

Known boss internal → display name mappings:

| Internal Name (itempoollist) | Display Name (NameData) |
|------------------------------|-------------------------|
| `MeatheadRider_Jockey` | Saddleback |
| `Thresher_BioArmoredBig` | Bio-Thresher Omega |
| `Timekeeper_Guardian` | Guardian Timekeeper |
| `BatMatriarch` | Skyspanner Kratch |
| `TrashThresher` | Sludgemaw |
| `StrikerSplitter` | Mimicron |
| `Destroyer` | Bramblesong |

These mappings can be used to translate internal boss names to user-friendly display names in tools.

---

## Compatibility Issues

Some NCS files use Oodle compression parameters not supported by open-source decompressors:

| File | Size | Notes |
|------|------|-------|
| audio_event0 | ~18MB | Large audio mappings |
| coordinated_effect_filter0 | ~300KB | Effect filters |
| DialogQuietTime0 | ~20MB | Dialog timing |
| DialogStyle0 | ~370KB | Dialog styling |

These (~2.4% of files) require the official Oodle SDK for decompression.

---

## Parsing Implementation Notes

This section documents implementation details discovered during parser development.

### Entry Section Format Variations

The entry section after the format code has multiple encoding patterns based on format code and content type:

**1. Simple Format** (`01 XX c?`) - abjx:
```
[0x01] [string_count] [0xc0 | field_count]
```
- Example: achievement `01 0a c2` = 10 strings, 2 fields per entry
- Field count encoded as `0xc0 | count` (0xc2 = 2 fields)
- Control section: `01 00 XX YY` before category names

**2. Extended Format** (`XX XX 01`) - abjx:
```
[string_count] [field_count] [0x01]
```
- Example: itempoollist `1d 06 01` = 29 strings, 6 fields
- String count in first byte (typically > 0x10)
- Field count directly in second byte (2-10)

**3. Direct Format** (`01 XX YY`) - abjx:
```
[0x01] [field_count] [marker]
```
- Example: hit_region `01 03 30` = 3 fields, marker 0x30 ('0')
- Field count directly in second byte
- Third byte varies (0x30, 0x7f, etc.) - meaning unclear

**4. Variable Format** (`01 XX 7f`) - abjx:
```
[0x01] [count] [0x7f]
```
- Example: rarity `01 0b 7f` = 11 entries?
- 0x7f (127) appears to be a special marker
- Different control section pattern (not `01 00`)

**5. Legacy Format** - abjl:
```
[varies significantly]
```
- Example: manufacturer uses completely different structure
- No standard entry section pattern
- Requires format-specific parsing

### String Table Boundaries

The string table is bounded by:
- **Start**: First printable string after entry section
- **End**: Control section marker (`01 00 XX YY`)

Important: The control section marks the TRUE end of the string table. Category names
(none, base, basegame) come AFTER the control section and should be tracked separately.

### Control Section Detection

Pattern: `01 00 XX YY` where:
- `0x01` = marker byte
- `0x00` = separator
- `XX` = entry/index count
- `YY` = type/mode byte (0xe9, 0x62, 0x06, etc.)

Detection heuristic: Look for `01 00` followed by valid count byte, then verify
the next bytes contain category names like "none" or "base".

### Category Names vs Field Abbreviations

After the control section:

**Category Names** (DLC identifiers):
- Simple lowercase strings: `none`, `base`, `basegame`
- No underscores, periods, or special characters
- Part of the combined string table for binary section indexing

**Field Abbreviations**:
- Contain `.` or `!` characters: `corid_aid.a!`
- Encode field names compactly (e.g., `cor` = correlation, `id` = id, `aid` = achievementid)
- The trailing `!` is stripped when adding to string table
- Appear between category names and binary data markers
- ARE part of the combined string table (required for correct table_id indexing)
- May be terminated by control byte rather than null

### Binary Section Structure

The section divider `7a 00 00 00 00 00` marks a transition point:
- Before: Entry data markers (`XX XX 00 00` patterns)
- After: Bit-packed binary data

The binary section after the divider has two main parts:

#### Part 1: Bit-Packed String Indices

The first ~32 bytes contain bit-packed indices into the combined string table.
The bit width is determined by the size of the combined string table:
- 14 strings → 4 bits per index
- 6 strings → 3 bits per index
- 18 strings → 5 bits per index

Example from `achievement.bin` (4-bit indices):
```
1d 15 d7 55 e3 fb 2d fb...
```
Decoded: 13, 1, 5, 1, 7, 13, 5, 5, 3, 14, 11, 15...
- Index 13 → "achievement" (table_id)
- Index 1 → "10"
- Index 5 → "1224_missions_side"
- etc.

#### Part 2: Structured Metadata Section

Following the bit-packed indices is a byte-based metadata section with:
- Values mostly in 0x08-0x30 range
- 0x28 (or 0x20) separators between entry groups
- 0x00 0x00 terminator

**Entry Groups:**
Each group of values between separators corresponds to one entry in the string table.
The number of entry groups matches the number of entries.

Example from `achievement.bin` (5 entries):
```
15 11 13 14 14 14 12 25 08 08 | 28 | 13 0a 19 | 28 | 13 | 28 | 13 | 28 | 12 | 28 28 | 00 00
Entry 0: [21, 17, 19, 20, 20, 20, 18, 37, 8, 8]
Entry 1: [19, 10, 25]
Entry 2: [19]
Entry 3: [19]
Entry 4: [18]
```

The values may represent:
- Bit offsets into the packed section
- Field widths or character counts
- Position/length metadata for string reconstruction

#### Combined String Table Structure

For `abjx` format files, the combined string table contains:
1. **Primary strings** - from the string table section (indices 0 to N-1)
2. **Category names** - DLC identifiers like "none", "base", "basegame" (indices N to N+K-1)
3. **Field abbreviation** - if present, like "corid_aid.a" (next index)
4. **Type name** - the schema/type identifier like "achievement" (final index)

The `table_id` (first value in binary section) indexes into this combined table:
- `hit_region`: table_id=0 → "HitR_AI_Crit" (first primary string)
- `achievement`: table_id=13 → "achievement" (type name)

The table_id appears to point to either the first data entry name or the schema type name,
depending on the file's structure.

#### Compact Binary Format

Some NCS files (e.g., `rarity`) use a compact format instead of the separator-based format.
This format is detected by the absence of 0x28 separators and the presence of a `0x80 0x80` header.

**Structure:**
```
[Bit-packed indices] [0x80 0x80] [fixed-width records] [00 00] [tail data]
```

**Fixed-Width Records:**
Each entry has a fixed number of bytes (typically 2) without separators:
```
Example from rarity.bin (10 entries, 2 bytes each):
0x80 0x80 | 13 0f | 08 11 | 0d 0d | 23 08 | 08 24 | 27 11 | 0e 22 | 13 0d | 1c 1b | 26 27 | 00 00
           Entry0  Entry1  Entry2  Entry3  Entry4  Entry5  Entry6  Entry7  Entry8  Entry9  Term
```

The `0x80 0x80` header distinguishes this from the separator format, where values are in the 0x08-0x40 range.

#### Tag-Based Format (Advanced)

Some NCS files use a tag-based format with type bytes:
- 0x61 ('a') = TagType::Pair (string reference)
- 0x62 ('b') = TagType::U32 (32-bit unsigned)
- 0x63 ('c') = TagType::U32F32 (u32 and f32 interpretation)
- 0x64-0x66 = TagType::List (list terminated by "none")
- 0x70 ('p') = TagType::Variant (2-bit subtype)
- 0x7a ('z') = TagType::End

This format uses remap tables and variable-length encoding for complex data structures.

### String Validation Heuristics

Valid strings should:
- Be at least 2 characters long
- Not contain garbage characters (`!`, `@`, `#`, etc.) unless field abbreviations
- Pure numeric strings (like "10", "24") ARE valid
- Short strings (2-3 chars) should be all lowercase or known keywords

Invalid patterns:
- Mixed case short strings (like "zR", "D3") - likely binary garbage
- Trailing/leading spaces
- Multiple consecutive spaces
- Strings with high underscore-to-letter ratio

---

## Tools

### bl4-ncs Library

Native Rust NCS parsing:

```rust
use bl4_ncs::{decompress_ncs, scan_for_ncs};

// Scan pak file for NCS chunks
let chunks = scan_for_ncs(&pak_data);

// Decompress a chunk
let decompressed = decompress_ncs(&chunk_data)?;
```

---

## Inventory Part Definitions (inv.bin)

The `inv.bin` file is the **authoritative source** for valid weapon and gear parts. Unlike BL3's explicit PartSet/PartPool assets, BL4 defines part validity entirely through NCS.

**IMPORTANT:** INV files use a **tag-based binary format** different from other NCS files. The file contains:

```
[Header (magic + metadata)]
[Type name: "inv"]
[Dependencies (39 null-terminated strings)]
[Format code: "abcefhijl"]
[String table (18,393 null-terminated strings)]
[Binary data section (976,718 bytes, tag-based encoding)]
```

The format code "abcefhijl" describes the tag system used in the binary section. Each letter represents a different tag type used to encode property structures.

### Binary Section: Tag-Based Encoding

The binary section uses tag bytes to mark different property types. Each tag (a-l) appears at specific offsets before the actual data values.

**Tag Bytes:**
- 'a' = 0x61
- 'b' = 0x62
- 'c' = 0x63
- 'e' = 0x65
- 'f' = 0x66
- 'h' = 0x68
- 'i' = 0x69
- 'j' = 0x6a
- 'l' = 0x6c

**Serial Index Extraction:**

Serial indices are encoded at various offsets after tag markers:
- Tag 'f' + 27 bytes: Extracts ~3,471 indices (primary source)
- Tag 'a' + 5 bytes: Extracts ~2,921 indices (secondary source)
- Combined extraction: ~5,972 total serial index occurrences

Values are stored as:
- u8 (single byte) for indices < 256
- u16 little-endian for indices >= 256

**Current Parser Status:**

The `bl4 ncs extract` command with `--extract-type serial-indices` extracts ~5,972 serial index occurrences (target: 5,513). The 8.3% over-extraction suggests some false positives in the heuristic approach, but provides near-complete coverage of serial indices.

### File Structure

```
Offset  Content
------  -------
0x00    Header (8 bytes)
0x08    Type name ("inv")
0x0d    Dependencies (39 null-terminated strings):
        - inv_comp
        - primary_augment
        - secondary_augment
        - core_augment
        - barrel
        - barrel_acc
        - body
        - body_acc
        - foregrip
        - grip
        ... (total 39 deps)
0x1fb   Format code ("abcefhijl")
0x225   String table (18,393 null-terminated strings)
0x169e7c END OF FILE
```

### Header: Part Slot Types

The header defines valid part slot categories:

```
inv_comp
primary_augment
secondary_augment
core_augment
barrel
barrel_acc
body
body_acc
foregrip
grip
magazine
magazine_ted_thrown
magazine_acc
scope
scope_acc
secondary_ammo
hyperion_secondary_acc
payload_augment
payload
class_mod_body
passive_points
action_skill_mod
body_bolt
body_mag
element
firmware
stat_augment
body_ele
unique
turret_weapon
tediore_acc
tediore_secondary_acc
endgame
enemy_augment
active_augment
underbarrel
underbarrel_acc_vis
underbarrel_acc
barrel_licensed
```

### Weapon Type Definitions

Weapon types are defined by `{MANUFACTURER}_{WEAPONTYPE}` entries:

| Pattern | Example | Description |
|---------|---------|-------------|
| `DAD_PS` | Daedalus Pistol | 56 valid parts |
| `JAK_SG` | Jakobs Shotgun | Shotgun parts |
| `VLA_AR` | Vladof AR | Assault rifle parts |
| `TOR_HW` | Torgue Heavy Weapon | Heavy parts |

**Manufacturers:** BOR, DAD, JAK, MAL, ORD, TED, TOR, VLA

**Weapon Types:** PS (Pistol), SG (Shotgun), AR (Assault Rifle), SM (SMG), SR (Sniper), HW (Heavy)

### Weapon Type → Part Hierarchy

Parts are listed sequentially after their weapon type definition until the next weapon type:

```
DAD_PS                          <- Weapon type entry (line 4188)
  NexusSerialized, ..., Daedalus Pistol
  Weapon_PS
  /Game/Gear/Weapons/Pistols/DAD/Body_DAD_PS.Body_DAD_PS
  ...
  DAD_PS_Barrel_01              <- Valid parts start
  DAD_PS_Barrel_01_A
  DAD_PS_Barrel_01_B
  DAD_PS_Barrel_01_C
  DAD_PS_Barrel_01_D
  DAD_PS_Body
  DAD_PS_Body_A
  DAD_PS_Body_B
  ...
  DAD_PS_Underbarrel_06         <- Last part (line 4516)
DAD_SG                          <- Next weapon type (line 4517)
```

**Part Naming Convention:**
```
{MANUFACTURER}_{WEAPONTYPE}_{SLOT}_{VARIANT}
```

Examples:
- `DAD_PS_Barrel_01` - Daedalus Pistol, Barrel slot, base variant
- `DAD_PS_Barrel_01_A` - Daedalus Pistol, Barrel slot, A variant
- `JAK_SG_Grip_03` - Jakobs Shotgun, Grip slot, variant 3

### Part Count by Weapon Type

Extracted from NCS:

| Weapon Type | Part Count |
|-------------|------------|
| DAD_PS | 56 |
| JAK_PS | ~50 |
| TED_PS | ~45 |
| ... | ... |

### Legendary Compositions

Legendary weapons are defined by `comp_05_legendary_*` entries with mandatory parts:

```
comp_05_legendary_Zipgun        <- Legendary composition
  uni_zipper                    <- Unique naming part (red text)
  part_barrel_01_Zipgun         <- Mandatory unique barrel

comp_05_legendary_DiscJockey
  uni_discjockey
  part_barrel_02_DiscJockey

comp_05_legendary_OM            <- Oscar Mike
  part_barrel_unique_OM

comp_05_legendary_GoreMaster
  part_barrel_02_GoreMaster
```

**Structure:**
1. `comp_05_legendary_{name}` - Composition identifier
2. `uni_{name}` - Unique naming part for display name/red text
3. `part_barrel_*_{name}` or `part_unique_*` - Mandatory unique part(s)

### Extracting Valid Parts

To get all valid parts for a weapon type:

```bash
# Find weapon type line numbers
bl4 ncs show inv4.bin --all-strings | grep -n "^  - DAD_PS$"

# Extract parts between weapon types (lines 4188-4516)
bl4 ncs show inv4.bin --all-strings | sed -n '4188,4516p' | grep "DAD_PS_"
```

### Part Validation Logic

```rust
/// Check if a part is valid for a weapon type
fn is_valid_part(weapon_type: &str, part_name: &str, inv_data: &NcsDocument) -> bool {
    // Find weapon type entry index
    let type_idx = inv_data.find_entry(weapon_type)?;

    // Find next weapon type entry
    let next_type_idx = inv_data.find_next_weapon_type(type_idx)?;

    // Part is valid if it appears between type and next type
    inv_data.entries[type_idx..next_type_idx]
        .iter()
        .any(|e| e.name == part_name)
}
```

### Serial Index Structure

Each part in inv.bin has a `serialindex` field that provides a unique identifier for serialization.

**Structure:**
```
serialindex: {
  status: "Active" | "Inactive"
  index: u32              // The actual serial index number
  _category: "inv_type"   // Always "inv_type" for inventory parts
  _scope: "Root" | "Sub"  // "Root" for item types, "Sub" for parts
}
```

**Item Types (Root scope):**
- Each weapon type (e.g., `DAD_PS`, `BOR_SG`) has a Root serialindex
- Used to identify the base item type in serialized data

**Parts (Sub scope):**
- Each part within an item type has a Sub serialindex
- Indices are unique within each item type but may repeat across types
- The slot type (barrel, grip, etc.) determines which part pool the index references
```

### Memory vs NCS Part Names

Part names differ between memory extraction and NCS:

| NCS Name | Memory Name |
|----------|-------------|
| `DAD_PS_Barrel_01` | `DAD_PS.part_barrel_01` |
| `DAD_PS_Body_A` | `DAD_PS.part_body_a` |
| `DAD_PS_Grip_04` | `DAD_PS.part_grip_04_hyp` |

The NCS has more parts (56 for DAD_PS) than memory extraction captured (34).
**Always use NCS as the authoritative source.**

### Extracting Item Parts

Use the CLI to extract complete item-to-parts mappings:

```bash
# Extract all item parts (weapons + shields) to JSON
bl4 ncs extract inv4.bin -t item-parts --json -o item_parts.json

# View weapon parts for a specific directory
bl4 ncs extract /path/to/ncsdata/pakchunk4-Windows_0_P -t item-parts
```

Output structure:
```json
[
  {
    "item_id": "DAD_PS",
    "parts": ["DAD_PS_Barrel_01", "DAD_PS_Barrel_01_A", ...],
    "legendary_compositions": [...]
  },
  {
    "item_id": "Armor_Shield",
    "parts": ["part_core_atl_protractor", "part_ra_armor_segment_primary", ...],
    "legendary_compositions": []
  }
]
```

---

## Actor Definitions (gbxactor.bin)

The `gbxactor.bin` file contains definitions for game actors including characters, AI, abilities, and entities.

### File Structure

```
Type: gbxactor
Format: abef
Entry count: ~1740 entries
```

### Entry Categories

| Category | Pattern | Description |
|----------|---------|-------------|
| `Actor_*` | `Actor_PLD_AS_Scourge_*` | Player abilities, projectiles |
| `Char_AI` | `Char_AI` | Base AI character definition |
| `Char_Enemy` | `Char_Enemy` | Base enemy character |
| `Char_NPC` | `Char_NPC` | Base NPC character |
| `Char_{Type}` | `Char_Paladin`, `Char_ExoSoldier` | Player character types |
| `Char_{Enemy}` | `Char_ArmyBandit_*`, `Char_Psycho*` | Enemy types |
| `Char_Gadget_*` | `Char_Gadget_AutoTurret_Base` | Gadget/turret actors |
| `Team_*` | `Team_Player`, `Team_Bandit` | Team definitions |
| `MPart_*` | `MPartRand_Skin_Human` | Mesh part randomizers |

### Character Inheritance

Characters inherit from base types in a hierarchy:

```
Char_AI
├── Char_Enemy
│   ├── Char_ArmyBandit_SHARED
│   ├── Char_PsychoBasic
│   └── ...
├── Char_NPC
└── Char_Gadget_AutoTurret_Base
```

### Properties

gbxactor entries define behavior properties:

```
PatrolPauseTime: NumericRange
bUsePatrolFidgets: Bool
FidgetCooldown: Float
bCanEngagePlayers: Bool
Element: NoElement | Corrosive | Cryo | Fire | Shock | Radiation
SpawnPattern_Enemies_Default
```

### Actor vs Inventory

| File | Contains |
|------|----------|
| `gbxactor.bin` | Characters, AI, abilities, teams, spawn patterns |
| `inv.bin` | Weapons, shields, gear, parts, compositions |

gbxactor does NOT contain weapon parts - those are exclusively in inv.bin.

---

## Future Work

Areas requiring further analysis:

1. **Hash Table Decoding** - The u32 values after the ASCII prefix likely form a hash lookup table
2. **Entry-to-Category Mapping** - How the binary section maps entries to their categories
3. **Cross-file References** - How NCS files reference each other (e.g., ItemPool → ItemPoolList)
4. **Runtime Behavior** - How the game loads and uses NCS data
5. **Tail Section Semantics** - Full meaning of the 0x28-separated values in the tail section

---

*This documentation is based on reverse engineering analysis of Borderlands 4 game files.*
