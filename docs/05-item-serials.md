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
- Element type (Kinetic, Corrosive, Shock, Radiation, Cryo, Fire)
- Every part (barrel, grip, scope, magazine)
- Random seed for stat calculations
- Additional flags (some correlate with rarity in database)

The encoding is compact. A 40-character serial describes an item that would need hundreds of bytes in a more verbose format.

---

## The Decoding Pipeline

Serials transform through multiple stages. Understanding each stage reveals how the pieces fit together.

```text
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

```text
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~
```

Every 5 characters encode 4 bytes. The math: 85⁵ ≈ 4.4 billion, which fits in 32 bits (4 bytes) with room to spare.

To decode, look up each character's position in the alphabet, combine them as a base-85 number, then extract 4 bytes big-endian:

```text
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

## Item Type: Determined by First Token, Not Character

**Important discovery**: The third character in a serial (like `b`, `e`, `r`) is NOT an explicit type encoding. It's simply a byproduct of Base85-encoding the first token's bits. The **actual item type** is determined by parsing the first token after the magic header:

| First Token | Item Type | What It Contains |
|-------------|-----------|------------------|
| VarInt (prefix `100`) | Weapon | Pistols, shotguns, rifles, SMGs, snipers |
| VarBit (prefix `110`) | Equipment | Shields, grenades, class mods, gadgets |

This means two serials with different "type characters" might represent the same category of item, and vice versa. Always determine type from the bitstream, not the character.

## Two Serial Formats

BL4 uses two distinct token structures, distinguished by the first token after the 7-bit magic header:

### Weapon Format (VarInt-first)

Weapons start with a VarInt encoding a combined manufacturer/weapon-type ID:

```text
[0] VarInt: manufacturer_weapon_id   (e.g., 2 = Jakobs Pistol, 14 = Ripper Shotgun)
[1] SoftSeparator
[2] VarInt: 0
[3] SoftSeparator
[4] VarInt: 8
[5] SoftSeparator
[6] VarInt: level_code               <- LEVEL ENCODED HERE
[7] Separator
[8] VarInt: 4
[9] SoftSeparator
[10] VarInt: seed                    <- Random seed for stats
[11] Separator
[12] Separator
[13+] Part tokens...
```

### Equipment Format (VarBit-first)

Equipment (shields, grenades, class mods) starts with a VarBit encoding the category:

```text
[0] VarBit: category_identifier      <- Category * divisor
[1] Separator
[2] VarBit: level_code               <- LEVEL ENCODED HERE
[3] Separator
[4] String: (often empty)
[5] VarInt: (varies)
[6+] More data and parts...
```

For VarBit-first serials, the category is extracted using a divisor:
```text
Category ≈ first_varbit / 385   (most equipment)
Category ≈ first_varbit / 8192  (some shields)
```

The divisor isn't exact—categories are approximate matches. The bl4 tools handle this automatically.

## Type-Aware Category Lookups

**Critical**: Category numbers overlap between item types. The same category number means different things for different item types.

For example, category 20:
- For weapons: Daedalus SMG
- For r-type shields: Energy Shield

This means you must know the item type before interpreting the category. The bl4 tools use type-aware lookup:

```rust
pub fn category_name_for_type(item_type: char, category: i64) -> Option<&'static str> {
    match item_type {
        'r' => SHIELD_CATEGORY_NAMES.get(&category)
            .or_else(|| CATEGORY_NAMES.get(&category)),
        _ => CATEGORY_NAMES.get(&category),
    }
}
```

Known shield categories (r-type items):
| Category | Type |
|----------|------|
| 16 | Energy Shield |
| 20 | Energy Shield |
| 21 | Energy Shield |
| 24 | Energy Shield |
| 28 | Armor Shield |
| 31 | Armor Shield |

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
```text
Original: 84 A5 86 06 ...
Mirrored: 21 A5 61 60 ...
```

