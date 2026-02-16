# Appendix B: Weapon Parts Reference

This appendix catalogs all known weapon parts extracted from BL4 game files, organized by manufacturer and weapon type.

---

## Part Naming Convention

Parts follow this pattern:

```text
{MFG}_{TYPE}_{Part}_{Variant}_L{Level}_{SubVariant}
```

| Component | Description | Examples |
|-----------|-------------|----------|
| MFG | Manufacturer code | DAD, JAK, TOR |
| TYPE | Weapon type | AR, SG, PS, SM, SR, HW |
| Part | Part category | Scope, Barrel, Grip |
| Variant | Part variant | 01, 02 |
| Level | Part tier | L1, L2, LB (legendary) |
| SubVariant | Sub-variant | 01, Mask, ADSMask |

---

## Manufacturers

| Code | Name | Serial ID | Weapon Types |
|------|------|-----------|--------------|
| BOR | Ripper | - | SM, SG, HW, SR |
| COV | Children of the Vault | - | Various |
| DAD | Daedalus | 4 | AR, SM, PS, SG |
| DPL | Dahl | - | Turrets/Gadgets only |
| JAK | Jakobs | 129 | AR, PS, SG, SR |
| MAL | Maliwan | 138 | SM, SG, HW, SR |
| ORD | Order | 15 | AR, PS, SR |
| TED | Tediore | 10 | AR, SG, PS |
| TOR | Torgue | 6 | AR, SG, HW, PS |
| VLA | Vladof | 134 | AR, SM, HW, SR, PS |

**Serial ID** is the first VarInt in decoded item serials.

---

## Weapon Types

### Type Codes

| Code | Type | Description |
|------|------|-------------|
| AR | Assault Rifle | Full-auto/burst rifles |
| HW | Heavy Weapon | Launchers, miniguns |
| PS | Pistol | Handguns |
| SG | Shotgun | Spread weapons |
| SM | SMG | Submachine guns |
| SR | Sniper Rifle | Precision weapons |

### EWeaponType Enum

Internal weapon type enumeration (from usmap):

| Value | Type |
|-------|------|
| 0 | None |
| 1 | Pistol |
| 2 | SMG |
| 3 | Shotgun |
| 4 | AssaultRifle |
| 5 | Sniper |
| 6 | Heavy |
| 7 | Count |

---

## Scope Parts by Manufacturer

### BOR (Ripper)

**Heavy Weapons:**

| Part | Variants |
|------|----------|
| BOR_HW_Scope_Barrel_01 | Base |
| BOR_HW_Scope_Barrel_02 | Base |

**Shotguns:**

| Part | Variants |
|------|----------|
| BOR_SG_Scope_01_L1 | 001-009 |
| BOR_SG_Scope_01_L2 | 002-008 |
| BOR_SG_Scope_02_L1 | 001-008 |
| BOR_SG_Scope_02_L2 | 001-005 |
| BOR_SG_Scope_Irons | 001 |

**SMGs:**

| Part | Variants |
|------|----------|
| BOR_SM_Scope_01_L1 | 009-010 |
| BOR_SM_Scope_01_L2 | 001-011 |
| BOR_SM_Scope_02_L1 | beam 1-8, LED, Reticle |
| BOR_SM_Scope_02_L2 | 001-007, beam |

**Sniper Rifles:**

| Part | Variants |
|------|----------|
| BOR_SR_Scope_01_L1 | 001-014, VFX |
| BOR_SR_Scope_01_L2 | 001-002, beam |
| BOR_SR_Scope_02_L1 | 002-009 |
| BOR_SR_Scope_02_L2 | 001, beam |

---

### DAD (Daedalus)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| DAD_AR_Scope_01_L1 | 01-09, BracketFix |
| DAD_AR_Scope_01_L2 | 01-06 |
| DAD_AR_Scope_02_L1 | 01, 08 |
| DAD_AR_Scope_02_L2 | 01-09 |

**Pistols:**

| Part | Variants |
|------|----------|
| DAD_PS_Scope_01_L1 | 01 |
| DAD_PS_Scope_01_L2 | 01-08 |
| DAD_PS_Scope_02_L1 | 01-04 |
| DAD_PS_Scope_02_L2 | 02-03 |

**Shotguns:**

