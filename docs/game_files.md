# Borderlands 4 Game File Structure

This document describes the file structure found in BL4's pak files.

## Overview

- **Engine**: Unreal Engine 5.5
- **Asset Format**: IoStore (.utoc/.ucas containers) with Zen package format
- **Total Assets**: ~119,299 files in pak archives, 81,097 extracted to manifest
- **Extraction Tools**:
  - `uextract` (custom Rust tool) - Primary extraction with property parsing
  - `retoc` - IoStore unpacking and legacy conversion

## Top-Level Content Structure

```
OakGame/Content/
├── AI/                     # Enemy AI, NPCs, bosses
├── Atlases/                # Texture atlases
├── Cinematics/             # Cutscene assets
├── Common/                 # Shared materials/resources
├── Dialog/                 # Dialogue assets
├── Editor/                 # Editor-only assets
├── Fonts/                  # Font assets
├── GameData/               # Core game configuration
├── Gear/                   # All equipment (weapons, shields, etc.)
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

## Player Characters (Vault Hunters)

```
PlayerCharacters/
├── Customizations/         # Player cosmetics
├── DarkSiren/              # Character: Dark Siren
├── ExoSoldier/             # Character: Exo Soldier
├── Gravitar/               # Character: Gravitar
├── Paladin/                # Character: Paladin
├── _Shared/                # Shared character resources
└── Temporary/              # Development/testing
```

## Gear System

### Equipment Types

```
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

```
Gear/Weapons/
├── _Manufacturer/          # Manufacturer-specific data
│   ├── BOR/                # Borg (new manufacturer?)
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
        ├── Order/              # Order faction(?) weapons
        ├── Rarity/             # Weapon rarity modifiers
        ├── ScopeData/          # Scope parts
        ├── TED/                # Tediore-specific data
        ├── UnderbarrelData/    # Underbarrel attachments
        ├── UniqueData/         # Legendary/unique data
        └── WeaponStats/        # Base stat definitions
```

## GameData System

```
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

## AI System

```
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

## Key Data Structures

### Loot Balance Structures

#### Struct_EnemyDrops
Fields (DoubleProperty type):
- `Guns_Probability` - Chance to drop guns
- `Guns_HowMany` - Number of guns to drop
- `Shields_Probability` - Chance to drop shields
- `Shields_HowMany` - Number of shields
- `GrenadesOrGadgets_Probability` - Grenade/gadget drop chance
- `GrenadesGadgets_HowMany` - Number of grenades/gadgets
- `ClassMods_Probability` - Class mod drop chance
- `ClassMods_HowMany` - Number of class mods
- `RepKits_Probability` - Repair kit drop chance
- `RepKits_HowMany` - Number of repair kits
- `Enhancements_Probability` - Enhancement drop chance
- `Enhancements_HowMany` - Number of enhancements
- `CurrencyOrAmmo_Probability` - Currency/ammo drop chance
- `CurrencyAmmo_HowMany` - Amount of currency/ammo
- `EXP_Multiplier` - Experience multiplier
- `Rarity_Modifier` - Rarity boost modifier

#### Struct_DedicatedDropProbability
Fields (DoubleProperty type):
- `Primary` - Primary dedicated drop chance
- `Secondary` - Secondary drop chance
- `Tertiary` - Tertiary drop chance
- `Quaternary` - Quaternary drop chance
- `Shiny` - Shiny variant chance
- `TrueBoss` - True boss drop chance
- `TrueBossShiny` - True boss shiny chance

**Extracted Default Values** (from uextract with .usmap mappings):
| Tier | Probability |
|------|-------------|
| Primary | 20% (0.2) |
| Secondary | 8% (0.08) |
| Tertiary | 3% (0.03) |
| Quaternary | 0% (0.0) |
| Shiny | 1% (0.01) |
| TrueBoss | 0% (0.0) |
| TrueBossShiny | 0% (0.0) |

#### Struct_Balance_Rarity
Fields:
- `Stat_Scale` - Stat scaling factor
- `Damage_Scale_Level` - Damage scaling per level
- `Growth_Exponent` - Growth curve exponent
- `Base_Weight` - Base drop weight for rarity tier
- `Local_Modifier` - GbxAttributeDef pointer for local modifiers

**Extracted Default Values**:
| Field | Default |
|-------|---------|
| Stat_Scale | 1.0 |
| Damage_Scale_Level | 1.0 |
| Growth_Exponent | 1.0 |
| Base_Weight | 1.0 |

#### Struct_Weapon_RarityInit
Weapon stat modifiers by rarity (FloatProperty):
- `Accuracy` - Accuracy modifier
- `Damage` - Damage modifier
- `Recoil` - Recoil modifier
- `Spread` - Spread modifier
- `Sway` - Sway modifier
- `EquipTime` - Equip time modifier
- `ZoomTime` - Zoom time modifier
- `AccImpulse` - Accuracy impulse modifier

### Loot Schedule Structure

#### LootScheduleStruct
Fields (IntProperty):
- `MinGameStage` - Minimum game stage for item
- `MaxGameStage` - Maximum game stage for item

## File Naming Conventions

