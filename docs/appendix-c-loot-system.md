# Appendix C: Loot System Internals {#sec-loot-system}

This appendix explains how BL4's loot system decides what to drop, how rare each drop is, and how the bl4 toolkit estimates item rarity from serial data.

---

## Two Ways to Get a Legendary

Every legendary in BL4 reaches the player through one of two paths: **dedicated drops** or **world drops**. They use different probability models, and understanding the difference matters for estimating how rare a given item is.

### Dedicated Drops

Dedicated drops are tied to specific bosses. Each boss has an `ItemPoolList` in NCS that names 1--3 legendary items, each assigned to a **tier** that determines its drop chance per kill:

| Tier | Chance per kill |
|------|:---------:|
| Primary | 6% |
| Secondary | 4.5% |
| Tertiary | 3% |

The first item in a boss's pool is Primary (highest chance), the second is Secondary, and so on. When you kill a boss, the game rolls independently for each dedicated item --- you can get zero, one, or (rarely) multiple legendaries from a single kill.

::: {.callout-note title="Shiny and TrueBoss Variants"}
Bosses have additional tier variants for special modes. **TrueBoss** (Chaos mode) raises the dedicated drop rate to 25%, making legendaries roughly 4x more common per kill. **Shiny** items are cosmetic variants at 1%, and **TrueBossShiny** at 3%.
:::

This is the simplest path to a legendary: kill a boss, roll against a known percentage. If the Hellwalker is the Saddleback's Primary drop, you have a 6% chance per kill. Farm the boss enough and you'll get one.

### World Drops

World drops are the other path. When any enemy dies, it can drop gear from the general loot pool. The rarity of that gear is selected by a weighted roll:

| Rarity | Weight | Probability |
|--------|-------:|:-----------:|
| Common | 100.0 | ~94.18% |
| Uncommon | 6.0 | ~5.65% |
| Rare | 0.14 | ~0.132% |
| Epic | 0.045 | ~0.0424% |
| Legendary | 0.0003 | ~0.000283% |

These weights come from the `rarity_balance` table in `gbx_ue_data_table0.bin`. The game sums all weights (106.1853) and picks a tier proportionally. Legendary is 0.0003 out of 106.1853 --- roughly 1 in 353,000 world drops.

But that's the chance of getting *any* legendary. The chance of getting a *specific* one is much lower. If you rolled Legendary on a world drop and the game selects from the Pistol pool, there are 9 legendary pistols across all manufacturers. Your chance of the specific one you wanted is the tier probability divided by the pool size: roughly 1 in 3.2 million.

---

## How a Drop Resolves

When an enemy dies, the game runs through several systems in sequence:

1. **Base loot roll** (`Struct_EnemyDrops`): determines what *categories* drop --- guns, shields, grenades, class mods, currency. Each category has its own probability and quantity. A standard boss might drop 2 guns and 1 shield.

2. **Rarity selection**: for each item that drops, the rarity weight table determines what tier it rolls. This is where Common (94%) vs Legendary (0.0003%) is decided.

3. **Pool selection**: the game picks a specific item from the pool matching that rarity and item category. A legendary gun roll selects from `itempool_guns_05_legendary`.

4. **Dedicated drop check**: independently of the base loot, the boss's dedicated pool is checked. Each assigned legendary rolls independently at its tier percentage (Primary 6%, Secondary 4.5%, etc.).

5. **Luck modifier**: the player's luck stat modifies rarity weights at step 2. Higher luck increases the weight of rarer tiers. The exact formula uses `GrowthExponent`, `BaseWeight`, and `GameStageVariance` properties on the `RarityWeightData` class, but the specific curve is resolved at runtime and hasn't been fully extracted.

Steps 2--3 and step 4 are independent paths. An item can appear as both a dedicated drop (guaranteed from a specific boss) and a world drop (from the general pool). The dedicated path is far more likely for any given item.

---

## Pool Sizes and Per-Item Odds

The rarity tier probability tells you how likely *any* legendary is. The pool size tells you how likely a *specific* one is. These are different questions.

Legendary items are organized into pools by manufacturer and weapon type. Jakobs Shotguns have 2 legendaries. Vladof Assault Rifles have 3. The bl4 toolkit's `drop_pools.tsv` manifest captures these counts.

For world drops, the game selects across all manufacturers within a weapon type. If it rolls "legendary pistol", it's choosing from all 9 legendary pistols (Jakobs, Daedalus, Tediore, etc. combined). The per-item probability for a specific legendary pistol from a world drop is:

```
tier_probability / world_pool_size = 0.000283% / 9 = ~0.0000314%
```

That's roughly 1 in 3.2 million world drops for a specific legendary pistol. This is why dedicated boss farming (6% per kill for a specific item) is orders of magnitude more efficient than hoping for world drops.

---

## Rarity Estimation

The bl4 toolkit can estimate an item's rarity from its serial string. The `rarity_estimate()` method on `ItemSerial` combines several data sources:

1. **Rarity tier**: decoded from the serial's `inv_comp` part (comp_01 through comp_05)
2. **Tier probability**: looked up from the rarity weight table
3. **Manufacturer and type codes**: extracted from the serial's token stream (VarInt-first for weapons, VarBit-first for equipment)
4. **Pool data**: matched against the compile-time `drop_pools.tsv` manifest for legendary count, world pool size, and boss source count

For legendaries, the estimate divides the tier probability by the world pool size to get per-item odds. For other rarities, it reports only the tier probability (since the pool selection is less meaningful --- a "common Jakobs pistol" isn't a specific item the way a legendary is).

```text
Rarity estimate:
  Tier: Legendary (0.000283%, ~1 in 353,490)
  Pool: Jakobs Shotgun (2 legendaries, 9 in world pool)
  Per-item: ~1 in 3,181,413
  Boss sources: 2
```

::: {.callout-warning}
These are estimates, not exact values. The rarity weight table is extracted from NCS data tables and matches community testing results, but luck modifiers, game stage scaling, and category selection probabilities are resolved at runtime and aren't captured here.
:::

---

## What We Know vs. Don't Know

**Extracted from NCS data:**

- Dedicated drop probabilities per tier (from `Table_DedicatedDropProbability`)
- Rarity weights for the base tier selection (from `rarity_balance`)
- Boss-to-legendary assignments and tier ordering (from `itempoollist.bin`)
- Pool sizes per manufacturer/weapon type (from drops manifest)
- Enemy drop category probabilities and quantities (from `Table_EnemyDrops`)

**Not yet extracted:**

- Luck system curve (how much luck shifts rarity weights, resolved at runtime)
- Category selection probabilities within a rarity tier (shield vs weapon vs grenade)
- Game stage scaling effects on weights (the `GrowthExponent` / `GameStageVariance` interaction)
- Tier assignments for most dedicated drops (only 1 of 133 boss drops has explicit tier context in NCS --- the rest are inferred by pool ordering)

---

## NCS Data Sources

Drop information is stored across several NCS files within pak archives:

| File | Contents |
|------|----------|
| `itempoollist.bin` | Boss → legendary item mappings with tier assignments |
| `itempool.bin` | Item pool definitions, rarity weights, world drop membership |
| `gbx_ue_data_table0.bin` | `Table_DedicatedDropProbability`, `rarity_balance`, `Table_EnemyDrops`, `Table_LootableBalance` |
| `loot_config.bin` | Global loot configuration parameters |
| `preferredparts.bin` | Part preferences for item generation |

Numeric values in these tables are stored as strings in NCS (`"0.060000"`, `"1.500000"`). The binary section's bit-packed indices point into the string table where these values live.

---

## Reference: Dedicated Drop Probability Tiers

From `Table_DedicatedDropProbability` in `gbx_ue_data_table0.bin`. Schema defined in `Struct_DedicatedDropProbability.uasset` with a single `DoubleProperty` field.

| Tier | Row Name | Index | Probability |
|------|----------|:-----:|:-----------:|
| Primary | `Primary_2_<GUID>` | 2 | 6% |
| Secondary | `Secondary_4_<GUID>` | 4 | 4.5% |
| Tertiary | `Tertiary_6_<GUID>` | 6 | 3% |
| Shiny | `Shiny_9_<GUID>` | 9 | 1% |
| TrueBoss | `TrueBoss_12_<GUID>` | 12 | 25% |
| TrueBossShiny | `TrueBossShiny_14_<GUID>` | 14 | 3% |
| Quaternary | `Quaternary_16_<GUID>` | 16 | 0% |

---

## Reference: Rarity Weights

From `rarity_balance` in `gbx_ue_data_table0.bin`. Total weight: 106.1853.

| Tier | Component ID | Weight | Probability |
|------|--------------|-------:|:-----------:|
| Common | `comp_01_common` | 100.0 | 94.18% |
| Uncommon | `comp_02_uncommon` | 6.0 | 5.65% |
| Rare | `comp_03_rare` | 0.14 | 0.132% |
| Epic | `comp_04_epic` | 0.045 | 0.0424% |
| Legendary | `comp_05_legendary` | 0.0003 | 0.000283% |

---

## Reference: Enemy Drop Fields

From `Struct_EnemyDrops`. Each enemy tier has rows in `Table_EnemyDrops` controlling what categories drop and in what quantities.

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

## Reference: Boss → Legendary Mappings

Extracted from `itempoollist.bin`. Each boss has 1--3 dedicated legendary drops. The first item listed is Primary tier (6%), second is Secondary (4.5%), third is Tertiary (3%).

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

---

## Reference: Item Pools

### Boss Pool Lists

| Pool | Description |
|------|-------------|
| ItemPoolList_Enemy_BaseLoot_Boss | Standard boss drops |
| ItemPoolList_Enemy_BaseLoot_BossRaid | Raid boss drops |
| ItemPoolList_Enemy_BaseLoot_BossMini | Mini-boss drops |
| ItemPoolList_Enemy_BaseLoot_BossVault | Vault boss drops |
| ItemPoolList_*_TrueBoss | True Boss (Chaos mode) variants |

### Rarity-Tiered Weapon Pools

| Pool | Rarity |
|------|--------|
| `itempool_guns_01_common` | Common |
| `itempool_guns_02_uncommon` | Uncommon |
| `itempool_guns_03_rare` | Rare |
| `itempool_guns_04_epic` | Epic |
| `itempool_guns_05_legendary` | Legendary |

### Special Pools

| Pool | Description |
|------|-------------|
| ItemPool_FishCollector_Reward_Legendary | Fish collector reward |
| ItemPool_BlackMarket_Comp_BOR_HW_DiscJockey | Black Market exclusive |
| ItemPool_BlackMarket_Comp_BOR_HW_Streamer | Black Market exclusive |

---

## Reference: Drop Source Types

| Type | Description |
|------|-------------|
| Boss | Dedicated boss drop with per-kill tier probability |
| Mission | Side/main mission reward (guaranteed on completion) |
| BlackMarket | Black Market vendor exclusive |
| Special | Fish Collector, challenges, event rewards |
| WorldDrop | General legendary pool (rarity-weighted) |

---

## Reference: Item Composition in NCS

Legendary items follow a consistent naming pattern:

```
MANUFACTURER_TYPE.comp_05_legendary_NAME
```

Examples: `JAK_SG.comp_05_legendary_Hellwalker`, `MAL_SM.comp_05_legendary_PlasmaCoil`, `DAD_SHIELD.comp_05_legendary_angel`.

| Component | Rarity |
|-----------|--------|
| `comp_01_common` | Common |
| `comp_02_uncommon` | Uncommon |
| `comp_03_rare` | Rare |
| `comp_04_epic` | Epic |
| `comp_05_legendary` | Legendary |

---

## Reference: Codes

### Weapon Types

| Code | Type |
|------|------|
| AR | Assault Rifle |
| PS | Pistol |
| SG | Shotgun |
| SM | SMG |
| SR | Sniper Rifle |
| HW | Heavy Weapon |

### Manufacturers

| Code | Manufacturer | Specialty |
|------|--------------|-----------|
| BOR | Ripper | - |
| DAD | Daedalus | Shields |
| JAK | Jakobs | High damage, slow fire |
| MAL | Maliwan | Elemental |
| ORD | Order | - |
| TED | Tediore | Throw-to-reload |
| TOR | Torgue | Explosives |
| VLA | Vladof | High fire rate |

### Gear Slots

| Slot | Type Code |
|------|-----------|
| Weapon 1--4 | AR, PS, SG, SM, SR, HW |
| Shield | SHIELD |
| Grenade | GRENADE |
| Class Mod | CM |
| Artifact | ARTIFACT |
| Repair Kit | RK, REPAIR_KIT |

---

## Reference: Drops Manifest Format

The `drops.json` manifest generated by `bl4 drops generate` contains:

```json
{
  "version": 1,
  "probabilities": {
    "Primary": 0.06,
    "Secondary": 0.045,
    "Tertiary": 0.03,
    "Shiny": 0.01,
    "TrueBoss": 0.25,
    "TrueBossShiny": 0.03
  },
  "drops": [
    {
      "source": "MeatheadRider_Jockey",
      "source_display": "Saddleback",
      "source_type": "Boss",
      "manufacturer": "JAK",
      "gear_type": "SG",
      "item_name": "Hellwalker",
      "item_id": "JAK_SG.comp_05_legendary_Hellwalker",
      "pool": "itempool_jak_sg_05_legendary_Hellwalker_shiny",
      "drop_tier": "Primary",
      "drop_chance": 0.06
    }
  ]
}
```

The companion `drop_pools.tsv` summarizes legendary counts and boss source counts per manufacturer/weapon type pool. It is embedded at compile time for the rarity estimation API.

---

## Reference: Loot System Classes

Classes relevant to the loot pipeline, discovered via memory analysis:

### Pool System

| Class | Role |
|-------|------|
| ItemPoolDef | Defines a loot pool (`/Script/GbxGame.ItemPoolDef`) |
| ItemPoolEntry | Single item in a pool |
| ItemPoolListDef | Ordered list of pools (boss drop lists) |
| ItemPoolSelectorDef | Selection logic between pools |

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

### Rarity Resolution

| Class | Role |
|-------|------|
| RarityWeightData | Weight configuration (BaseWeight, GrowthExponent, GameStageVariance) |
| LocalRarityModifierData | Local rarity modifiers (area/event bonuses) |
| InventoryRarityDataTableValueResolver | Resolves rarity values from DataTables at runtime |
| InventoryRarityDef | Rarity tier definition |
| LootChanceDefinedValueRow | DataTable row for loot chance configuration |

### Luck System

| Class | Role |
|-------|------|
| LuckGlobals | Global luck settings |
| LuckCategoryDef | Luck category definition |
| LuckCategoryAttribute | Per-category luck attribute |
| LuckCategoryValueResolver | Resolves luck values at runtime |

Three luck category groups: `LuckCategories` (base), `EnemyBasedLuckCategories` (per-enemy), `PlayerBasedLuckCategories` (player-specific).

---

*Data from NCS file extraction and live memory analysis.*
