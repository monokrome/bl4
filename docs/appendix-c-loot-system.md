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

## Dedicated Drop Tables

### Boss → Legendary Mappings

Each boss has 1-3 dedicated legendary drops. Extracted from `itempoollist.bin` NCS files.

| Boss | Primary Drop | Secondary Drop | Third Drop |
|------|--------------|----------------|------------|
| Arjay | ORD_SR Fisheye | DAD_SG HeartGun | - |
| Backhive | VLA_SR StopGap | - | - |
| Bango | JAK_PS Phantom_Flame | BOR_SM Prince | - |
| BatMatriarch | TOR_SG Linebacker | BOR_SM hellfire | - |
| BattleWagon | VLA_SR Finnty | TOR_AR Bugbear | - |
| BlasterBrute | JAK_SG Slugger | MAL_SG Kaleidosplode | - |
| Bloomreaper | MAL_SG Mantra | - | - |
| CityCat | DAD_SG Bod | - | - |
| CloningLeader | TED_PS Sideshow | - | - |
| Destroyer | JAK_SR Boomslang | BOR_SG Convergence | - |
| Donk | JAK_AR Rowdy | TED_PS Inscriber | - |
| Drillerhole | ORD_AR GMR | MAL_SR Katagawa | - |
| DroneKeeper | ORD_PS Bully | TED_AR DividedFocus | - |
| FirstCorrupt | JAK_PS KingsGambit | DAD_PS Rangefinder | - |
| GlidePackPsycho | TOR_SG LeadBalloon | - | - |
| Grasslands_Commander | BOR_SG GoldenGod | BOR_SG GoreMaster | VLA_SM Onslaught |
| Grasslands_Guardian | TED_SG HeavyTurret | - | - |
| Hovercart | VLA_AR WomboCombo | TOR_AR PotatoThrower | - |
| KOTOMotherbaseBrute | TED_PS ATLien | - | - |
| KotoLieutenant | ORD_PS RocketReload | VLA_AR DualDamage | - |
| LeaderHologram | TED_AR Chuck | - | - |
| MeatPlantGunship | MAL_SR Asher | - | - |
| MeatheadRider | VLA_AR Lucian | - | - |
| MeatheadRider_Jockey | JAK_SG Hellwalker | - | - |
| MountainCommander | TED_PS RubysGrasp | - | - |
| MountainGuardian | VLA_SM KaoSon | - | - |
| Pango | BOR_SR Stray | - | - |
| Redguard | VLA_AR WF | JAK_AR Rowan | JAK_AR BonnieClyde |
| RockAndRoll | JAK_PS QuickDraw | JAK_SG TKsWave | - |
| ShatterlandsCommanderElpis | MAL_SM OhmIGot | - | - |
| ShatterlandsCommanderFortress | TOR_PS QueensRest | - | - |
| ShatterlandsGuardian | TED_SG Anarchy | - | - |
| SideCity_Psycho | ORD_AR Goalkeeper | JAK_PS SeventhSense | - |
| SkullOrchid | TOR_PS Roach | - | - |
| SoldierAncient | DAD_AR Om | - | - |
| SpiderJumbo | ORD_PS NoisyCricket | - | - |
| StealthPredator | BOR_SR Vamoose | - | - |
| StrikerSplitter | DAD_SM Luty | - | - |
| SurpriseAttack | DAD_SM Bloodstarved | - | - |
| Thresher_BioArmoredBig | JAK_SR Truck | - | - |
| Timekeeper_Guardian | MAL_SM PlasmaCoil | DAD_AR StarHelix | - |
| Timekeeper_TKBoss | ORD_SR Symmetry | JAK_SR Ballista | - |
| TrashThresher | MAL_SG Kickballer | VLA_SM BeeGun | - |
| UpgradedElectiMole | DAD_PS Zipgun | JAK_SG RainbowVomit | - |

### Weapon Type Codes

| Code | Weapon Type |
|------|-------------|
| AR | Assault Rifle |
| PS | Pistol |
| SG | Shotgun |
| SM | SMG |
| SR | Sniper Rifle |

