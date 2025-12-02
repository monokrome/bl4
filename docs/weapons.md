# Borderlands 4 Weapon Data

Extracted weapon part information from game assets. This data maps manufacturer codes, weapon types, and part naming conventions.

## Table of Contents

1. [Manufacturers](#manufacturers)
2. [Weapon Types](#weapon-types)
3. [Part Types](#part-types)
4. [Part Naming Convention](#part-naming-convention)
5. [Scope Parts by Manufacturer](#scope-parts-by-manufacturer)
6. [Known Legendaries](#known-legendaries)
7. [Rarity System](#rarity-system)
8. [Serial Index Mapping](#serial-index-mapping)

---

## Manufacturers

| Code | Name | Serial ID | Notes |
|------|------|-----------|-------|
| `BOR` | Borg | - | SMGs, Shotguns, Heavy Weapons, Sniper Rifles |
| `COV` | Children of the Vault | - | Various weapon types |
| `DAD` | Daedalus | 4 | ARs, SMGs, Pistols, Shotguns |
| `DPL` | Dahl | - | Turrets/Gadgets (not weapons) |
| `JAK` | Jakobs | 129 | High damage, semi-auto weapons across all types |
| `MAL` | Maliwan | 138 | Elemental weapons, energy-based |
| `ORD` | Order | 15 | ARs, Pistols, Sniper Rifles |
| `TED` | Tediore | 10 | Throwable reloads, ARs, Shotguns, Pistols |
| `TOR` | Torgue | 6 | Explosive weapons, gyrojet rounds |
| `VLA` | Vladof | 134 | High fire rate, ARs, SMGs, Heavy Weapons, Snipers |

**Serial ID** is the VarInt value that appears as the first token in decoded item serials.

---

## Weapon Types

| Code | Type | Description |
|------|------|-------------|
| `AR` | Assault Rifle | Full-auto/burst fire rifles |
| `HW` | Heavy Weapon | Launchers, miniguns |
| `PS` | Pistol | Semi-auto/full-auto handguns |
| `SG` | Shotgun | Spread weapons |
| `SM` | SMG | Submachine guns |
| `SR` | Sniper Rifle | Long-range precision weapons |

---

## Part Types

| Part | Description |
|------|-------------|
| `Barrel` | Main barrel, affects damage/fire rate |
| `Scope` | Optics, affects zoom/accuracy |
| `Mode` | Fire mode selector |
| `Grip` | Weapon grip (not fully enumerated) |
| `Stock` | Weapon stock (not fully enumerated) |
| `Underbarrel` | Under-barrel attachments |
| `Foregrip` | Front grip attachments |

---

## Part Naming Convention

Parts follow this naming pattern:

```
{MFG}_{TYPE}_{Part}_{Variant}_L{Level}_{SubVariant}
```

### Components

| Component | Description | Examples |
|-----------|-------------|----------|
| `MFG` | Manufacturer code | `DAD`, `JAK`, `TOR` |
| `TYPE` | Weapon type | `AR`, `SG`, `PS` |
| `Part` | Part category | `Scope`, `Barrel` |
| `Variant` | Part variant number | `01`, `02` |
| `Level` | Part tier/level | `L1`, `L2`, `LB` (legendary?) |
| `SubVariant` | Sub-variant/texture | `01`, `02`, `Mask`, `Texture` |

### Examples

| Part Name | Breakdown |
|-----------|-----------|
| `DAD_AR_Scope_01_L1_01` | Daedalus AR Scope, variant 1, level 1, sub-variant 1 |
| `TOR_SG_Scope_02_L2_Mask` | Torgue Shotgun Scope, variant 2, level 2, mask asset |
| `JAK_PS_Scope_01_L2_03` | Jakobs Pistol Scope, variant 1, level 2, sub-variant 3 |
| `VLA_SR_Scope_01_L2_ADSMask` | Vladof Sniper Scope, variant 1, level 2, ADS mask |

---

## Scope Parts by Manufacturer

### BOR (Borg)

**Heavy Weapons:**
- `BOR_HW_Scope_Barrel_01`
- `BOR_HW_Scope_Barrel_02`

**Shotguns:**
- `BOR_SG_Scope_01_L1` (variants 001-009)
- `BOR_SG_Scope_01_L2` (variants 002-008)
- `BOR_SG_Scope_02_L1` (variants 001-008)
- `BOR_SG_Scope_02_L2` (variants 001-005)
- `BOR_SG_Scope_Irons_001`

**SMGs:**
- `BOR_SM_Scope_01_L1` (variants 009-010)
- `BOR_SM_Scope_01_L2` (variants 001-011)
- `BOR_SM_Scope_02_L1` (beam variants 1-8, LED, Reticle)
- `BOR_SM_Scope_02_L2` (variants 001-007, beam variants)

**Sniper Rifles:**
- `BOR_SR_Scope_01_L1` (variants 001-014, VFX)
- `BOR_SR_Scope_01_L2` (variants 001-002, beam variants)
- `BOR_SR_Scope_02_L1` (variants 002-009)
- `BOR_SR_Scope_02_L2` (variants 001, beam variants)

### DAD (Daedalus)

**Assault Rifles:**
- `DAD_AR_Scope_01_L1` (variants 01-09, BracketFix)
- `DAD_AR_Scope_01_L2` (variants 01-06)
- `DAD_AR_Scope_02_L1` (variants 01, 08)
- `DAD_AR_Scope_02_L2` (variants 01-09)

**Pistols:**
- `DAD_PS_Scope_01_L1` (variant 01)
- `DAD_PS_Scope_01_L2` (variants 01-08)
- `DAD_PS_Scope_02_L1` (variants 01-04)
- `DAD_PS_Scope_02_L2` (variants 02-03)

**Shotguns:**
- `DAD_SG_Scope_01_L1` (variants 01-02)
- `DAD_SG_Scope_01_L2` (variants 02, 04)
- `DAD_SG_Scope_02_L1` (variants 01-02)
- `DAD_SG_Scope_02_L2` (variants 01-03)

**SMGs:**
- `DAD_SM_Scope_01_L1` (variants 01-03)
- `DAD_SM_Scope_01_L2` (variants 01-08, 011-015)
- `DAD_SM_Scope_02_L1` (variant 01)
- `DAD_SM_Scope_02_LB` (variants 01-03, possibly legendary barrel)

### JAK (Jakobs)

**Assault Rifles:**
- `JAK_AR_Scope_01_L1` (variants 01-03)
- `JAK_AR_Scope_01_L2` (variants 01-03)
- `JAK_AR_Scope_02_L1` (variants 01-03)
- `JAK_AR_Scope_02_L2` (variants 01-02)

**Pistols:**
- `JAK_PS_Scope_01_L1` (variants 01-03)
- `JAK_PS_Scope_01_L2` (variants 01-03)
- `JAK_PS_Scope_02_L1` (variants 01-03)
- `JAK_PS_Scope_02_L2` (variants 01-03)

**Shotguns:**
- `JAK_SG_Scope_01_L1` (variant 01)
- `JAK_SG_Scope_01_L2` (variant 02)
- `JAK_SG_Scope_02_L1` (variants 01, 03)
- `JAK_SG_Scope_02_L2` (variant 01)

**Sniper Rifles:**
- `JAK_SR_Scope_01_L1` (variants 01-03, ADSMask)
- `JAK_SR_Scope_01_L2` (variant 01)
- `JAK_SR_Scope_02_L1` (variant 01)
- `JAK_SR_Scope_02_L2` (variants 01-02)

### MAL (Maliwan)

**Heavy Weapons:**
- `MAL_HW_Scope_01` (variants 01-07, Proje)
- `MAL_HW_Scope_02` (variants 01-02, Proje, Shield)

**Shotguns:**
- `MAL_SG_Scope_01_L1` (variant 01)
- `MAL_SG_Scope_01_L2` (variant 01)
- `MAL_SG_Scope_02_L1` (Base variants)
- `MAL_SG_Scope_02_L2` (variants 01-06, ADSMask, Projes)

**SMGs:**
- `MAL_SM_Scope_01_L1` (variants 01-05, Base, Proje)
- `MAL_SM_Scope_01_L2` (variants 01-07)
- `MAL_SM_Scope_02_L1` (variants 01-03)
- `MAL_SM_Scope_02_L2` (variants 01-02)

**Sniper Rifles:**
- `MAL_SR_Scope_01_L1` (variants 01-08, ADSMask, Proje)
- `MAL_SR_Scope_01_L2` (variants 01-07, ADSMask)
- `MAL_SR_Scope_02_L1` (variant 02, ADSMask, Proje)
- `MAL_SR_Scope_02_L2` (variants 01-03)

### ORD (Order)

**Assault Rifles:**
- `ORD_AR_Scope_01_L1` (variants 01, 002)
- `ORD_AR_Scope_01_L2` (variant 01)
- `ORD_AR_Scope_02_L2` (with Mask)

**Pistols:**
- `ORD_PS_Scope_01_L1` (variant 01)
- `ORD_PS_Scope_01_L2` (variants 01-02)
- `ORD_PS_Scope_02_L1` (variant 01, ADSMask)
- `ORD_PS_Scope_02_L2` (variants 01-03, ADSMask)

**Sniper Rifles:**
- `ORD_SR_Scope_01_L1` (variant 01)
- `ORD_SR_Scope_01_L2` (variant 01, ADSMask)
- `ORD_SR_Scope_02_L1` (variants 01-02, ADSMask)
- `ORD_SR_Scope_02_L2` (variants 01-02, 002, ADSMask)

### TED (Tediore)

**Assault Rifles:**
- `TED_AR_Scope_01_L1` (variants 01-03, Elements)
- `TED_AR_Scope_01_L2` (variants 01-04)
- `TED_AR_Scope_02_L1` (ModA, Mask)
- `TED_AR_Scope_02_L2` (variants 01, 04)

**Pistols:**
- `TED_PS_Scope_01_L1` (variants 01-06)
- `TED_PS_Scope_01_L2` (variants 01-02)
- `TED_PS_Scope_02_L1` (variants 01-04, Line)
- `TED_PS_Scope_02_L2` (variants 01-06)

**Shotguns:**
- `TED_SG_Scope_01_L1` (variants 01-04, 010, 012)
- `TED_SG_Scope_01_L2` (variants 01-02, ADSMask)
- `TED_SG_Scope_02_L1` (variants 01, 04, ModB02)
- `TED_SG_Scope_02_L2` (variant 01, ModB, Proje)

### TOR (Torgue)

**Assault Rifles:**
- `TOR_AR_Scope_01_L1` (variants 01-06)
- `TOR_AR_Scope_01_L2` (variants 01-05)
- `TOR_AR_Scope_02_L1` (variants 01-04)
- `TOR_AR_Scope_02_L2` (variants 01-04, ModB)

**Heavy Weapons:**
- `TOR_HW_Scope_01` (variants 01-02)
- `TOR_HW_Scope_02` (variant 01)
- `TOR_HW_Barrel_01` / `TOR_HW_Barrel_02` (with Mask)

**Pistols:**
- `TOR_PS_Scope_01_L1` (variants 01-03, ModB)
- `TOR_PS_Scope_01_L2` (variants 01-03)
- `TOR_PS_Scope_02_L1` (variants 01-04, Elements_Updated)
- `TOR_PS_Scope_02_L2` (variant 01)

**Shotguns:**
- `TOR_SG_Scope_01_L1` (variants 01-06)
- `TOR_SG_Scope_01_L2` (variants 01-03, B, Compass)
- `TOR_SG_Scope_02_L1` (variant 01)
- `TOR_SG_Scope_02_L2` (variants 01-04)

### VLA (Vladof)

**Assault Rifles:**
- `VLA_AR_Scope_01_L1` (variants 01-04, BASE)
- `VLA_AR_Scope_01_L2` (variants 01-03)
- `VLA_AR_Scope_02_L1` (variant 01)
- `VLA_AR_Scope_02_L2` (variants 01-02)

**Heavy Weapons:**
- `VLA_HW_Scope_01` (variants 01-04)
- `VLA_HW_Scope_02` (variants 01-05, ADSMask)
- `VLA_HW_Barrel_02` (with Mask)

**SMGs:**
- `VLA_SM_Scope_01_L1` (variants 01-08, B, BASE)
- `VLA_SM_Scope_01_L2` (variant 01)
- `VLA_SM_Scope_02_L1` (variants 01-02)
- `VLA_SM_Scope_02_L2` (variant 01, Mask, Triangles, Tris)

**Sniper Rifles:**
- `VLA_SR_Scope_01_L1` (variants 01-07)
- `VLA_SR_Scope_01_L2` (variants 01-05, ADSMask)
- `VLA_SR_Scope_02_L1` (variants 01, 03)
- `VLA_SR_Scope_02_L2` (variants 01-03)

---

## Known Legendaries

### By Manufacturer-Weapon Combo

| ID | Internal Name | Type |
|----|---------------|------|
| `BOR_SM.comp_05_legendary_p` | Unknown legendary SMG | SMG |
| `TED_SG.comp_05_legendary_a` | Unknown legendary Shotgun | Shotgun |
| `TOR_Linebacker` | Linebacker | Shotgun |
| `DAD_AR_Lumberjack` | Lumberjack | Assault Rifle |

### From Previous Documentation

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

## Rarity System

| Code | Tier | Color |
|------|------|-------|
| `comp_01` | Common | White |
| `comp_02` | Uncommon | Green |
| `comp_03` | Rare | Blue |
| `comp_04` | Epic | Purple |
| `comp_05` | Legendary | Orange |

### Internal Rarity Format

```
{MFG}_{TYPE}.comp_0{N}_{rarity}_{name}
```

Examples:
- `TOR_SG.comp_05_legendary_Linebacker`
- `BOR_SM.comp_05_legendary_p`

---

## Barrel Parts

### BOR Heavy Weapons

| Part | Notes |
|------|-------|
| `BOR_HW_Barrel_01` | Variants 001-010 |
| `BOR_HW_Barrel_02` | Variants 001-005, A variant |
| `BOR_HW_Barrel_1` | Variants 001-005 |

### MAL Heavy Weapons

| Part | Notes |
|------|-------|
| `MAL_HW_Barrel_02_MIRV` | MIRV launcher variant |

### TOR Heavy Weapons

| Part | Notes |
|------|-------|
| `TOR_HW_Barrel_01` | With Mask |
| `TOR_HW_Barrel_02` | With Mask, Splitter variant |

### VLA Heavy Weapons

| Part | Notes |
|------|-------|
| `VLA_HW_Barrel_02` | With Mask |

---

## Notes

- **Part Levels**: `L1` and `L2` appear to represent different part tiers
- **LB suffix**: May indicate legendary/unique barrel variants
- **Mask/ADSMask**: Texture masks for ADS (aim down sights) rendering
- **Proje/Projes**: Projectile-related assets
- **ModA/ModB**: Alternative model variants
- **Elements**: Elemental effect variants

---

---

## Serial Index Mapping

### Current Status

Item serials contain Part tokens like `{8}`, `{14}`, `{252:4}` that reference weapon parts. The mapping between these indices and actual part asset names (like `DAD_AR_Scope_01_L1`) has not yet been fully decoded.

### Decoded Serial Structure

From analyzing weapon serials, the format is:

```
Magic Header (7 bits: 0010000)
├── VarBit: Item ID/Seed (large value like 180928)
├── Separator
├── VarBit: Level (49-51 range)
├── Separator
├── Part tokens: {index} or {index:value}
├── VarInt values
├── Strings (encoded names/data)
└── More Part tokens
```

**Example Decoded Weapon:**
```
Serial: @Ugr$ZCm/&(L!f36aLp/zzG}*+OOac
Decoded: 180928 | 51 | {0:1} | 9 0 , 4 , 1943331 "Y'ecz" , , , , ,
```

### Part Token Examples Found

| Token | Notes |
|-------|-------|
| `{0:1}` | Common across weapons, likely a base part |
| `{126:211}` | Part index 126 with value 211 |
| `{168}` | Part index with no value |
| `{2325:3}` | High index with small value |

### What We Know

- Parts are encoded as `{index}` or `{index:value}`
- Index values range from 0 to 2000+
- The "value" component may represent variant, level, or state
- Part names extracted from game files (670+ unique patterns) don't directly map to indices
- The mapping likely exists in UE5 DataTable assets or compiled into the binary

### Next Steps for Index Mapping

1. **Empirical Testing** (Most Practical)
   - Create test weapons in-game with different parts
   - Decode their serials and track which index changes
   - Build correlation between index values and part names

2. **Static Binary Analysis**
   - Use radare2/Ghidra on `Borderlands4.exe`
   - Search for part name strings referenced by index
   - Find the serialization/deserialization routines

### Tools Status

| Tool | Status | Use |
|------|--------|-----|
| `retoc` | ✅ Installed | IoStore extraction, asset unpacking |
| `uextract` | ✅ Created | UE5 asset parsing with property extraction |
| `radare2` | ✅ Available | Binary analysis (careful with DRM!) |
| `bl4-research` | ✅ Created | Project research tools in `crates/bl4-research` |

### Asset Extraction Results

Using `retoc unpack`, we extracted 19,623 files from pakchunk4 including:
- Weapon meshes and textures
- Animation assets (fire modes, etc.)
- Scope/barrel visual assets

The `.uasset` files are in UE5 Zen format and can be parsed with uextract.

---

## Extracted Part Names

The following parts were extracted from game memory dumps. These represent the visual/mesh parts but not their serial indices.

### Part Count by Manufacturer

| Manufacturer | Parts |
|--------------|-------|
| BOR | 60 |
| DAD | 79 |
| JAK | 99 |
| MAL | 84 |
| ORD | 59 |
| TED | 62 |
| TOR | 66 |
| VLA | 68 |

### Full Part List

See `.tmp/dump_parts.txt` for the complete list of 532 extracted part names.

Common patterns observed:
- `{MFG}_{TYPE}_{Part}_{Variant}` (e.g., `DAD_AR_Barrel_01`)
- TED grip variants with FX suffixes (e.g., `TED_AR_Grip_05_B_FX_TED_Homing`)
- Maliwan mode switches (e.g., `MAL_SR_Modeswitch_03_Rockets`)

---

*Extracted from BL4 game assets using retoc IoStore extraction*
