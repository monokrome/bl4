# Chapter 5: Item Serials

Every item in BL4—weapons, shields, class mods—is encoded as a compact string called a "serial." This chapter breaks down how serials work and how to decode them.

---

## What Is an Item Serial?

A serial is a shareable string that completely describes an item:

```
@Ugr$ZCm/&tH!t{KgK/Shxu>k
```

This encodes:
- Item type (weapon, shield, etc.)
- Manufacturer
- Level
- All parts (barrel, grip, scope, etc.)
- Rarity
- Random seed for stat rolls

!!! note
    **Serials are portable.** You can share them with friends, and they'll get an identical item. This is how "gun codes" work in Borderlands communities.

---

## Serial Structure

```
@Ug<type><base85_data>
│  │ │    └── Encoded item data
│  │ └── Item type character
│  └── Magic prefix
└── Start marker
```

| Component | Example | Description |
|-----------|---------|-------------|
| Prefix | `@Ug` | Constant identifier |
| Type | `r` | Item category (weapon, equipment, etc.) |
| Data | `$ZCm/&tH!...` | Base85-encoded bitstream |

---

## Item Type Characters

| Char | Category | Description |
|------|----------|-------------|
| `a`-`d` | Weapons | Pistols, some shotguns |
| `e` | Equipment | Shields, enhancements |
| `f`-`g` | Weapons | Shotguns, some ARs |
| `r` | Weapons | Mixed (common type) |
| `u` | Utilities | Grenades, consumables |
| `v`-`z` | Weapons | ARs, SMGs, snipers |
| `!` | ClassMod | Dark Siren class mods |
| `#` | ClassMod | Paladin class mods |

!!! tip
    The type character affects decoding. Different types have slightly different bitstream layouts.

---

## The Decoding Pipeline

Serials go through multiple transformations:

```
"@Ugr$ZCm/&tH!..."     Original serial string
         ↓
"gr$ZCm/&tH!..."       Strip "@U" prefix (keep 'g' and everything after)
         ↓
[0x84, 0xA5, 0x86...]  Base85 decode to bytes
         ↓
[0x21, 0xA5, 0x61...]  Bit-mirror each byte
         ↓
001 0000 1101...       Parse as bitstream (tokens)
         ↓
{Category: 22, Level: 50, Parts: [...]}
```

Note: The item type character (e.g., `r`) is extracted from position 3 of the original string but is NOT removed from the Base85 data.

---

## Step 1: Base85 Decoding

### The Custom Alphabet

BL4 uses a custom 85-character alphabet (NOT standard ASCII85):

```
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~
```

Character positions:
| Char | Value | | Char | Value | | Char | Value |
|------|-------|-|------|-------|-|------|-------|
| `0` | 0 | | `A` | 10 | | `a` | 36 |
| `1` | 1 | | `B` | 11 | | `b` | 37 |
| ... | | | ... | | | ... | |
| `9` | 9 | | `Z` | 35 | | `z` | 61 |

Special characters: `! # $ % & ( ) * + - ; < = > ? @ ^ _ \` { / } ~`

### Decoding Process

Every 5 Base85 characters → 4 bytes (big-endian):

```rust
fn base85_decode(input: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTU\
        VWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

    let mut result = Vec::new();

    for chunk in input.as_bytes().chunks(5) {
        // Convert 5 chars to value (base 85)
        let mut value: u64 = 0;
        for &ch in chunk {
            let pos = ALPHABET.iter().position(|&c| c == ch).unwrap();
            value = value * 85 + pos as u64;
        }

        // Extract 4 bytes (big-endian)
        result.push((value >> 24) as u8);
        result.push((value >> 16) as u8);
        result.push((value >> 8) as u8);
        result.push(value as u8);
    }

    result
}
```

**Example**:
```
Input:  "gr$ZC" (first 5 chars of "gr$ZCm/&")
Values: [42, 53, 64, 35, 12]  (positions in alphabet)

Calculation:
42 × 85⁴ + 53 × 85³ + 64 × 85² + 35 × 85¹ + 12 × 85⁰
= 42 × 52,200,625 + 53 × 614,125 + 64 × 7,225 + 35 × 85 + 12
= 2,192,426,250 + 32,548,625 + 462,400 + 2,975 + 12
= 2,225,440,262

As bytes (big-endian): [0x84, 0xA5, 0x86, 0x06]
```

