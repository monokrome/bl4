# Borderlands 4 Memory Structure

This document contains findings from memory analysis using scanmem and other tools. The goal is to understand how Borderlands 4 stores game data in memory, which will help us understand the save file format.

## Tools Used

- **scanmem/GameConqueror** - Memory scanning for runtime values
- **radare2** - Binary analysis and disassembly
- **gdb** - Runtime debugging and breakpoint analysis
- **strings** - String extraction from binaries
- **gcore** - Process memory dumps

## Game Process Information

- **Binary Path**: `/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Binaries/Win64/Borderlands4.exe`
- **Platform**: Windows (via Proton)
- **Architecture**: x64 PE32+
- **Engine**: Unreal Engine (based on .pak files)
- **Base Address**: 0x140000000

## Analysis Methodology

1. Attach scanmem to running Borderlands 4 process
2. Search for known values (character level, money, etc.)
3. Modify values in-game and re-scan to narrow down addresses
4. Document memory addresses, data types, and patterns
5. Look for structures and relationships between values
6. Create memory dumps at different character levels for comparison

## Memory Dumps

Full process memory dumps created using `gcore` for comparison analysis:

**Level 50 Characters (Hard difficulty):**
- `share/data/bl4_level50_amon_hard.6007` (22GB)
- `share/data/bl4_level50_harlowe_hard.6007` (22GB)
- `share/data/bl4_level50_rava_hard.6007` (21GB)
- `share/data/bl4_level50_vex_hard.6007` (22GB)

**Level 30 Character:**
- `share/data/bl4_level30_vex_unknown.6007` (21GB) - difficulty unknown

**Level 2 Character:**
- `share/data/bl4_level2_harlowe_easy.6007` (21GB) - easy difficulty

**Notes:**
- All dumps taken from process PID 6007 (Borderlands4.exe under Proton)
- Dumps include full process memory space including game data, Wine/Proton overhead
- Multiple level 50 dumps allow identifying common vs. character-specific data
- Level progression (2 → 30 → 50) helps verify discovered memory addresses

## Character Stats

### Level

**Discovery Method:**
- Started with level 50 character → 11,842,673 initial matches
- Filtered repeatedly while playing → ~600k matches
- Switched to level 30 character → 36 matches
- Switched to level 1 character → 4 final matches

**Candidate Addresses (all showed value 50 → 30 → 1):**

1. **Address 0x970d6698**
   - Region: 2566, Offset: +0x6698
   - Memory Type: misc
   - Possible Data Types: I32, I16, I8
   - Value: 50 (at time of discovery)

2. **Address 0xbf5d2ec0**
   - Region: 5797, Offset: +0x2ec0
   - Memory Type: misc
   - Possible Data Types: I64, I32, I16, I8
   - Value: 50 (at time of discovery)

3. **Address 0xc94e6750**
   - Region: 6468, Offset: +0x26750
   - Memory Type: misc
   - Possible Data Types: I32, I16, I8
   - Value: 50 (at time of discovery)

4. **Address 0x1516719a0**
   - Region: 11009, Offset: +0x4df9a0
   - Memory Type: misc
   - Possible Data Types: I64, I32, I16, I8
   - Value: 50 (at time of discovery)

**Data Type:** Likely I32 (int32) - most common for level values
**Size:** 4 bytes (if I32)
**Range:** 1-50 (observed)
**Notes:**
- All 4 addresses consistently changed with character level (50→30→1)
- **VERIFIED via core dump analysis:** All 4 addresses contain correct level values:
  - Level 50 dumps (amon, harlowe, rava, vex): all show 50
  - Level 30 dump (vex): all show 30
  - Level 2 dump (harlowe): all show 2
- All 4 addresses are legitimate level storage locations (likely for different purposes: UI, stats, calculations)
- Addresses are in different memory regions - may represent different contexts
- Running under Proton/Wine - absolute addresses will differ on native Windows
- **XP-driven:** Level cannot be directly modified; it's calculated from XP. Must modify XP and trigger level-up.

### Experience Points (XP)
- **Address**:
- **Data Type**:
- **Size**:
- **Range**:
- **Notes**:

### Health
- **Current Health Address**:
- **Max Health Address**:
- **Data Type**:
- **Size**:
- **Notes**:

### Shield
- **Current Shield Address**:
- **Max Shield Address**:
- **Data Type**:
- **Size**:
- **Notes**:

### Skill Points
- **Available Points Address**:
- **Data Type**:
- **Size**:
- **Notes**:

## Currency & Resources

### Money

**Discovery Method:**
- Searched for current cash value (30,335,188) → 4 initial matches
- Value changed to 30,352,148 during search

**Candidate Addresses:**

1. **Address 0xd476dcb0**
   - Region: 7333, Offset: +0x8dcb0
   - Memory Type: misc
   - Possible Data Types: I64, I32
   - Value: 30,352,148 (at time of discovery)

2. **Address 0x1950379d0**
   - Region: 13757, Offset: +0x179d0
   - Memory Type: misc
   - Possible Data Types: I64, I32
   - Value: 30,352,148 (at time of discovery)

3. **Address 0x1cb02a3b0**
   - Region: 17224, Offset: +0x1a3b0
   - Memory Type: misc
   - Possible Data Types: I64, I32
   - Value: 30,352,148 (at time of discovery)

**Data Type:** Likely I32 (int32) - common for currency values
**Size:** 4 bytes (if I32)
**Range:** 0 to ~2 billion (I32 max)
**Notes:**
- 3 addresses found on first search
- Can verify by buying/selling items to change value
- Multiple addresses likely for UI display, stats, and actual storage

### Eridium
- **Address**:
- **Data Type**:
- **Size**:
- **Range**:
- **Notes**:

### Golden Keys
- **Address**:
- **Data Type**:
- **Size**:
- **Range**:
- **Notes**:

## Inventory

### Weapon Slots

#### Weapon 3 - Ammo Reserve

**Discovery Method:**
- Searched for 240 (current ammo) → 6 initial matches
- Equipped different weapon → 2 matches remained

**Confirmed Addresses:**

1. **Address 0xa67e46c8**
   - Region: 3769, Offset: +0x46c8
   - Memory Type: misc
   - Possible Data Types: I32, I16, I8u
   - Value: 240 (at time of discovery)

2. **Address 0xbbf03640**
   - Region: 5509, Offset: +0x13640
   - Memory Type: misc
   - Possible Data Types: I64, I32, I16, I8u
   - Value: 240 (at time of discovery)

**Data Type:** Likely I32 (int32) or I16 (int16) for ammo counts
**Notes:**
- Verified by equipping different weapon - only these 2 addresses persisted
- Both addresses represent weapon slot 3's ammo reserves
- **Attempted modification:** Set both to 288, but values reset on reload
- These are **cached/display values** - source of truth is elsewhere (inventory structure or save file)
- For persistent ammo modification, need to modify underlying inventory data structure

### Other Weapon Slots
- **Slot 1 Address**:
- **Slot 2 Address**:
- **Slot 4 Address**:
- **Structure**:
- **Notes**:

### Backpack
- **Capacity Address**:
- **Item Count Address**:
- **Item Array Start**:
- **Item Structure**:
- **Notes**:

### Bank
- **Capacity Address**:
- **Item Count Address**:
- **Item Array Start**:
- **Notes**:

## Item Structure

### Item Data Format
- **Size per item**:
- **Fields**:
  - Offset 0x00:
  - Offset 0x04:
  - Offset 0x08:
  - ...
- **Notes**:

## Story Progress

### Mission/Quest Data
- **Base Address**:
- **Structure**:
- **Notes**:

### Fast Travel Locations
- **Base Address**:
- **Structure**:
- **Notes**:

### Playthroughs
- **Current Playthrough Address**:
- **Data Type**:
- **Notes**:

## Skill Trees

### Active Skills
- **Skill Tree Base Address**:
- **Structure**:
- **Notes**:

## Platform Offset Comparison

This table tracks memory offsets across different platforms for in-memory editing.

| Field | Wine/Proton Address | Wine/Proton Offset | Win64 Address | Win64 Offset | Data Type | Size | Notes |
|-------|---------------------|-----------------------|---------------|--------------|-----------|------|-------|
| Level (candidate 1) | 0x970d6698 | Region 2566 +0x6698 | TBD | TBD | I32 | 4 | |
| Level (candidate 2) | 0xbf5d2ec0 | Region 5797 +0x2ec0 | TBD | TBD | I32 | 4 | |
| Level (candidate 3) | 0xc94e6750 | Region 6468 +0x26750 | TBD | TBD | I32 | 4 | |
| Level (candidate 4) | 0x1516719a0 | Region 11009 +0x4df9a0 | TBD | TBD | I32 | 4 | |
| XP | TBD | TBD | TBD | TBD | TBD | TBD | |
| Money | TBD | TBD | TBD | TBD | TBD | TBD | |
| Eridium | TBD | TBD | TBD | TBD | TBD | TBD | |
| Golden Keys | TBD | TBD | TBD | TBD | TBD | TBD | |
| Current Health | TBD | TBD | TBD | TBD | TBD | TBD | |
| Max Health | TBD | TBD | TBD | TBD | TBD | TBD | |
| Current Shield | TBD | TBD | TBD | TBD | TBD | TBD | |
| Max Shield | TBD | TBD | TBD | TBD | TBD | TBD | |
| Skill Points | TBD | TBD | TBD | TBD | TBD | TBD | |

