# Borderlands 4 Parts System

## Overview

BL4 uses a composition-based parts system defined in NCS (Nexus Config Store) files.
This differs from BL3's explicit PartSet system.

## Data Sources

| File | Content | Source |
|------|---------|--------|
| `share/manifest/item_parts.json` | Complete item-to-parts mapping (1614 parts) | NCS extraction (authoritative) |
| `share/manifest/parts_database.json` | Memory-extracted parts with serial indices (934 parts) | Memory (incomplete) |

### Extracting Item Parts

```bash
# Extract all item parts from NCS
bl4 ncs extract /path/to/ncsdata/pakchunk4-*/Nexus-Data-inv4.bin -t item-parts --json -o item_parts.json

# View parts for all items
bl4 ncs extract inv4.bin -t item-parts
```

## Item Types

### Weapons (30 types)

| Manufacturer | PS | SG | AR | SM | SR | HW |
|--------------|:--:|:--:|:--:|:--:|:--:|:--:|
| BOR (Ripper) | - | ✓ | - | ✓ | ✓ | ✓ |
| DAD (Daedalus) | ✓ | ✓ | ✓ | ✓ | - | - |
| JAK (Jakobs) | ✓ | ✓ | ✓ | - | ✓ | - |
| MAL (Maliwan) | - | ✓ | - | ✓ | ✓ | ✓ |
| ORD (Order) | ✓ | - | ✓ | - | ✓ | - |
| TED (Tediore) | ✓ | ✓ | ✓ | - | - | - |
| TOR (Torgue) | ✓ | ✓ | ✓ | - | - | ✓ |
| VLA (Vladof) | - | - | ✓ | ✓ | ✓ | ✓ |

### Shields (1 type)

- `Armor_Shield` - 70 parts including:
  - `part_core_*` - Manufacturer-specific shield cores (44 parts)
  - `part_ra_*` - Reactive armor augments (26 parts)

## Part Naming Convention

### Weapons
```
{MANUFACTURER}_{WEAPONTYPE}_{SLOT}_{VARIANT}
```

Examples:
- `DAD_PS_Barrel_01` - Daedalus Pistol, Barrel slot, base
- `DAD_PS_Barrel_01_A` - Daedalus Pistol, Barrel slot, A variant
- `JAK_SG_Grip_03` - Jakobs Shotgun, Grip slot, variant 3

### Shields
```
part_{type}_{name}
part_core_{manufacturer}_{effect}
part_ra_{augment}_{slot}
```

Examples:
- `part_core_dad_accelerator` - Daedalus shield core
- `part_ra_armor_segment_primary` - Primary reactive armor augment

## Part Categories

### Weapon Parts
- Barrel (typically 2 base types × 4 variants = 10 parts)
- Body (5 variants: base + A/B/C/D)
- Grip (5-8 parts)
- Magazine (4-6 parts)
- Scope (10-11 parts including iron sights)
- Foregrip (3 parts)
- Underbarrel (4-6 parts)
- Top Accessory (5-8 parts)

### Shield Parts
- Core (manufacturer-specific, 44 total)
- Reactive Armor Primary Augment (13 types)
- Reactive Armor Secondary Augment (13 types)

## Part Counts by Item Type

| Item Type | Parts |
|-----------|------:|
| Armor_Shield | 70 |
| BOR_HW | 15 |
| BOR_SG | 55 |
| BOR_SM | 57 |
| BOR_SR | 55 |
| DAD_AR | 56 |
| DAD_PS | 56 |
| DAD_SG | 56 |
| DAD_SM | 55 |
| JAK_AR | 56 |
| JAK_PS | 53 |
| JAK_SG | 53 |
| JAK_SR | 55 |
| MAL_HW | 15 |
| MAL_SG | 56 |
| MAL_SM | 58 |
| MAL_SR | 56 |
| ORD_AR | 54 |
| ORD_PS | 59 |
| ORD_SR | 54 |
| TED_AR | 57 |
| TED_PS | 60 |
| TED_SG | 58 |
| TOR_AR | 55 |
| TOR_HW | 15 |
| TOR_PS | 56 |
| TOR_SG | 53 |
| VLA_AR | 68 |
| VLA_HW | 20 |
| VLA_SM | 65 |
| VLA_SR | 63 |
| **Total** | **1614** |

## Composition System

Items use compositions organized by rarity tier:
- `comp_01_common`
- `comp_02_uncommon`
- `comp_03_rare`
- `comp_04_epic`
- `comp_05_legendary`

Each composition can reference:
- Part slot assignments
- Rarity weights (`firmware_weight_XX_rarity`)
- Unique part mandates (for legendaries)

## Legendary Compositions

Legendary items use `comp_05_legendary_*` compositions that mandate specific unique parts:

```
comp_05_legendary_Zipgun
  uni_zipper              <- Unique naming part (red text)
  part_barrel_01_Zipgun   <- Mandatory unique barrel

comp_05_legendary_GoreMaster
  part_barrel_02_GoreMaster

comp_05_legendary_OM      <- Oscar Mike
  part_barrel_unique_OM
```

## Part Validation

