# Appendix D: Game File Structure

This appendix provides a complete reference of BL4's file structure, asset organization, and content layout.

---

## Overview

| Property | Value |
|----------|-------|
| Engine | Unreal Engine 5.5 |
| Asset Format | IoStore (.utoc/.ucas) with Zen packages |
| Total Assets | ~119,299 files in pak archives |
| Extracted to Manifest | 81,097 assets |
| Internal Codename | Oak2 |
| Max Players | 4 |

---

## File Locations

### Steam (Linux)

```text
~/.steam/steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/
```

### Steam (Windows)

```text
C:\Program Files (x86)\Steam\steamapps\common\Borderlands 4\OakGame\Content\Paks\
```

---

## Pak Chunk Contents

| Chunk | Contents |
|-------|----------|
| pakchunk0-Windows_0_P | Core game assets, weapons, gear |
| pakchunk2-Windows_0_P | Audio (Wwise .bnk) |
| pakchunk3-Windows_0_P | Localized audio |
| pakchunk10-Windows_0_P | Large assets |

---

## Top-Level Content Structure

```text
OakGame/Content/
├── AI/                     # Enemy AI, NPCs, bosses
├── Atlases/                # Texture atlases
├── Cinematics/             # Cutscene assets
├── Common/                 # Shared materials/resources
├── Dialog/                 # Dialogue assets
├── Editor/                 # Editor-only assets
├── Fonts/                  # Font assets
├── GameData/               # Core game configuration
├── Gear/                   # All equipment
├── GeometryCollections/    # Physics destruction meshes
├── Gore/                   # Gore effects
├── Grapple/                # Grappling hook system
├── InteractiveObjects/     # World interactables
├── LevelArt/               # Level-specific art
├── LevelLighting/          # Lighting setups
├── Maps/                   # Game maps/levels
├── Missions/               # Mission data
├── Pickups/                # Item pickup visuals
├── PlayerCharacters/       # Vault Hunters
├── UI/                     # UI assets
├── uiresources/            # UI resource files
├── WeatherOcclusionBakedData/
└── World/                  # World building assets
```

---

## Player Characters (Vault Hunters)

```text
PlayerCharacters/
├── Customizations/         # Player cosmetics
├── DarkSiren/              # Character: Dark Siren
├── ExoSoldier/             # Character: Exo Soldier
├── Gravitar/               # Character: Gravitar
├── Paladin/                # Character: Paladin
├── _Shared/                # Shared character resources
└── Temporary/              # Development/testing
```

---

## Gear System

### Equipment Types

```text
Gear/
├── ArmorShard/             # Armor shard items
├── Enhancements/           # Enhancement items
├── Firmware/               # Firmware upgrades
├── Gadgets/                # Deployable gadgets
│   ├── HeavyWeapons/       # Heavy weapon gadgets
│   ├── Terminals/          # Terminal gadgets
│   └── Turrets/            # Turret gadgets
├── GrenadeGadgets/         # Grenades
│   ├── Manufacturer/
│   │   └── VLA/            # Vladof grenades
│   └── _Shared/
├── RepairKits/             # Repair kit items
├── ShieldBooster/          # Shield boosters
├── shields/                # Shields
│   ├── BalanceData/
│   ├── Manufacturer/
│   │   └── VLA/            # Vladof shields
│   └── _Shared/
├── Vehicles/               # Vehicle equipment
│   └── HoverDrives/        # Hover drive upgrades
├── Weapons/                # Guns
└── _Shared/                # Shared gear resources
    ├── BalanceData/
    │   ├── Anoints/        # Anointment system
    │   ├── Economy/        # Currency/cost data
    │   └── Rarity/         # Rarity definitions
```

### Weapon System