---

## Step 2: Bit Mirroring

After Base85 decoding, each byte is bit-reversed:

```
Original:    10000111  (0x87)
Mirrored:    11100001  (0xE1)
```

```rust
fn mirror_byte(b: u8) -> u8 {
    let mut result = 0;
    for i in 0..8 {
        if (b >> i) & 1 == 1 {
            result |= 1 << (7 - i);
        }
    }
    result
}

// Or use lookup table for speed
const MIRROR_TABLE: [u8; 256] = [
    0x00, 0x80, 0x40, 0xC0, 0x20, 0xA0, 0x60, 0xE0,
    // ... 256 entries
];
```

!!! note
    **Why mirror?** This is likely an obfuscation technique or byte-order conversion from how the game internally stores data.

---

## Step 3: Bitstream Parsing

The mirrored bytes form a bitstream read MSB-first.

### Magic Header

First 7 bits must be `0010000` (0x10):

```
Bits: 0 0 1 0 0 0 0 ...
      └──────────┘
         Magic (0x10)
```

### Token Types

Tokens are identified by their prefix bits:

| Prefix | Token | Description |
|--------|-------|-------------|
| `00` | Separator | Hard boundary (renders as `\|`) |
| `01` | SoftSeparator | Soft boundary (renders as `,`) |
| `100` | VarInt | Variable-length integer |
| `101` | Part | Part reference with optional value |
| `110` | VarBit | Bit-length-prefixed integer |
| `111` | String | Length-prefixed ASCII string |

### Token Formats

#### VarInt (prefix `100`)

Nibble-based variable integer:

```
100 [4-bit value][1-bit cont][4-bit value][1-bit cont]...

Example: 100 1010 1 0011 0
         │   │    │ │    └── cont=0, stop
         │   │    │ └── value=3
         │   │    └── cont=1, continue
         │   └── value=10 (0xA)
         └── VarInt marker

Result: (3 << 4) | 10 = 58
```

Values are assembled LSB-first (shift left 4 bits per nibble).

#### VarBit (prefix `110`)

Length-prefixed integer:

```
110 [5-bit length] [N bits of data]

Example: 110 01010 1101001011
         │   │     └── 10 bits of data
         │   └── length=10
         └── VarBit marker

Result: 0b1101001011 = 843
```

#### Part (prefix `101`)

Part reference with optional values:

```
101 [VarInt index] [1-bit type flag]

If type=1:
  [VarInt value] [000 terminator]

If type=0:
  [2-bit subtype]
    Subtype 10: Empty part (no data)
    Subtype 01: [values...][00 terminator]
```

**Notation**:
- `{42}` — Part index 42, no value
- `{42:7}` — Part index 42, value 7
- `{42:[1 2 3]}` — Part index 42, multiple values

#### String (prefix `111`)

```
111 [VarInt length] [7-bit ASCII chars × length]

Example: 111 100 0101 0 1001000 1101001
         │   │         │       │
         │   │         │       └── 'i' (0x69)
         │   │         └── 'H' (0x48)
         │   └── length=5 (VarInt)
         └── String marker

Result: "Hi..."
```

---

## Decoded Token Stream

Here's what a typical weapon serial looks like decoded:

```
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k

Tokens:
180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41

Interpretation:
180928       ← First VarInt (item ID or manufacturer)
|            ← Separator
50           ← Level (displayed level + offset)
|            ← Separator
{0:1}        ← Part with value
21           ← VarInt
{4}          ← Part (likely ReloadSpeed based on index)
,            ← Soft separator
2 , , 105 102 41  ← Additional data
```

---

## Manufacturer IDs

The first VarInt after the magic header identifies the manufacturer:

| ID | Manufacturer | Code |
|----|--------------|------|
| 4 | Daedalus | DAD |
| 6 | Torgue | TOR |
| 10 | Tediore | TED |
| 15 | Order | ORD |
| 129 | Jakobs | JAK |
| 134 | Vladof | VLA |
| 138 | Maliwan | MAL |

!!! note
    IDs for BOR and COV are still being researched. The ID appears to be a hash or index into the manufacturer table.

    *For a complete list of manufacturers and their weapon types, see [Appendix B: Weapon Parts Reference](appendix-b-weapon-parts.md).*

