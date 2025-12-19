# Chapter 5: Item Serials

The first time you see an item serial—something like `@Ugr$ZCm/&tH!t{KgK/Shxu>k`—it looks like line noise. Random characters that couldn't possibly mean anything. But that string contains a complete weapon: its manufacturer, every part attached to it, the level, the random seed that determined its stats. Everything needed to reconstruct the item perfectly.

This chapter decodes how serials work. By the end, you'll understand every transformation from that cryptic string to a fully-described weapon.

---

## What's Encoded in a Serial

A serial is self-contained. Given just the string, the game can create an identical item anywhere—your inventory, a friend's inventory, another platform entirely. This is how "gun codes" work in Borderlands communities. Copy a serial, share it, and the recipient gets the exact same weapon.

Inside that string:
- Item type (weapon, shield, class mod)
- Manufacturer
- Level
- Every part (barrel, grip, scope, magazine)
- Random seed for stat calculations
- Any special modifiers

The encoding is compact. A 40-character serial describes an item that would need hundreds of bytes in a more verbose format.

---

## The Decoding Pipeline

Serials transform through multiple stages. Understanding each stage reveals how the pieces fit together.

```
"@Ugr$ZCm/&tH!..."  →  Strip "@U" prefix
"gr$ZCm/&tH!..."    →  Base85 decode to bytes
[0x84, 0xA5, ...]   →  Bit-mirror each byte
[0x21, 0xA5, ...]   →  Parse as bitstream tokens
{Category: 22, Level: 50, Parts: [...]}
```

The prefix `@U` marks this as a BL4 serial. The third character indicates item type—`r` for a weapon, `e` for equipment, and so on. After stripping the prefix, everything else is Base85-encoded binary data.

---

## Base85: Custom Alphabet

BL4 doesn't use standard ASCII85. It uses a custom 85-character alphabet:

```
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~
```

Every 5 characters encode 4 bytes. The math: 85⁵ ≈ 4.4 billion, which fits in 32 bits (4 bytes) with room to spare.

To decode, look up each character's position in the alphabet, combine them as a base-85 number, then extract 4 bytes big-endian:

```
Characters: g r $ Z C
Positions:  42 53 64 35 12

Value = 42×85⁴ + 53×85³ + 64×85² + 35×85 + 12
      = 2,225,440,262

Bytes: [0x84, 0xA5, 0x86, 0x06]
```

---

## Bit Mirroring: The Obfuscation Layer

After Base85 decoding, each byte gets bit-reversed. 0x87 (binary `10000111`) becomes 0xE1 (binary `11100001`).