| Part | Variants |
|------|----------|
| DAD_SG_Scope_01_L1 | 01-02 |
| DAD_SG_Scope_01_L2 | 02, 04 |
| DAD_SG_Scope_02_L1 | 01-02 |
| DAD_SG_Scope_02_L2 | 01-03 |

**SMGs:**

| Part | Variants |
|------|----------|
| DAD_SM_Scope_01_L1 | 01-03 |
| DAD_SM_Scope_01_L2 | 01-08, 011-015 |
| DAD_SM_Scope_02_L1 | 01 |
| DAD_SM_Scope_02_LB | 01-03 (legendary barrel) |

---

### JAK (Jakobs)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| JAK_AR_Scope_01_L1 | 01-03 |
| JAK_AR_Scope_01_L2 | 01-03 |
| JAK_AR_Scope_02_L1 | 01-03 |
| JAK_AR_Scope_02_L2 | 01-02 |

**Pistols:**

| Part | Variants |
|------|----------|
| JAK_PS_Scope_01_L1 | 01-03 |
| JAK_PS_Scope_01_L2 | 01-03 |
| JAK_PS_Scope_02_L1 | 01-03 |
| JAK_PS_Scope_02_L2 | 01-03 |

**Shotguns:**

| Part | Variants |
|------|----------|
| JAK_SG_Scope_01_L1 | 01 |
| JAK_SG_Scope_01_L2 | 02 |
| JAK_SG_Scope_02_L1 | 01, 03 |
| JAK_SG_Scope_02_L2 | 01 |

**Sniper Rifles:**

| Part | Variants |
|------|----------|
| JAK_SR_Scope_01_L1 | 01-03, ADSMask |
| JAK_SR_Scope_01_L2 | 01 |
| JAK_SR_Scope_02_L1 | 01 |
| JAK_SR_Scope_02_L2 | 01-02 |

---

### MAL (Maliwan)

**Heavy Weapons:**

| Part | Variants |
|------|----------|
| MAL_HW_Scope_01 | 01-07, Proje |
| MAL_HW_Scope_02 | 01-02, Proje, Shield |

**Shotguns:**

| Part | Variants |
|------|----------|
| MAL_SG_Scope_01_L1 | 01 |
| MAL_SG_Scope_01_L2 | 01 |
| MAL_SG_Scope_02_L1 | Base |
| MAL_SG_Scope_02_L2 | 01-06, ADSMask, Projes |

**SMGs:**

| Part | Variants |
|------|----------|
| MAL_SM_Scope_01_L1 | 01-05, Base, Proje |
| MAL_SM_Scope_01_L2 | 01-07 |
| MAL_SM_Scope_02_L1 | 01-03 |
| MAL_SM_Scope_02_L2 | 01-02 |

**Sniper Rifles:**

| Part | Variants |
|------|----------|
| MAL_SR_Scope_01_L1 | 01-08, ADSMask, Proje |
| MAL_SR_Scope_01_L2 | 01-07, ADSMask |
| MAL_SR_Scope_02_L1 | 02, ADSMask, Proje |
| MAL_SR_Scope_02_L2 | 01-03 |

---

### ORD (Order)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| ORD_AR_Scope_01_L1 | 01, 002 |
| ORD_AR_Scope_01_L2 | 01 |
| ORD_AR_Scope_02_L2 | Mask |

**Pistols:**

| Part | Variants |
|------|----------|
| ORD_PS_Scope_01_L1 | 01 |
| ORD_PS_Scope_01_L2 | 01-02 |
| ORD_PS_Scope_02_L1 | 01, ADSMask |
| ORD_PS_Scope_02_L2 | 01-03, ADSMask |

**Sniper Rifles:**

| Part | Variants |
|------|----------|
| ORD_SR_Scope_01_L1 | 01 |
| ORD_SR_Scope_01_L2 | 01, ADSMask |
| ORD_SR_Scope_02_L1 | 01-02, ADSMask |
| ORD_SR_Scope_02_L2 | 01-02, 002, ADSMask |

---

### TED (Tediore)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| TED_AR_Scope_01_L1 | 01-03, Elements |
| TED_AR_Scope_01_L2 | 01-04 |
| TED_AR_Scope_02_L1 | ModA, Mask |
| TED_AR_Scope_02_L2 | 01, 04 |