---

## Serial Layout by Type

Different item types have different layouts:

### Weapons (types a-d, f-g, v-z)

```
<manufacturer_id> , 0 , 8 , <subtype> | 4 , <seed> | | {parts...}
```

| Field | Position | Description |
|-------|----------|-------------|
| Manufacturer | Token 1 | See ID table |
| Constants | Tokens 2-4 | Usually `0, 8, <subtype>` |
| Separator | Token 5 | `\|` |
| Seed intro | Token 6 | Usually `4` |
| Seed | Token 7 | Random seed for rolls |
| Parts start | After `\|\|` | List of part indices |

### Equipment (type e)

```
<type_id> | <level> | "" <manufacturer> , <seed> | | {parts...}
```

Equipment includes a level field and empty string marker.

### Class Mods (types !, #)

```
<class_id> , 0 , 8 , <subtype> | 4 , <seed> | | {stat_parts...} {skill_parts...}
```

Class mods have both stat modifier parts (0-250) and skill parts (2000+).

---

## Using bl4 to Decode

### Basic Decode

```bash
$ bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Weapon)
Manufacturer: Unknown (180928)
Decoded bytes: 18
Hex: 21 30 C0 42 0C 48 08 32 0C 4E 08 86 74 72 4B 5C 00 8E
Tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

### Verbose Output

```bash
$ bl4 decode --verbose '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Shows raw bytes, bit positions, each token parsed
```

### Debug Mode

```bash
$ bl4 decode --debug '@Ugr$ZCm/&tH!t{KgK/Shxu>k' 2>&1

