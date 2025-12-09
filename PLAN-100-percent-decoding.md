# Plan: 100% Reliable Item Serial Decoding

## Current State (Updated Dec 2025)

Based on test validation:
- **Weapons:** ~75% parts found (after adding MAL_SG cat 19, bor_sr cat 25)
- **Equipment:** ~16% parts found (class mod categories still missing)

### Recent Investigation Results

**UObject-Based Extraction Attempt (FAILED)**

We attempted to extract category mappings directly from `InventoryPartDef` UObjects by reading their `SerialIndex.Category` property. Results:

1. ✅ GUObjectArray successfully parsed (469,504 objects)
2. ✅ FNamePool multi-block resolution working (358 blocks)
3. ❌ InventoryPartDef exists as ScriptStruct, NOT as UClass
4. ❌ Part definitions not registered in GUObjectArray
5. ❌ Part names exist in FName pool but without UObject instances

**Conclusion:** Part definitions appear to be compiled into game code as static data rather than instantiated as UObjects at runtime. The UObject-based extraction approach will not work.

**New CLI Commands Added:**
- `bl4 memory generate-object-map` - Creates JSON map of all UObjects for fast lookups
- `bl4 memory find-objects-by-pattern` - Searches for objects by name pattern
- `bl4 memory extract-parts` - Attempted UObject extraction (non-functional due to above)

## Root Causes Analysis

### 1. Missing Part Categories (HIGH PRIORITY)

The parts database lacks complete coverage. Test output shows:

**Weapons - Missing categories:**
- Category 25: Unknown category (indices 0, 13, 184) - likely a weapon subtype not in memory dump

**Equipment - Missing categories (critical gaps):**
- Category 44: Unknown (indices 0, 2, 254)
- Category 55: Unknown (indices 0, 5)
- Category 97: Unknown (indices 10, 254, 8, 254)
- Category 140: Unknown (indices 2, 5, 246)
- Category 151: Unknown (indices 1, 254)
- Category 289: Unknown (index 0)

**Note:** These categories exist in real item serials but weren't captured in the initial memory dump extraction.

### 2. Part Index Type Overflow (HIGH PRIORITY)

In `parts.rs:454` and `parts.rs:515`:
```rust
let idx = index as i16;  // u64 cast to i16
```

Equipment serials contain indices like **38,872** which overflows when cast to i16 (max 32,767). This causes:
- Silent wraparound to negative values
- Database lookups fail silently
- No error reported to user

### 3. High Part Indices (MEDIUM PRIORITY)

Many parts have indices > 100:
- Weapon category 22: index 252
- Weapon category 20: indices 241, 246
- Equipment category 279: index 250
- Equipment category 97: index 254

The memory dump may not have captured all parts, or these indices represent:
- Rare/legendary-only parts
- DLC parts
- Randomly-generated part suffixes

### 4. Part Group ID Calculation Edge Cases (LOW PRIORITY)

The current calculation:
```rust
// Weapons: group_id = first_varbit / 8192
// Equipment: group_id = first_varbit / 384
```

May not handle all item type variants correctly. Some serials may use different divisors.

---

## Implementation Plan

### Phase 1: Fix Type Safety Issues

**Task 1.1: Change part index storage from i16 to i32/u32**

Files: `crates/bl4/src/parts.rs`

- Change `PartEntry.index` from `i16` to `i32`
- Change `CategoryPartInfo.index` from `i16` to `i32`
- Update database key type from `(i64, i16)` to `(i64, i32)`
- Update JSON deserialization to expect i32

**Task 1.2: Add bounds checking on part lookups**

- Add validation when casting u64 -> i32
- Log warning when index exceeds reasonable bounds (e.g., > 65535)
- Return descriptive error for overflow cases

### Phase 2: Expand Parts Database

**Task 2.1: Identify missing categories**

Need to determine what categories 25, 44, 55, 97, 140, 151, 289 represent:
- Run game with debug logging to capture part pool loads
- Analyze pak manifest for ItemDef/PartDef references
- Cross-reference with usmap struct definitions

**Task 2.2: Extract complete part pools from new memory dump**

The current `parts_database.json` was extracted from a limited memory state. Need:
- Fresh memory dump with all item types in inventory
- Dump parts from multiple save files (different rarities, manufacturers)
- Specifically target equipment items (shields, grenades, enhancements)

**Task 2.3: Build comprehensive category mapping**

Update `share/manifest/parts_database.json` with:
- Category 25 (unknown weapon type)
- Category 44-55 (equipment subtypes)
- Category 97 (unknown)
- Category 140 (unknown)
- Category 151 (unknown)
- Category 289 (possible shield variant)

### Phase 3: Validate Part Group ID Calculation

**Task 3.1: Analyze edge cases**

Collect serials that decode incorrectly and analyze:
- Does the calculated group_id exist in our mapping?
- Are there alternative divisors for certain item types?
- Document any item types that don't follow the standard pattern

**Task 3.2: Update group_id calculation if needed**