```text
Gear/Weapons/
├── _Manufacturer/          # Manufacturer-specific data
│   ├── BOR/                # Borg
│   ├── JAK/                # Jakobs
│   └── TED/                # Tediore
├── Pistols/                # Pistol weapons
├── Shotguns/               # Shotgun weapons
├── SMG/                    # SMG weapons
├── Sniper/                 # Sniper weapons
└── _Shared/
    └── BalanceData/
        ├── BarrelData/         # Barrel parts
        ├── _BaseWeaponData/    # Base weapon stats
        ├── BorgChargeData/     # Borg charge mechanics
        ├── COV/                # COV overheat/repair
        ├── Elemental/          # Elemental damage types
        ├── MagazineData/       # Magazine parts
        ├── Order/              # Order faction weapons
        ├── Rarity/             # Weapon rarity modifiers
        ├── ScopeData/          # Scope parts
        ├── TED/                # Tediore-specific data
        ├── UnderbarrelData/    # Underbarrel attachments
        ├── UniqueData/         # Legendary/unique data
        └── WeaponStats/        # Base stat definitions
```

---

## GameData System

```text
GameData/
├── Activities/             # Activity/event system
├── Animation/              # Animation configs
├── Audio/                  # Audio settings
├── Balance/                # Game balance tables
│   └── Structs/
│       ├── Struct_ChallengeReward_ECHOTokens
│       └── Struct_BossReplay_Costs
├── Cinematics/             # Cinematic triggers
├── Damage/                 # Damage system
│   └── StatusEffects/      # Status effect definitions
├── DataTables/             # Generic data tables
│   └── Structs/
│       ├── Struct_FloatColumn
│       └── Struct_BaseDamage
├── Discovery/              # Discovery/exploration system
├── Globals/                # Global game settings
├── Impacts/                # Impact effects
├── Input/                  # Input bindings
├── Loot/                   # Loot system
│   ├── Balance/
│   │   └── DataTables/
│   │       ├── Struct_EnemyDrops
│   │       └── Struct_DedicatedDropProbability
│   └── LootSchedule/
├── Lootables/              # Lootable containers
├── Map/                    # Map data
├── Missions/               # Mission definitions
├── StatusEffects/          # Status effect data
├── WaypointPath/           # Navigation paths
└── WorldPainter/           # World generation
```

---

## AI System

```text
AI/
├── ArmyBandit/             # Bandit enemy faction
├── ArmyOrder/              # Order enemy faction
├── Bosses/                 # Boss enemies
│   ├── GrassBoss/
│   ├── Guardian/
│   ├── MountBoss/
│   ├── ShatterBoss/
│   ├── TKBoss/
│   └── _Shared/
├── Creatures/              # Non-humanoid enemies
├── Critters/               # Small creatures
├── NPC/                    # Non-enemy NPCs
└── _Shared/                # Shared AI resources
```

---

## File Naming Conventions

| Prefix | Type | Example |
|--------|------|---------|
| Struct_* | Structure definitions | Struct_EnemyDrops |
| DT_* | Data Tables | DT_WeaponStats |
| Body_* | Body/mesh definitions | Body_Pistol_01 |
| DST_* | Destruction definitions | DST_Barrel |
| M_* | Materials | M_Metal_Base |
| MI_* | Material Instances | MI_Weapon_Red |
| MF_* | Material Functions | MF_Damage_Flash |
| AS_* | Animation Sequences | AS_Reload |
| Script_* | Blueprint scripts | Script_WeaponFire |
| StatusEffect_* | Status effects | StatusEffect_Burn |

---

## Weapon Part Types

From `NexusConfigStoreInventory` in DefaultGame.ini:

### Core Weapon Parts

| Part | Description |
|------|-------------|
| body | Main weapon body |
| body_acc | Body accessories |
| body_mag | Body magazine attachment |
| body_ele | Body elemental |
| body_bolt | Bolt mechanism |
| barrel | Weapon barrel |
| barrel_acc | Barrel accessories |
| barrel_licensed | Licensed barrel variants |
| magazine | Magazine |
| magazine_acc | Magazine accessories |
| magazine_borg | Borg magazine type |
| magazine_ted_thrown | Tediore thrown magazine |

### Attachment Parts

| Part | Description |
|------|-------------|
| scope | Optical scopes |
| scope_acc | Scope accessories |
| rail | Rail attachments |
| bottom | Bottom attachments |
| grip | Weapon grip |
| foregrip | Forward grip |
| underbarrel | Underbarrel attachment |
| underbarrel_acc | Underbarrel accessories |
| underbarrel_acc_vis | Visible underbarrel accessories |