# Shows bit-by-bit parsing decisions
```

---

## Part Group ID Encoding

The first token in a serial encodes the **Part Group ID**, which determines which part pool to use for decoding Part tokens. Different item categories use different multipliers:

### Weapons (types r, a-d, f-g, v-z)

```
first_token = group_id * 8192 + offset
group_id = first_token / 8192
```

**Example:** Serial with first token `180928`:
- `180928 / 8192 = 22` (remainder 704)
- Group ID 22 = **Vladof SMG**

### Equipment (type e)

```
first_token = group_id * 384 + offset
group_id = first_token / 384
```

**Example:** Serial with first token `107200`:
- `107200 / 384 = 279` (remainder 64)
- Group ID 279 = **Shield (Energy type)**

### Complete Part Group ID Reference

These mappings were extracted from memory analysis and verified against decoded serials.

**Pistols (2-7):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 2 | Daedalus | DAD_PS | 74 |
| 3 | Jakobs | JAK_PS | 73 |
| 4 | Tediore | TED_PS | 81 |
| 5 | Torgue | TOR_PS | 70 |
| 6 | Order | ORD_PS | 75 |
| 7 | Vladof | VLA_PS | - |

**Shotguns (8-12):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 8 | Daedalus | DAD_SG | 74 |
| 9 | Jakobs | JAK_SG | 89 |
| 10 | Tediore | TED_SG | 76 |
| 11 | Torgue | TOR_SG | 69 |
| 12 | Bor | BOR_SG | 73 |

**Assault Rifles (13-18):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 13 | Daedalus | DAD_AR | 78 |
| 14 | Jakobs | JAK_AR | 74 |
| 15 | Tediore | TED_AR | 79 |
| 16 | Torgue | TOR_AR | 73 |
| 17 | Vladof | VLA_AR | 89 |
| 18 | Order | ORD_AR | 73 |

**SMGs (20-23):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 20 | Daedalus | DAD_SM | 77 |
| 21 | Bor | BOR_SM | 73 |
| 22 | Vladof | VLA_SM | 84 |
| 23 | Maliwan | MAL_SM | 74 |

**Snipers (26-29):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 26 | Jakobs | JAK_SR | 72 |
| 27 | Vladof | VLA_SR | 82 |
| 28 | Order | ORD_SR | 75 |
| 29 | Maliwan | MAL_SR | 76 |

**Heavy Weapons (244-247):**

| ID | Manufacturer | Code | Parts Count |
|----|--------------|------|-------------|
| 244 | Vladof | VLA_HW | 22 |
| 245 | Torgue | TOR_HW | 32 |
| 246 | Bor | BOR_HW | 25 |
| 247 | Maliwan | MAL_HW | 19 |

**Shields (279-288):**

| ID | Type | Code | Parts Count |
|----|------|------|-------------|
| 279 | Energy Shield | energy_shield | 22 |
| 280 | Bor Shield | bor_shield | 4 |
| 281 | Daedalus Shield | dad_shield | 3 |
| 282 | Jakobs Shield | jak_shield | 3 |
| 283 | Armor Shield | Armor_Shield | 26 |
| 284 | Maliwan Shield | mal_shield | 9 |
| 285 | Order Shield | ord_shield | 3 |
| 286 | Tediore Shield | ted_shield | 3 |
| 287 | Torgue Shield | tor_shield | 3 |
| 288 | Vladof Shield | vla_shield | 3 |

**Gadgets and Gear (300-330):**

| ID | Type | Parts Count |
|----|------|-------------|
| 300 | Grenade Gadget | 82 |
| 310 | Turret Gadget | 52 |
| 320 | Repair Kit | 107 |
| 330 | Terminal Gadget | 61 |

**Enhancements (400-409):**

| ID | Manufacturer | Parts Count |
|----|--------------|-------------|
| 400 | Daedalus | 1 |
| 401 | Bor | 1 |
| 402 | Jakobs | 4 |
| 403 | Maliwan | 4 |
| 404 | Order | 4 |
| 405 | Tediore | 4 |
| 406 | Torgue | 4 |
| 407 | Vladof | 4 |
| 408 | COV | 1 |
| 409 | Atlas | 1 |

!!! tip "Parts Database"
    The full parts database with 2615 parts across 53 categories is available at `share/manifest/parts_database.json`. Use `bl4 memory build-parts-db` to regenerate it from a memory dump.

---

## Part Index Mapping

Part indices in serials are **relative to the Part Group**. The same index means different things in different groups.

| Index | Type | Example Prefixes |
|-------|------|------------------|
| 2 | Damage | "Tortuous", "Agonizing" |
| 3 | CritDamage | "Bleeding", "Hemorrhaging" |
| 4 | ReloadSpeed | "Frenetic", "Manic" |
| 5 | MagSize | "Bloated", "Gluttonous" |
| 7-10 | body_mod_a-d | "Chosen", "Cursed", etc. |
| 15-18 | barrel_mod_a-d | "Herald", "Harbinger" |

The actual part pool depends on:
- Part Group ID (encodes manufacturer + weapon/item type)
- Rarity tier

!!! warning
    Part indices are NOT global. `{4}` on a Maliwan SMG means something different than `{4}` on a Jakobs Pistol. Always decode the Part Group ID first to know which part pool to use.

---

## Comparing Serials

To understand what a field means, compare similar items:

### Example: Two Linebackers

```
Serial 1: @Ugd$YMq/.&{!gQaYQ1)<G9C8D6LFPL0ux
Serial 2: @Ugd$YMq/.&{!gQaYQ1)<?B8b6LFPL0ux

Difference at byte 15-16:
  Serial 1: ... 3C G9 C8 ...  (< G 9 C 8)
  Serial 2: ... 3C ?B 8b ...  (< ? B 8 b)
```

These weapons had different accuracy stats. By isolating the byte difference, we can deduce that bytes 15-16 encode accuracy.

---

## Practical: Decoding Step by Step

Let's manually decode a short serial:

### Input: `@Ugr$ZCm/&`

### Step 1: Parse Structure

```
@U       → Strip (header prefix)
gr$ZCm/& → Base85 data (includes 'g' which encodes more data)
r        → Item type extracted separately (char at index 3)
```

Note: The `g` after `@U` is NOT stripped - it's part of the Base85 data. Only `@U` is removed.

### Step 2: Base85 Decode `gr$ZCm/&`

```
Characters: g r $ Z C   (first chunk of 5)
Positions:  42 53 64 35 12

Group 1: gr$ZC (5 chars)
Value = 42×85⁴ + 53×85³ + 64×85² + 35×85 + 12
      = 2,225,440,262
Bytes = [0x84, 0xA5, 0x86, 0x06]

