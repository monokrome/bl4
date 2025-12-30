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
allowing concatenation without an explicit separator

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
| `itempool` | Item pool definitions | many |
| `itempoollist` | Item pool lists | many |
| `loot_config` | Loot configuration | many |
| `Mission` | Mission data | many |
| `preferredparts` | Part preferences | 1 |
| `trait_pool` | Trait pool definitions | many |
| `vending_machine` | Vending inventory | many |

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

## Future Work

Areas requiring further analysis:

1. **Hash Table Decoding** - The u32 values after the ASCII prefix likely form a hash lookup table
2. **Entry-to-Category Mapping** - How the binary section maps entries to their categories
3. **Cross-file References** - How NCS files reference each other (e.g., ItemPool → ItemPoolList)
4. **Runtime Behavior** - How the game loads and uses NCS data
5. **Tail Section Semantics** - Full meaning of the 0x28-separated values in the tail section

---

*This documentation is based on reverse engineering analysis of Borderlands 4 game files.*
