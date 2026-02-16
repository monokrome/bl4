# Chapter 8: Parts System

Every weapon in Borderlands 4 is an assembly. A Daedalus pistol isn't one object — it's a barrel, a grip, a magazine, a scope, and a body bolted together, each chosen from a pool of compatible parts. Swap the barrel and you change damage. Swap the grip and you change handling. The combinations run into the millions, and every one of them needs to encode into a compact serial string.

BL4's parts system lives in NCS (Nexus Config Store) files, not in Unreal Engine assets. This is a fundamental departure from Borderlands 3, where parts were defined in PartSet and PartPool uassets that tools like FModel could read directly. In BL4, you won't find a single `InventoryPartDef` in the pak files. Part definitions are native C++ objects compiled into the game binary, and the only structured record of what parts exist — and which ones go together — is the NCS inventory data.

This chapter maps the entire parts system: what parts exist, how they're named, how they compose into items, and where the remaining gaps are.

---

## Data Sources

Two sources provide part data, and they don't agree.

| Source | File | Parts | What It Provides |
|--------|------|------:|------------------|
| NCS extraction | `share/manifest/item_parts.json` | 1,614 | Complete item-to-parts mapping (authoritative) |
| Memory extraction | `share/manifest/parts_database.json` | 5,368 | Parts with serial indices, category IDs |

NCS extraction is the authoritative source for which parts exist and which items they belong to. It's static — extracted from the game's pak files, deterministic, reproducible:

```bash
# Extract all item parts from NCS
bl4 ncs extract /path/to/ncsdata/pakchunk4-*/Nexus-Data-inv4.bin -t item-parts --json -o item_parts.json
```

Memory extraction captures runtime data from the game process. It provides serial indices (the numeric IDs needed to encode parts into serial strings) and category assignments, but it's incomplete — parts that aren't loaded in the current game session don't appear. A dump taken in the main menu will miss parts that only load when specific items are in your inventory.

The two sources are complementary. NCS tells you *what* parts exist. Memory tells you *how* they're indexed.

---

## Item Types

BL4 has 30 weapon types and one shield type. Weapons are organized by manufacturer and weapon class — not every manufacturer makes every class.

### Weapons

| Manufacturer | PS | SG | AR | SM | SR | HW |
|--------------|:--:|:--:|:--:|:--:|:--:|:--:|
| BOR (Ripper) | - | Y | - | Y | Y | Y |
| DAD (Daedalus) | Y | Y | Y | Y | - | - |
| JAK (Jakobs) | Y | Y | Y | - | Y | - |
| MAL (Maliwan) | - | Y | - | Y | Y | Y |
| ORD (Order) | Y | - | Y | - | Y | - |
| TED (Tediore) | Y | Y | Y | - | - | - |
| TOR (Torgue) | Y | Y | Y | - | - | Y |
| VLA (Vladof) | - | - | Y | Y | Y | Y |

*Table 8.1: Weapon types by manufacturer. PS=Pistol, SG=Shotgun, AR=Assault Rifle, SM=SMG, SR=Sniper Rifle, HW=Heavy Weapon.*

That's 8 manufacturers and 6 weapon classes. Each manufacturer produces 3-4 weapon classes, giving 30 distinct weapon types.

### Shields

Shields are a single type: `Armor_Shield`, with 70 parts split between manufacturer-specific cores (44 parts) and reactive armor augments (26 parts).

---

## Part Naming Convention

Part names follow predictable patterns, and learning them makes the raw data readable at a glance.

### Weapon Parts

```text
{MANUFACTURER}_{WEAPONTYPE}_{SLOT}_{VARIANT}
```

The manufacturer is a three-letter code (`DAD`, `JAK`, `BOR`, etc.). The weapon type is a two-letter abbreviation (`PS`, `SG`, `AR`, `SM`, `SR`, `HW`). The slot identifies where the part attaches. The variant distinguishes between alternatives.

Examples:

- `DAD_PS_Barrel_01` — Daedalus Pistol, Barrel slot, base variant
- `DAD_PS_Barrel_01_A` — Daedalus Pistol, Barrel slot, A variant
- `JAK_SG_Grip_03` — Jakobs Shotgun, Grip slot, variant 3
- `VLA_AR_Scope_IronSight` — Vladof Assault Rifle, iron sight scope

### Shield Parts

```text
part_{type}_{name}
part_core_{manufacturer}_{effect}
part_ra_{augment}_{slot}
```

Examples:

- `part_core_dad_accelerator` — Daedalus shield core with accelerator effect
- `part_ra_armor_segment_primary` — Primary reactive armor augment

### Licensed and Special Parts

Some parts break the standard convention:

- `LicensedPart_WeaponSpecific_UB_Tier1` — Tiered weapon-specific underbarrel
- `part_barrel_licensed_ted_mirv` — Tediore licensed barrel with MIRV reload
- `comp_05_legendary_Zipgun` — Legendary composition for the Zipgun

---

## Part Categories

### Weapon Parts

Each weapon type draws from the same eight part slots, though the counts vary:

| Slot | Typical Count | Notes |
|------|-------------:|-------|
| Barrel | ~10 | 2 base types x 4 variants, plus legendaries |
| Body | 5 | Base + A/B/C/D variants |
| Grip | 5-8 | |
| Magazine | 4-6 | |
| Scope | 10-11 | Includes iron sights |
| Foregrip | 3 | |
| Underbarrel | 4-6 | |
| Top Accessory | 5-8 | |

*Table 8.2: Weapon part slots and typical part counts.*

### Shield Parts

Shields use a different slot structure:

- **Core** (44 total) — manufacturer-specific, determines the shield's base behavior
- **Reactive Armor Primary** (13 types) — augments primary defense
- **Reactive Armor Secondary** (13 types) — augments secondary defense

### Part Counts by Item Type

The full breakdown across all 31 item types totals 1,614 parts:

| Item Type | Parts | | Item Type | Parts |
|-----------|------:|-|-----------|------:|
| Armor_Shield | 70 | | ORD_AR | 54 |
| BOR_HW | 15 | | ORD_PS | 59 |
| BOR_SG | 55 | | ORD_SR | 54 |
| BOR_SM | 57 | | TED_AR | 57 |
| BOR_SR | 55 | | TED_PS | 60 |
| DAD_AR | 56 | | TED_SG | 58 |
| DAD_PS | 56 | | TOR_AR | 55 |
| DAD_SG | 56 | | TOR_HW | 15 |
| DAD_SM | 55 | | TOR_PS | 56 |
| JAK_AR | 56 | | TOR_SG | 53 |
| JAK_PS | 53 | | VLA_AR | 68 |
| JAK_SG | 53 | | VLA_HW | 20 |
| JAK_SR | 55 | | VLA_SM | 65 |
| MAL_HW | 15 | | VLA_SR | 63 |
| MAL_SG | 56 | | | |
| MAL_SM | 58 | | **Total** | **1,614** |
| MAL_SR | 56 | | | |

*Table 8.3: Part counts by item type.*

Heavy weapons (BOR_HW, MAL_HW, TOR_HW) have notably fewer parts at 15 each, except Vladof heavy weapons at 20. Most standard weapons cluster around 53-60 parts.

---

## Category Mappings

The serial format assigns each item type a numeric category ID. Decoding a serial requires knowing which category you're looking at — it determines how part indices map to actual parts.

### Weapons

| Range | Type | Categories |
|-------|------|------------|
| 2-7 | Pistols | DAD, JAK, TED, TOR, ORD, VLA |
| 8-12 | Shotguns | DAD, JAK, TED, TOR, BOR |
| 13-18 | Assault Rifles | DAD, JAK, TED, TOR, VLA, ORD |
| 19 | Maliwan Shotgun | MAL_SG |
| 20-23 | SMGs | DAD, BOR, VLA, MAL |
| 25 | BOR Sniper | BOR_SR |
| 26-29 | Snipers | JAK, VLA, ORD, MAL |
| 244-247 | Heavy Weapons | VLA, TOR, BOR, MAL |

*Table 8.4: Weapon category ID ranges.*

### Equipment

| Category | Type | Status |
|----------|------|--------|
| 44 | Dark Siren Class Mod | Named, no parts in dump |
| 55 | Paladin Class Mod | Named, no parts in dump |
| 97 | Gravitar Class Mod | Named, 2 parts mapped |
| 140 | Exo Soldier Class Mod | Named, no parts in dump |
| 151 | Firmware | Named, no parts in dump |
| 279-288 | Shields | Energy, BOR, DAD, JAK, Armor, MAL, ORD, TED, TOR, VLA |
| 289 | Shield Variant | Named, unknown subtype |
| 300-330 | Gadgets | Grenade (300), Turret (310), Repair (320), Terminal (330) |
| 400-409 | Enhancements | DAD, BOR, JAK, MAL, ORD, TED, TOR, VLA, COV, ATL |