**Pistols:**

| Part | Variants |
|------|----------|
| TED_PS_Scope_01_L1 | 01-06 |
| TED_PS_Scope_01_L2 | 01-02 |
| TED_PS_Scope_02_L1 | 01-04, Line |
| TED_PS_Scope_02_L2 | 01-06 |

**Shotguns:**

| Part | Variants |
|------|----------|
| TED_SG_Scope_01_L1 | 01-04, 010, 012 |
| TED_SG_Scope_01_L2 | 01-02, ADSMask |
| TED_SG_Scope_02_L1 | 01, 04, ModB02 |
| TED_SG_Scope_02_L2 | 01, ModB, Proje |

---

### TOR (Torgue)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| TOR_AR_Scope_01_L1 | 01-06 |
| TOR_AR_Scope_01_L2 | 01-05 |
| TOR_AR_Scope_02_L1 | 01-04 |
| TOR_AR_Scope_02_L2 | 01-04, ModB |

**Heavy Weapons:**

| Part | Variants |
|------|----------|
| TOR_HW_Scope_01 | 01-02 |
| TOR_HW_Scope_02 | 01 |
| TOR_HW_Barrel_01 | Mask |
| TOR_HW_Barrel_02 | Mask, Splitter |

**Pistols:**

| Part | Variants |
|------|----------|
| TOR_PS_Scope_01_L1 | 01-03, ModB |
| TOR_PS_Scope_01_L2 | 01-03 |
| TOR_PS_Scope_02_L1 | 01-04, Elements_Updated |
| TOR_PS_Scope_02_L2 | 01 |

**Shotguns:**

| Part | Variants |
|------|----------|
| TOR_SG_Scope_01_L1 | 01-06 |
| TOR_SG_Scope_01_L2 | 01-03, B, Compass |
| TOR_SG_Scope_02_L1 | 01 |
| TOR_SG_Scope_02_L2 | 01-04 |

---

### VLA (Vladof)

**Assault Rifles:**

| Part | Variants |
|------|----------|
| VLA_AR_Scope_01_L1 | 01-04, BASE |
| VLA_AR_Scope_01_L2 | 01-03 |
| VLA_AR_Scope_02_L1 | 01 |
| VLA_AR_Scope_02_L2 | 01-02 |

**Heavy Weapons:**

| Part | Variants |
|------|----------|
| VLA_HW_Scope_01 | 01-04 |
| VLA_HW_Scope_02 | 01-05, ADSMask |
| VLA_HW_Barrel_02 | Mask |

**SMGs:**

| Part | Variants |
|------|----------|
| VLA_SM_Scope_01_L1 | 01-08, B, BASE |
| VLA_SM_Scope_01_L2 | 01 |
| VLA_SM_Scope_02_L1 | 01-02 |
| VLA_SM_Scope_02_L2 | 01, Mask, Triangles |

**Sniper Rifles:**

| Part | Variants |
|------|----------|
| VLA_SR_Scope_01_L1 | 01-07 |
| VLA_SR_Scope_01_L2 | 01-05, ADSMask |
| VLA_SR_Scope_02_L1 | 01, 03 |
| VLA_SR_Scope_02_L2 | 01-03 |

---

## Barrel Parts

### Heavy Weapons

| Manufacturer | Part | Notes |
|--------------|------|-------|
| BOR | BOR_HW_Barrel_01 | 001-010 |
| BOR | BOR_HW_Barrel_02 | 001-005, A variant |
| MAL | MAL_HW_Barrel_02_MIRV | MIRV launcher |
| TOR | TOR_HW_Barrel_01 | With Mask |
| TOR | TOR_HW_Barrel_02 | Splitter variant |
| VLA | VLA_HW_Barrel_02 | With Mask |

---

## Part Index Organization

### The Self-Describing Design

A critical discovery: **parts don't have external indices assigned to them—each part stores its own index internally**.

Every part UObject contains a `GbxSerialNumberIndex` at offset +0x28:

```text
Part UObject + 0x28:
├── Scope (1 byte)   ← Always 2 for inventory parts
├── Status (1 byte)  ← Reserved
└── Index (2 bytes)  ← This part's serial index
```

There is no separate "part → index" mapping file. The mapping IS the parts themselves. This "reverse mapping" design means:

- Each part is self-describing and carries its own identity
- Adding new parts (e.g., DLC) doesn't require updating a central registry
- Indices are guaranteed stable because they're intrinsic to each part
- Memory extraction gives us authoritative data, not a derived mapping

### How Indices Are Assigned

Part indices within each category are assigned based on the game's internal registration order, **not alphabetically**. Understanding this is critical for correctly decoding and encoding item serials.

### Registration Order Pattern

Parts appear to be registered in groups by functional type:

| Order | Part Type | Example |
|-------|-----------|---------|
| 1 | Unique/special variants | `part_barrel_01_zipgun` |
| 2 | Body parts | `part_body`, `part_body_a-d` |
| 3 | Base barrels | `part_barrel_01`, `part_barrel_02` |
| 4 | Shield/defensive | `part_shield_default`, `part_shield_ricochet` |
| 5 | Magazines | `part_mag_01`, `part_mag_02` |
| 6 | Scopes | `part_scope_ironsight`, `part_scope_01_*` |
| 7 | Grips | `part_grip_01`, `part_grip_02` |
| 8 | Underbarrel/foregrip | `part_underbarrel_*`, `part_foregrip_*` |
| 9 | Body magazines | `part_body_mag_smg`, `part_body_mag_ar` |
| 10 | Barrel variants | `part_barrel_01_a-d`, `part_barrel_02_a-d` |
| 11 | Licensed parts | `part_barrel_licensed_jak`, `part_barrel_licensed_ted` |

### Index Gaps

Some categories have non-contiguous indices. For example, a category might have parts at indices 1-53, skip 54-56, then continue at 57. These gaps may represent:

- Reserved slots for future DLC parts
- Parts that were removed during development
- Internal versioning or compatibility placeholders

### Implications for Modding

When working with item serials:

1. **Never assume alphabetical order** — Part `part_barrel_01` might have index 7, not index 0
2. **Use runtime-extracted data** — Only memory dumps capture the true `GbxSerialNumberIndex` values
3. **Validate against known items** — Decode existing item serials to verify index mappings
4. **Account for gaps** — Don't assume contiguous indices when iterating

---

## Part Compatibility Rules (Verified)

This section documents **verified** part compatibility rules derived from analyzing actual weapon drops and reference data. These rules are encoded in the part naming conventions and enforced by the game engine.

### Rule 1: Prefix Determines Weapon Type

Parts are only valid for weapons matching their prefix:

| Prefix | Manufacturer | Weapon Type |
|--------|--------------|-------------|
| `DAD_PS.*` | Daedalus | Pistol |
| `DAD_AR.*` | Daedalus | Assault Rifle |
| `JAK_SG.*` | Jakobs | Shotgun |
| `VLA_SM.*` | Vladof | SMG |

A `DAD_PS.part_barrel_01` **cannot** appear on a `DAD_AR` weapon.

### Rule 2: Barrel Accessories Require Matching Barrel

Barrel accessories are **barrel-specific**. The naming convention encodes the dependency:

```text
part_barrel_XX_Y  →  only valid with part_barrel_XX
```

| Accessory Part | Valid With | Invalid With |
|----------------|------------|--------------|
| `DAD_PS.part_barrel_01_a` | `DAD_PS.part_barrel_01` | `DAD_PS.part_barrel_02` |
| `DAD_PS.part_barrel_01_b` | `DAD_PS.part_barrel_01` | `DAD_PS.part_barrel_02` |
| `DAD_PS.part_barrel_02_a` | `DAD_PS.part_barrel_02` | `DAD_PS.part_barrel_01` |
| `DAD_PS.part_barrel_02_c` | `DAD_PS.part_barrel_02` | `DAD_PS.part_barrel_01` |

**Pattern**: Extract the barrel number from accessory name (`barrel_XX_*`) and match to base barrel (`barrel_XX`).

### Rule 3: Scope Accessories Require Matching Scope AND Lens

Scope accessories have a **two-dimensional dependency** on both scope type and lens type:

```text
part_scope_acc_sXX_lYY_Z  →  only valid with part_scope_XX_lens_YY
```

