# Appendix C: Loot System Internals

This appendix documents the internal workings of BL4's loot system, based on memory analysis and game file extraction.

---

## Loot Pool Architecture

### Core Classes

| Class | Description |
|-------|-------------|
| ItemPoolDef | Defines a loot pool |
| ItemPoolEntry | Single item in a pool |
| ItemPoolListDef | List of multiple pools |
| ItemPoolSelectorDef | Selection logic |
| ItemPoolInstanceData | Runtime instance data |
| ItemPoolSelectorStateDef | Selection state |

Script path: `/Script/GbxGame.ItemPoolDef`

### Pool Types Enum

```cpp
enum class ELootPoolTypes {
    All = 0,
    BaseLoot = 1,
    AdditionalLoot = 2,
    DedicatedDrops = 3,
    GearDrivenDrops = 4,
    MAX = 5
};
```

---

## Rarity Tiers

### Tier Definitions

| Tier | Component ID | Material Path |
|------|--------------|---------------|
| Common | comp_01_common | DA_MD_BOR_Common |
| Uncommon | comp_02_uncommon | DA_MD_BOR_Uncommon |
| Rare | comp_03_rare | DA_MD_BOR_Rare |
| Epic | comp_04_epic | DA_MD_BOR_Epic |
| Legendary | comp_05_legendary | DA_MD_BOR_Legendary_01 |

Material path pattern: `/Game/Gear/Weapons/_Shared/Materials/BOR/DA_MD_BOR_{Rarity}`

### Price Modifier Attributes

| Tier | Attribute |
|------|-----------|
| Common | attr_calc_pricemod_rarity_common |
| Uncommon | attr_calc_pricemod_rarity_uncommon |
| Rare | attr_calc_pricemod_rarity_rare |
| Epic | attr_calc_pricemod_rarity_epic |
| Legendary | attr_calc_pricemod_rarity_legendary |

---

## Loot Weight System

### Weight Properties

| Property | Description |
|----------|-------------|
| GrowthExponent | Level scaling exponent |
| BaseWeight | Base drop weight |
| GameStageVariance | Variance by game stage |
| RelativeGameStage | Relative stage modifier |
| GameStageTable | Stage lookup table |
| LootGameStages | Game stages for loot |
| RankLootRarityTable | Rarity by rank table |

### Weight Classes

| Class | Description |
|-------|-------------|
| RarityWeightData | Weight configuration |
| LocalRarityModifierData | Local rarity modifiers |

---

## Luck System

### Core Classes

| Class | Description |
|-------|-------------|
| LootGlobalsDef | Global loot settings |
| LuckCategoryAttribute | Luck category attribute |
| LuckCategoryAttributesState | Runtime luck state |
| LuckCategoryDef | Luck category definition |
| LuckCategoryValueResolver | Resolves luck values |
| LuckGlobals | Global luck settings |

### Luck Categories

| Category | Description |
|----------|-------------|
| LuckCategories | Base luck modifiers |
| EnemyBasedLuckCategories | Per-enemy modifiers |
| PlayerBasedLuckCategories | Player-specific modifiers |

---

## Drop Probability Tables

### Dedicated Drop Probability (Struct_DedicatedDropProbability)

| Tier | Probability |
|------|-------------|
| Primary | 20% (0.2) |
| Secondary | 8% (0.08) |
| Tertiary | 3% (0.03) |
| Quaternary | 0% (0.0) |
| Shiny | 1% (0.01) |
| TrueBoss | 0% (0.0) |
| TrueBossShiny | 0% (0.0) |

### Enemy Drops (Struct_EnemyDrops)

Fields (DoubleProperty type):

| Field | Description |
|-------|-------------|
| Guns_Probability | Chance to drop guns |
| Guns_HowMany | Number of guns |
| Shields_Probability | Shield drop chance |
| Shields_HowMany | Number of shields |
| GrenadesOrGadgets_Probability | Grenade/gadget chance |
| GrenadesGadgets_HowMany | Number to drop |
| ClassMods_Probability | Class mod chance |
| ClassMods_HowMany | Number of class mods |
| RepKits_Probability | Repair kit chance |
| RepKits_HowMany | Number of repair kits |
| Enhancements_Probability | Enhancement chance |
| Enhancements_HowMany | Number of enhancements |
| CurrencyOrAmmo_Probability | Currency/ammo chance |
| CurrencyAmmo_HowMany | Amount |
| EXP_Multiplier | Experience multiplier |
| Rarity_Modifier | Rarity boost |

---

## Known Item Pools

### Boss Pools

| Pool | Description |
|------|-------------|
| ItemPoolList_Enemy_BaseLoot_Boss | Standard boss drops |
| ItemPoolList_Enemy_BaseLoot_BossRaid | Raid boss drops |
| ItemPoolList_ShatterlandsCommanderFortress_TrueBoss | True Boss |
| ItemPoolList_Timekeeper_TKBoss_TrueBoss | Timekeeper True Boss |

### Rarity Pools

| Pool | Description |
|------|-------------|
| itempool_guns_01_common | Common weapons |
| itempool_guns_02_uncommon | Uncommon weapons |
| itempool_guns_03_rare | Rare weapons |
| itempool_guns_04_epic | Epic weapons |
| itempool_guns_05_legendary | Legendary weapons |

### Special Pools