**Step 4: Parse bitstream**
```text
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
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
# Output shows tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

---

## Part Group IDs (Categories)

The Part Group ID (also called Category ID) determines which part pool to use for decoding. Each ID corresponds to a manufacturer/weapon-type combination.

**Important:** The first VarInt in a weapon serial (the "serial ID") is NOT the same as the Part Group ID. There's a mapping between them. For example, serial ID 2 = Jakobs Pistol, but Part Group ID 2 = Daedalus Pistol. The bl4 tools handle this conversion automatically via `serial_id_to_parts_category()`.

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

**Equipment/Gadgets (300-409):**

Gadget categories use a type/subtype pattern. The base type is `category / 10 * 10`, and the subtype is `category % 10`:

| Category | Type | Code |
|----------|------|------|
| 300-309 | Grenade Gadget | grenade_gadget |
| 310-319 | Turret Gadget | turret_gadget |
| 320-329 | Repair Kit | repkit |
| 330-339 | Terminal Gadget | terminal_gadget |
| 400-409 | Enhancement | enhancement |

For example, category 321 = Repair Kit (type 32), subtype 1.

The bl4 tools handle this automatically:
```rust
// For gadget range (300-399), try base type
if (300..400).contains(&category) {
    let base = category / 10 * 10;
    return CATEGORY_NAMES.get(&base);
}
```

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

Level is encoded at different positions depending on item format:

- **Weapons**: 4th VarInt (position 6 in token list) — direct encoding
- **Equipment**: VarBit immediately after the first separator (position 2) — **0-indexed storage**

### Equipment Level Storage (0-indexed)

Equipment levels are stored as `level - 1`:

| VarBit Value | Display Level | Notes |
|--------------|---------------|-------|
| 0 | 1 | Minimum level |
| 29 | 30 | Mid-game |
| 49 | 50 | Max level |
| 50+ | Invalid | Beyond current cap |

**Verification:** All items with `/)}}` pattern (level 50) have VarBit=49. Tested across Throwing Knives, Energy Shields, Class Mods, and Grenades.

### Observed Values

| Code | Binary | In-Game Level | DB Rarity Label |
|------|--------|---------------|-----------------|
| 30 | `00011110` | 30 | Common |
| 50 | `00110010` | 50 | Common |
| 142 | `10001110` | 44 | Common |
| 192 | `11000000` | 50 | Epic |
| 200 | `11001000` | 50 | Legendary |

For codes 1-50, level equals the code directly. For codes 128+, the formula `level = 2 × (code - 120)` works for values up to 145 (which gives level 50).

Codes 192 and 200 both correspond to level 50 items but appear on items with different rarity labels in the database. The bit differences:
- Bit 6 (0x40): Set in 192 and 200, clear in 142
- Bit 3 (0x08): Set in 200, clear in 192

### Equipment First VarBit

For VarBit-first equipment, the first VarBit encodes category plus additional data:

```text
category = first_varbit / divisor
remainder = first_varbit % divisor
```

Observed correlations between remainder bits and database rarity:
- VarBit 107200 (remainder 64, bits 7-6 = 1) → DB shows "Epic"
- VarBit 78528 (remainder 192, bits 7-6 = 3) → DB shows "Legendary"

---

## Element Encoding

Element types are encoded as Part tokens. The observed pattern:

```text
Part Index = 128 + Element ID
```

| Element ID | Element | Part Token | Verified On |
|------------|---------|------------|-------------|
| 0 | Kinetic (None) | `{128}` | Hellhound |
| 5 | Corrosive | `{133}` | Seventh Sense |
| 8 | Shock | `{136}` | Armored Pre-Emptive Bod |
| 9 | Radiation | `{137}` | (from research notes) |
| 13 | Cryo | `{141}` | Seventh Sense |
| 14 | Fire | `{142}` | Hellwalker |

Multi-element weapons contain multiple element tokens:
```text
Tokens: ... {136} {141} ...  → Shock + Cryo
```

---

## Part Index Bit 7: Root vs Sub Scope

**Major discovery (Jan 2026)**: For Part token indices **> 142** (beyond the element range), **bit 7 indicates the part's scope**:

```text
Bit 7 = 0 → Root scope (core weapon structure)
Bit 7 = 1 → Sub scope (modular attachments)
```

The actual part index is stored in the **lower 7 bits**. To look up the part, strip bit 7:

```python
if index > 142:
    actual_index = index & 0x7F  # Keep only bits 0-6
```

### Examples

| Serial Index | Binary | Bit 7 | Actual Index | Part Name | Scope Type |
|--------------|--------|-------|--------------|-----------|------------|
| 4 | `00000100` | 0 | 4 | part_body_b | Root (core) |
| 35 | `00100011` | 0 | 35 | part_scope_acc_s02_l02_b | Root (core) |
| 170 | `10101010` | **1** | 42 | part_grip_03 | **Sub (attachment)** |
| 166 | `10100110` | **1** | 38 | part_grip_04_hyp | **Sub (attachment)** |
| 174 | `10101110` | **1** | 46 | part_underbarrel_02_meathook | **Sub (attachment)** |
| 196 | `11000100` | **1** | 68 | part_foregrip_01 | **Sub (attachment)** |

### Scope Types

**Root parts** (bit 7 = 0): Define the weapon's fundamental structure
- Body, barrel, magazine
- Scope, sights
- Shield accessories

**Sub parts** (bit 7 = 1): Modular attachments that customize the weapon
- Grips, foregrips
- Underbarrel attachments
- Rail accessories

This matches the `GbxSerialNumberIndex.scope` field documented below (Root=1, Sub=2).

**Validation**: Tested on Rainbow Vomit (Jakobs Legendary Shotgun). All 7 decoded parts with bit 7 flag correctly resolved to valid Jakobs Shotgun parts, improving resolution from 30% → 70%.

---

## The UE5 Part System

Behind serials, parts are defined as UE5 objects. The `GbxSerialNumberIndex` structure links parts to their encoding:

```text
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

```text
Serial 1: @Ugd$YMq/.&{!gQaYQ1)<G9C8...
Serial 2: @Ugd$YMq/.&{!gQaYQ1)<?B8b...
```

Decode both, align the tokens, find where they diverge. The difference reveals what that section encodes. Two weapons with identical parts but different accuracy will differ only in the accuracy-related bytes.

---

## Decoding Examples

### Weapon Serial

Serial: `@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-`

Decoded tokens:
```text
9 ,  0 ,  8 ,  200 | 4 ,  1367 | | {172} {4} {6} {164} {167} {142} {65} {19:7}
```

- First VarInt (9): Jakobs Shotgun
- 4th VarInt (200): Level 50, additional flags set
- Seed (1367): Random seed after first separator
- Part token `{142}`: Fire element (128 + 14)

### Equipment Serial

Serial: `@Uge8jxm/)@{!bAp5s!;381FF>eS^@w`

Decoded tokens:
```text
107200 | 50 | "" 4 ,  56 | | {9} {4} {250:131} {5}
```

- First VarBit (107200): Category 279 (Energy Shield), remainder 64
- Second VarBit (50): Level 50
- Part tokens follow after separators

---

## Exercises

**Exercise 1: Identify Item Types**

Given these serials, what category is each?
1. `@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_`
2. `@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_`
3. `@Ug!$ZCm/&tH!t{KgK/Shxu>k`

**Exercise 2: Decode a Manufacturer**

Use `bl4 serial decode` on a weapon serial. What Part Group ID does it use? What manufacturer does that correspond to?

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