### Manufacturer Codes

| Code | Manufacturer |
|------|--------------|
| BOR | Borealis |
| DAD | Dahlia Defense |
| JAK | Jakobs |
| MAL | Maliwan |
| ORD | Ordnance |
| TED | Tediore |
| TOR | Torgue |
| VLA | Vladof |

---

## Known Item Pools

### Boss Pools

| Pool | Description |
|------|-------------|
| ItemPoolList_Enemy_BaseLoot_Boss | Standard boss drops |
| ItemPoolList_Enemy_BaseLoot_BossRaid | Raid boss drops |
| ItemPoolList_Enemy_BaseLoot_BossMini | Mini-boss drops |
| ItemPoolList_Enemy_BaseLoot_BossVault | Vault boss drops |
| ItemPoolList_*_TrueBoss | True Boss (Chaos mode) variants |

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

::: {.callout-warning}
Static binary analysis is difficult. Runtime analysis via memory attachment is recommended.
:::

---

## Drop Probability Research

### Community Testing Results

::: {.callout-note title="Community-Derived Values"}
From player testing (~3,000 boss kills, Jan 2025):

- **Dedicated drop rate**: ~5% per kill for any individual item
- **World drop rate**: ~4% base legendary chance
- Difficulty level does NOT affect drop rates
:::

### NCS Dedicated Drop Tiers

The `Table_DedicatedDropProbability` in `itempoollist.bin` references a DataTable with the following tier rows:

| Tier | Row Name | Index | Probability |
|------|----------|:-----:|:-----------:|
| Primary | `Primary_2_<GUID>` | 2 | 20% |
| Secondary | `Secondary_4_<GUID>` | 4 | 8% |
| Tertiary | `Tertiary_6_<GUID>` | 6 | 3% |
| Shiny | `Shiny_9_<GUID>` | 9 | 1% |
| TrueBoss | `TrueBoss_12_<GUID>` | 12 | 0% |
| TrueBossShiny | `TrueBossShiny_14_<GUID>` | 14 | 0% |
| Quaternary | `Quaternary_16_<GUID>` | 16 | 0% |

The schema is defined in `Struct_DedicatedDropProbability.uasset` with a single `DoubleProperty` field.

### Data Coverage

**What NCS extraction provides:**

1. Which items are dedicated drops for which bosses
2. World drop pool membership
3. Boss display names (via NameData entries)
4. Tier assignment for items with explicit tier context

**What NCS cannot provide:**

1. Actual numeric probability values per tier (extracted from DataTable schemas, not NCS)
2. Base legendary drop rate (runtime value)
3. Category selection probabilities (shield vs weapon)
4. Tier assignments for most items (only 1 of 133 boss drops has explicit tier in NCS)

### Future Investigation

1. Memory scan for float values 0.05, 0.04, etc. when game is running
2. Check if tier indices correlate to probabilities (e.g., 2 → 2%, 4 → 4%)
3. Extract actual DataTable values (not just struct schemas) from pak files
4. Investigate if items without explicit tiers default to Primary

---

## Using the Drops Command

The `bl4 drops` command provides a CLI interface for querying drop information:

### Find Item Drop Locations

Find where an item drops, sorted by highest drop rate:

```bash
bl4 drops find hellwalker
```

Output:
```
Drop locations for 'hellwalker' (sorted by drop rate):

Source                         Type         Tier           Chance
----------------------------------------------------------------
MeatheadRider_Jockey           Boss         Primary           20%
```

### Query Source Drops

List all items dropped by a specific source:

```bash
bl4 drops source Timekeeper
```

Output:
```
Drops from 'Timekeeper' (sorted by drop rate):

Item                      Type     Tier           Chance
-------------------------------------------------------
symmetry                  ORD_SR   Primary           20%
PlasmaCoil                MAL_SM   Primary           20%
ballista                  JAK_SR   Secondary          8%
timekeeper                TED_SHIELD Secondary          8%
star_helix                DAD_AR   Tertiary           3%
```