**Notes:**
- Absolute addresses will differ due to ASLR (Address Space Layout Randomization)
- Focus on relative offsets from base addresses for portability
- Region numbers are Wine/Proton-specific and won't apply to native Windows
- Win64 offsets will be verified once we move to native Windows testing

## Character Data Structure

**STATUS: UNTESTED - May be visual/cached values only**

Like the ammo addresses, these may be display/cache values that reset on reload.
Need to test modification persistence before confirming these are the base data structures.

**Base Address Example:** 0x970d6698 (Wine/Proton, will vary due to ASLR)

**Structure Layout:**

```c
struct PlayerStats {
    // Offset 0x00 - Character Level
    int32_t level;                    // Values: 2, 30, 50 (confirmed)

    // Offset 0x04 - Skill Points (CONFIRMED character-specific)
    int32_t skill_points;             // Level 2: 26, Level 30: 20
                                      // Level 50 varies by character:
                                      //   Amon: 12, Harlowe: 8, Rava: 16, Vex: 4
                                      // Likely available/unspent skill points
                                      // This value is repeated at +0xac and +0xcc

    // Offset 0x08 - Unknown Float (XP Progress?)
    float unknown_08;                 // Values: 27.57 (lvl2), 0.507 (lvl30), 0.0 (lvl50)
                                      // Possibly XP progress to next level?
                                      // Level 50 = 0.0 (max level, no next level)

    // Offset 0x0C - Unknown Integer
    int32_t unknown_0c;               // Values: 8 (lvl2), 3 (lvl30), 1 (lvl50)
                                      // Decreases as level increases

    // More fields follow...
};
```

### Memory Hex Dumps

#### Address 0x970d6698 across character levels:

**Level 50 (Amon - Hard):**
```
0x970d6698:  0x00000032  0x0000000c  0x00000000  0x00000001
0x970d66a8:  0x00345753  0x00000000  0x0000000c  0x00000000
0x970d66b8:  0x00000000  0x00000000  0x00000001  0x00000000
0x970d66c8:  0x0002fbd8  0x00000000  0x0000000c  0x00000000
```

**Level 30 (Vex - Unknown difficulty):**
```
0x970d6698:  0x0000001e  0x00000014  0x3f01c8d7  0x00000003
0x970d66a8:  0x000c8872  0x00000000  0x00000015  0x00000000
0x970d66b8:  0x00000190  0x00000000  0x00000003  0x00000000
0x970d66c8:  0x00013434  0x00000000  0x00000014  0x00000000
```

**Level 2 (Harlowe - Easy):**
```
0x970d6698:  0x00000002  0x0000001a  0x41dc2897  0x00000008
0x970d66a8:  0x0000044c  0x00000000  0x0000001c  0x00000000
0x970d66b8:  0x000000f3  0x00000000  0x00000008  0x00000000
0x970d66c8:  0x00000373  0x00000000  0x0000001a  0x00000000
```

#### Same Level, Different Characters (Level 50):

**Amon (Level 50 - Hard):**
```
0x970d6698:  0x00000032  0x0000000c  0x00000000  0x00000001
0x970d66a8:  0x00345753  0x00000000  0x0000000c  0x00000000
```

**Harlowe (Level 50 - Hard):**
```
0x970d6698:  0x00000032  0x00000008  0x00000000  0x00000001
0x970d66a8:  0x00345753  0x00000000  0x00000008  0x00000000
```

**Rava (Level 50 - Hard):**
```
0x970d6698:  0x00000032  0x00000010  0x00000000  0x00000001
0x970d66a8:  0x00345753  0x00000000  0x00000010  0x00000000
```

