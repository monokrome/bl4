# Borderlands 4 Loot System

Research notes from live memory analysis of BL4 running under Proton.

> **Note**: These are historical research notes from early reverse engineering sessions. For the latest extracted loot data, see `share/manifest/items_database.json` which contains 62 item pools.

## Memory Analysis Session

**Date**: 2024-11-27 (Session 2)
**Platform**: Linux (GE-Proton)
**PID**: 115750
**Executable Base**: `0x140000000`
**Code Region**: `0x140001000-0x14e61c000` (~230MB)

### Binary Analysis Notes

The executable is protected/obfuscated (likely Denuvo). Static analysis shows:
- No exports (stripped)
- References `Borderlands4.pdb` but PDB not included
- Code uses heavy obfuscation (XOR, NOT, RCL patterns)
- Runtime analysis is more effective than static analysis

---

## FName Pool Locations

The FName string pool contains UE reflection strings. Found at multiple regions:

| Region | Description |
|--------|-------------|
| `0x05b25000-0x05f96000` | Class/struct names |
| `0x1cd44000-0x1cd55000` | Property/field names |

---

## Loot Pool System

### Core Classes

Found via pattern scanning for "ItemPool":

| Class | Address | Description |
|-------|---------|-------------|
| `ItemPoolEntry` | `0x5b25140` | Single item in a pool |
| `ItemPoolDef` | `0x5b25150` | Defines a loot pool |
| `ItemPoolListDef` | `0x5b25160` | List of pools |
| `ItemPoolInstanceData` | `0x5b25175` | Runtime instance data |
| `ItemPoolSelectorStateDef` | `0x5b251b0` | Selection state |

### Pool Types (Enum)

Found at `0x65610d50`:

```
ELootPoolTypes::All
ELootPoolTypes::BaseLoot
ELootPoolTypes::AdditionalLoot
ELootPoolTypes::DedicatedDrops
ELootPoolTypes::GearDrivenDrops
ELootPoolTypes::MAX
```

---

## Rarity Tiers

Found at `0x79105c0` - complete rarity tier definitions:

| Tier | Component ID | Price Modifier Attr | Material |
|------|--------------|---------------------|----------|
| Common | `comp_01_common` (95) | `attr_calc_pricemod_rarity_common` | `DA_MD_BOR_Common` |
| Uncommon | `comp_02_uncommon` (96) | `attr_calc_pricemod_rarity_uncommon` | `DA_MD_BOR_Uncommon` |
| Rare | `comp_03_rare` (97) | `attr_calc_pricemod_rarity_rare` | `DA_MD_BOR_Rare` |
| Epic | `comp_04_epic` (98) | `attr_calc_pricemod_rarity_epic` | `DA_MD_BOR_Epic` |
| Legendary | `comp_05_legendary` | `attr_calc_pricemod_rarity_legendary` | `DA_MD_BOR_Legendary_01` |

Asset path pattern: `/Game/Gear/Weapons/_Shared/Materials/BOR/DA_MD_BOR_{Rarity}`

---

## Loot Weight System

### Weight Properties

Found at `0x1cd44680`:

| Property | Description |
|----------|-------------|
| `GrowthExponent` | Level scaling exponent |
| `BaseWeight` | Base drop weight |
| `GameStageVariance` | Variance by game stage |
| `RelativeGameStage` | Relative stage modifier |
| `GameStageTable` | Stage lookup table |
| `LootGameStages` | Game stages for loot |
| `RankLootRarityTable` | Rarity by rank table |

### Rarity Weight Data

Found at `0x5f9548e`:

| Class | Description |
|-------|-------------|
| `RarityWeightData` | Weight configuration for rarity |
| `LocalRarityModifierData` | Local rarity modifiers |

---

## Luck System

### Core Classes

Found at `0x5f955e0`:

| Class | Description |
|-------|-------------|
| `LootGlobalsDef` | Global loot settings definition |
| `LuckCategoryAttribute` | Luck category attribute |
| `LuckCategoryAttributesState` | Runtime luck state |
| `LuckCategoryDef` | Luck category definition |
| `LuckCategoryValueResolver` | Resolves luck values |
| `LuckGlobals` | Global luck settings |

### Luck Categories

Found at `0x1cd44720`:

| Category | Description |
|----------|-------------|
| `LuckCategories` | Base luck categories |
| `EnemyBasedLuckCategories` | Enemy-specific luck modifiers |
| `PlayerBasedLuckCategories` | Player-specific luck modifiers |

---

## Lootable Objects

### Classes

Found at `0x5f954c0`:

| Class | Description |
|-------|-------------|
| `GbxCondition_CanOpenLootable` | Condition check for opening |
| `GbxCondition_ShouldLootableShowLockedPrompt` | Lock prompt condition |
| `LootableObjectInstanceProxy` | Instance proxy |
| `LootableObjectBehaviorMod` | Behavior modifier |
| `LootableObjectBodySettings` | Body settings |
| `LootableObjectBodyState` | Runtime body state |

---

## String Search Results

### "LootPool" (19 matches)

Primary locations:
- `0x4bf814de` - Unknown region
- `0x65610d66` - Enum strings
- `0x150329201` - Data region

### "Legendary" (1004 matches)

Asset paths like:
```
/Game/Gear/_Shared/Materials/Materials/_Global/Legendary/M_LEG_04_Glass
```

### "BaseWeight" (4 matches)

- `0x1cd4469a` - FName pool
- `0x1502cf6c2` - Data region

### "RarityWeight" (11 matches)

Primary in FName pool at `0x5f9548e`

### "Luck" (120 matches)

Concentrated in:
- `0x5f955f4` - Class definitions
- `0x1cd44736` - Property names

---

## Potential Patching Targets

For testing simple memory patches, these areas are of interest:

1. **Rarity Weight Calculation**
   - Find the function that reads `BaseWeight` and `RarityWeight`
   - Patch comparison or multiplier

2. **Luck Modifier Application**
   - `LuckCategoryValueResolver` likely contains the luck calculation
   - Patch to always return max luck

3. **Pool Selection**
   - `ItemPoolSelector` determines which pool drops
   - Patch to bias toward legendary pools

### Finding Code References

To find code that references these strings, search for:
1. LEA instructions loading string addresses
2. MOV instructions with FName indices
3. CALL instructions near string references

Example pattern to find `BaseWeight` usage:
```
48 8D ?? ?? ?? ?? ??  ; LEA with RIP-relative offset to "BaseWeight"
```

---

## Next Steps

1. **Pattern scan for code references** - Find functions that use these strings
2. **Trace function calls** - Identify the drop rate calculation function
3. **Locate float values** - Find 0.0-1.0 probability values in data sections
4. **Test simple patches** - NOP out rarity checks as proof of concept

---

## UE5 Core Structures

### CoreUObject Module

FName strings found at `0x44001d0`:
- `CoreUObject`
- `EnumProperty`
- `OptionalProperty`
- `Utf8StrProperty`
- `AnsiStrProperty`
- `GbxDefPtrProperty` (Gearbox extension)
- `GbxInlineStructProperty` (Gearbox extension)

### UObject String Locations

373 "UObject" references found. Primary locations:
- `0x44001dc` - CoreUObject module names
- `0x5b10d25` - FName pool
- `0x1aed71f4` - Data region

---

## Float Constants Analysis

Searched for common probability floats:

| Value | Float Hex | Matches | Notes |
|-------|-----------|---------|-------|
| 0.05 (5%) | `CD CC 4C 3D` | 19,422 | Too common |
| 0.1 (10%) | `CD CC CC 3D` | 111,225 | Very common |
| 0.025 (2.5%) | `CD CC CC 3C` | 1,676 | Potential drop rates |

Most 0.025 matches were in mesh/material data, not loot data.

---

## RNG Functions

### RDRAND Instruction Usage

108 `RDRAND` instructions found across the binary. These are the hardware RNG entry points.

### Pattern: `0F C7 F0` (rdrand eax)

Locations in code regions need further analysis to find which are used for loot rolls.

---

## Summary: What We Have

**Working:**
- Process attachment via `bl4 memory`
- Memory reading from all mapped regions
- Pattern scanning for strings and bytes
- Memory writing capability (untested with game state)

**Found:**
- FName string pools with all class/property names
- Loot-related class names (ItemPoolDef, RarityWeightData, LuckGlobals)
- Enum values for loot pool types
- Location of CoreUObject module data

**Needed:**
- Static binary analysis (Ghidra) to find function addresses
- Locate GUObjectArray pointer for object iteration
- Identify the specific function that rolls loot rarity
- Find live RarityWeightData instances in memory

---

## Loot Chance System

### DataTable Row Structure

Found `LootChanceDefinedValueRow` at `0x5f96454`:

| Class | Description |
|-------|-------------|
| `LootChanceDefinedValueRow` | DataTable row for loot chance values |
| `GetSummary_Chance` | Function to get chance summary |

This appears to be a DataTable structure for configuring loot chances.

### Binary Import Table (RNG Functions)