### Manufacturer-Specific

| Part | Description |
|------|-------------|
| tediore_acc | Tediore accessories |
| tediore_secondary_acc | Tediore secondary accessories |
| hyperion_secondary_acc | Hyperion secondary accessories |
| turret_weapon | Turret weapon parts |

### Element & Augments

| Part | Description |
|------|-------------|
| element | Primary element |
| secondary_ele | Secondary element |
| secondary_ammo | Secondary ammo type |
| primary_augment | Primary augment slot |
| secondary_augment | Secondary augment slot |
| enemy_augment | Enemy-dropped augment |
| active_augment | Active skill augment |
| endgame | Endgame modifications |
| unique | Unique/legendary parts |

### Grenade Parts

| Part | Description |
|------|-------------|
| payload | Grenade payload |
| payload_augment | Payload augment |
| stat_augment | Stat augment |
| curative | Healing effect |
| augment | General augment |
| utility_behavior | Utility behavior |

### Class Mod Parts

| Part | Description |
|------|-------------|
| class_mod_body | Class mod body |
| action_skill_mod | Action skill modifier |
| core_augment | Core augment |
| core_plus_augment | Core plus augment |
| passive_points | Passive skill points |
| special_passive | Special passive abilities |
| stat_group1/2/3 | Stat groups |

### Other

| Part | Description |
|------|-------------|
| firmware | Firmware upgrades |
| augment_element | Elemental augments |
| augment_element_resist | Elemental resistance |
| augment_element_nova | Nova effect |
| augment_element_splat | Splat effect |
| augment_element_immunity | Elemental immunity |
| detail | Detail parts |
| skin | Weapon skins |
| vile | Vile rarity parts |
| pearl_elem | Pearl elemental |
| pearl_stat | Pearl stat bonus |

---

## Key DataTables

### Weapon Naming

| Asset Path | Contents |
|------------|----------|
| /Game/Gear/Weapons/_Shared/NamingStrategies/WeaponNamingStruct | Weapon prefix naming |
| /Game/Gear/Weapons/_Shared/NamingStrategies/DAD_LicensedPart_Table_Struct | Daedalus licensed parts |
| /Game/Gear/Weapons/_Shared/NamingStrategies/TED_PayloadPrefix_Table_Struct | Tediore payload prefixes |

### Balance Data