If patterns emerge, update `ItemSerial::part_group_id()` to handle:
- Different divisors for different item type chars
- Special handling for utility items (type 'u')
- Class mods ('!' and '#') may have different encoding

### Phase 4: Improve Error Handling

**Task 4.1: Add detailed decode failure reporting**

- Return `Result<(ItemSerial, Vec<DecodeWarning>)>` instead of just `Result<ItemSerial>`
- Track which parts failed to resolve
- Report missing categories vs missing indices differently

**Task 4.2: Add validation mode**

New CLI flag `--validate` that:
- Attempts to resolve all parts
- Reports coverage statistics
- Identifies specific gaps

### Phase 5: Documentation & Testing

**Task 5.1: Expand test coverage**

- Add test serials for each known item type
- Add tests for edge cases (high indices, unknown categories)
- Regression tests for the 62%/15% baseline

**Task 5.2: Update CLAUDE.md**

- Document complete category mapping
- Document known limitations
- Track validation progress

---

## Success Criteria

1. **Weapon serial decoding:** 95%+ parts resolved
2. **Equipment serial decoding:** 90%+ parts resolved
3. **No silent failures:** All unresolved parts logged with category/index
4. **Type safety:** No integer overflow possible
5. **Complete category mapping:** All categories in test serials documented

---

## Investigation Tasks (Data Gathering)

Before implementation, need to:

1. **Analyze equipment serials with high indices (38,872)**
   - Is this a valid index or a parsing bug?
   - Could indicate wrong divisor for equipment group_id

2. **Map unknown categories to game content**
   - Category 25: Gap between SMGs (20-23) and Snipers (26-29)
   - Category 44-55: Between weapon high (247) and shield (279)
   - Category 97, 140, 151: Between shields (288) and gadgets (300)

3. **Fresh memory dump**
   - Ideally with varied inventory
   - Multiple rarity tiers
   - All equipment types equipped

---

## Priority Order (Revised)

1. ~~**UObject Extraction** - Extract from InventoryPartDef~~ **BLOCKED** - Parts not in GUObjectArray
2. **Manual Category Mapping** - Reverse engineer equipment categories from serials
3. **Phase 1** - Type safety (prevents crashes, reveals true failure points)
4. **Phase 2.3** - Update database with manual mappings
5. **Phase 4** - Better error reporting
6. **Phase 5** - Documentation

---

## Alternative Approaches (Since UObject extraction failed)

### Option A: Pak File Parsing
- Parse `.pak` files directly for `InventoryPartDef` DataTable assets
- UAsset format may contain serialized SerialIndex data
- Requires understanding Gearbox's custom asset formats

### Option B: Manual Category Inference
- Collect more sample serials with known item types
- Correlate decoded category IDs with visual inspection in-game
- Build mappings incrementally through testing

### Option C: Runtime Hook / Debugging
- Attach debugger to running game
- Set breakpoints on part serialization functions
- Capture category values as they're accessed

### Option D: Community Data
- Check if BL3/BLTPS tools have category mappings
- Gearbox uses similar systems across games
- May be able to adapt existing mappings

---

## Known Equipment Category Mappings (To Investigate)

From decoded equipment serials, these categories need identification:

| Category | Likely Type | Evidence |
|----------|-------------|----------|
| 44 | Class Mod (Dark Siren?) | Token values in class mod serials |
| 55 | Class Mod (Paladin?) | Similar pattern to cat 44 |
| 97 | Class Mod (Gravitar?) | Equipment serial with classmod prefix |
| 140 | Class Mod (Exo Soldier?) | Fourth character class |
| 151 | Firmware | `firmware.*` part names in FName pool |
| 289 | Shield variant? | Near shield range (279-288) |

---

## Notes

The 38,872 index issue was investigated - it was a parser over-parsing bug that has been fixed. The parser was reading additional VarBit tokens beyond the actual part indices.

~~The 38,872 index in equipment decoding (category 23 = Maliwan SMG) is suspicious - category 23 is a weapon category, but the serial type is 'e' (equipment). This suggests:~~
~~- The Part Group ID calculation may be wrong for some equipment~~
~~- Or the serial is malformed/special case~~

### Technical Details from Investigation

**GbxSerialNumberIndex Structure (12 bytes):**
```
offset 0x00: category (i64) - Part Group ID
offset 0x08: scope    (u8)  - Root/Sub scope indicator
offset 0x09: status   (u8)  - Active/Static/etc.
offset 0x0A: index    (i16) - Part index within group
```

**InventoryPartDef Class Hierarchy:**
```
UObject
  └─ GbxHasStructType
       └─ GbxDef
            └─ GbxSerialNumberAwareDef (contains SerialIndex property)
                 └─ InventoryPartDef
```

**Why Parts Aren't in GUObjectArray:**
- Part definitions are blueprint-compiled assets
- They're loaded as DataTable entries, not individual UObjects
- The SerialIndex is stored inline in the DataTable rows
- Only aggregate container objects appear in GUObjectArray