| Address | Function | DLL |
|---------|----------|-----|
| `0x150b74e18` | `std::_Random_device` | MSVCP140.dll |
| `0x150b75050` | `BCryptGenRandom` | bcrypt.dll |
| `0x150b759d0` | `rand` | api-ms-win-crt-utility-l1-1-0.dll |
| `0x150b759d8` | `srand` | api-ms-win-crt-utility-l1-1-0.dll |

---

## Recommended Next Steps

1. ~~**Use Ghidra** to analyze `Borderlands4.exe` statically~~ (Binary is obfuscated)
2. **Runtime tracing** - Set breakpoints on RNG imports during loot drops
3. **Trace code paths** from loot container open to item generation
4. **Identify the comparison** that checks random value against drop rate
5. **Patch the comparison** to always succeed (NOP or force jump)

---

## Legendary Item Database (from Memory)

Found at `0x94e7870` - complete list of legendary item class references:

### Weapons

| Item ID | Display Name | Type | Manufacturer |
|---------|--------------|------|--------------|
| `comp_05_legendary` | (base legendary) | - | - |
| `comp_05_legendary_hellfire` | Hellfire | SM? | - |
| `comp_05_legendary_prince` | Prince | - | - |
| `DAD_AR.comp_05_legendary_OM` | OM | AR | Daedalus |
| `DAD_SG.comp_05_legendary_HeartGUn` | Heart Gun | SG | Daedalus |
| `JAK_AR.comp_05_legendary_rowan` | Rowan's Call | AR | Jakobs |
| `JAK_PS.comp_05_legendary_kingsgambit` | King's Gambit | PS | Jakobs |
| `JAK_SR.comp_05_legendary_ballista` | Ballista | SR | Jakobs |
| `MAL_HW.comp_05_legendary_GammaVoid` | Gamma Void | HW | Maliwan |
| `MAL_SM.comp_05_legendary_OhmIGot` | Ohm I Got | SM | Maliwan |
| `TOR_HW.comp_05_legendary_ravenfire` | Ravenfire | HW | Torgue |
| `TOR_SG.comp_05_legendary_Linebacker` | Linebacker | SG | Torgue |
| `VLA_HW.comp_05_legendary_AtlingGun` | Atling Gun | HW | Vladof |
| `VLA_SM.comp_05_legendary_KaoSon` | Kaoson | SM | Vladof |

### Weapon Asset Paths (from Memory)

Found at `0x79107f0`:
```
/Game/Gear/Weapons/_Shared/Materials/BOR/DA_MD_BOR_Legendary_01
/Game/Gear/Weapons/_Shared/Materials/_Global/DA_MD_LavaFlow
```

Weapon part naming convention:
- `part_body_b`, `part_body_c`, `part_body_d` - Body variants
- `part_barrel_01_b`, `part_barrel_01_c`, `part_barrel_01_d` - Barrel variants
- `part_barrel_01_hellfire` - Unique legendary barrel
- `part_fire` - Fire element part
- `firmware` - Weapon firmware slot
- `endgame` - Endgame flag

### Equipment

| Item ID | Display Name | Type |
|---------|--------------|------|
| `ted_grenade_gadget.comp_05_legendary_PredatorDrone` | Predator Drone | Grenade |
| `tor_repair_kit.comp_05_legendary_ShinyWarPaint` | Shiny War Paint | Repair Kit |
| `tor_shield.comp_05_legendary_firewerks` | Firewerks | Shield |
| `jak_grenade_gadget.comp_05_spinning_blade` | Spinning Blade | Grenade |
| `BOR_REPAIR_KIT.comp_05_legendary_Augmenter` | Augmenter | Repair Kit |

### Black Market Items

Found ItemPool references for Black Market legendary items:
- `ItemPool_BlackMarket_Comp_BOR_HW_DiscJockey`
- `ItemPool_BlackMarket_Comp_BOR_HW_Streamer`

### Boss Drop Tables

Found at `0x28860e20`:
- `ItemPoolList_ShatterlandsCommanderFortress_TrueBoss`
- `ItemPoolList_Timekeeper_TKBoss_TrueBoss`

These are the dedicated loot pools for True Boss kills.

### Class Path

The class descriptor for ItemPoolDef is at:
```
/Script/GbxGame.ItemPoolDef
```

---

## Inventory System Classes

Found at `0x5b24de0` - key inventory classes for item generation:

| Class | Description |
|-------|-------------|
| `InventoryParam` | Inventory parameter |
| `InventoryParamsDef` | Parameter definitions |
| `InventoryRarityDataTableValueResolver` | Resolves rarity from DataTable |
| `InventoryRarityDef` | Rarity definition |
| `InventoryRewardAspect` | Reward aspect |
| `InventoryScriptAspect` | Script aspect |
| `InventorySelectionCriterion` | Selection criterion |
| `InventorySelectionCriteria` | Selection criteria |
| `InventorySerialNumber` | Item serial number |
| `InventorySkillsAspect` | Skills aspect |
| `InventorySkinDef` | Skin definition |
| `InventoryStatsPropertyResolver` | Stats property resolver |
| `InventoryStatsContainer` | Stats container |
| `InventoryStatsContainerContextResolver` | Stats context resolver |
| `InventoryStatsContainerValueResolver` | Stats value resolver |
| `InventoryStatTags` | Stat tags |
| `InventoryStatAttribute` | Stat attribute |

### Key Class: InventoryRarityDataTableValueResolver

This class resolves rarity values from a DataTable. Potential patching target:
- Patch the resolver to always return legendary rarity
- Or modify the DataTable values directly

---

## Injection Templates

### Two Implementation Modes

BL4 supports two injection approaches:

| Mode | Flag | Status | Description |
|------|------|--------|-------------|
| **Preload** | `--preload` | Working | Uses `LD_PRELOAD` to intercept RNG syscalls |
| **Direct** | (default) | Not implemented | Direct memory patching via SDK offsets |

### Preload Mode (Working)

The preload mode uses `libbl4_preload.so` to intercept random number generation at the syscall level:

```bash
# Run game with RNG bias for better drops
bl4 memory --preload apply dropRate=max dropRarity=legendary

# This generates an LD_PRELOAD command with BL4_RNG_BIAS environment variable
```

**How it works:**
- Intercepts `getrandom()` and similar RNG syscalls via `LD_PRELOAD`
- Sets `BL4_RNG_BIAS=max` to bias all random numbers toward favorable outcomes
- Works at syscall level - doesn't need SDK offsets or game structure knowledge

### Direct Mode (Not Implemented)

The direct injection mode (`bl4 memory apply dropRate=max`) is stub code that prints what would need to be done:

```bash
# Current output (not functional):
bl4 memory apply dropRate=max
# > Template: dropRate=max
# > Status: Not yet implemented
# > Requires: Finding RarityWeightData instances
```

**What's needed for direct mode:**
- `dropRate`: Find `RarityWeightData` instances and modify `BaseWeight`/`GrowthExponent`
- `dropRarity`: Patch ItemPool selection code to force specific rarity
- `luck`: Find `LuckGlobals` instance and modify values

**Known FName addresses** (for future implementation):
| Template | FName | Address |
|----------|-------|---------|
| dropRate | RarityWeightData | 0x5f9548e |
| dropRate | BaseWeight | 0x6f3a44c4 |
| dropRate | GrowthExponent | 0x6f3a44b4 |
| luck | LuckGlobals | 0x5f95658 |
| luck | LuckCategories | 0x6f3a4560 |

### Implementation Strategy (Future)

For direct mode implementation:

1. Find live `InventoryRarityDataTableValueResolver` instances via FName search
2. Locate the float weight values in the DataTable
3. Patch weights to favor legendary (increase legendary weight, decrease others)

Alternative: Patch the comparison code that checks random value against weight threshold.

---

## Extracted Items Database

The `share/manifest/items_database.json` file contains extracted item pool and stat data from pak files.

**Generate:**
```bash
bl4-research items-db -m share/manifest
```

**Contents:**
- `item_pools`: 62 unique loot pools with references showing what enemies/containers use them
- `items`: 26 balance data items with stat modifiers (Scale/Add/Value/Percent)
- `stats_summary`: Summary of all stat types, categories, and manufacturers

**Sample Pools:**
- `ItemPoolList_Enemy_BaseLoot_Boss` - Boss enemy drops
- `ItemPoolList_Enemy_BaseLoot_BossRaid` - Raid boss drops
- `itempool_guns_01_common` - Common weapon pool
- `itempool_guns_04_epic` - Epic weapon pool
- `ItemPool_FishCollector_Reward_Legendary` - Legendary reward pool

**Sample Stats:**
- `Damage_Scale`, `Damage_Value` - Base damage modifiers
- `CritDamage_Add` - Critical damage bonus
- `FireRate_Scale`, `FireRate_Value` - Fire rate modifiers
- `ReloadTime_Scale` - Reload speed modifier
- `ElementalChance_Scale` - Elemental proc chance

See [Extraction - bl4-research](extraction.md#bl4-research-manifest-generation) for usage details.

---

*Generated from live memory analysis using `bl4 memory` tool*
