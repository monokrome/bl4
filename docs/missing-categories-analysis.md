# Missing Part Categories Analysis

## Summary

This document tracks part category mappings in bl4. Categories are derived from serial analysis and memory dump extraction.

## Current Category Mappings (Implemented)

From `/crates/bl4/src/parts.rs` `category_name()` function:

### Weapons

| Range | Type | Categories |
|-------|------|------------|
| 2-7 | Pistols | DAD, JAK, TED, TOR, ORD, VLA |
| 8-12 | Shotguns | DAD, JAK, TED, TOR, BOR |
| 13-18 | Assault Rifles | DAD, JAK, TED, TOR, VLA, ORD |
| 19 | Maliwan Shotgun | MAL_SG ✓ |
| 20-23 | SMGs | DAD, BOR, VLA, MAL |
| 25 | Bor Sniper | bor_sr ✓ |
| 26-29 | Snipers | JAK, VLA, ORD, MAL |
| 244-247 | Heavy Weapons | VLA, TOR, BOR, MAL |

### Equipment

| Category | Type | Status |
|----------|------|--------|
| 44 | Dark Siren Class Mod | ✓ Named (no parts in dump) |
| 55 | Paladin Class Mod | ✓ Named (no parts in dump) |
| 97 | Gravitar Class Mod | ✓ Named + 2 parts mapped |
| 140 | Exo Soldier Class Mod | ✓ Named (no parts in dump) |
| 151 | Firmware | ✓ Named (no parts in dump) |
| 279-288 | Shields | energy, bor, dad, jak, armor, mal, ord, ted, tor, vla |
| 289 | Shield Variant | ✓ Named (unknown subtype) |
| 300-330 | Gadgets | grenade(300), turret(310), repair(320), terminal(330) |
| 400-409 | Enhancements | DAD, BOR, JAK, MAL, ORD, TED, TOR, VLA, COV, ATL |

## Implementation Status

### Completed (Jan 2026)

**Major breakthrough**: Discovered bit 7 flag in Part token indices indicates Root vs Sub scope:

1. **Bit 7 decoding implemented**:
   - For indices > 142, bit 7 = 0 means Root scope (core parts)
   - For indices > 142, bit 7 = 1 means Sub scope (modular attachments)
   - Actual part index is in lower 7 bits (index & 0x7F)

2. **Category derivation from NCS**:
   - Added `category_from_prefix()` function to bl4-ncs
   - Derives categories from manufacturer prefixes (BOR_SG → 12, JAK_SG → 9, etc.)
   - NCS extraction yields 38 manufacturer-prefixed parts with categories

3. **Improved serial decoding**:
   - Rainbow Vomit (Jakobs Legendary Shotgun) resolution: 30% → 70%
   - All decoded parts validated as correct Jakobs Shotgun parts
   - Successfully resolves indices 170, 166, 174, 196 by stripping bit 7

4. **Updated parts database**:
   - Merged NCS extraction with existing database
   - Total: 5,368 parts across multiple categories
   - Manufacturer weapon parts now include NCS-sourced data

### Completed (Dec 2025)

1. **Weapon categories filled**:
   - Category 19: MAL_SG (Maliwan Shotgun)
   - Category 25: bor_sr (Bor Sniper)

2. **Class mod categories identified** (4 player classes):
   - Category 44: Dark Siren Class Mod
   - Category 55: Paladin Class Mod
   - Category 97: Gravitar Class Mod
   - Category 140: Exo Soldier Class Mod

3. **Other equipment categories**:
   - Category 151: Firmware
   - Category 289: Shield Variant

4. **Code updated**:
   - `crates/bl4/src/parts.rs` - `category_name()` includes all categories
   - `crates/bl4-cli/src/main.rs` - `known_groups` includes classmod_gravitar
   - `share/manifest/parts_database.json` - Regenerated with 2,615 parts across 56 categories

5. **CLI enhanced**:
   - `bl4 serial decode` now shows Category name and ID in output

### Still Missing (Jan 2026)

**Known gaps after bit 7 discovery:**

1. **Rainbow Vomit test**: 7/10 parts resolved (70%)
   - Missing indices: 73, 78 (genuinely absent from database)
   - These may be legendary-specific parts not yet captured

2. **Non-prefixed parts**: Cannot derive categories from NCS
   - `comp_*` (rarity modifiers) - exist in multiple categories
   - `part_firmware_*` - no manufacturer prefix
   - `part_ra_*` - unknown categorization
   - Requires memory dump or other source for category mapping

3. **Part definitions not in parts_dump.json:**
   - `classmod_dark_siren.*` - No parts extracted
   - `classmod_paladin.*` - No parts extracted
   - `classmod_exo_soldier.*` - No parts extracted

4. **Equipment serials**: Still low resolution
   - Most class mod/firmware parts not in database
   - Index 254 appears frequently (likely end-of-data marker)

## Evidence Sources

### Player Classes (from pak_manifest.json)

Four playable characters confirmed:
- DarkSiren (DS_*)
- Paladin (PA_*)
- Gravitar (GR_*)
- ExoSoldier (EX_*)

### Category Derivation Formula

```text
Weapons (type 'r'):     category = first_varbit_token / 8192
Equipment (type 'e'):   category = first_varbit_token / 384
```

### Parts Found in Memory

Only `classmod_gravitar` has parts in the FName pool dump:
- `classmod_gravitar.part_grav_asm_legendaryGravitar`
- `classmod_gravitar.part_grav_asm_skill_test`

Other class mod parts may exist but weren't loaded when the memory dump was taken.

## Data Location

The authoritative source for category mappings is the `GbxSerialNumberIndex` structure:

```text
GbxSerialNumberIndex:
  Category  : Int64   <- Part Group ID
  scope     : Byte    <- Root/Sub scope
  status    : Byte    <- Active/Static/etc.
  Index     : Int16   <- Part index within group
```

**Problem**: Part definitions are compiled into game code and NOT present in GUObjectArray at runtime. Part names exist in FName pool but UObject instances are not registered.

## Future Work

To complete part mappings:

1. **Memory dump with class mods equipped** - Take dumps while characters have class mods in inventory to capture more part names

2. **Pak file parsing** - Extract InventoryPartDef assets directly from pak files (requires UE5 asset parsing)

3. **Manual mapping** - Decode known items and map part indices by comparison

## Verification

Run validation tests:
```bash
cargo test -p bl4 validate -- --nocapture
```

Decode a serial to see category:
```bash
bl4 serial decode '@Uge8;)m/&zJ!tkr0N4>8ns8H{t!6ljj'
# Output includes: Category: Paladin Class Mod (55)
```