| Accessory Part | Valid With | Invalid With |
|----------------|------------|--------------|
| `DAD_PS.part_scope_acc_s01_l01_a` | `part_scope_01_lens_01` | `part_scope_01_lens_02`, `part_scope_02_*` |
| `DAD_PS.part_scope_acc_s01_l02_b` | `part_scope_01_lens_02` | `part_scope_01_lens_01`, `part_scope_02_*` |
| `DAD_PS.part_scope_acc_s02_l01_a` | `part_scope_02_lens_01` | `part_scope_01_*`, `part_scope_02_lens_02` |
| `DAD_PS.part_scope_acc_s02_l02_b` | `part_scope_02_lens_02` | `part_scope_01_*`, `part_scope_02_lens_01` |

**Pattern**: Parse `sXX` and `lYY` from accessory name, match to `scope_XX_lens_YY`.

### Rule 4: Some Magazine Modifiers Are Barrel-Specific

Certain magazine stat modifiers are tied to specific barrels:

```text
part_mag_*_barrel_XX  →  only valid with part_barrel_XX
```

| Modifier Part | Valid With |
|---------------|------------|
| `DAD_PS.part_mag_05_borg_barrel_01` | `part_barrel_01` + Ripper Mag |
| `DAD_PS.part_mag_05_borg_barrel_02` | `part_barrel_02` + Ripper Mag |

### Rule 5: Maximum Accessory Counts

Weapons have limits on how many accessories can appear:

| Constraint | Limit |
|------------|-------|
| Total accessories per weapon | Max 3 |
| Body accessories | Choose 1-2 |
| Barrel accessories | Choose 2-3 |
| Scope accessories | Not all scopes support 2 |

### Rule 6: Legendary Barrel Restrictions

Some legendary barrels **cannot** roll with barrel accessories:

| Barrel | Accessory Restriction |
|--------|----------------------|
| `JAK_SG.part_barrel_hot_slugger` | Never rolls with barrel accessories |
| `JAK_SG.part_barrel_hellwalker` | Doesn't roll with barrel accessories |
| Unique barrels (general) | Often have restricted accessory pools |

### Rule 7: Barrel-Type Constraints for Magazines

Some magazines are only valid for specific barrel configurations:

| Magazine | Valid Barrel Type |
|----------|-------------------|
| `JAK_SG` 6x mag (single barrel) | Single-barrel weapons |
| `JAK_SG` 6x mag (double barrel) | Double-barrel weapons |

### Validation Algorithm

To validate a weapon's part combination:

```python
def is_valid_combination(parts):
    barrel = find_barrel(parts)
    scope = find_scope(parts)

    for part in parts:
        # Rule 1: Prefix match
        if not same_prefix(part, barrel):
            return False

        # Rule 2: Barrel accessory match
        if is_barrel_accessory(part):
            if not matches_barrel(part, barrel):
                return False

        # Rule 3: Scope accessory match
        if is_scope_accessory(part):
            if not matches_scope_and_lens(part, scope):
                return False

        # Rule 4: Barrel-specific mag modifier
        if is_barrel_mag_modifier(part):
            if not matches_barrel(part, barrel):
                return False

    # Rule 5: Accessory count limits
    if count_accessories(parts) > 3:
        return False

    return True
```

### Implications for Save Editing

When generating weapon serials for testing:

1. **Don't mix accessories** — A `barrel_01` weapon cannot have `barrel_02_a` accessory
2. **Match scope accessories** — Check both scope type AND lens type
3. **Respect legendary restrictions** — Some unique barrels have no accessories
4. **Count accessories** — Stay within the 3-accessory limit

---

## Part Selection System (Theoretical)

::: {.callout-note title="Speculative"}
This section describes classes found in game metadata. Actual implementation may differ.
The "Part Compatibility Rules" section above contains verified behavior.
:::

### Class-Based Selection (from usmap)

The following classes exist in the game's type system, suggesting a structured selection mechanism:

| Class | Likely Purpose |
|-------|----------------|
| `PartTypeSelectionRules` | Rules for selecting parts by type |
| `PartTagSelectionRules` | Tag-based part filtering |
| `PartTagGameStageSelectionData` | Level requirements for parts |
| `InventorySelectionCriteria` | General selection criteria |

However, **no assets implementing these classes were found in pak files**. The selection logic may be:
- Hardcoded in C++ binary
- Convention-based (parsed from naming)
- Or a combination of both

---

## Part Slot Types

From `EWeaponPartValue` enum:

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