Group 2: m/& (3 chars → 2 output bytes)
Positions: 48, 82, 66
Value = 48×85² + 82×85 + 66 = 353,836
Bytes = [0x66, 0x2C]
```

### Step 3: Mirror Bytes

```
Original: 84 A5 86 06 66 2C
Mirrored: 21 A5 61 60 66 34
```

You can verify: `bl4 decode '@Ugr$ZCm/&'` outputs `Hex: 21a561606634`

### Step 4: Parse Bitstream

```
Binary: 00100001 10100101 01100001 01100000 01100110 00110100

Bit 0-6:   0010000  → Magic (0x10, valid!)
Bit 7-9:   110      → VarBit prefix
Bit 10-14: 10000    → Length = 16 bits
Bit 15-30: (16 bits of data) → Part Group ID encoded value

First VarBit decodes to 180928
Category = 180928 / 8192 = 22 (Vladof SMG)
```

---

## Exercises

### Exercise 1: Identify Item Type

Given these serials, identify the item category:

1. `@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_`
2. `@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_`
3. `@Ug!$ZCm/&tH!t{KgK/Shxu>k`

<details>
<summary>Answers</summary>

1. `e` → Equipment (shield/enhancement)
2. `w` → Weapon (SMG category)
3. `!` → ClassMod (Dark Siren)

</details>

### Exercise 2: Extract Manufacturer

Decode the first token of this weapon serial:

```
@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_
```

What manufacturer made this weapon?

<details>
<summary>Answer</summary>

Using bl4:
```bash
bl4 decode '@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_'
```

First VarInt: 138 → **Maliwan**

</details>

### Exercise 3: Compare Parts

Decode these two similar weapons and identify which tokens differ:

```
@Ugd$YMq/.&{!gQaYQ1)<G9C8
@Ugd$YMq/.&{!gQaYQ1)<?B8b
```

---

## UE5 Part Data Structures

Parts in BL4 are defined using these UE5 structures (discovered via usmap analysis):

### GbxSerialNumberIndex

The core structure linking parts to serial encoding:

```
Category  : Int64   ← Part Group ID
scope     : Byte    ← Root/Sub scope
status    : Byte    ← Active/Static/etc.
Index     : Int16   ← Part index within group
```

### InventoryPartDef

Defines individual item parts (barrels, grips, scopes, etc.):

```
SerialIndex           : GbxSerialNumberIndex  ← Encoding info
bCanBeRerolled        : Bool
Aspects               : Array<InventoryAspect> ← Stat modifiers
GestaltPartNames      : Array<FName>          ← Visual mesh parts
DebugDisplayDescription : String
params                : Struct                ← Custom parameters
```

### InventoryTypeDef

Defines weapon/equipment types with their part pools:

```
BaseType              : GbxDefPtr
Manufacturer          : GbxDefPtr
Rarity                : GbxDefPtr
PartTypes             : Array<FName>          ← Valid part types
PrefixPartList        : Array<PartListEntry>  ← Prefix name parts
TitlePartList         : Array<PartListEntry>  ← Title name parts
SuffixPartList        : Array<PartListEntry>  ← Suffix name parts
Aspects               : Array<InventoryAspect>
```

### EWeaponPartValue

Enum defining weapon part slot types:

| Value | Type |
|-------|------|
| 0 | Grip |
| 1 | Foregrip |
| 2 | Reload |
| 3 | Barrel |
| 4 | Scope |
| 5 | Melee |
| 6 | Mode |
| 7 | ModeSwitch |
| 8 | Underbarrel |
| 9-16 | Custom0-7 |

### Part Resolution Flow

```
Serial Token {42}
       ↓
Part Group ID (from first token)
       ↓
InventoryTypeDef.PartTypes[group_id]
       ↓
InventoryPartDef[index=42] in that pool
       ↓
SerialIndex.Category matches → Valid part
       ↓
Aspects → Applied stat modifiers
```

---

## Key Takeaways

1. **Serials are self-contained** — All item data in one string
2. **Custom Base85** — Not standard ASCII85
3. **Bit mirroring** — Extra obfuscation layer
4. **Token-based** — VarInt, Part, String, Separators
5. **Context-dependent** — Part indices vary by item type
6. **Part Group ID** — First token encodes which part pool to use

---

## Next Chapter

Now that you understand item encoding, let's extract data from the game's pak files.

**Next: [Chapter 6: Data Extraction](06-data-extraction.md)**