| Pool | Description |
|------|-------------|
| ItemPool_FishCollector_Reward_Legendary | Fish collector reward |
| ItemPool_BlackMarket_Comp_BOR_HW_DiscJockey | Black Market |
| ItemPool_BlackMarket_Comp_BOR_HW_Streamer | Black Market |

---

## Lootable Objects

### Classes

| Class | Description |
|-------|-------------|
| GbxCondition_CanOpenLootable | Open condition check |
| GbxCondition_ShouldLootableShowLockedPrompt | Lock prompt |
| LootableObjectInstanceProxy | Instance proxy |
| LootableObjectBehaviorMod | Behavior modifier |
| LootableObjectBodySettings | Body settings |
| LootableObjectBodyState | Runtime body state |

---

## Inventory System

### Core Classes

| Class | Description |
|-------|-------------|
| InventoryParam | Inventory parameter |
| InventoryParamsDef | Parameter definitions |
| InventoryRarityDataTableValueResolver | Rarity resolver |
| InventoryRarityDef | Rarity definition |
| InventorySerialNumber | Item serial number |
| InventoryStatsContainer | Stats container |
| InventoryStatsPropertyResolver | Stats resolver |
| InventoryStatTags | Stat tags |
| InventoryStatAttribute | Stat attribute |

### Key Class: InventoryRarityDataTableValueResolver

This class resolves rarity values from DataTables. Potential patching target for forcing specific rarity tiers.

---

## RNG Implementation

### Import Table Functions

| Address | Function | DLL |
|---------|----------|-----|
| 0x150b74e18 | std::_Random_device | MSVCP140.dll |
| 0x150b75050 | BCryptGenRandom | bcrypt.dll |
| 0x150b759d0 | rand | api-ms-win-crt-utility-l1-1-0.dll |
| 0x150b759d8 | srand | api-ms-win-crt-utility-l1-1-0.dll |

### RDRAND Usage

108 `RDRAND` instructions found in the binary. These are hardware RNG entry points.

Pattern: `0F C7 F0` (rdrand eax)

---

## Memory Addresses

### FName Locations

| Region | Description |
|--------|-------------|
| 0x05b25000-0x05f96000 | Class/struct names |
| 0x1cd44000-0x1cd55000 | Property/field names |

### Specific Addresses

| Data | Address | Notes |
|------|---------|-------|
| ItemPoolDef class | 0x5b25150 | Class definition |
| Pool types enum | 0x65610d50 | ELootPoolTypes |
| Rarity tiers | 0x79105c0 | Tier definitions |
| Weight properties | 0x1cd44680 | Weight data |
| Luck classes | 0x5f955e0 | Luck system |
| Legendary items | 0x94e7870 | Item database |

---

## Loot Chance System

### DataTable Structure

Found `LootChanceDefinedValueRow` for configuring loot chances.

| Class | Description |
|-------|-------------|
| LootChanceDefinedValueRow | DataTable row for chances |
| GetSummary_Chance | Chance summary function |

---

## Stat Modifiers

### From Extracted Data

| Stat | Description |
|------|-------------|
| Damage_Scale | Base damage multiplier |
| Damage_Value | Flat damage value |
| CritDamage_Add | Critical damage bonus |
| FireRate_Scale | Fire rate multiplier |
| FireRate_Value | Fire rate value |
| ReloadTime_Scale | Reload speed modifier |
| ElementalChance_Scale | Element proc chance |
| Accuracy_Scale | Accuracy modifier |
| Spread_Scale | Spread modifier |
| Recoil_Scale | Recoil modifier |

---

## Injection Approaches

### LD_PRELOAD Method (Linux/Proton)

Intercept RNG at syscall level:

```bash
# Bias RNG for better drops
LD_PRELOAD=/path/to/libbl4_preload.so BL4_RNG_BIAS=max ./game
```

This intercepts `getrandom()` and similar syscalls.

### Direct Memory Patching

Target locations for modification:

| Template | Target | Address |
|----------|--------|---------|
| dropRate | RarityWeightData | 0x5f9548e |
| dropRate | BaseWeight | 0x6f3a44c4 |
| dropRate | GrowthExponent | 0x6f3a44b4 |
| luck | LuckGlobals | 0x5f95658 |
| luck | LuckCategories | 0x6f3a4560 |

### Implementation Strategy

1. Find live `InventoryRarityDataTableValueResolver` instances
2. Locate float weight values in DataTable
3. Patch weights to favor legendary tier
4. Or patch comparison code for weight threshold

---

## Extracted Items Database

Located at `share/manifest/items_database.json`:

| Content | Count |
|---------|-------|
| Item pools | 62 |
| Balance items | 26 |
| Stat types | 73 |

### Sample Pools

- `ItemPoolList_Enemy_BaseLoot_Boss`
- `ItemPoolList_Enemy_BaseLoot_BossRaid`
- `itempool_guns_01_common`
- `itempool_guns_04_epic`
- `ItemPool_FishCollector_Reward_Legendary`

### Generate Database

```bash
bl4-research items-db -m share/manifest
```

---

## Binary Protection

The executable uses protection (likely Denuvo):

- No exports (stripped)
- References `Borderlands4.pdb` but PDB not included
- Heavy obfuscation (XOR, NOT, RCL patterns)
- Runtime analysis more effective than static

!!! warning
    Static binary analysis is difficult. Runtime analysis via memory attachment is recommended.

---

*Data from live memory analysis using `bl4 memory` tool.*
