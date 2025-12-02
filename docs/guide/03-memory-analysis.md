# Chapter 3: Memory Analysis

Memory analysis lets you see the game's live state—every weapon, every enemy, every stat. This chapter covers the techniques we used to reverse engineer BL4.

---

## Why Memory Analysis?

Some data can't be found in files:
- **Runtime values** — Current health, ammo count, equipped items
- **Decrypted data** — Save files are encrypted, memory isn't
- **Dynamic structures** — Object pools, linked lists, runtime allocations
- **Type information** — UE5 reflection data for usmap generation

!!! note
    Memory analysis complements static analysis (examining files). Use both together for best results.

---

## Getting a Memory Dump

### Windows: Process Hacker

1. Install [Process Hacker](https://processhacker.sourceforge.io/)
2. Run BL4, load a character with items you want to analyze
3. Right-click the game process → "Create dump file"
4. Choose "Full memory dump" (will be 15-25 GB)

### Windows: procdump

```bash
# Sysinternals procdump
procdump -ma Borderlands4.exe bl4_dump.dmp
```

### Linux (Proton): gcore

```bash
# Find the wine64-preloader process
ps aux | grep -i borderlands

# Create core dump (replace PID)
sudo gcore -o bl4_dump 107180

# Output: bl4_dump.107180
```

!!! warning
    Memory dumps are LARGE (15-25 GB for BL4). Ensure you have disk space and patience.

---

## Memory Dump Formats

### Windows MDMP (Minidump)

Contains memory regions plus metadata:
- Thread states
- Module list (loaded DLLs)
- Memory region descriptors
- Exception information (if crashed)

### Linux Core Dump

ELF format containing:
- Memory segments
- Register states
- Process metadata

### Raw Dump

Just bytes—no metadata. Simpler but requires knowing base addresses.

!!! tip
    The bl4 tools accept both MDMP and raw formats. Use `--dump` to specify the file.

---

## Virtual Memory Layout

Modern processes use virtual memory—addresses you see aren't physical RAM addresses.

### Windows 64-bit Layout (BL4)

| Address Range | Purpose |
|---------------|---------|
| `0x140000000` - `0x14e61c000` | Executable code (.text) |
| `0x14e61c000` - `0x15120e000` | Read-only data (.rdata) |
| `0x15120e000` - `0x15175c000` | Writable data (.data, .bss) |
| `0x15175c000` - `0x172d73000` | Additional sections |
| Heap regions (varying) | Dynamic allocations |
| Stack regions (varying) | Thread stacks |

### Key Regions for BL4

| Region | Contents |
|--------|----------|
| Code section | Game logic, UE5 engine code |
| Read-only data | Strings, vtables, constants |
| Writable data | Global variables, GNames, GUObjectArray |
| Heap | UObjects, game data, items |

---

## Finding Global Structures

### Pattern Scanning

Search for known byte sequences:

```bash
# Find "@Ug" item serial prefix in dump
grep -boa '@Ug' dump.raw | head -20

# Find "AES-256-ECB" save marker
grep -boa 'AES-256-ECB' dump.raw
```

### String-Based Discovery

FNamePool always starts with predictable strings:

```bash
# Find GNames by searching for first entries
grep -boa 'None' dump.raw | while read offset; do
    # Check if "ByteProperty" follows
    ...
done
```

### The bl4 Approach

Our tooling automates this:

```bash
# Discover GNames location
bl4 memory --dump share/dumps/game.raw discover gnames

# Output:
# Found FNamePool at 0x1512a1c80
# Block count: 356
# Verified: FName[0] = "None"
```

---

## Reading Process Memory

### Structure of a Read

To read a value from memory:

```rust
// 1. Calculate address
let base_address = 0x1513878f0;  // GUObjectArray
let offset = 0x08;                // Count field

// 2. Read bytes
let bytes = memory.read(base_address + offset, 4)?;

// 3. Interpret as type
let count = u32::from_le_bytes(bytes.try_into()?);
```

### Following Pointers

Most data is accessed through chains of pointers:

```rust
// To find a weapon's damage:

// 1. Start at GUObjectArray
let guobjectarray = 0x1513878f0;

// 2. Get first chunk pointer
let chunk0_ptr = memory.read_u64(guobjectarray)?;

// 3. Find weapon object (assume index 1234)
let item_ptr = chunk0_ptr + (1234 * 24);  // FUObjectItem is 24 bytes
let weapon_ptr = memory.read_u64(item_ptr)?;

// 4. Read damage at known offset
let damage = memory.read_f32(weapon_ptr + 0xC40)?;
```

!!! tip
    Keep a "pointer chain" diagram as you explore. It's easy to lose track of what points to what.

---

## The bl4 Memory Command

### Reading Raw Memory

```bash
# Read 64 bytes at address
bl4 memory --dump share/dumps/game.raw read 0x1513878f0 --size 64

# Output (hex dump):
# 0x1513878f0: 00 3C B5 91 01 00 00 00  45 26 18 00 00 00 00 00  .<......E&......
# 0x151387900: 00 02 00 00 07 00 00 00  ...
```

### Discovering Structures

```bash
# Find GNames
bl4 memory --dump share/dumps/game.raw discover gnames

# Find GUObjectArray
bl4 memory --dump share/dumps/game.raw discover guobjectarray

# Dump all FNames
bl4 memory --dump share/dumps/game.raw dump-names --limit 100
```

### Generating Usmap

```bash
# Full usmap generation from memory dump
bl4 memory --dump share/dumps/game.raw dump-usmap

# Output: mappings.usmap
# Names: 64917, Enums: 2986, Structs: 16849, Properties: 58793
```

---

## Pointer Validation

Not every 8-byte value is a valid pointer. Use validation:

### Valid Pointer Ranges

```rust
const MIN_VALID_POINTER: u64 = 0x10000;          // Below this is null/guard pages
const MAX_VALID_POINTER: u64 = 0x800000000000;   // User space limit

fn is_valid_pointer(addr: u64) -> bool {
    addr >= MIN_VALID_POINTER && addr < MAX_VALID_POINTER
}
```

### VTable Validation

UObjects have vtables pointing to code:

```rust
const CODE_START: u64 = 0x140001000;  // Start of .text
const CODE_END: u64 = 0x14e61c000;    // End of .text

fn is_valid_vtable(vtable_ptr: u64) -> bool {
    // VTable itself is in data section
    if !is_valid_pointer(vtable_ptr) {
        return false;
    }

    // First entry in vtable points to code
    let first_entry = memory.read_u64(vtable_ptr)?;
    first_entry >= CODE_START && first_entry < CODE_END
}
```

---

## Pattern Recognition

### Identifying UObjects

UObjects have recognizable patterns:

```
Offset 0x00: [8 bytes]  ← VTable (points to 0x140xxxxxx)
Offset 0x08: [4 bytes]  ← Flags (often 0x00000002)
Offset 0x0C: [4 bytes]  ← InternalIndex (small-ish integer)
Offset 0x10: [8 bytes]  ← ClassPrivate (pointer to another UObject)
Offset 0x18: [8 bytes]  ← NamePrivate (FName, small integer)
Offset 0x20: [8 bytes]  ← OuterPrivate (pointer or null)
```

### Identifying TArrays

Dynamic arrays have this pattern:

```
Offset 0x00: [8 bytes]  ← Data pointer (or null if empty)
Offset 0x08: [4 bytes]  ← Count (should be <= Max)
Offset 0x0C: [4 bytes]  ← Max (capacity, power of 2 often)
```

### Identifying Floats

Common float values in games:

| Pattern | Hex | Value | Common Use |
|---------|-----|-------|------------|
| `00 00 80 3F` | 0x3F800000 | 1.0 | Scale, multiplier |
| `00 00 00 40` | 0x40000000 | 2.0 | Damage multiplier |
| `00 00 C8 42` | 0x42C80000 | 100.0 | Health, percentages |
| `00 00 48 43` | 0x43480000 | 200.0 | Max values |

!!! tip
    `00 00 80 3F` (1.0f) appears constantly in game memory. It's a useful anchor point for finding related data.

---

## Item Serial Discovery

BL4 items in memory contain their serial strings:

### Finding Items by Serial

```bash
# Search for known item serial
grep -boa '@Ugr' dump.raw | head -5

# Examine context around first hit
xxd -s 0x14d21a8 -l 128 dump.raw
```

### Item Entry Structure (Linux/Proton)

```
Offset -0x08: 04 00 00 08 80 00 00 00   ← Header/flags
Offset +0x00: 40 55 67 72 24 5A 43...   ← Serial string "@Ugr$ZC..."
Offset +0x20: 00 00 00 00 00 00 00 00   ← Null padding
Offset +0x28: 00 00 00 00 00 00 80 3F   ← 1.0f (scale?)
```

### Correlating Items

Found two similar items? Compare their memory:

```bash
# Diff two memory regions
xxd -s 0x1000 -l 64 item1.bin > item1.txt
xxd -s 0x1000 -l 64 item2.bin > item2.txt
diff item1.txt item2.txt
```

Byte-by-byte differences reveal which bytes control which properties.

---

## Working with Memory Dumps

### Extracting Regions

```bash
# Extract specific region from dump
dd if=dump.raw of=region.bin bs=1 skip=$((0x1512a1c80)) count=$((0x10000))
```

### Searching for Strings

```bash
# Find all ASCII strings
strings -t x dump.raw | grep -i "damage"

# Find wide strings (UTF-16)
strings -t x -e l dump.raw | grep -i "weapon"
```

### Hex Pattern Search

```bash
# Find float 100.0 (0x42C80000)
xxd dump.raw | grep "00 c8 42 00"
```

---

## Live Process Analysis

Instead of dumps, you can read live process memory.

### Linux: /proc/pid/mem

```bash
# Find game PID
pgrep -f Borderlands4

# Read memory (requires root or same user)
sudo dd if=/proc/12345/mem bs=1 skip=$((0x1513878f0)) count=64 2>/dev/null | xxd
```

### Windows: ReadProcessMemory

```cpp
HANDLE hProcess = OpenProcess(PROCESS_VM_READ, FALSE, pid);
ReadProcessMemory(hProcess, (LPCVOID)0x1513878f0, buffer, 64, &bytesRead);
```

!!! warning
    Anti-cheat may detect process memory access. Use dumps for analysis, live access for verification only.

---

## Practical: Finding Health in Memory

Let's walk through finding a character's health value.

### Step 1: Get a Memory Dump

```bash
sudo gcore -o bl4 $(pgrep -f wine64-preloader)
```

### Step 2: Find AOakCharacter

We know the class hierarchy, so search for objects with that class:

```bash
# Use our tool to find objects
bl4 memory --dump bl4.dump list-objects --class AOakCharacter
```

### Step 3: Calculate Health Offset

From SDK analysis, we know:
- `AOakCharacter` has `HealthState` at offset 0x4640
- `HealthState` contains the current/max health

```bash
# Read at player character address + health offset
bl4 memory --dump bl4.dump read $(($PLAYER_ADDR + 0x4640)) --size 64
```

### Step 4: Interpret the Data

```
0x4640: [FGbxAttributeFloat Health]
  +0x00: padding/metadata
  +0x04: float CurrentValue
  +0x08: float BaseValue
```

---

## Exercises

### Exercise 1: Find Your Steam ID

BL4 save files are encrypted with your Steam ID. Find it in memory:

1. Create a memory dump while logged in
2. Search for your Steam ID as ASCII digits
3. Note the addresses where it appears

<details>
<summary>Hint</summary>

Your Steam ID appears in several places:
- Save file encryption context
- Steam API structures
- Player profile data

Search for the numeric string: `grep -boa '7656119xxxxxxxxxx' dump.raw`

</details>

### Exercise 2: Count Items

Find how many items are in your inventory:

1. Locate the player's inventory array (TArray)
2. Read the Count field at offset +0x08
3. Verify by counting items in-game

### Exercise 3: Find a Specific Weapon

1. Note a unique weapon's name in-game
2. Search for its FName in memory
3. Trace back to find the weapon UObject
4. Read its damage value

---

## Common Pitfalls

### ASLR

Address Space Layout Randomization changes base addresses each launch:

```
Session 1: GNames at 0x1512a1c80
Session 2: GNames at 0x1612a1c80  ← Different!
```

**Solution**: Use offsets from the PE base (0x140000000), which is usually fixed.

### Stale Data

Memory dumps are snapshots. Data changes constantly:

- Object indices change as things spawn/despawn
- Pointers become invalid when objects are freed
- Cached values may not reflect current state

**Solution**: Take multiple dumps at known game states for comparison.

### Wine/Proton Differences

Linux memory layouts differ slightly from native Windows:

| Aspect | Native Windows | Wine/Proton |
|--------|----------------|-------------|
| Dump format | MDMP | ELF core |
| Address mapping | Direct VA | May need translation |
| Memory layout | Standard PE | PE in Wine container |

---

## Key Takeaways

1. **Memory dumps capture live state** — See data that files can't show
2. **Pointer chains connect everything** — Learn to follow them
3. **Pattern recognition is key** — UObjects, arrays, floats have signatures
4. **Validate pointers** — Not every 8 bytes is an address
5. **Use the tools** — bl4 memory commands automate common tasks

---

## Next Chapter

Now that you can explore memory, let's look at BL4's save file format and how we decrypt it.

**Next: [Chapter 4: Save File Format](04-save-files.md)**
