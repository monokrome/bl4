# Borderlands 4 Data Structures

This document captures reverse engineering findings for Borderlands 4, focusing on item serial encoding and game data structures.

## Table of Contents

1. [Item Serial Format](#item-serial-format)
2. [Reference Tables](#reference-tables)
3. [Game Data Structures](#game-data-structures)
4. [Memory Analysis Details](#memory-analysis-details)
5. [Next Steps](#next-steps)

---

## Item Serial Format

Item serials are encoded strings that fully describe an item's properties. They appear in save files and can be shared between players.

### Serial Structure

```
@Ug<type><base85_data>
```

- **Prefix**: `@Ug` (constant)
- **Type**: Single character indicating item category
- **Data**: Custom Base85 encoded bitstream

### Decoding Pipeline

1. **Strip prefix**: Remove `@U` from the serial string
2. **Base85 decode**: Use custom alphabet, big-endian byte order
3. **Bit mirror**: Reverse bits in each byte (e.g., `0b10000111` → `0b11100001`)
4. **Parse bitstream**: Extract variable-length tokens

### Base85 Alphabet

```
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~
```

### Bitstream Structure

```
[7-bit magic: 0010000][tokens...][00 terminator][zero padding]
```

The bitstream consists of variable-length tokens with no byte alignment.

### Token Types

| Prefix | Bits | Name | Description |
|--------|------|------|-------------|
| `00` | 2 | Separator | Hard separator, renders as `\|` |
| `01` | 2 | SoftSeparator | Soft separator, renders as `,` |
| `100` | 3 | VarInt | Nibble-based variable integer |
| `101` | 3 | Part | Complex part structure with index and optional values |
| `110` | 3 | VarBit | Bit-length-prefixed integer |
| `111` | 3 | String | Length-prefixed 7-bit ASCII string |

### VarInt Encoding

Reads 4-bit values with continuation bits:

```
[4-bit value][1-bit cont][4-bit value][1-bit cont]...
```

- Continuation bit `1` = more nibbles follow
- Continuation bit `0` = stop
- Maximum 4 nibbles (16 bits total)
- Values assembled LSB-first (shift left by 4 per nibble)

### VarBit Encoding

```
[5-bit length][N-bit value]
```

- Length specifies how many data bits follow
- Length 0 means value is 0
- Value bits read MSB-first from the bitstream

### Part Structure

```
[VarInt index][1-bit type flag]
```

**Type flag = 1:**
```
[VarInt value][000 terminator]
```

**Type flag = 0:**
```
[2-bit subtype]
  Subtype 10: No additional data (empty part)
  Subtype 01: Value list [values...][00 terminator]
```

### String Encoding

```
[VarInt length][7-bit ASCII chars...]
```

Length is the number of characters. Each character is 7-bit ASCII.

### Formatted Output Notation

The `bl4 decode` command outputs tokens in human-readable format:

```
180928 | 50 | {0:1} 1660 | | {8} {14} {252:4} ,
```

| Notation | Meaning |
|----------|---------|
| `12345` | Integer value (VarInt or VarBit) |
| `\|` | Hard separator |
| `,` | Soft separator |
| `{index}` | Part with no value |
| `{index:value}` | Part with single value |
| `{index:[v1 v2]}` | Part with multiple values |
| `"text"` | String token |

### Item Type Characters

From save file analysis (counts from actual player saves):

| Char | Count | Category | Notes |
|------|-------|----------|-------|
| `r` | 420 | Weapons/Shields | Most common, multi-use |
| `e` | 304 | Equipment | Shields, enhancements |
| `b` | 102 | Weapons (pistols/misc) | Has different structure |
| `!` | 91 | ClassMod (Dark Siren) | Character-specific |
| `y` | 80 | Weapons (snipers) | |
| `d` | 69 | Weapons (shotguns) | |
| `f` | 51 | Weapons (shotguns) | |
| `w` | 50 | Weapons (SMGs) | |
| `#` | 42 | ClassMod (Paladin) | Character-specific |
| `v` | 38 | Weapons (ARs) | |
| `x` | 37 | Weapons (SMGs) | |
| `u` | 28 | Utilities/Snipers | Grenades, etc. |
| `a` | 28 | Weapons (pistols) | |
| `g` | 19 | Weapons (ARs) | |
| `c` | 16 | Weapons (pistols) | |
| `z` | 7 | Weapons (ARs) | |

**Groupings by structure:**
- **Weapons (a-d)**: `a`, `b`, `c`, `d` - Similar structure with manufacturer at start
- **Weapons (f-g)**: `f`, `g` - Similar structure
- **Weapons (v-z)**: `v`, `w`, `x`, `y`, `z` - Similar structure
- **Equipment**: `e` - Shields/enhancements, distinct structure
- **ClassMods**: `!`, `#` - Character class-specific
- **Utilities**: `u` - Grenades, utilities

### First VarInt = Manufacturer ID

The first VarInt in a weapon serial identifies the manufacturer:

| VarInt | Manufacturer | Internal Code | Weapon Types |
|--------|--------------|---------------|--------------|
| 4 | Daedalus | DAD | SMG, Shotgun, AR, Pistol |
| 6 | Torgue | TOR | Shotgun, AR, Heavy, Pistol |
| 10 | Tediore | TED | AR, Shotgun, Pistol |
| 14 | Ripper | - | Unknown |
| 15 | Order | ORD | Sniper, Pistol, AR |
| 129 | Jakobs | JAK | AR, Pistol, Shotgun, Sniper |
| 134 | Vladof | VLA | SMG, AR, Heavy, Sniper |
| 138 | Maliwan | MAL | SMG, Shotgun, Heavy, Sniper |

**Unconfirmed manufacturers** (from game files):
- BOR (Borg?) - SMG, Shotgun, Heavy, Sniper
- COV - Unknown

**Note**: These mappings are derived from weapons only. Class mods and other equipment use the "brand" field differently (e.g., class name for class mods).

### Weapon Type by Manufacturer (from pak extraction)

| Type | Manufacturers |
|------|---------------|
| SMG | BOR, DAD, MAL, VLA |
| Pistol | DAD, JAK, ORD, TED, TOR |
| Shotgun | BOR, DAD, JAK, MAL, TED, TOR |
| Sniper | BOR, JAK, MAL, ORD, VLA |
| Heavy | BOR, MAL, TOR, VLA |
| AssaultRifle | DAD, JAK, ORD, TED, TOR, VLA |

### Serial Structure by Item Type

Different item types have distinct serial structures. All share the same Base85 encoding but internal token layout varies.

#### Weapon Serial Structure (types a-d, f-g, v-z)

Most weapons follow this pattern:

```
<manufacturer_id>, 0, 8, <value> | 4, <seed/stat> | | {part1} {part2} ... {partN}
```

| Field | Position | Description |
|-------|----------|-------------|
| Manufacturer ID | First VarInt | 4=DAD, 6=TOR, 10=TED, 129=JAK, etc. |
| Constants | Tokens 2-4 | Usually `0, 8, 196` or `0, 8, 68` |
| Separator + 4 | After constants | `\| 4 ,` |
| Seed/Stat | Next VarInt | Random seed or stat value |
| Parts | After `\|\|` | List of `{index}` or `{index:value}` tokens |

**Examples by type:**

| Type | Example Tokens |
|------|----------------|
| `a` (Pistol) | `4 , 0 , 8 , 68 \| 4 , 1 \| \| {175} {4} {10} {14} {207}` |
| `c` (Pistol) | `10 , 0 , 8 , 68 \| 4 , 3937 \| \| {175} {4} {1} {13} {142} {143}` |
| `d` (Shotgun) | `9 , 0 , 8 , 196 \| 4 , 525 \| \| {104} {4} {2} {6} {1} {40}` |
| `f` (Shotgun) | `3 , 0 , 8 , 68 \| 4 , 1119 \| \| {96} {4} {10} {6} {8:7} {1}` |
| `r` (Multi) | `180928 \| 51 \| {0:1} 21 {4} , 2 , , 105 102 41` |
| `v` (AR) | `140 , 0 , 8 , 196 \| 4 , 216598 \| {88:2128} , \| 5 41559 {2}` |
| `w` (SMG) | `138 , 0 , 8 , 76 \| 4 , 2559 \| \| {104} {4} {2} {6} {12}` |
| `y` (Sniper) | `129 , 0 , 8 , 76 \| 4 , 1362 \| \| {96} {4} {6} {10} {1}` |
| `z` (AR) | `141 , 0 , 8 , 200 \| 4 , 3449 \| \| {37} {4} {2} {45} {128}` |

**Note**: The constant after `0, 8,` varies: 68, 76, 196, 200. This may indicate weapon sub-type or category.

#### Equipment Serial Structure (type e)

Equipment (shields, enhancements) uses a different pattern:

```
<type_id> | <level> | "" <manufacturer> , <seed> | | {parts...}
```

| Field | Position | Description |
|-------|----------|-------------|
| Type ID | First value | Encodes item subtype/element (see below) |
| Level | After separator | Level - 1 (49 = level 50) |
| Empty string | After separator | `""` literal |
| Manufacturer | After string | 4=DAD, etc. |
| Seed | After comma | Random seed |
| Parts | After `\|\|` | Part indices |

**Type ID patterns (shields):**
| Type ID | Element |
|---------|---------|
| 29376 | Fire |
| 53952 | Electric |
| 107200 | Other (varies) |

**Example: "Absorbing Pointed Guardian Angel" (Legendary fire shield):**
```
29376 | 49 | "" 4 , 1656 | | {14} {6} {246} , 129 193
  │      │      │   │        │                 └── Additional values
  │      │      │   │        └── Parts: {14}=prefix?, {6}=?, {246}=base
  │      │      │   └── Seed
  │      │      └── Manufacturer (4=Daedalus)
  │      └── Level (49 = level 50)
  └── Type ID (29376 = fire element)
```

**Example: "Adaptive Pointed Guardian Angel" (Epic electric shield):**
```
53952 | 49 | "" 4 , 545 | | {2} {5} {246} , 133 194 | {241:3}
                            └── Different prefix parts, same base {246}
```

#### ClassMod Serial Structure (types ! and #)

ClassMods follow weapon-like patterns but with skill-specific parts:

```
<class_id>, 0, 8, <type> | 4, <seed> | | {part1} {part2} ... {skillN} , <val1> <val2>
```

| Char | Character | Class ID |
|------|-----------|----------|
| `!` | Dark Siren | 247 |
| `#` | Paladin | 255 |

**Parts breakdown:**
- Parts in 0-250 range: Stat modifiers (same as weapons)
- Parts in 2000+ range: Skill modifications (unique to ClassMods)
- `{117}` appears frequently at the end (may be common terminator)

**Example: "Electric Avatar" (Dark Siren Legendary ClassMod):**
```
247 , 0 , 8 , 196 | 4 , 122 | | {206} {13} {2056} {2063} {183} {2080} {2187} {2087} {2211} {117} , 1 213
 │                              │     │    │      │                                    │
 │                              │     │    └──────┴── Skill modifiers (2000+ range)   │
 │                              │     └── body_mod_b+c (stat)                         │
 │                              └── part_206 (stat)                                   │
 └── Class ID (247 = Dark Siren)                                              Common terminator
```

**Example: "Ember Blacksmith" (Paladin Legendary ClassMod):**
```
255 , 0 , 8 , 196 | 4 , 2034 | | {142} {3} {197} {43} {113} {2126} {112} {226} {2191} {117} , 98 138
 │                               │     │    │                 │     │                   │
 │                               │     │    └── Stat parts    │     └── Skill parts   │
 │                               │     └── CritDamage         └── part_2126 (skill)   │
 └── Class ID (255 = Paladin)                                                  Terminator
```

The `type` value (196, 72, 200) may indicate ClassMod subtype or rarity tier.

#### Utility Serial Structure (type u)

Utilities (grenades, etc.) have extended part lists:

```
<value>, 0, 8, <type> | 4, <seed> | | {part1} {part2} ... {partN} , |
```

**Example utility:**
```
128 , 0 , 8 , 196 | 4 , 209 | | {100} {4} {10} {6} {14} {37} {45} {128} {131} {76} {66} {69} {192} {198} , |
```

Often has 12+ parts with trailing `, |`.

#### Type `b` Serial Structure (Special)

Type `b` serials have a more complex structure with embedded strings:

```
<val>, , <val>, , <val>, , <large_val> | "<string>" <val> | , "<string>" | | "<string>" <val> "<string>" | ,
```

This may be for items with extended metadata or named components.

### Common Serial Structure (Summary)

The double separator `||` typically precedes the parts list across all types.

### Part Index Mapping (Weapon Naming System)

The parts database is primarily a **weapon naming table**. Part indices in serials map to naming entries that determine weapon prefixes.

**Structure:**
- **Part index** (e.g., `{4}`, `{8}`) identifies the primary modifier type
- **Secondary modifiers** (stored in the database as `modd`, `modc`, `accuracy`, etc.) determine the specific prefix
- **Prefix name** is the third value in each stat entry (e.g., "Cursed", "Rotten", "Hungry")

| Index | Modifier Type | Example Prefixes |
|-------|---------------|------------------|
| 2 | Damage | "Tortuous", "Agonizing", "Festering" |
| 3 | CritDamage | "Bleeding", "Hemorrhaging", "Pooling" |
| 4 | ReloadSpeed | "Frenetic", "Manic", "Rotten" |
| 5 | MagSize | "Bloated", "Gluttonous", "Hoarding" |
| 7 | body_mod_a | "Chosen", "Promised", "Tainted" |
| 8 | body_mod_b | "Bestowed", "Cursed", "Offered" |
| 9 | body_mod_c | "Ritualized", "Summoned" |
| 10 | body_mod_d | "Strange" |
| 15-18 | barrel_mod_a through d | "Herald", "Harbinger", "Oracle", "Prophecy" |

**Example:** A weapon with part `{8}` (body_mod_b) and secondary modifier `modd` gets the prefix "Cursed" → displayed as "Cursed Linebacker"

The combination of primary part index + secondary modifier determines the final weapon prefix shown to players.

### CLI Usage

```bash
# Basic decode (shows tokens in formatted notation)
bl4 decode '@Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_'

# Example output:
# Serial: @Ugw$Yw2}TYgOvDMQhbq)?p-8<%Z7L5c7pfd;cmn_
# Item type: w (Weapon (variant v-z))
# Manufacturer: Maliwan
# Decoded bytes: 31
# Hex: 212b06019062704432239055e1542b0e8512bd22b654f054e8553856964e4e
# Tokens: 138 ,  0 ,  8 ,  196 | 4 ,  2328 | | {197} {4} {8:13} ...
# Named:  Maliwan ,  0 ,  8 ,  196 | 4 ,  2328 | | {part_197} {ReloadSpeed} {body_mod_b:13} ...

# Verbose output (shows all tokens, raw bytes, extracted fields)
bl4 decode --verbose '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Debug mode (shows bit-by-bit parsing to stderr)
bl4 decode --debug '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

---

## Reference Tables

### Manufacturer Codes

| Code | Manufacturer | Specialty |
|------|--------------|-----------|
| `BOR` | Unknown | SMGs |
| `DAD` | Daedalus | SMG, Shotgun, AR |
| `DPL` | Dahl | Burst fire |
| `JAK` | Jakobs | High damage, semi-auto |
| `MAL` | Maliwan | Elemental weapons |
| `ORD` | Unknown | Sniper, Pistol, AR |
| `TED` | Tediore | Throwable reloads |
| `TOR` | Torgue | Explosive weapons |
| `VLA` | Vladof | High fire rate |

### Weapon Type Codes

| Code | Type |
|------|------|
| `AR` | Assault Rifle |
| `HW` | Heavy Weapon |
| `PS` | Pistol |
| `SG` | Shotgun |
| `SM` | SMG |
| `SR` | Sniper Rifle |

### Rarity Tiers

| Code | Rarity |
|------|--------|
| `comp_01` | Common |
| `comp_02` | Uncommon |
| `comp_03` | Rare |
| `comp_04` | Epic |
| `comp_05` | Legendary |

### Known Legendary Weapons

| Internal Name | Display Name | Type | Manufacturer |
|---------------|--------------|------|--------------|
| `DAD_AR.comp_05_legendary_OM` | OM | AR | Daedalus |
| `DAD_SG.comp_05_legendary_HeartGUn` | Heart Gun | Shotgun | Daedalus |
| `JAK_AR.comp_05_legendary_rowan` | Rowan's Call | AR | Jakobs |
| `JAK_PS.comp_05_legendary_kingsgambit` | King's Gambit | Pistol | Jakobs |
| `JAK_PS.comp_05_legendary_phantom_flame` | Phantom Flame | Pistol | Jakobs |
| `JAK_SR.comp_05_legendary_ballista` | Ballista | Sniper | Jakobs |
| `MAL_HW.comp_05_legendary_GammaVoid` | Gamma Void | Heavy | Maliwan |
| `MAL_SM.comp_05_legendary_OhmIGot` | Ohm I Got | SMG | Maliwan |
| `TED_AR.comp_05_legendary_Chuck` | Chuck | AR | Tediore |
| `TED_PS.comp_05_legendary_Sideshow` | Sideshow | Pistol | Tediore |
| `TOR_HW.comp_05_legendary_ravenfire` | Ravenfire | Heavy | Torgue |
| `TOR_SG.comp_05_legendary_Linebacker` | Linebacker | Shotgun | Torgue |
| `VLA_AR.comp_05_legendary_WomboCombo` | Wombo Combo | AR | Vladof |
| `VLA_HW.comp_05_legendary_AtlingGun` | Atling Gun | Heavy | Vladof |
| `VLA_SM.comp_05_legendary_KaoSon` | Kaoson | SMG | Vladof |

---

## Game Data Structures

### Weapon Attribute System

BL4 uses an attribute system for weapon stats. Stats are calculated dynamically rather than stored directly.

**Attribute naming pattern:** `Att_<Category>_<Name>`

| Prefix | Category |
|--------|----------|
| `Att_Weapon_*` | Weapon-specific |
| `Att_Calc_*` | Calculated/derived |
| `Att_Grav_*` | Gravitar class |
| `Att_PLD_*` | Paladin class |

**Key stat properties:**

| Property | Description |
|----------|-------------|
| `BaseDamage` | Base weapon damage |
| `DamagePerShot` | Per-projectile damage |
| `ProjectilesPerShot` | Pellet count (x4, x6, etc.) |
| `Accuracy` | Weapon accuracy |
| `AccuracyImpulse` | Accuracy impulse modifier |
| `FireRate` | Firing rate |
| `ReloadTime` | Reload time |

### Weapon Part System

Weapons are composed of multiple parts, each affecting stats.

**Part categories:**
- Barrel
- Grip
- Stock
- Scope
- UnderBarrel
- Accessory

**Part data classes:**

| Class | Purpose |
|-------|---------|
| `GestaltPartDataSelector` | Part selection logic |
| `GestaltRandomPartData` | Random part generation |
| `GestaltOptionalPartData` | Optional part handling |
| `PartData` | Base part data |
| `PartList` | Available parts list |

**Stat calculation:**
```
Final Stat = Base Value × Part Modifier₁ × Part Modifier₂ × ...
```

Part modifiers are typically floats in the 0.5-1.0 range.

### Loot System

**ItemPool classes:**

| Class | Description |
|-------|-------------|
| `ItemPoolDef` | Defines a loot pool |
| `ItemPoolEntry` | Single pool entry |
| `ItemPoolListDef` | List of pools |
| `ItemPoolSelectorDef` | Selection logic |

**Weight properties:**

| Property | Description |
|----------|-------------|
| `BaseWeight` | Base drop weight |
| `RarityWeight` | Weight by rarity |
| `GrowthExponent` | Level scaling |
| `GameStageVariance` | Stage variance |

**Luck system:**
- `LuckCategories` - Luck modifier categories
- `EnemyBasedLuckCategories` - Enemy-specific
- `PlayerBasedLuckCategories` - Player-specific

---

## Memory Analysis Details

This section documents findings from memory dump analysis for future reference.

### Important Notes

**ASLR Warning**: Both Windows and Proton use Address Space Layout Randomization. Absolute virtual addresses change every game launch. The relative offsets and patterns documented here are consistent, but base addresses will vary.

### Environment

| Platform | Dump Method | Dump Size | Notes |
|----------|-------------|-----------|-------|
| Linux (GE-Proton10-25) | `gcore` | ~27 GB | File offset ≈ VA |
| Windows 11 (24H2) | Process Hacker | ~21 GB | Requires VA mapping extraction |

**Dump files** (in `share/dumps/`):
- `vex_level50_uvh5_bank.dmp` - Windows 11 (Process Hacker full dump)
- `vex_level50_uvh5_shotguns.dmp` - Windows 11 (Process Hacker full dump)
- `vex_level50_uvh5_bank.dump.107180` - Linux/GE-Proton10-25 (gcore)
- `vex_level50_uvh5_shotguns.dump.107180` - Linux/GE-Proton10-25 (gcore)

### Item Entry Structure - Linux (GE-Proton10-25)

Example serial `@Ugr$ZCm/&tH!t{KgK/Shxu>k` at file offset `0x14d21a8`:

```
Offset      Raw Bytes              Description
------      ---------              -----------
-0x40       00 00 00 00 00 00 00 00   (zeros)
-0x38       00 00 00 00 00 00 80 3f   1.0 float32 at +4
-0x30       00 00 00 00 00 00 00 00   (zeros)
-0x28       00 00 00 00 00 00 80 3f   1.0 float32 at +4
-0x20       00 00 00 00 00 00 00 00   (zeros)
-0x18       00 00 00 00 00 00 00 00   (zeros)
-0x10       00 00 00 00 00 00 00 00   (zeros)
-0x08       04 00 00 08 80 00 00 00   Flags/header
+0x00       40 55 67 72 24 5a 43...   Serial string (@Ugr$ZC...)
+0x20       00 00 00 00 00 00 00 00   (zeros after serial)
+0x28       00 00 00 00 00 00 80 3f   1.0 float32 at +4
+0x30       00 00 00 00 00 00 00 00   (zeros)
+0x38       00 00 00 00 00 00 80 3f   1.0 float32 at +4
```

### Item Entry Structure - Windows 11 (Native)

Example serial `@Ugr$ZCm/&tH!t{KgK/Shxu>k` at VA `0x11a6ed80` (raw offset `0x958fd80`):

```
Offset      Raw Bytes              Description
------      ---------              -----------
-0x28       80 d5 b2 4e 01 00 00 00   Pointer (0x14eb2d580)
-0x20       00 00 00 00 00 00 00 00   (zeros)
-0x18       dc 1d 00 00 00 00 00 00   ID (0x1ddc)
-0x10       01 00 00 00 00 00 00 00   Flags
-0x08       00 00 00 00 00 00 00 00   (zeros)
+0x00       40 55 67 72 24 5a 43...   Serial string (@Ugr$ZC...)
+0x20       80 d5 b2 4e 01 00 00 00   Pointer (0x14eb2d580)
+0x28       00 00 00 00 00 00 00 00   (zeros)
+0x30       d8 1d 00 00 00 00 00 00   ID (0x1dd8)
+0x38       01 00 00 00 00 00 00 00   Flags
+0x40       40 55 67 72 25 53 63...   Next serial (@Ugr%Sc...)
```

Items in Windows are packed in ~64-byte aligned entries.

### Signature Patterns

**Linux (Proton)**: Look for `00 00 80 3F` (1.0 float) at -0x38 and -0x28 before `@Ug`

**Windows**: Look for pointer + ID + flags pattern before `@Ug`

### Serial Byte Correlation

Comparing two Linebacker shotguns with different stats:

| Stat | Slot 3 | Slot 4 |
|------|--------|--------|
| Accuracy | 71% | 74% |
| Reload | 1.8s | 1.6s |
| Byte 9 | 0x71 | 0xb1 |

First bit divergence at bit 72 correlates with stat differences.

---

## Part Slot Types (from Memory)

Found `EWeaponPartValue` enum defining part slots:

| Slot | Description |
|------|-------------|
| Grip | Weapon grip |
| Foregrip | Front grip |
| Reload | Reload mechanism |
| Barrel | Main barrel |
| Scope | Optics/scope |
| Melee | Melee attachment |
| Mode | Fire mode |
| ModeSwitch | Mode switch mechanism |
| Underbarrel | Under-barrel attachment |
| Custom0-7 | Additional custom slots |

### Memory Dump Findings

The memory dump contains:
- Class definitions and enum values
- Weapon asset paths (e.g., `TOR_SG_Scope_01_L2_B`, `BOR_HW_Barrel_02`)
- Runtime part structures

**Not found**: Explicit part index → asset name mapping tables. These are likely stored in PAK files.

## Part Pool Data in Game Files

The game files **DO** contain part pool definitions, but they're structured differently than expected.

### What We Found

1. **Enhancement Core Parts** (`EnhancementCoreCombinationStruct.uasset`):
   - Contains ~44 part definitions with indices embedded in names
   - Example: `Part_Core_DAD_Accelerator_74` = index 74
   - Manufacturer-specific: ATL, BOR, COV, DAD, HYP, JAK, MAL, ORD, TED, TOR, VLA

2. **Weapon Naming Structures** (`WeaponNamingStruct.uasset`):
   - Maps stat types to indices for weapon prefixes:

   | Stat | Index |
   |------|-------|
   | Damage | 2 |
   | CritDamage | 9 |
   | FireRate | 10 |
   | ReloadSpeed | 11 |
   | MagSize | 12 |
   | Accuracy | 13 |
   | ElementalPower | 14 |
   | ADSProficiency | 16 |
   | Single | 18 |
   | DamageRadius | 21 |

3. **Licensed Part Tables** (`DAD_LicensedPart_Table_Struct.uasset`):
   - Maps magazine types to indices: `tor_mag_9`, `cov_mag_10`, `borg_mag_11`
   - Manufacturer-specific part combinations

4. **Weapon Part Names** (in skeletal mesh bones):
   - ~140+ barrel variants: `BOR_HW_Barrel_01`, `DAD_AR_Barrel_02`, etc.
   - Pattern: `{MFR}_{TYPE}_Barrel_{NUM}` where MFR=manufacturer, TYPE=weapon type

### Usmap Mapping File

A `.usmap` mapping file exists at `share/borderlands.usmap` (2.1MB) that contains the property schema for unversioned UE5 assets (16,731 structs, 2,979 enums).

**To use usmap with uextract:**
```bash
./target/release/uextract "/path/to/Paks" -o /tmp/output --usmap share/borderlands.usmap
```

**Important limitation:** The usmap contains **engine and game class definitions** but NOT user-defined DataTable row structs like `Struct_EnemyDrops` or `Struct_WeaponStats`. These DataTable row structs are the ones that contain the actual balance data we need.

### What's Still Needed

The **index-to-part mapping** that tells us "Part index 137 on a Maliwan SMG = MAL_SM_Barrel_02" requires:

1. Parsing DataTable row structs - these are user-defined and not in the usmap schema
2. Empirical correlation from known items in guncode_export.csv
3. Or memory analysis to capture runtime part pool data

---

## Data Extraction Status

### Extraction Complete

The manifest extraction pipeline is fully operational. Data is stored in `share/manifest/` (via git-lfs).

#### Extracted Data Summary

| File | Contents |
|------|----------|
| `pak_manifest.json` | 81,097 game assets indexed |
| `mappings.usmap` | 16,849 structs, 2,986 enums, 58,793 properties |
| `items_database.json` | 62 item pools, 26 items with stats, 73 stat types |
| `manufacturers.json` | 10 manufacturers with paths |
| `weapons_breakdown.json` | Weapon counts by type/manufacturer |
| `balance_data.json` | Balance data categories |
| `gear_types.json` | Gear type definitions |
| `naming.json` | Weapon naming strategies |
| `rarity.json` | Rarity tier data |
| `elemental.json` | Elemental type data |

#### Manufacturers (Complete)

| Code | Name | Notes |
|------|------|-------|
| BOR | Borg | SMG, Shotgun, Heavy, Sniper |
| COV | Children of the Vault | Various |
| DAD | Daedalus | SMG, Shotgun, AR, Pistol |
| DPL | Dahl | Turrets/Gadgets |
| JAK | Jakobs | AR, Pistol, Shotgun, Sniper |
| MAL | Maliwan | SMG, Shotgun, Heavy, Sniper |
| ORD | Order | Sniper, Pistol, AR |
| TED | Tediore | AR, Shotgun, Pistol |
| TOR | Torgue | Shotgun, AR, Heavy, Pistol |
| VLA | Vladof | SMG, AR, Heavy, Sniper |

#### Gear Types

- ClassMod, Enhancement, Firmware, Gadget, Grenade, RepairKit, Shield

#### Weapon Types

- AssaultRifle, Heavy, Pistol, Shotgun, SMG, Sniper

### What Serial Decoding Gives Us

- The serial **contains the item's parts/mods** - the actual configuration is encoded
- We can decode tokens like `{197} {4} {8:13} {1} {46}` from any serial
- These tokens represent indices into part pools

### Remaining Work for GUI Editor

1. **Part POOL definitions** - "For a Maliwan SMG, what are the valid barrels/grips/etc?"
   - Token `{4}` on a Maliwan SMG ≠ `{4}` on a Jakobs Pistol
   - Indices are likely per-weapon-type, not global

2. **Mod/augment pool definitions** - Same for shields, ClassMods, etc.
   - Shields have modifiers (prefixes like "Absorbing", "Berserkr", elements)

3. **Item name lookups** - Map base item to display name ("Guardian Angel", "Zipper")

### ClassMod Data Details

**What we have** in game files:
- **Skill scripts** (e.g., `SkillScript_PLD_CM_Blacksmith.uasset`) - Define ClassMod behavior/perks
- **Skill parameters** (e.g., `SkillParam_DarkSiren_PhaseAvatar_ClassMod_Technomancer.uasset`)
- **Attribute definitions** (e.g., `Att_Calc_PLD_CM_BlackSmith_GunDamage`)
- **DataTable references** (e.g., `DataTable_Paladin_ClassMods`)

**What's missing**:
- Item balance/definition files that specify part combinations
- Per-item stat ranges and rarity modifiers
- PartSet definitions for ClassMod assembly

**Reference data** (from guncode_export.csv) has 2,359 ClassMod entries including:

| Character | ClassMod Types |
|-----------|---------------|
| Dark Siren | Avatar, Illusionist, Kindread Spirits, Technomancer, Teen Witch |
| Paladin | Blacksmith, Elementalist, Furnace, ShatterWight, Viking |
| Gravitar | Driver, Generator, Pundit, Scientist, Slider |
| Exo Soldier | Filantropo, various L01-L06 types |

**64 unique ClassMod base types** found in reference:
Agente, Alchemist, Assistant, Avatar, Bio-Robot, Blacksmith, Bookworm, Brawler, Breaker, Buster, Chemist, Commander, Commando, Compiler, Controller, Cyborg, Dancer, Demolitionist, Driver, Elementalist, Esgrimidor, Eye, Filantropo, Firedancer, Furnace, Gearhead, Generator, Genio, Grenadier, Grenazerker, Guardian, Hunter, Icebringer, Illusionist, Instigator, Master, Mercenario, Naturalist, Outlaw, Physicist, Practitioner, Psion, Radiance, Radiologist, Reactor, Ritualist, Savant, Scientist, Shatterwight, Skeptic, Soldado, Soldier, Specialist, Spirits, Stormcaller, Tanker, Technomancer, Tecnico, Torchbearer, Transistor, Trooper, Viking, Weaver, Witch

**Note**: "Guardian Angel" is a **Shield** (Daedalus legendary), NOT a ClassMod. The reference has 100+ Guardian Angel shield variants with different prefixes and elements.

### Key Finding

Item stats in Borderlands are **derived from parts**, not stored directly. A weapon's DPS comes from:
- Base weapon type + manufacturer stats
- Barrel modifier × Grip modifier × Magazine modifier × ...
- Level scaling

The reference data (guncode_export.csv) was likely generated via:
- In-game item inspection
- Memory extraction during gameplay
- Modding tools that hook into game runtime

## Next Steps

### Completed

- [x] Pak manifest extraction (81,097 assets)
- [x] Usmap generation from memory dumps (16,849 structs, 58,793 properties)
- [x] Items database with drop pools and stat modifiers
- [x] Manufacturer table (all 10 manufacturers identified)
- [x] GNames pool discovery and FName resolution
- [x] UObject layout verification (standard UE5)

### Remaining Work

1. **Serial encoding** - Create/modify items (reverse of decoding)
2. **WASM bindings for ItemSerial** - Expose to JavaScript for browser editors
3. **Part index correlation** - Map decoded part indices to actual game parts
4. **Inventory manipulation API** - High-level API for save editing

### Research Tasks

1. **Part pool definitions** - Find where game stores valid parts per weapon type
2. **Compare BL3 tools** - How did they solve part mapping?

---

## UE5 Runtime Structure Discovery

This section documents findings from live process memory analysis for usmap generation.

### Status

Usmap generation is fully working. The `bl4 memory dump-usmap` command generates a complete `.usmap` file with 16,849 structs and 58,793 properties.

### Original Goal

Generate a complete `.usmap` file that includes user-defined DataTable row structs (like `Struct_EnemyDrops`, `Struct_WeaponStats`) which are missing from the current usmap.

### Approach: Live Process Scanning

BL4 runs under Wine/Proton on Linux. The `bl4 memory` command attaches to the running process or reads memory dumps directly.

#### Process Memory Layout (BL4 under Proton)

| Region | Address Range | Size | Permissions | Contents |
|--------|---------------|------|-------------|----------|
| PE Header | 0x140000000-0x140001000 | 4 KB | r-- | PE headers |
| Code Section | 0x140001000-0x14e61c000 | ~235 MB | r-x | Executable code |
| Read-only Data | 0x14e61c000-0x15120e000 | ~45 MB | r-- | Constants, strings |
| Data Section | 0x15120e000-0x15175c000 | ~5.4 MB | rw- | Global variables |
| Additional | 0x15175c000-0x172d73000 | ~530 MB | mixed | Runtime data |

**Key insight**: vtables for UObjects must point to the CODE section (0x140001000-0x14e61c000). This is the primary validation for identifying real UObjects.

### GNames Pool Discovery

**Status**: ✅ Fully Working

The GNames pool (FNamePool) stores all string names used by the engine.

**Discovery method**: Search for the characteristic pattern `"None\0ByteProperty"` in memory.

**BL4-specific findings** (from SDK dump and Windows dump analysis):

| Property | Value (pre-patch) | Value (Nov 2025 patch) | Notes |
|----------|-------------------|------------------------|-------|
| GUObjectArray | `0x1513868f0` | `0x1513878f0` | +0x1000 offset |
| GNames (FNamePool) | `0x1512a0c80` | `0x1512a1c80` | +0x1000 offset |
| GWorld | `0x15531c78` | `0x151532cb8` | +0x1040 offset |
| ProcessEvent | `0x14f7010` | `0x14f7010` | Unchanged |
| ProcessEvent VT Index | `0x49` | `0x49` | Unchanged |
| FName "Class" | Index 588 | Index 588 | `(0 << 16) \| (1176 >> 1)` |
| FName "None" | Index 0 | Index 0 | First entry in block 0 |

Constants defined in `crates/bl4-cli/src/memory.rs`:
```rust
// SDK Data Pointers (offsets from PE_IMAGE_BASE 0x140000000)
// Updated for Nov 2025 patch
pub const GOBJECTS_OFFSET: usize = 0x113878f0;  // VA = 0x1513878f0
pub const GNAMES_OFFSET: usize = 0x112a1c80;    // VA = 0x1512a1c80
pub const GWORLD_OFFSET: usize = 0x11532cb8;    // VA = 0x151532cb8
pub const PROCESS_EVENT_OFFSET: usize = 0x14f7010;
pub const PROCESS_EVENT_VTABLE_INDEX: usize = 0x49;
```

**FNamePool structure (UE5)**:
```
+0x00: Lock (8 bytes)
+0x08: CurrentBlock (4 bytes) - Number of allocated blocks
+0x0C: CurrentByteCursor (4 bytes) - Write position in current block
+0x10: Blocks[FNameMaxBlocks] - array of block pointers (8 bytes each)
```

**FName index encoding** (ComparisonIndex):
- Bits 0-15: Block offset (× 2 for actual byte offset within block)
- Bits 16-29: Block index
- Bits 30-31: Number (for numbered names like "Property_0")

**Decoding formula**:
```rust
let block_index = (comparison_index >> 16) as usize;
let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;
let block_ptr = read_u64(header_addr + 0x10 + block_index * 8);
let entry_addr = block_ptr + block_offset;
```

**FNameEntry format (UE5)**:
```
+0x00: Header (2 bytes)
       - Bit 0: bIsWide (UTF-16 vs ASCII)
       - Bits 1-5: ProbeHashBits
       - Bits 6-15: Length (up to 1024 chars)
+0x02: Characters (Length bytes for ASCII, Length*2 for UTF-16)
```

**Verified names from BL4**:
| Index | Name |
|-------|------|
| 0 | None |
| 1 | ByteProperty |
| 588 | Class |
| ... | ~2M+ names total |

### GUObjectArray Discovery

**Status**: Partially working - found candidates but validation challenging

The GUObjectArray contains pointers to all UObjects in the engine.

**Expected structure (UE5)**:
```cpp
struct FUObjectArray {
    FChunkedFixedUObjectArray Objects;  // Chunked array
    // ...
};

struct FChunkedFixedUObjectArray {
    FUObjectItem** Objects;     // Pointer to array of chunk pointers
    int32 MaxElements;          // Pre-allocated capacity
    int32 NumElements;          // Current count
    int32 MaxChunks;
    int32 NumChunks;
};
```

**FUObjectItem structure (24 bytes)**:
```cpp
struct FUObjectItem {
    UObject* Object;        // +0x00: Pointer to UObject
    int32 Flags;            // +0x08: Object flags
    int32 ClusterRootIndex; // +0x0C: Cluster index
    int32 SerialNumber;     // +0x10: Serial number for weak refs
    // +0x14-0x17: Padding
};
```

**Note**: Some UE5 versions use 16-byte FUObjectItem without ClusterRootIndex.

**Discovery attempts**:

1. **Pattern-based data scan**: Searched for `(ptr, null, count)` patterns where count ≈ 1.5M objects
   - Found ~60 candidates with count 1,581,805
   - Most had invalid chunk pointers or non-UObject data

2. **Validation criteria**:
   - Objects pointer must be valid heap address (0x100000000 - 0x800000000000)
   - First chunk pointer must be valid
   - Objects in chunk must have vtables in CODE section (0x140001000-0x14e61c000)
   - At least 3/10 sampled objects must be valid

3. **Results**: No candidates passed strict vtable validation
   - False positives had vtables in DATA section (not real UObjects)
   - May need LEA instruction scanning instead of data-only scanning

### UObject Structure Offsets

**Standard UE5 UObjectBase layout**:
```cpp
class UObjectBase {
    void* VTablePointer;         // +0x00
    EObjectFlags ObjectFlags;    // +0x08 (4 bytes)
    int32 InternalIndex;         // +0x0C
    UClass* ClassPrivate;        // +0x10
    FName NamePrivate;           // +0x18 (8 bytes: Index + Number)
    UObject* OuterPrivate;       // +0x20
};
```

**BL4 UObject layout (VERIFIED from SDK dump)**:

BL4 uses **standard UE5 UObject layout**, confirmed by SDK dump analysis:
```cpp
class UObject {
    void* vTable;                // +0x00 (8 bytes) - Must point to valid vtable
    int32 Flags;                 // +0x08 (4 bytes) - Object flags
    int32 InternalIndex;         // +0x0C (4 bytes) - Index in GUObjectArray
    class UClass* Class;         // +0x10 (8 bytes) - ClassPrivate pointer
    class FName Name;            // +0x18 (8 bytes: 4 ComparisonIndex + 4 Number)
    class UObject* Outer;        // +0x20 (8 bytes) - OuterPrivate (package/container)
    // Total header size: 0x28 (40 bytes)
};
```

These offsets are defined as constants in `crates/bl4-cli/src/memory.rs`:
```rust
pub const UOBJECT_VTABLE_OFFSET: usize = 0x00;
pub const UOBJECT_FLAGS_OFFSET: usize = 0x08;
pub const UOBJECT_INTERNAL_INDEX_OFFSET: usize = 0x0C;
pub const UOBJECT_CLASS_OFFSET: usize = 0x10;   // Standard UE5 position
pub const UOBJECT_NAME_OFFSET: usize = 0x18;    // Standard position
pub const UOBJECT_OUTER_OFFSET: usize = 0x20;   // Standard position
pub const UOBJECT_HEADER_SIZE: usize = 0x28;    // 40 bytes total
```

**Standard UE5 layout confirmed**:
| Field | Offset | Size | Notes |
|-------|--------|------|-------|
| VTablePointer | +0x00 | 8 | Points to vtable in code section |
| ObjectFlags | +0x08 | 4 | EObjectFlags bitmask |
| InternalIndex | +0x0C | 4 | Index in GUObjectArray |
| ClassPrivate | +0x10 | 8 | Pointer to UClass |
| NamePrivate | +0x18 | 8 | FName (ComparisonIndex + Number) |
| OuterPrivate | +0x20 | 8 | Package/container pointer |

**Note**: Earlier memory dump analysis found self-referential objects at offset +0x08 and +0x18. These were likely false positives due to pointer alignment patterns. The SDK dump from game hacking community confirms standard UE5 layout.

**Runtime offset discovery** (from UnrealMappingsDumper):

Offsets vary between UE versions and should be discovered at runtime:
1. Find a known object (e.g., "Actor" class)
2. Scan memory within 768 bytes to find where ClassPrivate points back to itself
3. Similar scanning for NamePrivate, OuterPrivate, SuperOffset

### Alternative Approaches

#### 1. DLL Injection (Windows)

**UnrealMappingsDumper**: https://github.com/TheNaeem/UnrealMappingsDumper
- Injects DLL into running game
- Uses runtime offset discovery
- Generates complete usmap with all types
- Requires Windows (doesn't work directly with Proton)

**UE4SS**: https://github.com/UE4SS-RE/RE-UE4SS
- Full modding framework with Lua scripting
- Built-in usmap dumper (Ctrl+Numpad 6)
- Also Windows-only

#### 2. Existing Usmap Files

**Nexus Mods**: https://www.nexusmods.com/borderlands4/mods/4
- Community-maintained BL4 usmap
- Updated when game patches

**Community resources**: Modding communities may have mappings

#### 3. Code Pattern Scanning

**patternsleuth**: https://github.com/trumank/patternsleuth
- Pattern detection for UE5 structures
- Used by UE4SS for robust cross-version scanning
- Could be ported to our Rust tooling

**LEA instruction scanning**:
```asm
; Common pattern for accessing global:
lea rax, [rip + offset]  ; 48 8D 05 XX XX XX XX
```
- Scan code section for LEA instructions
- Calculate target addresses from RIP-relative offsets
- Filter targets in data section range
- Validate as GUObjectArray structure

### Usmap File Format

**Header**:
```
+0x00: Magic (2 bytes) - 0xC430 for compressed
+0x02: Version (1 byte)
+0x03: Compression (1 byte) - 0=None, 1=Oodle, 2=Brotli, 3=ZStd
+0x04: CompressedSize (4 bytes) - if compressed
+0x08: DecompressedSize (4 bytes) - if compressed
+0x0C: Data...
```

**Data section** (after decompression):
```
NameCount (4 bytes)
Names[] - Array of length-prefixed strings
EnumCount (4 bytes)
Enums[] - Each: NameIndex, ValueCount, Values[]
StructCount (4 bytes)
Structs[] - Each: NameIndex, SuperIndex, PropertyCount, Properties[]
```

**Property entry**:
```
NameIndex (2 bytes) - Index into names array
PropertyType (1 byte) - See EPropertyType enum
ArrayDim (1 byte) - Array size (1 for non-arrays)
TypeData... - Type-specific data
```

### Current Status

All usmap generation components are now working:

| Component | Status | Notes |
|-----------|--------|-------|
| GNames discovery | ✅ Complete | Header at 0x1513b0c80, 356 blocks |
| GNames reading | ✅ Complete | All blocks accessible |
| FName resolution | ✅ Complete | FName[0]="None", [558]="Object", [588]="Class" all verified |
| Memory abstraction | ✅ Complete | Supports both live process and dump files |
| PE header parsing | ✅ Complete | Dynamic code section bounds |
| Pointer validation | ✅ Complete | Correct heap range 0x10000-0x800000000000 |
| UObject offsets | ✅ Complete | **Standard UE5**: Class@+0x10, Name@+0x18, Outer@+0x20 (from SDK) |
| GUObjectArray discovery | ✅ Complete | Working via SDK data pointers |
| Property extraction | ✅ Complete | 58,793 properties extracted |
| Usmap writing | ✅ Complete | 16,849 structs, 2,986 enums |

### Generated Usmap Statistics

The `bl4 memory dump-usmap` command produces:

| Metric | Count |
|--------|-------|
| Names | 64,917 |
| Enums | 2,986 |
| Enum Values | 17,291 |
| Structs | 16,849 |
| Properties | 58,793 |

Output file: `share/manifest/mappings.usmap`

### FProperty Layout Reference

For reference, the FProperty offsets used (standard UE5):
```
FField (base):
  +0x00: vtable
  +0x08: Owner (UField*)
  +0x10: Next (FField*)
  +0x18: NamePrivate (FName)
  +0x20: FlagsPrivate (uint32)

FProperty (extends FField):
  +0x28: ArrayDim (int32)
  +0x2C: ElementSize (int32)
  +0x30: PropertyFlags (uint64)
  +0x38: RepIndex (uint16)
  +0x3C: Offset_Internal (int32)
  +0x40: Type-specific data...
```

---

## SDK Class Layouts (Pre-Nov 2025 Patch)

Verified class layouts from SDK dump. Sizes are in bytes.

### Core UE5 Types

```cpp
struct FName {
    int32_t ComparisonIndex;  // 0x00
    int32_t Number;           // 0x04
}; // Size: 0x08

struct FVector {
    double X, Y, Z;           // 0x00, 0x08, 0x10
}; // Size: 0x18

struct FRotator {
    double Pitch, Yaw, Roll;  // 0x00, 0x08, 0x10
}; // Size: 0x18

struct FQuat {
    double X, Y, Z, W;        // 0x00, 0x08, 0x10, 0x18
}; // Size: 0x20

struct FTransform {
    FQuat Rotation;           // 0x00
    FVector Translation;      // 0x20
    char pad[8];              // 0x38
    FVector Scale3D;          // 0x40
    char pad[8];              // 0x58
}; // Size: 0x60

template<typename T>
struct TArray {
    T* _data;                 // 0x00
    int32_t _count;           // 0x08
    int32_t _max;             // 0x0C
}; // Size: 0x10

struct FString {
    wchar_t* pText;           // 0x00
    int32_t _count;           // 0x08
    int32_t _max;             // 0x0C
}; // Size: 0x10
```

### UObject Hierarchy

```cpp
class UObject {                           // Size: 0x28
    uint64_t vTable;                      // 0x00
    int32_t Flags;                        // 0x08
    int32_t InternalIndex;                // 0x0C
    UClass* Class;                        // 0x10
    FName Name;                           // 0x18
    UObject* Outer;                       // 0x20
};

class UField : public UObject {           // Size: 0x30
    UField* Next;                         // 0x28
};

class UStruct : public UField {           // Size: 0xB0
    char pad[16];                         // 0x30
    UStruct* Super;                       // 0x40
    UField* Children;                     // 0x48
    char pad[8];                          // 0x50
    int32_t Size;                         // 0x58
    int16_t MinAlignment;                 // 0x5C
    char pad[82];                         // 0x5E
};

class UClass : public UStruct {           // Size: 0x200
    char pad[96];                         // 0xB0
    UObject* DefaultObject;               // 0x110
    char pad[232];                        // 0x118
};
```

### Actor Hierarchy

```cpp
class AActor : public UObject {           // Size: 0x390
    char pad[416];                        // 0x28
    USceneComponent* RootComponent;       // 0x1C8
    char pad[448];                        // 0x1D0
};

class APawn : public AActor {             // Size: 0x410
    char pad[32];                         // 0x390
    APlayerState* PlayerState;            // 0x3B0
    char pad[8];                          // 0x3B8
    AController* Controller;              // 0x3C0
    char pad[72];                         // 0x3C8
};

class ACharacter : public APawn {         // Size: 0x748
    char pad[24];                         // 0x410
    USkeletalMeshComponent* Mesh;         // 0x428
    UCharacterMovementComponent* CharacterMovement; // 0x430
    char pad[784];                        // 0x438
};
```

### Controller Hierarchy

```cpp
class AController : public AActor {       // Size: 0x428
    char pad[8];                          // 0x390
    APlayerState* PlayerState;            // 0x398
    char pad[48];                         // 0x3A0
    APawn* Pawn;                          // 0x3D0
    char pad[8];                          // 0x3D8
    ACharacter* Character;                // 0x3E0
    char pad[64];                         // 0x3E8
};

class APlayerController : public AController { // Size: 0x958+
    char pad[16];                         // 0x428
    APawn* AcknowledgedPawn;              // 0x438
    char pad[8];                          // 0x440
    APlayerCameraManager* PlayerCameraManager; // 0x448
    char pad[168];                        // 0x450
    UCheatManager* CheatManager;          // 0x4F8
    UClass* CheatClass;                   // 0x500
    char pad[1104];                       // 0x508
};
```

### BL4/Oak Classes

```cpp
class AGbxPlayerController : public APlayerController { // Size: 0xDA8
    char pad[176];                        // 0x958
    ACharacter* PrimaryCharacter;         // 0xA08
    char pad[536];                        // 0xA10
    bool bUseGbxCurrencyManager;          // 0xC28
    char pad[7];                          // 0xC29
    UGbxCurrencyManager* CurrencyManager; // 0xC30
    char pad[288];                        // 0xC38
    bool bUseRewardsManager;              // 0xD58
    char pad[7];                          // 0xD59
    UGbxRewardsManager* RewardsManager;   // 0xD60
    char pad[64];                         // 0xD68
};

class AOakPlayerController : public AGbxPlayerController { // Size: 0x3C40
    char pad[376];                        // 0xDA8
    AOakCharacter* OakCharacter;          // 0xF20
    char pad[8880];                       // 0xF28
    bool bIsCurrentlyTargeted;            // 0x31D8
    char pad[48];                         // 0x31D9
    bool bFullyAimingAtTarget;            // 0x3209
    // ... more fields
};

class AGbxCharacter : public ACharacter { // Size: 0x3B80
    char pad[13368];                      // 0x748
};

class AOakCharacter : public AGbxCharacter { // Size: 0x9790
    char pad[1208];                       // 0x3B80
    FOakDamageState DamageState;          // 0x4038 (size 0x608)
    FOakCharacterHealthState HealthState; // 0x4640 (size 0x1E8)
    char pad[4976];                       // 0x4828
    ECharacterHealthCondition HealthCondition; // 0x5B98
    char pad[951];                        // 0x5B99
    FOakActiveWeaponsState ActiveWeapons; // 0x5F50 (size 0x210)
    char pad[960];                        // 0x6160
    // ... more fields
    FDownState DownState;                 // 0x6F40 (size 0x398)
    char pad[8];                          // 0x72D8
    AOakCharacter* ActorBeingRevived;     // 0x72E0
    // ... more fields
    FGbxAttributeFloat AmmoRegenerate;    // 0x95E8
};
```

### Inventory Classes

```cpp
class AInventory : public AActor {        // Size: 0x8B0
    char pad[1312];                       // 0x390
};

class AWeapon : public AInventory {       // Size: 0xD48
    char pad[912];                        // 0x8B0
    FDamageModifierData DamageModifierData; // 0xC40 (size 0x6C)
    char pad[12];                         // 0xCAC
    FGbxAttributeFloat ZoomTimeScale;     // 0xCB8
    char pad[132];                        // 0xCC4
};
```

### Character Definition (Asset Data)

From `test.cpp` - `FOakCharacterDef` defines character loadout configuration (DataAsset, not runtime):

```cpp
struct FOakCharacterDef : public FGbxCharacterDef {
    TArray<FActiveWeaponSlotData> ActiveWeaponSlots;     // 0x668
    float ActiveWeaponScaleThirdPerson;                  // 0x678
    float ActiveWeaponScaleFirstPerson;                  // 0x67C
    bool bEquipSingleItemFromEach;                       // 0x680
    TArray<FOakEquipWeaponData> EquippedWeapons;         // 0x688
    TArray<FOakInventoryItemSelectionData> EquippedItems; // 0x698
    // ... (see test.cpp for full definition)
    TArray<FInventoryItemSelectionData> AdditionalLoot;  // 0xC80
    FGameDataHandleProperty_ DeathLootPattern;           // 0xC90
    TArray<FName> DeathLootSockets;                      // 0xCA8
    // Total size: 0x10B8+
};

// Generic pointer wrapper used extensively in BL4
struct FGbxDefPtrProperty_ {
    char pad[8];                                         // 0x00
    int64_t FGbxDefPtrScriptType;                        // 0x08 (type info)
    int64_t FGbxDefPtr;                                  // 0x10 (T* ObjectPtr)
    char pad[8];                                         // 0x18
}; // Size: 0x20
```

### Currency System

```cpp
struct FSToken {
    int32_t Hash;                         // 0x00
    FName Name;                           // 0x04
}; // Size: 0x0C

struct FGbxCurrency {
    FSToken token;                        // 0x00
    char pad[4];                          // 0x0C
    uint64_t Amount;                      // 0x10
}; // Size: 0x18

class UGbxCurrencyManager : public UObject { // Size: 0x40
    char pad[8];                          // 0x28
    TArray<FGbxCurrency> currencies;      // 0x30
};
// Currency indices: 0=Cash, 1=Eridium, 2=Gold, 3=?
```

### Attribute Types

```cpp
struct FGbxAttributeFloat {
    char pad[4];                          // 0x00
    float Value;                          // 0x04
    float BaseValue;                      // 0x08
}; // Size: 0x0C

struct FGbxAttributeInteger {
    char pad[4];                          // 0x00
    int32_t Value;                        // 0x04
    int32_t BaseValue;                    // 0x08
}; // Size: 0x0C
```

### Component Offsets

| Component | Offset | Description |
|-----------|--------|-------------|
| ComponentToWorld | 0x240 | FTransform for scene components |
| Bones | 0x6A8 | TArray<FTransform> for skeletal meshes |
| Bones2 | 0x6B8 | Secondary bone array |

### Enums

```cpp
enum class ECharacterHealthCondition : int8_t {
    Healthy = 0,
    Injured = 1,
    Dead = 2
};

enum class EMovementMode : int8_t {
    MOVE_None = 0,
    MOVE_Walking = 1,
    MOVE_NavWalking = 2,
    MOVE_Falling = 3,
    MOVE_Swimming = 4,
    MOVE_Flying = 5,
    MOVE_Custom = 6,
    MOVE_MAX = 7
};
```

### Pattern Signatures

Pattern signatures for finding global pointers via code scanning:

```
gNames:   48 8D 0D ? ? ? ? E8 ? ? ? ? C6 05 ? ? ? ? ? 8B 05 ? ? ? ? 48 39 C3 0F 85 ? ? ? ? C6 44 24
gObjects: 48 8B 15 ? ? ? ? C1 E8 ? 48 8D 0C 49 C1 E1 ? 48 03 0C C2 8B 41 ? A9 ? ? ? ? 75 ? 89 C2 81 CA ? ? ? ? F0 0F B1 51 ? 75 ? 4C 89 E1
uWorld:   48 8B 05 ? ? ? ? 48 89 44 24 ? 48 8D 54 24 ? 4C 8D 44 24 ? E8
```

The `?` bytes are wildcards. The patterns contain LEA/MOV instructions with RIP-relative offsets pointing to the global variables.

### Mesh & Visibility

| Offset | Field | Description |
|--------|-------|-------------|
| Mesh + 0x39C | LastSubmitTime | Last frame submitted for rendering |
| Mesh + 0x3A0 | LastRenderTimeOnScreen | Last frame rendered on screen |

**Visibility check**: If `LastSubmitTime > LastRenderTimeOnScreen`, mesh was occluded (behind wall).

### Skeletal Mesh

| Offset | Field | Description |
|--------|-------|-------------|
| USkeletalMesh + 0x308 | SkeletalReference | Reference to skeleton data |

**Bone TMap structure** (size 0x0C per entry):
- +0x00: FName (bone name)
- +0x08: int32 ID (bone index)

### Known Issues

- `OakCharacter->HealthState->HealthTypeStates` returns invalid/zero values
- Health may need to be read via alternative path or different offsets

---

### Key Findings Summary

**Working infrastructure**:
- `FNamePool` fully mapped with 356 blocks
- Name lookup verified: index 588 → "Class"
- PE header parsed for code section bounds (multi-range support)
- Valid pointer range: `0x10000 - 0x800000000000` (Windows heap)
- Valid vtable range: `0x140000000 - 0x175000000` (code sections)

**UObject Layout (VERIFIED from SDK dump)**:
- BL4 uses **standard UE5 UObject layout**:
  - `Flags` at offset **+0x08** (4 bytes)
  - `InternalIndex` at offset **+0x0C** (4 bytes)
  - `ClassPrivate` at offset **+0x10** (standard UE5 position!)
  - `NamePrivate` at offset **+0x18** (standard)
  - `OuterPrivate` at offset **+0x20** (standard)
- Header size: 0x28 bytes (40 bytes)
- Previous "self-referential at +0x08" findings were false positives due to pointer alignment patterns

**SDK Data Pointers**:

| Version | GOBJECTS | GNAMES | GWORLD | PROCESS_EVENT |
|---------|----------|--------|--------|---------------|
| Pre-Nov 2025 | `0x113868f0` | `0x112a0c80` | `0x11531c78` | `0x14f7010` |
| Nov 2025 patch | `0x113878f0` | `0x112a1c80` | `0x11532cb8` | `0x14f7010` |

Note: Nov 2025 patch shifted offsets by +0x1000.

**UClass Metaclass Search**:
- Exhaustive search with correct offsets (Class@+0x10, Name@+0x18) found:
  - 28 objects with FName "Class" but NONE self-referential
  - 612 self-referential objects at +0x10 but NONE with FName "Class"
- The UClass metaclass is likely:
  1. In the executable's .data section (not heap memory)
  2. Not captured in our partial memory dump
  3. Accessible via GUObjectArray enumeration (preferable approach)

**Constants centralized** in `memory.rs`:
```rust
// Pointer validation
pub const MIN_VALID_POINTER: usize = 0x10000;
pub const MAX_VALID_POINTER: usize = 0x800000000000;
pub const MIN_VTABLE_ADDR: usize = 0x140000000;
pub const MAX_VTABLE_ADDR: usize = 0x175000000;
```

### UClass Metaclass Discovery Methodology

The UClass metaclass is the foundation of UE5's reflection system. Finding it enables enumeration of all classes, structs, and properties in the game.

#### Why Find the UClass Metaclass?

In Unreal Engine's type system:
- Every UObject has a `ClassPrivate` pointer to its UClass
- The UClass for "Class" is special: its `ClassPrivate` points to **itself**
- Once found, any object with `ClassPrivate == UClassMetaclass` is a UClass instance
- This enables enumerating all game classes without needing GUObjectArray

#### Discovery Algorithm

**Step 1: Establish code section bounds**
```
Parse PE header at 0x140000000
Find sections with IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_CNT_CODE
BL4 code: 0x140001000-0x14e61c000, 0x15218c000-0x15f273720
```

**Step 2: Scan data sections for self-referential pattern**

For each 8-byte aligned address in writable data (0x151000000-0x175000000):
```
1. Read vtable pointer at offset 0x00
   - Must be valid pointer in executable range

2. Validate vtable[0] points to CODE
   - Read 8 bytes at vtable address
   - First entry must be in code section bounds
   - This confirms it's a real vtable, not random data

3. Check ClassPrivate at offset 0x08 (BL4 compact layout)
   - Read 8 bytes at current_address + 0x08
   - If ClassPrivate == current_address → FOUND self-referential UClass!
```

**Step 3: Resolve name to confirm**
```
Read FName at offset 0x18 (BL4 compact layout)
Decode via FNamePool: block = (index >> 16), offset = (index & 0xFFFF) * 2
```

#### BL4-Specific Discovery Results

**Comprehensive Self-Referential Object Scan**:

| ClassPrivate Offset | Self-Ref Objects Found | FName "Class" (588) Found |
|---------------------|------------------------|---------------------------|
| +0x08 | 16,058 | 0 |
| +0x10 | 612 | 0 |
| +0x18 | 3,625 | 0 |

**Objects with FName "Class" (588) at NamePrivate@+0x18**:
- Found 28 valid objects (with valid vtables)
- **None are self-referential** at any tested offset
- Example: `0x1912f49c0` has ClassPrivate pointing to `0x14f730a50` (not self)

**Key Observation**: Standard UClass metaclass pattern (self-ref + FName "Class") **does not exist** in this dump!

**Example self-referential objects (ClassPrivate@+0x18)**:
| Address | FName at +0x30 | Notes |
|---------|----------------|-------|
| 0x1514d3ed0 | "always_loaded" | Large FName idx (10764831) |
| 0x1516ae988 | "Title" | FName idx 8342127 |
| 0x190d3ca90 | "ByteProperty" | Standard UE type |

**Anomaly Analysis**:
1. Many self-referential objects exist at various ClassPrivate offsets
2. Objects named "Class" exist but are NOT self-referential
3. Self-referential objects have OTHER names ("ByteProperty", "Title", etc.)
4. This suggests either:
   - BL4 modified UClass to NOT be self-referential
   - The metaclass uses a different identification pattern
   - Memory dump is missing critical regions (>100MB excluded)
   - **Gearbox renamed "Class" FName** to something else in the metaclass

**Alternative Discovery Approach - Property Type Pattern**:

The self-referential objects at +0x18 include recognizable UE types:
- `0x190d3ca90` - FName "ByteProperty" (standard UE property type!)
- `0x1516ae988` - FName "Title"
- `0x1514d3ed0` - FName "always_loaded"

These ARE likely UClass/UStruct instances. The "ByteProperty" object is particularly interesting -
it's a standard UE reflection type. This suggests we can enumerate types by:

1. Finding all self-referential objects (they are UClass instances)
2. Collecting their FNames to build the type list
3. Using vtable patterns to group related types

This approach bypasses the need for finding the specific "Class" metaclass.

#### Finding All UClass Instances (Standard UE5 Layout)

Once a metaclass address is identified:
```rust
const UCLASS_METACLASS_ADDR: usize = 0x????????;  // To be discovered via GUObjectArray

// For any UObject at address `obj` (standard UE5 layout):
let class_ptr = read_u64(obj + 0x10);  // ClassPrivate at +0x10 (standard UE5)
if class_ptr == UCLASS_METACLASS_ADDR {
    // This object IS a UClass
    let name_index = read_u32(obj + 0x18);  // NamePrivate at +0x18
    let class_name = resolve_fname(name_index);
}
```

#### CLI Usage

```bash
# Find UClass metaclass via self-referential scan
bl4 memory --dump share/dumps/vex.raw find-class-u-class

# List all UClass instances (once metaclass is found)
bl4 memory --dump share/dumps/vex.raw list-uclasses
```

### Completed Steps

All usmap generation steps have been completed:

1. ✅ **UClass enumeration** - Working via GUObjectArray
2. ✅ **GUObjectArray discovery** - SDK data pointers identified
3. ✅ **Property extraction** - 58,793 properties extracted
4. ✅ **Usmap generation** - 16,849 structs, 2,986 enums written

The generated usmap is stored at `share/manifest/mappings.usmap`.

---

*Last updated: December 2025. Usmap generation complete with 16,849 structs and 58,793 properties. SDK data pointers updated for Nov 2025 patch. All memory analysis infrastructure working.*