**Vex (Level 50 - Hard):**
```
0x970d6698:  0x00000032  0x00000004  0x00000000  0x00000001
0x970d66a8:  0x00345753  0x00000000  0x00000004  0x00000000
```

**Observations:**
- Level (+0x00) identical: 0x32 (50)
- Skill points (+0x04) vary: 12, 8, 16, 4 (character-specific)
- XP progress (+0x08) identical: 0.0 (max level)
- Unknown (+0x0c) identical: 1
- Constant (+0xa8) identical: 0x00345753 (possibly class type or game constant)
- Skill points repeated at +0xac and +0xcc

## Drop Rate Research

### Approach

1. Use scanmem to find drop rate values in memory
2. Track memory addresses where drop rates are stored
3. Identify data structures and tables
4. Document memory offsets and patterns
5. Use gdb to trace RNG callsites and identify loot generation code

### Memory Regions

**Cosmetics/Material Data** (~0x10e2e9000 region)
- Contains Material Instance Dynamic (MID) references
- Character skin/clothing data ("MID_MI_DarkSiren", "Siren_Jacket_ClothSim")
- Item serials stored as strings in this region
- Note: Decoded item stats NOT stored adjacent to serials

### RNG Callsites

Most frequent (likely graphics/particles):
- `0x1464803a4` - Called 7779 times (most common)
- `0x148f0c97a` - Called 213 times
- `0x146508f7e` - Called 188 times

Infrequent callsites (potential loot generation):
- `0x1478c5ea7` - Called 1 time ⭐
- `0x149bed3c7` - Called 3 times ⭐
- `0x14b219c28` - Called 3 times ⭐
- `0x145e3c144` - Called 4 times ⭐
- `0x147a23b14` - Called 4 times ⭐
- `0x148e2ceaa` - Called 4 times ⭐
- `0x148f435a4` - Called 4 times ⭐
- `0x1495a45e1` - Called 5 times ⭐

### Boss Loot RNG Code (Verification Needed)

Address `0x1478c5ea1` was confirmed as boss loot RNG code 2 months ago:
- Implements `(rand() / RAND_MAX) * range` pattern
- Modifying to return 0.91 made all boss drops legendary
- May have changed with game patches - needs verification

**Test Procedure:**
1. Load `.tmp/boss_loot_test.gdb` script in gdb
2. Manually enable breakpoint before boss kill: `enable 1`
3. Kill boss and observe hits
4. Disable after: `disable 1`
5. Check `.tmp/boss_loot_hits.log` for RNG values

**Assembly (0x1478c5e96 - 0x1478c5ec3):**
```
mov esi, [rsp+0x48]     ; Load range parameter
call rand               ; 0x1478c5ea1
and eax, 0x7fff
cvtsi2ss xmm0, eax      ; Convert to float
divss xmm0, [const]     ; Normalize (divide by RAND_MAX)
cvtsi2ss xmm1, esi      ; Convert range to float
mulss xmm1, xmm0        ; result = normalized_rand * range
cvttss2si eax, xmm1     ; Convert back to int
```

Next step: Verify address still controls boss loot, then trace back to find where drop rate thresholds are compared.

## Patterns & Observations

### Data Structure Patterns
- Multiple copies of same value (e.g., skill points at +0x04, +0xac, +0xcc) suggest caching or context-specific storage
- Many discovered addresses are cached/display values that don't persist modifications
- True data source likely in serialized inventory structures or save file

### Pointer Chains
- Need to identify base pointers to structures rather than direct addresses
- ASLR makes absolute addresses unreliable

### Array Structures
- Item serials stored as strings in memory
- Decoded item stats not adjacent to serial strings

## scanmem Commands Reference

```bash
# Attach to process
scanmem <pid>

# Search for specific value
> <value>

# Filter results by new value
> <new_value>

# List matches
> list

# Write to address
> set <address>=<value>
```

## Open Questions

- Where is the true XP value stored (not just display)?
- How to find base pointers to structures instead of cached values?
- What is the complete character data structure size?
- How are item stats decoded from serials in memory?
- Has boss loot RNG address changed since last verification?
- Where are drop rate thresholds stored?

## Next Steps

1. Verify boss loot RNG address (0x1478c5ea1) still functional
2. Locate XP storage for level manipulation
3. Find base pointers to character data structures
4. Map complete character structure beyond first few fields
5. Test modification persistence for discovered addresses
6. Compare memory structure to save file data
7. Identify encryption boundaries and data serialization format
8. Use radare2 to find class definitions and confirm field names