- `Struct_*` - Structure definitions (type schemas)
- `DT_*` - Data Tables (actual data rows)
- `Body_*` - Body/mesh definitions
- `DST_*` - Destruction definitions
- `M_*` / `MI_*` - Materials / Material Instances
- `MF_*` - Material Functions
- `AS_*` - Animation Sequences
- `Script_*` - Blueprint scripts
- `StatusEffect_*` - Status effect definitions

## Manufacturer Codes

Known manufacturer codes found:
- `BOR` - Borg (new manufacturer?)
- `JAK` - Jakobs
- `TED` - Tediore
- `VLA` - Vladof

## Weapon Part Types

From `NexusConfigStoreInventory` in DefaultGame.ini:

### Core Weapon Parts
- `body` - Main weapon body
- `body_acc` - Body accessories
- `body_mag` - Body magazine attachment
- `body_ele` - Body elemental
- `body_bolt` - Bolt mechanism
- `barrel` - Weapon barrel
- `barrel_acc` - Barrel accessories
- `barrel_licensed` - Licensed barrel variants
- `magazine` - Magazine
- `magazine_acc` - Magazine accessories
- `magazine_borg` - Borg magazine type
- `magazine_ted_thrown` - Tediore thrown magazine

### Attachment Parts
- `scope` - Optical scopes
- `scope_acc` - Scope accessories
- `rail` - Rail attachments
- `bottom` - Bottom attachments
- `grip` - Weapon grip
- `foregrip` - Forward grip
- `underbarrel` - Underbarrel attachment
- `underbarrel_acc` - Underbarrel accessories
- `underbarrel_acc_vis` - Visible underbarrel accessories

### Manufacturer-Specific
- `tediore_acc` - Tediore accessories
- `tediore_secondary_acc` - Tediore secondary accessories
- `hyperion_secondary_acc` - Hyperion secondary accessories
- `turret_weapon` - Turret weapon parts

### Element & Augments
- `element` - Primary element
- `secondary_ele` - Secondary element
- `secondary_ammo` - Secondary ammo type
- `primary_augment` - Primary augment slot
- `secondary_augment` - Secondary augment slot
- `enemy_augment` - Enemy-dropped augment
- `active_augment` - Active skill augment
- `endgame` - Endgame modifications
- `unique` - Unique/legendary parts

### Grenade Parts
- `payload` - Grenade payload
- `payload_augment` - Payload augment
- `stat_augment` - Stat augment
- `curative` - Healing effect
- `augment` - General augment
- `utility_behavior` - Utility behavior

### Class Mod Parts
- `class_mod_body` - Class mod body
- `action_skill_mod` - Action skill modifier
- `core_augment` - Core augment
- `core_plus_augment` - Core plus augment
- `passive_points` - Passive skill points
- `special_passive` - Special passive abilities
- `stat_group1/2/3` - Stat groups

### Other
- `firmware` - Firmware upgrades
- `augment_element` - Elemental augments
- `augment_element_resist` - Elemental resistance
- `augment_element_nova` - Nova effect
- `augment_element_splat` - Splat effect
- `augment_element_immunity` - Elemental immunity
- `detail` - Detail parts
- `skin` - Weapon skins
- `vile` - Vile rarity parts
- `pearl_elem` - Pearl elemental
- `pearl_stat` - Pearl stat bonus

## Project Information

From DefaultGame.ini:
- **Internal Codename**: Oak2
- **Display Title**: Borderlands 4
- **Company**: Gearbox Entertainment
- **Max Players**: 4

## Notes

1. **Structure vs Data**: Files named `Struct_*` are type definitions (schemas), not actual data tables. The actual values are stored in DataTable assets that reference these structures. Our extraction found 44 `Struct_*` BalanceData files with template/init values.

2. **IoStore Format**: BL4 uses UE5's IoStore container format with Zen packages. The `uextract` tool can parse unversioned properties using IEEE 754 double/float interpretation.

3. **Extraction Status**: Full pak extraction yields 81,097 assets. Only ~301 have parseable stat values (the template structures).

4. **Missing Data**: Per-item balance data and ClassMod definitions are not found in the expected locations. Item stats appear to be derived from parts at runtime rather than stored in data files.

5. **New Systems**: BL4 introduces several new systems not present in BL3:
   - Repair Kits
   - Enhancements
   - Firmware upgrades
   - Gadgets (Turrets, Terminals, Heavy Weapons)
   - Armor Shards
   - Shield Boosters
   - Hover Drives (vehicles)
   - Borg manufacturer with charge mechanics
   - Pearl and Vile rarity tiers

## Extraction Results Summary

From `share/manifest/pak_summary.json`:

| Field | Value |
|-------|-------|
| Total Assets | 81,097 |
| Stats Count | 519 |
| Naming Strategies | 3 |
| Manufacturers | 9 (BOR, TOR, VLA, COV, MAL, TED, DAD, JAK, ORD) |

### Balance Data Categories Found

- firmware
- Heavy
- gadget
- repair_kit
- unknown
- grenade
- shield
- weapon

### Gear Types Found

- RepairKit
- Shield
- Gadget
- Enhancement
- ClassMod
- Firmware
- Grenade