*Table 8.5: Equipment category ID ranges.*

### Category Derivation

Categories can be derived from serial data using different formulas depending on item type:

```text
Weapons (type 'r'):     category = first_varbit_token / 8192
Equipment (type 'e'):   category = first_varbit_token / 384
```

The `category_from_prefix()` function in `bl4-ncs` can also derive categories from manufacturer prefixes when processing NCS data directly (e.g., `BOR_SG` maps to category 12, `JAK_SG` maps to category 9).

---

## Composition System

Items don't just have parts — they have *compositions* that control which parts appear at each rarity tier. The composition system is how the game decides that a common Daedalus pistol gets basic parts while a legendary one gets unique, named parts.

### Rarity Tiers

Five composition tiers correspond to the game's rarity levels:

| Tier | Composition Prefix | Rarity |
|------|-------------------|--------|
| 1 | `comp_01_common` | Common (white) |
| 2 | `comp_02_uncommon` | Uncommon (green) |
| 3 | `comp_03_rare` | Rare (blue) |
| 4 | `comp_04_epic` | Epic (purple) |
| 5 | `comp_05_legendary` | Legendary (orange) |

*Table 8.6: Composition tiers.*

Each composition can reference:

- Part slot assignments (which parts are eligible at this rarity)
- Rarity weights (`firmware_weight_XX_rarity`) controlling drop frequency
- Unique part mandates for legendaries

### Legendary Compositions

Legendary items use `comp_05_legendary_*` compositions that mandate specific unique parts. These compositions name the legendary and pin particular parts that give it its identity:

```text
comp_05_legendary_Zipgun
  uni_zipper              <- Unique naming part (red text flavor)
  part_barrel_01_Zipgun   <- Mandatory unique barrel

comp_05_legendary_GoreMaster
  part_barrel_02_GoreMaster

comp_05_legendary_OM      <- Oscar Mike
  part_barrel_unique_OM
```

The unique naming part (prefixed `uni_`) is what gives the weapon its red-text name in the game UI. The mandatory parts give the weapon its distinctive behavior — the Zipgun's barrel is what makes it a Zipgun.

---

## Licensed Parts

Licensed parts are BL4's cross-pollination system. They let weapons gain abilities from other manufacturers — a Jakobs shotgun with a Tediore reload, or a Vladof rifle with a Maliwan underbarrel.

### License Types

| License | Effect | Slot |
|---------|--------|------|
| Jakobs Ricochet | Critical hits ricochet to nearby targets | Barrel Acc |
| Hyperion Shield | Weapon shield while aiming | Barrel Acc |
| Hyperion Grip | Accuracy bonuses | Grip |
| Tediore Reload | Throw weapon on reload | Barrel Acc, Grip |
| Torgue Mag | Explosive magazine | Magazine |
| Borg Mag | Borg magazine bonus | Magazine |
| Forge Mag | COV repair mechanic | Magazine |
| Atlas UB | Atlas underbarrel | Underbarrel |
| Daedalus UB | Daedalus underbarrel | Underbarrel |
| Maliwan UB | Maliwan underbarrel | Underbarrel |

*Table 8.7: Licensed part types.*

### Tediore Reload Variants

The Tediore reload license has the most variation — six distinct payload types:

| Part | Effect |
|------|--------|
| `part_barrel_licensed_ted` | Default throw |
| `part_barrel_licensed_ted_combo` | Combo reload |
| `part_barrel_licensed_ted_mirv` | MIRV explosion |
| `part_barrel_licensed_ted_shooting` | Gun continues shooting mid-air |
| `part_barrel_licensed_ted_replicator` | Gun replicates on throw |
| `part_barrel_licensed_ted_replicator_multi` | Multi-replicator variant |

*Table 8.8: Tediore reload variants.*

### Weapon-Specific Underbarrels

Some weapons have tiered underbarrel parts that unlock progressively:

- `LicensedPart_WeaponSpecific_UB_Tier0_Vladof` — Vladof-exclusive tier 0
- `LicensedPart_WeaponSpecific_UB_Tier1` through `Tier4` — Tiered unlocks

### KL (Killer License) Parts

Certain weapons have dedicated `_KL` suffix slots:

- `DAD_PS_KL` — Daedalus Pistol killer license
- `JAK_PS_KL` — Jakobs Pistol killer license

These appear to be dedicated slots for applying licensed part effects, separate from the standard part slots.

---

## Serial Index Architecture

To encode a part into a serial string, the game needs a numeric index for every part. These indices are assigned by the `GbxSerialNumberProvider` system at runtime.