### Using item_parts.json (Recommended)

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
```

## Source of Truth

**NCS inv.bin is the authoritative source for valid parts.**

| Source | Parts for DAD_PS | Status |
|--------|------------------|--------|
| NCS inv.bin | 56 | Complete |
| Memory extraction | 34 | Incomplete |

Memory extraction misses parts that aren't currently loaded in game memory.
Always use NCS-extracted data for part validation.

## Key Differences from BL3

| Aspect | BL3 | BL4 |
|--------|-----|-----|
| Part definitions | PartSet/PartPool uassets | NCS inv.bin |
| Part lists | GestaltPartListData for all items | Only for AI/creatures |
| Weapon assets | Include part pool references | Just mesh components |
| Validation | Check PartSet for weapon | Check NCS sequence |

## Licensed Parts (Cross-Manufacturer)

Licensed parts allow weapons to gain abilities from other manufacturers. Each license
grants a specific effect that wouldn't normally be available on that manufacturer's weapons.

### License Types

| License | Effect | Slot |
|---------|--------|------|
| **Jakobs Ricochet** | Critical hits ricochet to nearby targets | Barrel Acc |
| **Hyperion Shield** | Weapon shield effect | Barrel Acc |
| **Hyperion Grip** | Accuracy bonuses | Grip |
| **Tediore Reload** | Throw weapon on reload (multiple variants) | Barrel Acc, Grip |
| **Torgue Mag** | Explosive magazine effect | Magazine |
| **Borg Mag** | Borg magazine bonus | Magazine |
| **Forge Mag** | COV repair mechanic | Magazine |
| **Atlas UB** | Atlas underbarrel attachment | Underbarrel |
| **Daedalus UB** | Daedalus underbarrel attachment | Underbarrel |
| **Maliwan UB** | Maliwan underbarrel attachment | Underbarrel |

### Tediore Reload Variants

Tediore licensed parts have multiple payload variants:

| Part | Effect |
|------|--------|
| `part_barrel_licensed_ted` | Default throw |
| `part_barrel_licensed_ted_combo` | Combo reload |
| `part_barrel_licensed_ted_mirv` | MIRV explosion |
| `part_barrel_licensed_ted_shooting` | Gun continues shooting |
| `part_barrel_licensed_ted_replicator` | Gun replicates |
| `part_barrel_licensed_ted_replicator_multi` | Multi-replicator |

### Weapon-Specific Underbarrels

Some weapons have tiered weapon-specific underbarrel parts:

- `LicensedPart_WeaponSpecific_UB_Tier0_Vladof` - Vladof-only tier 0
- `LicensedPart_WeaponSpecific_UB_Tier1` through `Tier4` - Tiered unlocks

### KL (Killer License) Parts

Some weapons have special `_KL` suffix parts:
- `DAD_PS_KL` - Daedalus Pistol killer license slot
- `JAK_PS_KL` - Jakobs Pistol killer license slot

These appear to be dedicated slots for licensed part effects.

## Level Gating (MinGameStage)

Parts and features are gated by player level using `Att_MinGameStage_*` attributes.
These control when parts can appear on dropped/purchased items.

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

### Licensed Part Level Gates

Each licensed part has its own MinGameStage attribute:

```
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

**Note**: The actual level values for these gates are not stored in NCS files.
They're likely defined in binary data tables or engine code.

## Unproven Aspects

1. **Level Gate Values**: The `Att_MinGameStage_*` attributes exist but the actual
   level thresholds (e.g., "Jakobs Ricochet unlocks at level 15") are not yet located.

2. **Rarity Filtering**: Parts have `firmware_weight_XX_rarity` values in NCS.
   It's unclear if common-weighted parts can appear on legendary items.

3. **License Availability**: We've found licensed parts appear on specific weapons
   (e.g., Borg SG can have Jakobs/Hyperion/Tediore licenses) but the complete
   mapping of which weapons can receive which licenses is incomplete.

4. **Per-Item Restrictions**: Legendaries may have additional restrictions
   via their compositions beyond what's in the base part list.

## Serial Index Investigation

Serial indices are required to encode/decode item serial numbers. Each part needs
a numeric index that gets packed into the bitstream.

### Investigation Results

**Finding: Serial indices are only partially stored in NCS files.**

We investigated multiple potential sources:

1. **NCS inv.bin - BOR parts**: BOR (Ripper) manufacturer parts have serial indices
   embedded as inline null-terminated strings directly after the part name:
   ```
   BOR_SG_Grip_01\0 42\0 part_grip_02\0 ...
   BOR_SG_Grip_02\0 43\0 part_grip_03\0 ...
   ```
   **36 BOR parts have inline indices** extracted via `bl4 ncs extract -t parts`.

2. **NCS inv.bin - Other manufacturers**: DAD, JAK, MAL, ORD, TED, TOR, VLA parts
   do NOT have inline serial indices. They go directly from part name to attributes:
   ```
   DAD_PS_Barrel_01\0 dad_ps_barrel_01_damage\0 DmgSrc_Gun_Pistol\0 ...
   ```

3. **usmap Schema**: Shows `GbxSerialNumberAwareDef` class with `SerialIndex` property.
   The `serialindex` field name exists in NCS schema but values aren't populated
   for most parts.

4. **PAK uassets**: Searched 81,517 .uasset files. Found ZERO assets of type
   `GbxSerialNumberAwareDef` or `InventoryPartDef`. Part definitions are native C++ objects.

### Conclusion

- **BOR parts (36)**: Serial indices CAN be extracted from NCS inv.bin
- **Other manufacturers (1578+ parts)**: Serial indices are assigned at runtime
  when the game engine registers parts with the `GbxSerialNumberProvider` system

Memory extraction remains the only complete source for all manufacturer indices.

### Data Extraction Commands

```bash
# Extract BOR parts with inline serial indices (36 parts)
bl4 ncs extract /path/to/inv4.bin -t parts --json

# Extract complete item-to-parts mapping (all 1614 parts, no indices)
bl4 ncs extract /path/to/inv4.bin -t item-parts --json
```

### Limitations

- Only 36/1614 parts (~2.2%) have NCS-extractable serial indices
- All extractable indices are from BOR manufacturer
- For complete serial encoding/decoding, memory extraction is required
- Index mappings may change between game versions
