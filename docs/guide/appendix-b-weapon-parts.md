# Appendix B: Weapon Parts Reference

This appendix catalogs all known weapon parts extracted from BL4 game files, organized by manufacturer and weapon type.

---

## Part Naming Convention

Parts follow this pattern:

```
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
| BOR | Borg | - | SM, SG, HW, SR |
| COV | Children of the Vault | - | Various |
| DAD | Daedalus | 4 | AR, SM, PS, SG |
| DPL | Dahl | - | Turrets/Gadgets only |
| JAK | Jakobs | 129 | AR, PS, SG, SR |
| MAL | Maliwan | 138 | SM, SG, HW, SR |
| ORD | Order | 15 | AR, PS, SR |
| TED | Tediore | 10 | AR, SG, PS |
| TOR | Torgue | 6 | AR, SG, HW, PS |
| VLA | Vladof | 134 | AR, SM, HW, SR |

**Serial ID** is the first VarInt in decoded item serials.

---

## Weapon Types

| Code | Type | Description |
|------|------|-------------|
| AR | Assault Rifle | Full-auto/burst rifles |
| HW | Heavy Weapon | Launchers, miniguns |
| PS | Pistol | Handguns |
| SG | Shotgun | Spread weapons |
| SM | SMG | Submachine guns |
| SR | Sniper Rifle | Precision weapons |

---

## Scope Parts by Manufacturer

### BOR (Borg)

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

```
{MFG}_{TYPE}.comp_0{N}_{rarity}_{name}
```

Examples:
- `TOR_SG.comp_05_legendary_Linebacker`
- `VLA_SM.comp_05_legendary_KaoSon`

---

## Part Count by Manufacturer

| Manufacturer | Total Parts |
|--------------|-------------|
| BOR | 60 |
| DAD | 79 |
| JAK | 99 |
| MAL | 84 |
| ORD | 59 |
| TED | 62 |
| TOR | 66 |
| VLA | 68 |
| **Total** | **577** |

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

---

*Extracted from BL4 game assets using retoc IoStore extraction.*