### List All Sources

```bash
bl4 drops list --sources
```

### List All Items

```bash
bl4 drops list
```

### Generate Drops Manifest

Extract drop information from NCS data:

```bash
bl4 drops generate "/path/to/ncs_native" -o share/manifest/drops.json
```

---

## File Representations

### NCS Files

Drop information is stored in NCS (Nexus Config Store) files within pak archives:

| File | Purpose |
|------|---------|
| `itempoollist.bin` | Boss → legendary item mappings |
| `itempool.bin` | Item pool definitions (rarity weights, world drops) |
| `loot_config.bin` | Global loot configuration |
| `preferredparts.bin` | Part preferences for items |

### Item Composition Pattern

Legendary items follow a consistent naming pattern in NCS files:

```
MANUFACTURER_TYPE.comp_05_legendary_NAME
```

Examples:
- `JAK_SG.comp_05_legendary_Hellwalker` - Jakobs Shotgun "Hellwalker"
- `MAL_SM.comp_05_legendary_PlasmaCoil` - Maliwan SMG "PlasmaCoil"
- `DAD_SHIELD.comp_05_legendary_angel` - Dahlia Defense Shield "Guardian Angel"

### Component Tiers

| Component | Rarity |
|-----------|--------|
| `comp_01_common` | Common |
| `comp_02_uncommon` | Uncommon |
| `comp_03_rare` | Rare |
| `comp_04_epic` | Epic |
| `comp_05_legendary` | Legendary |

### Drops Manifest (drops.json)

The generated manifest contains:

```json
{
  "version": 1,
  "probabilities": {
    "Primary": 0.20,
    "Secondary": 0.08,
    "Tertiary": 0.03,
    "Shiny": 0.01,
    "TrueBoss": 0.08,
    "TrueBossShiny": 0.03
  },
  "drops": [
    {
      "source": "MeatheadRider_Jockey",
      "source_type": "Boss",
      "manufacturer": "JAK",
      "gear_type": "SG",
      "item_name": "Hellwalker",
      "item_id": "JAK_SG.comp_05_legendary_Hellwalker",
      "pool": "itempool_jak_sg_05_legendary_Hellwalker_shiny",
      "drop_tier": "Primary",
      "drop_chance": 0.20
    }
  ]
}
```

### Source Types

| Type | Description |
|------|-------------|
| Boss | Dedicated boss drop |
| Mission | Side/main mission reward |
| BlackMarket | Black Market vendor exclusive |
| Special | Fish Collector, challenges, etc. |
| WorldDrop | General legendary pool |

---

## Slot System

### Gear Slots

Items are categorized into slots based on their type:

| Slot | Type Code | Examples |
|------|-----------|----------|
| Weapon 1-4 | AR, PS, SG, SM, SR, HW | Assault Rifles, Pistols, etc. |
| Shield | SHIELD | Defensive shields |
| Grenade | GRENADE | Grenade mods |
| Class Mod | CM | Class-specific mods |
| Artifact | ARTIFACT | Trinkets/artifacts |
| Repair Kit | RK, REPAIR_KIT | Healing items |

### Weapon Type Codes

| Code | Full Name |
|------|-----------|
| AR | Assault Rifle |
| PS | Pistol |
| SG | Shotgun |
| SM | SMG (Submachine Gun) |
| SR | Sniper Rifle |
| HW | Heavy Weapon |

### Manufacturer Codes

| Code | Manufacturer | Specialty |
|------|--------------|-----------|
| BOR | Borealis | - |
| DAD | Dahlia Defense | Shields |
| JAK | Jakobs | High damage, slow fire |
| MAL | Maliwan | Elemental |
| ORD | Ordnance | - |
| TED | Tediore | Throw-to-reload |
| TOR | Torgue | Explosives |
| VLA | Vladof | High fire rate |

---

*Data from live memory analysis using `bl4 memory` tool and NCS file extraction.*