Why? Probably obfuscation. It makes casual inspection harder and might relate to how the game's internal serialization works. For our purposes, it's just another step to reverse:

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
```

---

## Token Parsing: The Real Structure

The mirrored bytes form a bitstream parsed MSB-first. The first 7 bits must be `0010000` (0x10)—a magic number validating this as a proper serial.

After the magic header, the stream contains tokens identified by prefix bits:

| Prefix | Token Type | Purpose |
|--------|------------|---------|
| `00` | Separator | Hard boundary between sections |
| `01` | SoftSeparator | Softer boundary (like commas) |
| `100` | VarInt | Variable-length integer |
| `101` | Part | Part reference with optional value |
| `110` | VarBit | Bit-length-prefixed integer |
| `111` | String | Length-prefixed ASCII string |

**VarInt** encodes integers in nibbles (4-bit chunks). Each nibble has 4 bits of value plus 1 continuation bit. Keep reading nibbles until the continuation bit is 0.

**VarBit** starts with a 5-bit length, then that many bits of data. More efficient for known-size values.

**Part** tokens reference parts by index, optionally with associated values. `{42}` means part index 42, `{42:7}` means part 42 with value 7.

---

## Two Serial Formats

BL4 uses two distinct formats, distinguished by the first token after the header:

**VarBit-first format** (type `r`, some equipment): The first token is a VarBit encoding the Part Group ID times a multiplier. Compact, used for most weapons.

**VarInt-first format** (types `a-d`, `f-g`, `u-z`): The first token is a VarInt encoding manufacturer plus weapon type. Extended format with more metadata.

For VarBit-first serials:
```
Part Group ID = first_varbit / 8192  (weapons)
Part Group ID = first_varbit / 384   (equipment)
```

For VarInt-first serials, the first VarInt directly encodes a combined manufacturer/weapon-type ID.

---

## Decoding a Serial Manually

Let's walk through `@Ugr$ZCm/&tH!t{KgK/Shxu>k`:

**Step 1: Structure**
- Prefix: `@U` (stripped)
- Type character at position 3: `r` (weapon)
- Base85 data: `gr$ZCm/&tH!...`

**Step 2: Base85 decode**
First 5 characters `gr$ZC`:
- Positions: 42, 53, 64, 35, 12
- Value: 2,225,440,262
- Bytes: [0x84, 0xA5, 0x86, 0x06]

Continue for remaining characters.

**Step 3: Bit-mirror each byte**
```
Original: 84 A5 86 06 ...
Mirrored: 21 A5 61 60 ...
```

**Step 4: Parse bitstream**
```
Binary: 00100001 10100101 01100001 ...
        └──────┘ └────────────────...
         Magic   Tokens begin
         (0x10)
```

First token after magic: prefix `110` = VarBit
- 5-bit length: 16
- 16 bits of data: 180928

Part Group ID = 180928 / 8192 = 22 (Vladof SMG)

The bl4 tool handles all this:
```bash
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
# Output shows tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

---

## Part Group IDs

The Part Group ID determines which part pool to use for decoding. Each ID corresponds to a manufacturer/weapon-type combination:

**Pistols (2-7):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 2 | Daedalus | DAD_PS |
| 3 | Jakobs | JAK_PS |
| 4 | Tediore | TED_PS |
| 5 | Torgue | TOR_PS |
| 6 | Order | ORD_PS |
| 7 | Vladof | VLA_PS |

**Shotguns (8-12):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 8 | Daedalus | DAD_SG |
| 9 | Jakobs | JAK_SG |
| 10 | Tediore | TED_SG |
| 11 | Torgue | TOR_SG |
| 12 | Bor | BOR_SG |

**Assault Rifles (13-18):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 13 | Daedalus | DAD_AR |
| 14 | Jakobs | JAK_AR |
| 15 | Tediore | TED_AR |
| 16 | Torgue | TOR_AR |
| 17 | Vladof | VLA_AR |
| 18 | Order | ORD_AR |

**SMGs (20-23):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 20 | Daedalus | DAD_SM |
| 21 | Bor | BOR_SM |
| 22 | Vladof | VLA_SM |
| 23 | Maliwan | MAL_SM |

**Snipers (26-29):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 26 | Jakobs | JAK_SR |
| 27 | Vladof | VLA_SR |
| 28 | Order | ORD_SR |
| 29 | Maliwan | MAL_SR |

**Heavy Weapons (244-247):**

| ID | Manufacturer | Code |
|----|--------------|------|
| 244 | Vladof | VLA_HW |
| 245 | Torgue | TOR_HW |
| 246 | Bor | BOR_HW |
| 247 | Maliwan | MAL_HW |

**Shields (279-288):**

| ID | Type | Code |
|----|------|------|
| 279 | Energy Shield | energy_shield |
| 280 | Bor Shield | bor_shield |
| 281-288 | Manufacturer variants | dad/jak/mal/ord/ted/tor/vla_shield |

**Equipment (300-409):**
- 300: Grenade Gadget
- 310: Turret Gadget
- 320: Repair Kit
- 330: Terminal Gadget
- 400-409: Enhancements by manufacturer

---

## Part Indices Are Context-Dependent

Part token `{4}` doesn't mean the same part across all weapons. The index is relative to the Part Group. Index 4 on a Vladof SMG might be a specific barrel, while index 4 on a Jakobs Pistol is something completely different.

This is why you must decode the Part Group ID first. Without knowing which pool you're indexing into, part tokens are meaningless.

Common part indices (within each category):

| Index | Typical Part Type |
|-------|-------------------|
| 2 | Damage modifier |
| 3 | Crit damage modifier |
| 4 | Reload speed modifier |
| 5 | Magazine size modifier |
| 7-10 | Body variants |
| 15-18 | Barrel variants |

The full parts database (`share/manifest/parts_database.json`) contains 2,615 parts across 53 categories, extracted from memory analysis.

---

## Level Encoding

For VarInt-first format serials, the fourth token encodes item level with a bit-packed formula:

Levels 1-15 encode directly as the token value.

Levels 16+ use: `level = 16 + bits[6:1] + 8 * bit0`

Where `bit0` is the lowest bit and `bits[6:1]` is the middle 6 bits.

| Token | Level |
|-------|-------|
| 9 | 9 |
| 128 | 16 |
| 129 | 24 |
| 132 | 18 |
| 138 | 21 |
| 142 | 23 |

---

## The UE5 Part System

Behind serials, parts are defined as UE5 objects. The `GbxSerialNumberIndex` structure links parts to their encoding:

```
GbxSerialNumberIndex
├── Category (Int64): Part Group ID
├── scope (Byte): Root=1, Sub=2
├── status (Byte): Active, Static, etc.
└── Index (Int16): Position in category
```

Each `InventoryPartDef` contains this structure plus the part's stat modifiers, visual mesh references, and other properties.

The game's internal registration order determines indices—not alphabetical sorting. This is why we extract mappings from memory dumps rather than inferring them from file names.

---

## Comparing Serials

When you have two similar items and want to find what differs:

```
Serial 1: @Ugd$YMq/.&{!gQaYQ1)<G9C8...
Serial 2: @Ugd$YMq/.&{!gQaYQ1)<?B8b...
```

Decode both, align the tokens, find where they diverge. The difference reveals what that section encodes. Two weapons with identical parts but different accuracy will differ only in the accuracy-related bytes.

---

## Practical Usage

The bl4 tool decodes serials instantly:

```bash
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Output:
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Weapon)
Part Group: 22 (VLA_SM)
Tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

For more detail:
```bash
bl4 decode --verbose '@Ugr$ZCm/&...'   # Shows raw bytes and bit positions
bl4 decode --debug '@Ugr$ZCm/&...'     # Shows bit-by-bit parsing
```

---

## Exercises

**Exercise 1: Identify Item Types**

Given these serials, what category is each?
1. `@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_`
2. `@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_`
3. `@Ug!$ZCm/&tH!t{KgK/Shxu>k`

**Exercise 2: Decode a Manufacturer**

Use `bl4 decode` on a weapon serial. What Part Group ID does it use? What manufacturer does that correspond to?

**Exercise 3: Compare Two Items**

Find two similar weapons in your inventory. Decode both serials. Which tokens differ? Can you correlate the differences to visible stats?

<details>
<summary>Exercise 1 Answers</summary>

1. `e` at position 3 → Equipment (shield/enhancement)
2. `w` at position 3 → Weapon (SMG category)
3. `!` at position 3 → Class Mod

</details>

---

## What's Next

Serials encode items completely—but where do the part definitions come from? The Part Group IDs we use come from analyzing game data. The mappings between indices and actual parts come from memory dumps and pak file extraction.

Next, we'll explore how to extract data from BL4's game files, including the investigation into whether authoritative category mappings exist anywhere we can reach them.

**Next: [Chapter 6: Data Extraction](06-data-extraction.md)**

