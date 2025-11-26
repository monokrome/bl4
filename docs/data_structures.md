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

| Char | Count | Primary Type |
|------|-------|--------------|
| `r` | 6174 | shields |
| `e` | 4853 | enhancements |
| `v` | 676 | assault rifles |
| `x` | 596 | SMGs |
| `#` | 583 | classmod_paladin |
| `!` | 572 | classmod_dark_siren |
| `w` | 526 | SMGs |
| `f` | 480 | shotguns |
| `c` | 436 | pistols |
| `u` | 434 | snipers |
| `y` | 342 | snipers |
| `d` | 330 | shotguns |
| `b` | 218 | pistols |
| `g` | 154 | assault rifles |
| `a` | 93 | pistols |
| `z` | 73 | assault rifles |

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

### Common Serial Structure

Most weapons follow this pattern:

```
<manufacturer_id>, 0, 8, 196 | 4, <stat_value> | | {part1} {part2} ... {partN}
```

The `0, 8, 196` appears constant for standard weapons. Class mods and equipment use different patterns (e.g., `| 49 | ""`).

**Example weapon** (pistol):
```
4 ,  0 ,  8 ,  196 | 4 ,  3657 | | {198} {4} {12} {2} {10} {8} {207} {199} {11} {137}
│                    │   │        └── Parts list
│                    │   └── Stat/seed value
│                    └── Constant separator pattern
└── Manufacturer ID (4 = Daedalus pistol)
```

**Example class mod** (different pattern):
```
255 ,  0 ,  8 ,  196 | 4 ,  2708 | | {138} {7} {2189} {2213} ... {117} ,  100 6071
                                     └── Skill parts in 2000+ range
```

The double separator `||` typically precedes the parts list.

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

## Data Extraction Status

### What We've Extracted (via uextract tool)

| Category | Count | With Stats | Notes |
|----------|-------|------------|-------|
| Total Assets | 81,097 | 301 | From pak files |
| Weapons | 3,200 | 23 | Template structures only |
| Shields | 191 | 4 | Template structures only |
| Gadgets | 157 | 6 | |
| ClassMods | 40 | 0 | Scripts/params exist, no item definitions |
| Repair Kits | 29 | 3 | |
| Enhancements | 37 | 1 | |
| Firmware | - | 1 | |
| Grenades | - | 3 | |

### Reference Data Comparison (guncode_export.csv)

The reference CSV contains 16,541 items with full stats:

| Category | Reference Count | Our Count | Gap |
|----------|-----------------|-----------|-----|
| Weapons | 5,707 | 23 | Missing actual item instances |
| Shields | 2,534 | 4 | Only init templates |
| Class Mods | 2,359 | 0 | Completely absent |
| Repair Kits | 2,336 | 3 | Only init templates |
| Enhancements | 2,260 | 1 | Only init templates |
| Gadgets | 1,344 | 6 | Only init templates |

### What We Have vs What We Need

**What we extracted** are `Struct_*` templates - schema definitions with default/init values:
- `Struct_Weapon_Barrel_Init.uasset` - Default barrel stat values
- `Struct_Weapon_Magazine_Init.uasset` - Default magazine values
- etc.

**What serial decoding gives us:**
- The serial **contains the item's parts/mods** - the actual configuration is encoded
- We can decode tokens like `{197} {4} {8:13} {1} {46}` from any serial
- These tokens likely represent indices into part pools

**What we need** for a GUI editor:

1. **Part POOL definitions** - "For a Maliwan SMG, what are the valid barrels/grips/etc?"
   - Token `{4}` on a Maliwan SMG ≠ `{4}` on a Jakobs Pistol
   - Indices are likely per-weapon-type, not global

2. **Mod/augment pool definitions** - Same for shields, ClassMods, etc.
   - Shields have modifiers (prefixes like "Absorbing", "Berserkr", elements)
   - Unclear if these use "parts" terminology or something else

3. **Item name lookups** - Map base item to display name ("Guardian Angel", "Zipper")

4. **Manufacturer ID table** - Complete mapping (mostly done: DAD=4, TOR=6, etc.)

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

### Understanding the Problem

The serial **already contains** all the parts/mods on an item. What we're missing is the **part pool data** that tells us:
- What valid options exist for each slot on each item type
- What name/effect corresponds to each index in the pool

### High Priority

1. **Find part pool definitions** - Where does the game store "Maliwan SMG can have barrels X, Y, Z"?
   - Check for PartSet, InvBal, or similar assets we haven't found
   - May require FModel for full DataTable extraction
   - Could be in compiled Blueprint bytecode

2. **Correlate reference serials with decoded data** - Use guncode_export.csv
   - Compare items with known parts to find patterns
   - Build mapping empirically if game files don't reveal it

3. **Determine terminology** - Are shield/ClassMod modifiers called "parts"?
   - Or do they use augments/mods/components?
   - Affects where to look in game files

### Medium Priority

4. **Complete manufacturer ID table** - A few IDs still unknown (BOR, COV)
5. **Implement serial encoding** - Create/modify items (reverse of decoding)
6. **GUI editor data model** - Define what data structure the editor needs

### Research Tasks

7. **FModel DataTable extraction** - May reveal part pool data we can't parse with uextract
8. **Memory analysis** - Runtime extraction of part pools from running game
9. **Compare BL3 tools** - How did they solve this? May have similar structure

---

*Last updated: Corrected understanding of part pool requirements*
