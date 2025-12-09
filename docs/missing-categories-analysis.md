# Missing Part Categories Analysis

## Summary

The part category mappings in bl4 are **hardcoded** based on initial serial analysis, not extracted from game data. This document details the missing categories and their likely mappings.

## Current Category Mappings

From `/crates/bl4-cli/src/main.rs` lines 2100-2159:

| Range | Type | Categories |
|-------|------|------------|
| 2-7 | Pistols | DAD, JAK, TED, TOR, ORD, VLA |
| 8-12 | Shotguns | DAD, JAK, TED, TOR, BOR |
| 13-18 | Assault Rifles | DAD, JAK, TED, TOR, VLA, ORD |
| 19 | GAP | ? |
| 20-23 | SMGs | DAD, BOR, VLA, MAL |
| 24-25 | GAP | ? |
| 26-29 | Snipers | JAK, VLA, ORD, MAL |
| 30-243 | GAP | |
| 244-247 | Heavy Weapons | VLA, TOR, BOR, MAL |
| 248-278 | GAP | |
| 279-288 | Shields | energy, bor, dad, jak, armor, mal, ord, ted, tor, vla |
| 289 | GAP | ? |
| 290-299 | GAP | |
| 300-330 | Gadgets | grenade(300), turret(310), repair(320), terminal(330) |
| 331-399 | GAP | |
| 400-409 | Enhancements | DAD, BOR, JAK, MAL, ORD, TED, TOR, VLA, COV, ATL |

## Missing Weapon Type Mappings

From `share/manifest/parts_dump.json`, these prefixes exist but have no category:

| Prefix | Parts Count | Likely Category |
|--------|-------------|-----------------|
| MAL_SG | 74 | 19 (gap before SMGs) |
| bor_sr | 71 | 24 or 25 (gap before snipers) |

## Missing Categories from Serial Validation

From test output in `cargo test -p bl4 validate`:

### Weapon Serials (Type 'r'):
- Category 25: Unknown - indices 0, 13, 184 (likely bor_sr)

### Equipment Serials (Type 'e'):
From decoded serials with first token / 384:

| Token | Category | Possible Type |
|-------|----------|---------------|
| 17088 | 44 | Class Mod (Dark Siren?) |
| 21184 | 55 | Class Mod (Paladin?) |
| 37248 | 97 | Class Mod (Gravitar?) |
| 53760 | 140 | Class Mod (Exo Soldier?) |
| 57984 | 151 | Firmware? |
| 111296 | 289 | Shield variant? |

## Evidence from Memory Dump

Part names found in memory include:
- `classmod_dark_siren.passive_*`
- `classmod_paladin.passive_*`
- `classmod_exo_soldier.passive_*`
- `classmod_gravitar.part_*`
- `firmware.*`

## Data Location

The authoritative source for category mappings is the `GbxSerialNumberIndex` structure embedded in `InventoryPartDef` UObject instances:

```
GbxSerialNumberIndex:
  Category  : Int64   <- Part Group ID
  scope     : Byte    <- Root/Sub scope
  status    : Byte    <- Active/Static/etc.
  Index     : Int16   <- Part index within group
```

**Problem**: Part definitions are compiled into game code and NOT stored in pak files. They can only be extracted from memory at runtime.

## Recommended Actions

### Immediate (Pattern-based guesses):

1. Add to `known_groups` in main.rs:
   ```rust
   ("MAL_SG", 19, "Maliwan Shotgun"),
   ("bor_sr", 24, "Bor Sniper"),  // or 25
   ```

2. Add class mod categories (needs verification):
   ```rust
   ("classmod_dark_siren", 44, "Dark Siren Class Mod"),
   ("classmod_paladin", 55, "Paladin Class Mod"),
   ("classmod_gravitar", 97, "Gravitar Class Mod"),
   ("classmod_exo_soldier", 140, "Exo Soldier Class Mod"),
   ```

3. Add firmware/shield variant:
   ```rust
   ("firmware", 151, "Firmware"),
   // 289 might be another shield subtype
   ```

### Long-term (Accurate extraction):

Implement UObject scanning in memory.rs:
1. Find `InventoryPartDef` UClass pointer
2. Walk `GUObjectArray` for all instances
3. For each instance, read `SerialIndex.Category`
4. Correlate with object name FName
5. Build complete category->prefix mapping

## Files to Update

- `/crates/bl4-cli/src/main.rs` - Add to `known_groups` vector
- `/crates/bl4/src/parts.rs` - Add to `category_name()` function
- `share/manifest/parts_database.json` - Regenerate with `build-parts-db`

## Verification Method

After adding mappings, run:
```bash
cargo test -p bl4 validate -- --nocapture
```

Look for reduction in "Missing parts by category" output.