---

## Weapon Naming System

Parts affect weapon prefix names through the naming table system.

### Primary Indices

| Index | Stat Type | Example Prefixes |
|-------|-----------|------------------|
| 2 | Damage | Tortuous, Agonizing, Festering |
| 3 | CritDamage | Bleeding, Hemorrhaging, Pooling |
| 4 | ReloadSpeed | Frenetic, Manic, Rotten |
| 5 | MagSize | Bloated, Gluttonous, Hoarding |
| 7 | body_mod_a | Chosen, Promised, Tainted |
| 8 | body_mod_b | Bestowed, Cursed, Offered |
| 9 | body_mod_c | Ritualized, Summoned |
| 10 | body_mod_d | Strange |
| 15-18 | barrel_mod | Herald, Harbinger, Oracle, Prophecy |

### Naming Indices (from WeaponNamingStruct)

| Field | Index | GUID |
|-------|-------|------|
| Damage | 2 | 9DFA8E9A4AF1B3A1... |
| CritDamage | 9 | C4432C8C40CA15F0... |
| FireRate | 10 | 459C49044DE26DE5... |
| ReloadSpeed | 11 | 61FAACA14D48B609... |
| MagSize | 12 | C735EA434D50CD82... |
| Accuracy | 13 | 5B35CC194CB71AE4... |
| ElementalPower | 14 | 842A58234E5D5D79... |
| ADSProficiency | 16 | 02D519604FE47BA5... |
| Single | 18 | 240AB1EB411BED6B... |
| DamageRadius | 21 | EE89495D493F3450... |

---

## Known Legendaries

### By Manufacturer

| Internal Name | Display Name | Type | Manufacturer |
|---------------|--------------|------|--------------|
| DAD_AR.comp_05_legendary_OM | OM | AR | Daedalus |
| DAD_SG.comp_05_legendary_HeartGUn | Heart Gun | SG | Daedalus |
| JAK_AR.comp_05_legendary_rowan | Rowan's Call | AR | Jakobs |
| JAK_PS.comp_05_legendary_kingsgambit | King's Gambit | PS | Jakobs |
| JAK_PS.comp_05_legendary_phantom_flame | Phantom Flame | PS | Jakobs |
| JAK_SR.comp_05_legendary_ballista | Ballista | SR | Jakobs |
| MAL_HW.comp_05_legendary_GammaVoid | Gamma Void | HW | Maliwan |
| MAL_SM.comp_05_legendary_OhmIGot | Ohm I Got | SM | Maliwan |
| TED_AR.comp_05_legendary_Chuck | Chuck | AR | Tediore |
| TED_PS.comp_05_legendary_Sideshow | Sideshow | PS | Tediore |
| TOR_HW.comp_05_legendary_ravenfire | Ravenfire | HW | Torgue |
| TOR_SG.comp_05_legendary_Linebacker | Linebacker | SG | Torgue |
| VLA_AR.comp_05_legendary_WomboCombo | Wombo Combo | AR | Vladof |
| VLA_HW.comp_05_legendary_AtlingGun | Atling Gun | HW | Vladof |
| VLA_SM.comp_05_legendary_KaoSon | Kaoson | SM | Vladof |

---

## Rarity System

| Code | Tier | Color |
|------|------|-------|
| comp_01 | Common | White |
| comp_02 | Uncommon | Green |
| comp_03 | Rare | Blue |
| comp_04 | Epic | Purple |
| comp_05 | Legendary | Orange |

### Internal Format

```text
{MFG}_{TYPE}.comp_0{N}_{rarity}_{name}
```

Examples:
- `TOR_SG.comp_05_legendary_Linebacker`
- `VLA_SM.comp_05_legendary_KaoSon`

---

## Part Count by Category

Data extracted from memory dump using `bl4 memory dump-parts` and `bl4 memory build-parts-db`.

### Weapons