| Asset Path | Contents |
|------------|----------|
| /Game/Gear/Weapons/_Shared/BalanceData/BarrelData/* | Barrel stat modifiers |
| /Game/Gear/Weapons/_Shared/BalanceData/MagazineData/* | Magazine stat modifiers |
| /Game/Gear/Weapons/_Shared/BalanceData/Rarity/* | Rarity tier modifiers |
| /Game/Gear/Weapons/_Shared/BalanceData/Elemental/* | Elemental damage data |
| /Game/GameData/DataTables/Structs/Struct_BaseDamage | Class mod stat tables |

---

## Asset Path Mapping

Game paths use `/Game/` prefix:

| Game Path | Extracted Path |
|-----------|----------------|
| /Game/Gear/Weapons/... | OakGame/Content/Gear/Weapons/... |
| /Script/OakGame.ClassName | Engine script class (not extractable) |

---

## Gear Types Found

| Type | Description |
|------|-------------|
| ClassMod | Character class modifications |
| Enhancement | Enhancement items |
| Firmware | Firmware upgrades |
| Gadget | Deployable gadgets |
| Grenade | Grenade items |
| RepairKit | Repair kits |
| Shield | Shield equipment |

---

## New Systems in BL4

BL4 introduces several systems not present in BL3:

| System | Description |
|--------|-------------|
| Repair Kits | Healing/repair items |
| Enhancements | Enhancement slot items |
| Firmware | Firmware upgrade system |
| Gadgets | Turrets, Terminals, Heavy Weapons |
| Armor Shards | Armor shard items |
| Shield Boosters | Shield booster pickups |
| Hover Drives | Vehicle hover drive upgrades |
| Borg | New manufacturer with charge mechanics |
| Pearl | Pearl rarity tier |
| Vile | Vile rarity tier |

---

## Extraction Results

From `share/manifest/pak_summary.json`:

| Field | Value |
|-------|-------|
| Total Assets | 81,097 |
| Stats Count | 519 |
| Naming Strategies | 3 |
| Manufacturers | 9 |

### Manufacturer Codes

BOR, TOR, VLA, COV, MAL, TED, DAD, JAK, ORD

### Balance Data Categories

- firmware
- Heavy
- gadget
- repair_kit
- unknown
- grenade
- shield
- weapon

---

## Notes

1. **Structure vs Data**: Files named `Struct_*` are type definitions (schemas), not actual data tables.

2. **IoStore Format**: BL4 uses UE5's IoStore container format with Zen packages.

3. **Missing Data**: Per-item balance data is derived from parts at runtime rather than stored in data files.

4. **Compression**: BL4 uses Oodle compression for IoStore containers.

---

## NCS Format (Nexus Config Store)

NCS is Gearbox's format for storing item pool definitions, part data, and other game configuration that isn't in standard PAK assets.

### Why NCS Matters

Standard PAK extraction returns 0 results for key classes:
- `ItemPoolDef` - Item pool definitions
- `ItemPoolListDef` - Item pool lists
- `loot_config` - Loot configuration

These are stored in **NCS format**, not as standard uasset files. The class definitions exist in `scriptobjects.json`, but the actual data lives in NCS.

### File Types in NCS Format

| Type | Description |
|------|-------------|
| `gbx_ue_data_table` | Gearbox UE data tables with item definitions |
| `gbxactor` | Actor definitions with loot references |
| `itempool` | Item pool definitions (what can drop) |
| `ItemPoolList` | Item pool list configurations |
| `loot_config` | Loot configuration with drop rates |
| `Mission` + `rewards` | Mission reward definitions |
| `vending_machine` | Vending machine inventory |

### gBx Header Format

NCS files use the "gBx" magic header:

```
Offset  Size  Description
------  ----  -----------
0x00    3     Magic bytes: "gBx" (0x67 0x42 0x78)
0x03    1     Variant byte: '9', '6', 'r', 0xEF, or 0xE0
0x04    ?     Oodle-compressed payload
```

**Variant bytes observed:**
- `0x39` ('9') - Most common
- `0x36` ('6')
- `0x72` ('r')
- `0xEF`
- `0xE0`

Example locations in pakchunk0-Windows_0_P.pak:
- Offset 18327014: `67 42 78 39...` (gBx9)
- Offset 60169283: `67 42 78 ef...`
- Offset 78874017: `67 42 78 72...` (gBxr)

### Compression

NCS uses **Oodle** compression (version 9):
- DLL: `oo2core_9_win64.dll`
- Primary function: `OodleLZ_Decompress`
- Additional: `OodleLZ_Compress`, `Oodle_GetConfigValues`

### Decompressed Content Format

After decompression, NCS content uses a typed hierarchical format with `field|type` notation:

#### Type Notation

| Suffix | Meaning | Example |
|--------|---------|---------|
| `\|map` | Nested map/object | `children\|map` |
| `\|leaf:` | String leaf value | `tags\|leaf:` |
| `\|leaf:typename` | Typed leaf | `damagesource\|leaf:damagesource` |
| `\|empty` | Boolean/empty flag | `newobjective\|empty` |

#### Known Field Names (71 total)

**Top-level Structure:**
- `gbx_sections|map` - Gearbox sections mapping
- `children|map` - Nested child objects
- `dependencies|map` - Asset dependencies
- `generateddependencies|map` - Generated dependencies
- `sections|map` - General sections
- `configs|map` - Configuration data
- `attributes|map` - Attribute data

**Damage System:**
- `damagesource|leaf:damagesource` - Damage source reference
- `damagesource|map` - Damage source mapping
- `damagetags|leaf:` - Damage tags
- `hitdamagesource|leaf:damagesource` - Hit damage source
- `damagesourceoverride|leaf:damagesource` - Override
- `reflecteddamagetags|leaf:` - Reflected damage tags
- `reflectedprojectiledamagetags|leaf:` - Projectile reflection
- `maxchargedamagetags|leaf:` - Max charge damage
- `overheatdamagetags|leaf:` - Overheat damage
- `repairkitdamagetags|leaf:` - Repair kit damage
- `lessthanthresholddamagetags|leaf:` - Threshold damage
- `playerdamagetags|leaf:` - Player damage

**Activity/Area System:**
- `activityareaactortags|leaf:` - Activity area actor tags
- `activityareatags|leaf:` - Activity area tags
- `areatags|leaf:` - Area tags
- `requiredactivityareatags|leaf:` - Required activity area
- `requiredactivitytags|leaf:` - Required activity
- `excludedactivityareaactortags|leaf:` - Excluded activity area actors
- `excludedactivitytags|leaf:` - Excluded activity

**Tag System:**
- `tags|leaf:` / `tags|map` - Tag data
- `excludetags|leaf:` / `excludetags|map` - Exclusion tags
- `excludeaddtags|leaf:` - Add to exclusion
- `require_tags|leaf:` - Required tags
- `reject_tags|leaf:` - Rejected tags
- `requiredbasetags|leaf:` - Required base tags
- `requiredtags|leaf:` - Required tags
- `excludedbasetags|leaf:` - Excluded base tags
- `excludedplayerdamagetags|leaf:` - Excluded player damage

**Weapon/Combat:**
- `weaponfire|map` - Weapon fire config
- `effectparameters|map` - Effect parameters
- `effectparametersbysurface|map` - Surface effects
- `effectoverrides|map` - Effect overrides
- `ampedtag|leaf:` - Amped tag
- `tedioreemptytags|leaf:` - Tediore empty
- `tediorehalffullormoretags|leaf:` - Tediore half+

**Mission System:**
- `invisiblemissiontypes|leaf:` - Invisible missions
- `poamissiontypes|leaf:` - POA missions
- `primarytrackedmissiontypes|leaf:` - Primary tracked
- `temporarytrackedmissiontypes|leaf:` - Temporary tracked
- `worldeventmissiontypes|leaf:` - World events
- `missionfailsafetimerpickuptypes|leaf:` - Failsafe timer

**UI/Display:**
- `display_items|map` - Display items
- `pips|map` - Pip display
- `wheelsetups|map` - Wheel UI setups
- `loaded_input_actions|map` - Input actions
- `mat26_augersight|map` - Auger sight material

**Other:**
- `stats|leaf:` - Statistics data
- `apply|leaf:` - Apply action
- `remove|leaf:` - Remove action
- `remove_states|leaf:` - Remove states
- `criteria|map` - Criteria conditions
- `entry_points|map` - Entry points
- `exit_points|map` - Exit points
- `exclusive|leaf:` - Exclusive flag
- `alias_nodes|map` - Alias nodes
- `factdependencies|map` - Fact dependencies
- `data|leaf:damagesource` - Data reference
- `newobjective|empty` - New objective flag
- `momentsuegarden|empty` - Moments UE garden
- `checkleavenoescaperoom01|empty` - Escape room check

### Hash Function

Field names are hashed using **FNV-1a 64-bit**:
- Offset basis: `0xcbf29ce484222325`
- Prime: `0x100000001b3`

### SerialIndex in NCS

The SerialIndex structure maps parts to their serialization indices:

```
Offset  Size  Description
------  ----  -----------
0x00    1     Category (weapon platform)
0x01    1     Scope/Status flags
0x02    2     Index (little-endian)
```

**Important:** NCS indices are runtime slot positions, NOT the serial encoding indices used in item serials. The manifest's alphabetical indices are used for serial encoding.

### NCS Parser Tool

A community parser exists (Cr4nkSt4r's NcsParser.exe):
- Loads Oodle DLL dynamically
- Decodes function names from XOR-encoded strings (key: 0xA7)
- Outputs JSON using nlohmann JSON v3.12.0
- Requires `oo2core_9_win64.dll` from game files

### Implementation Notes

To parse NCS files in Rust:
1. Use `oodle-safe` crate with game's Oodle DLL
2. Read gBx header to get compressed/decompressed sizes
3. Call `OodleLZ_Decompress` on payload
4. Parse decompressed content using field|type notation
5. Use FNV-1a hash for field name lookups

---

*Extracted from BL4 game files using retoc and uextract tools.*