### GbxSerialNumberIndex Structure

```text
GbxSerialNumberIndex:
  Category  : Int64   <- Part group ID
  scope     : Byte    <- Root/Sub scope
  status    : Byte    <- Active/Static/etc.
  Index     : Int16   <- Part index within group
```

Each part is self-describing — the serial format encodes the category, scope, and index together, so a serial can be decoded without knowing the item type in advance.

### The Bit 7 Flag: Root vs. Sub Scope

A breakthrough in January 2026 revealed that bit 7 of the part token index encodes the scope:

- For indices > 142, bit 7 = 0 means **Root** scope (core parts)
- For indices > 142, bit 7 = 1 means **Sub** scope (modular attachments)
- The actual part index is in the lower 7 bits: `index & 0x7F`

This discovery improved serial decoding resolution significantly. The Rainbow Vomit legendary shotgun (a Jakobs legendary) went from 30% part resolution to 70% once bit 7 stripping was applied. Indices like 170, 166, 174, and 196 — which had no matches in the parts database — resolved correctly to Jakobs Shotgun parts once the high bit was stripped.

### Registration Order

Part indices aren't stored in NCS for most manufacturers. Only BOR (Ripper) parts have inline serial indices embedded as null-terminated strings directly after the part name:

```text
BOR_SG_Grip_01\0 42\0 part_grip_02\0 ...
BOR_SG_Grip_02\0 43\0 part_grip_03\0 ...
```

That's 36 BOR parts with extractable indices — roughly 2.2% of the total 1,614 parts. For the other 98%, indices are assigned at runtime when the game engine registers parts with `GbxSerialNumberProvider`. This means the only complete source for serial indices is a memory dump from a running game process.

::: {.callout-warning}
Serial index assignments may change between game versions. A parts database extracted from one patch may produce incorrect decodes on another. Always verify against the current game version.
:::

---

## Part Validation

With the parts data extracted, you can validate whether a given part belongs to a given item type.

### Using item_parts.json

```python
import json

with open('share/manifest/item_parts.json') as f:
    items = json.load(f)

# Build lookup: item_id -> set of valid parts
valid_parts = {item['item_id']: set(item['parts']) for item in items}

# Check if a part is valid for an item
def is_valid_part(item_id: str, part_name: str) -> bool:
    return part_name in valid_parts.get(item_id, set())

# Example
is_valid_part('DAD_PS', 'DAD_PS_Barrel_01')  # True
is_valid_part('DAD_PS', 'JAK_PS_Barrel_01')  # False
```

### Using the CLI

```bash
# Extract parts for a specific item type
bl4 ncs extract inv4.bin -t item-parts | grep "DAD_PS_"

# Decode a serial to see its category and parts
bl4 serial decode '@Uge8;)m/&zJ!tkr0N4>8ns8H{t!6ljj'
# Output includes: Category: Paladin Class Mod (55)
```

### NCS vs. Memory: Source of Truth

| Source | Parts for DAD_PS | Status |
|--------|------------------|--------|
| NCS inv.bin | 56 | Complete |
| Memory extraction | 34 | Incomplete |

NCS is the authoritative source for valid parts. Memory extraction misses parts that aren't currently loaded in the game process. Always use NCS-extracted data for part validation. Use memory data only when you need serial indices.

---

## Level Gating

Parts and features are gated by player level through `Att_MinGameStage_*` attributes. These control when parts can appear on dropped or vendor items — a level 5 character won't find Jakobs Ricochet licensed parts.

### Known Level-Gated Categories

| Category | Attribute Pattern |
|----------|-------------------|
| Weapon Types | `Att_MinGameStage_WeaponType_*` |
| Shield Types | `Att_MinGameStage_ShieldType_*` |
| Manufacturers | `Att_MinGameStage_Manufacturer_*` |
| Elements | `Att_MinGameStage_Element_*` |
| Licensed Parts | `Att_MinGameStage_LicensedPart_*` |
| Enhancement Tiers | `Att_MinGameStage_Enhancement_Stats_Tier*` |
| Gadgets | `Att_MinGameStage_Gadget_*` |

*Table 8.9: Level gating categories.*

Each licensed part has its own MinGameStage attribute. The full list:

```text
Att_MinGameStage_LicensedPart_JakobsRicochet
Att_MinGameStage_LicensedPart_HyperionShield
Att_MinGameStage_LicensedPart_HyperionGrip
Att_MinGameStage_LicensedPart_TedioreReload_TopACC
Att_MinGameStage_LicensedPart_TedioreReload_Grip
Att_MinGameStage_LicensedPart_TorgueMag
Att_MinGameStage_LicensedPart_ForgeMag
Att_MinGameStage_LicensedPart_BorgMag
Att_MinGameStage_LicensedPart_Atlas_UB
Att_MinGameStage_LicensedPart_Daedalus_UB
Att_MinGameStage_LicensedPart_Maliwan_UB
Att_MinGameStage_LicensedPart_WeaponSpecific_UB_Tier0_Vladof
Att_MinGameStage_LicensedPart_WeaponSpecific_UB_Tier1
Att_MinGameStage_LicensedPart_WeaponSpecific_UB_Tier2
Att_MinGameStage_LicensedPart_WeaponSpecific_UB_Tier3
Att_MinGameStage_LicensedPart_WeaponSpecific_UB_Tier4
```

::: {.callout-note}
The actual level thresholds (e.g., "Jakobs Ricochet unlocks at level 15") are not stored in NCS files. They're likely defined in binary data tables or engine code and haven't been located yet.
:::

---

## Known Gaps

The parts system is roughly 95% mapped, but several areas remain incomplete.

### Rainbow Vomit Legendary Parts

The Rainbow Vomit (Jakobs Legendary Shotgun) serves as our benchmark for decode resolution. After the bit 7 discovery, 7 of 10 parts resolve correctly (70%). Three indices remain unresolved: 73, 201, and 206.

These correspond to legendary-specific parts found in pak extraction but absent from the NCS inventory files:

- `part_barrel_RainbowVomit`
- `part_mag_RainbowVomit`
- 10 elemental body variants: `part_body_ele_RainbowVomit_Cor_Fire_Shock`, etc.

Twelve legendary-specific parts total. Their serial indices need manual mapping or memory extraction.

### Non-Prefixed Parts

Parts without manufacturer prefixes can't have categories derived from NCS:

- `comp_*` — rarity modifiers that exist in multiple categories
- `part_firmware_*` — no manufacturer prefix
- `part_ra_*` — reactive armor parts with unknown categorization

These require memory dumps or manual mapping.

### Missing Class Mod Parts

Three of four class mod categories have no parts in any dump:

| Category | Class | Parts Found |
|----------|-------|-------------|
| 44 | Dark Siren | 0 |
| 55 | Paladin | 0 |
| 97 | Gravitar | 2 |
| 140 | Exo Soldier | 0 |

Only Gravitar has any parts mapped (`part_grav_asm_legendaryGravitar` and `part_grav_asm_skill_test`). The others require memory dumps taken while the relevant class mods are equipped.

### Equipment Low Resolution

Equipment serials (class mods, firmware) still decode at low resolution. Most part indices map to nothing in the current database. Index 254 appears frequently and may be an end-of-data marker rather than an actual part.

---

## Key Differences from BL3

If you've worked with Borderlands 3 modding, the parts system will feel familiar in concept but different in every implementation detail.

| Aspect | BL3 | BL4 |
|--------|-----|-----|
| Part definitions | PartSet/PartPool uassets | NCS inv.bin (native C++ objects) |
| Part lists | GestaltPartListData for all items | Gestalt only for AI/creatures |
| Weapon assets | Include part pool references | Mesh components only |
| Validation | Check PartSet for weapon type | Check NCS composition |
| Category mappings | Extractable from pak | Runtime-assigned, memory-only |
| Serial indices | Static in assets | Runtime-assigned by GbxSerialNumberProvider |

*Table 8.10: BL3 vs. BL4 parts system comparison.*

The biggest practical difference: in BL3, a motivated person with FModel could extract complete part data from the pak files alone. In BL4, you need NCS parsing for part lists *and* memory extraction for serial indices. No single source gives you everything.

---

## Unresolved Questions

A few aspects of the parts system remain unproven:

1. **Rarity filtering** — Parts have `firmware_weight_XX_rarity` values in NCS. It's unclear whether common-weighted parts can appear on legendary items.

2. **License availability** — We've confirmed that some weapons accept specific licenses (e.g., BOR SG can have Jakobs/Hyperion/Tediore licenses), but the complete mapping of which weapons accept which licenses is incomplete.

3. **Per-item restrictions** — Legendaries may impose restrictions through their compositions beyond what the base part list shows.

These gaps are tractable. More memory dumps across different characters, levels, and loadouts will fill them in. The next chapter covers the bl4 CLI tools that make this extraction practical.