| Category ID | Manufacturer | Weapon Type | Parts Count |
|-------------|--------------|-------------|-------------|
| 2 | Daedalus | Pistol | 74 |
| 3 | Jakobs | Pistol | 73 |
| 4 | Tediore | Pistol | 81 |
| 5 | Torgue | Pistol | 70 |
| 6 | Order | Pistol | 75 |
| 8 | Daedalus | Shotgun | 74 |
| 9 | Jakobs | Shotgun | 89 |
| 10 | Tediore | Shotgun | 76 |
| 11 | Torgue | Shotgun | 69 |
| 12 | Bor | Shotgun | 73 |
| 13 | Daedalus | Assault Rifle | 78 |
| 14 | Jakobs | Assault Rifle | 74 |
| 15 | Tediore | Assault Rifle | 79 |
| 16 | Torgue | Assault Rifle | 73 |
| 17 | Vladof | Assault Rifle | 89 |
| 18 | Order | Assault Rifle | 73 |
| 19 | Maliwan | Shotgun | 74 |
| 20 | Daedalus | SMG | 77 |
| 21 | Bor | SMG | 73 |
| 22 | Vladof | SMG | 84 |
| 23 | Maliwan | SMG | 74 |
| 25 | Bor | Sniper | 71 |
| 26 | Jakobs | Sniper | 72 |
| 27 | Vladof | Sniper | 82 |
| 28 | Order | Sniper | 75 |
| 29 | Maliwan | Sniper | 76 |
| 244 | Vladof | Heavy | 22 |
| 245 | Torgue | Heavy | 32 |
| 246 | Bor | Heavy | 25 |
| 247 | Maliwan | Heavy | 19 |

### Class Mods

| Category ID | Player Class | Parts Count |
|-------------|--------------|-------------|
| 44 | Dark Siren | 0 (not in dump) |
| 55 | Paladin | 0 (not in dump) |
| 97 | Gravitar | 2 |
| 140 | Exo Soldier | 0 (not in dump) |

### Firmware

| Category ID | Type | Parts Count |
|-------------|------|-------------|
| 151 | Firmware | 0 (parts under gadget prefixes) |

Note: Firmware parts exist under `grenade_gadget.part_firmware_*`, `heavy_weapon_gadget.part_firmware_*`, and `repair_kit.part_firmware_*` prefixes.

### Shields

| Category ID | Type | Parts Count |
|-------------|------|-------------|
| 279 | Energy Shield | 22 |
| 280 | Bor Shield | 4 |
| 281 | Daedalus Shield | 3 |
| 282 | Jakobs Shield | 3 |
| 283 | Armor Shield | 26 |
| 284 | Maliwan Shield | 9 |
| 285 | Order Shield | 3 |
| 286 | Tediore Shield | 3 |
| 287 | Torgue Shield | 3 |
| 288 | Vladof Shield | 3 |
| 289 | Shield Variant | Unknown |

### Gadgets and Gear

| Category ID | Type | Parts Count |
|-------------|------|-------------|
| 300 | Grenade Gadget | 82 |
| 310 | Turret Gadget | 52 |
| 320 | Repair Kit | 107 |
| 330 | Terminal Gadget | 61 |

### Enhancements

| Category ID | Manufacturer | Parts Count |
|-------------|--------------|-------------|
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

### Summary

| Category | Total Parts |
|----------|-------------|
| Weapons (all types) | 1,928 |
| Class Mods | 2 |
| Shields | 79 |
| Gadgets | 302 |
| Enhancements | 28 |
| Unmapped | 276 |
| **Total** | **2,615** |

---

## Variant Suffixes

| Suffix | Meaning |
|--------|---------|
| Mask | Texture mask asset |
| ADSMask | Aim-down-sights mask |
| Proje/Projes | Projectile-related |
| ModA/ModB | Alternative model |
| Elements | Elemental effects |
| VFX | Visual effects |
| BASE | Base variant |
| _a, _b, _c, _d | Stat variants |
| _01, _02, _03 | Numbered variants |

---

## Data Files

The complete parts database is available at:

- **`share/manifest/parts_dump.json`** - Raw part names grouped by prefix
- **`share/manifest/parts_database.json`** - Full database with category/index mappings

Use `bl4 memory dump-parts` and `bl4 memory build-parts-db` to regenerate from a fresh memory dump.

---

For complete category mappings, composition system details, and licensed parts documentation, see [Chapter 8: Parts System](08-parts-system.md).

---

*Extracted from BL4 memory dumps and NCS data using bl4 analysis tools.*

*Last updated: February 2026 — NCS extraction expanded to 5,360 parts across 120 categories. See `share/manifest/parts_database.json` for the current authoritative source.*
