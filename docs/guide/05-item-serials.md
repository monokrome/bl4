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
"gr$ZCm/&tH!..."       Strip "@U" prefix, keep type
         ↓
[0x21, 0x30, 0xC0...]  Base85 decode to bytes
         ↓
[0x84, 0x0C, 0x03...]  Bit-mirror each byte
         ↓
100 0010 0011...       Parse as bitstream (tokens)
         ↓
{Manufacturer: 4, Level: 50, Parts: [...]}
```

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
Input:  "$ZCm/"
Values: [67, 35, 12, 48, 84]  (positions in alphabet)

Calculation:
67 × 85⁴ + 35 × 85³ + 12 × 85² + 48 × 85¹ + 84 × 85⁰
= 67 × 52,200,625 + 35 × 614,125 + 12 × 7,225 + 48 × 85 + 84
= 3,497,441,875 + 21,494,375 + 86,700 + 4,080 + 84
= 3,519,027,114

As bytes (big-endian): [0xD1, 0xB8, 0x1C, 0x2A]
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

## Part Index Mapping

Part indices in serials correspond to weapon naming/stats:

| Index | Type | Example Prefixes |
|-------|------|------------------|
| 2 | Damage | "Tortuous", "Agonizing" |
| 3 | CritDamage | "Bleeding", "Hemorrhaging" |
| 4 | ReloadSpeed | "Frenetic", "Manic" |
| 5 | MagSize | "Bloated", "Gluttonous" |
| 7-10 | body_mod_a-d | "Chosen", "Cursed", etc. |
| 15-18 | barrel_mod_a-d | "Herald", "Harbinger" |

The actual part pool depends on:
- Weapon type (SMG vs Shotgun)
- Manufacturer
- Rarity

!!! warning
    Part indices are NOT global. `{4}` on a Maliwan SMG means something different than `{4}` on a Jakobs Pistol.

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
@U   → Strip (header)
g    → Strip (part of header)
r    → Item type: weapon (variant r)
$ZCm/& → Base85 data
```

### Step 2: Base85 Decode `$ZCm/&`

```
Characters: $ Z C m / &
Positions:  67 35 12 48 84 69

Group 1: $ZCm/ (5 chars)
Value = 67×85⁴ + 35×85³ + 12×85² + 48×85 + 84
      = 3,519,027,114
Bytes = [0xD1, 0xB8, 0x1C, 0x2A]

Group 2: & (1 char, padded)
Value = 69 × 85⁰ = 69
Bytes = [0x00, 0x00, 0x00, 0x45]
```

### Step 3: Mirror Bytes

```
Original: D1 B8 1C 2A 00 00 00 45
Mirrored: 8B 1D 38 54 00 00 00 A2
```

### Step 4: Parse Bitstream

```
Binary: 10001011 00011101 00111000 01010100 ...

Bit 0-6:   0010000  → Magic (valid!)
Bit 7-9:   100      → VarInt prefix
Bit 10-13: 0101     → Value 5
Bit 14:    1        → Continue
Bit 15-18: 1000     → Value 8
Bit 19:    0        → Stop

VarInt = (8 << 4) | 5 = 133
...
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

## Key Takeaways

1. **Serials are self-contained** — All item data in one string
2. **Custom Base85** — Not standard ASCII85
3. **Bit mirroring** — Extra obfuscation layer
4. **Token-based** — VarInt, Part, String, Separators
5. **Context-dependent** — Part indices vary by item type

---

## Next Chapter

Now that you understand item encoding, let's extract data from the game's pak files.

**Next: [Chapter 6: Data Extraction](06-data-extraction.md)**
