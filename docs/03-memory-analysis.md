# Chapter 3: Memory Analysis

There's data you can find in files, and there's data you can only find in memory. Your character's current health, the weapon in your hand, the damage numbers floating off enemies—these exist only while the game runs. To see them, we need to look inside the running process.

This chapter teaches you to capture and navigate game memory. It's where theory becomes practice, where the patterns from Chapter 2 appear as real bytes you can read and interpret.

---

## Why Look at Memory?

Files are static. They contain assets, definitions, and saved states. But games are dynamic. At any moment, hundreds of objects exist in memory that don't correspond to any file: spawned enemies, equipped items, active effects, player stats.

Memory analysis lets you:

**See decrypted data.** Save files are encrypted, but the game has to decrypt them to use them. In memory, everything is plaintext.

**Find runtime structures.** The reflection system that tells us property offsets? It's in memory. The global arrays tracking every object? Memory. The name pool mapping indices to strings? Memory.

**Watch live changes.** Change your health in-game and watch the memory value update. This confirms your understanding and reveals how systems connect.

**Extract type information.** The usmap files that make pak parsing possible come from dumping the reflection system out of memory. There's no other source for this data.

---

## Capturing a Memory Dump

The first step is getting the game's memory into a file you can analyze offline. The process differs by platform, but the result is the same: a multi-gigabyte file containing everything the game had loaded.

**On Windows**, use Process Hacker or Sysinternals procdump. Right-click the game process, create a full memory dump. Expect 15-25 GB for BL4.

**On Linux with Proton**, use gcore. Find the wine64-preloader process ID, then:

```bash
sudo gcore -o bl4_dump $(pgrep -f wine64-preloader)
```

The dump takes a minute or two. When it finishes, you have a snapshot of everything—every weapon, every enemy, every byte of game state frozen in time.

---

## Virtual Memory and Address Space

When you see an address like `0x1513878f0`, that's a virtual address. It doesn't map directly to physical RAM—the operating system and CPU handle translation. What matters for reverse engineering is understanding the layout.

BL4 on Windows loads at a base address around `0x140000000`. From there:

```text
0x140000000 - 0x14e61c000   Executable code (.text section)
0x14e61c000 - 0x15120e000   Read-only data (.rdata)
0x15120e000 - 0x15175c000   Writable data (.data, .bss)
(varying addresses)         Heap allocations
(varying addresses)         Thread stacks
```

The code section contains compiled game logic. Read-only data holds strings, vtables, and constants. Writable data contains global variables—including the crucial GNames and GUObjectArray pointers we need. Heap allocations hold dynamic objects like weapons and characters.

Knowing these ranges helps validate pointers. If you think you've found a vtable pointer but it points to 0x300000000 (not in any valid range), you know you've misinterpreted something.

---

## Finding the Global Structures

Every Unreal game has two critical global structures we need to locate:

**GNames (FNamePool)**: The string pool where all names live. Without it, we can't resolve FName indices to actual strings.

**GUObjectArray**: The master list of all UObjects. It's our index to everything in the game.

The FNamePool has a predictable signature. The first few entries are always "None", "ByteProperty", "IntProperty", and so on—Unreal's built-in types. Searching for the byte sequence "None\0ByteProperty" gets you close.

The bl4 tools automate discovery:

```bash
bl4 memory --dump bl4_dump.core discover gnames
# Output: Found FNamePool at 0x1512a1c80

bl4 memory --dump bl4_dump.core discover guobjectarray
# Output: Found GUObjectArray at 0x1513878f0
```

Once you have these addresses, everything else becomes accessible. Need to find all weapons? Walk GUObjectArray, resolve each object's class name through GNames, filter for "Weapon". Need to know a property's offset? Find the UClass, walk its property chain, resolve names through GNames.

---

## Following Pointer Chains

Most interesting data requires following multiple pointers. Think of it as a treasure map where each step reveals the next.

```{mermaid}
flowchart LR
    A["GUObjectArray"] -->|"chunk[0]"| B["Object Item\n(24 bytes)"]
    B -->|"ptr at +0x00"| C["UObject\n(40 bytes)"]
    C -->|"ClassPrivate\n+0x10"| D["UClass"]
    C -->|"property offset"| E["Target Value\n(damage, health, etc.)"]
```

To find a weapon's damage value, the chain might be:

1. Start at GUObjectArray (known address)
2. Read the chunk pointer at offset 0x00
3. Calculate item offset: `chunk_ptr + (object_index * 24)`
4. Read the object pointer from the item
5. Read ClassPrivate at object + 0x10
6. Verify the class name is "Weapon"
7. Read the damage value at weapon + 0xC40

Each read requires careful interpretation. Is this a 4-byte integer or an 8-byte pointer? Little-endian, remember, so `78 56 34 12` is actually 0x12345678.

The bl4 tools handle this:

```bash
# Read 64 bytes at GUObjectArray
bl4 memory --dump bl4_dump.core read 0x1513878f0 --size 64

# Output shows the chunk pointer and element count
```

---

## Recognizing Patterns

After enough time in hex dumps, certain patterns become instant recognition.

**UObjects** start with a vtable pointer (usually 0x140xxxxxx or 0x141xxxxxx), followed by flags at +0x08, an internal index at +0x0C, class pointer at +0x10, FName at +0x18, and outer at +0x20. If you see that 40-byte header pattern, you're looking at a UObject.

**TArrays** are 16 bytes: a data pointer (or null), then a 4-byte count, then a 4-byte capacity. The count is always less than or equal to capacity. Capacity is often a power of 2.

**Floats** have recognizable patterns too. The value 1.0 is always `00 00 80 3F`. The value 100.0 is `00 00 C8 42`. When you're looking at unknown data and see `00 00 80 3F`, you've probably found a float field with value 1.0.

```text
Common float patterns:
0x3F800000 = 1.0    (scale, multiplier, percentage)
0x40000000 = 2.0    (damage multiplier, maybe)
0x42C80000 = 100.0  (health, percentage base)
0x43480000 = 200.0  (max values)
```

---

## The bl4 Memory Commands

The bl4 project provides commands for common memory operations:

**Reading raw memory:**
```bash
bl4 memory --dump bl4_dump.core read 0x1513878f0 --size 64
```

**Looking up FNames by string:**
```bash
bl4 memory --dump bl4_dump.core fname-search "Damage"
```

**Generating a usmap:**
```bash
bl4 memory --dump bl4_dump.core dump-usmap
# Creates mappings.usmap with all reflection data
```

**Searching for strings:**
```bash
bl4 memory --dump bl4_dump.core scan-string "DAD_AR.part_body" -B 128 -A 128
```

**Looking up an FName by index:**
```bash
bl4 memory --dump bl4_dump.core fname 12345
```

These commands encapsulate the pointer-chasing and interpretation logic, letting you focus on what you're trying to find rather than how to read it.

---

## Practical Example: Finding Item Serials

Item serials—those shareable weapon codes—exist in memory as strings. Finding them reveals where item data lives.

```bash
# Search for the @Ug prefix that starts all serials
grep -boa '@Ug' bl4_dump.core | head -10
```

Each hit is a potential item. Examine the surrounding bytes:

```bash
# Look at context around a hit
xxd -s 0x14d21a8 -l 128 bl4_dump.core
```

You'll see the serial string plus surrounding metadata—maybe a length prefix, maybe pointers to other item data. Compare multiple items at similar offsets to understand the structure.

---

## Comparing Memory States

One of the most powerful techniques is differential analysis. Take two dumps, change one thing in-game, and compare.

Scenario: You want to find where item level is stored.

1. Take a dump with a level 50 weapon equipped
2. Use an in-game mechanic to change its level (or edit the save)
3. Take another dump with the same weapon at level 51
4. Compare the memory regions around where you found the item

```bash
# Extract item regions from both dumps
dd if=dump1.core of=item1.bin bs=1 skip=$((0x14d21a8)) count=$((256))
dd if=dump2.core of=item2.bin bs=1 skip=$((0x14d21a8)) count=$((256))

# Compare
xxd item1.bin > item1.txt
xxd item2.bin > item2.txt
diff item1.txt item2.txt
```

The bytes that changed between dumps are candidates for the level field. Usually only a few bytes differ, making the answer obvious.

---

## Validating Pointers

Not every 8-byte sequence is a valid pointer. Invalid pointers lead to wrong interpretations. Always validate.

A valid pointer in BL4:
- Is above 0x10000 (below that is guard pages)
- Is below 0x800000000000 (user space limit)
- Points to mapped memory (not arbitrary addresses)

For vtable pointers specifically:
- The pointer itself should be in the data section
- The first entry it points to should be in the code section

```rust
// Pseudocode for vtable validation
let vtable_ptr = read_u64(object_addr);
if vtable_ptr < 0x150000000 || vtable_ptr > 0x155000000 {
    // Not in .rdata where vtables live
    return false;
}

let first_entry = read_u64(vtable_ptr);
if first_entry < 0x140001000 || first_entry > 0x14e61c000 {
    // First vtable entry should point to code
    return false;
}
// Probably a valid UObject
```

---

## Dealing with ASLR

Address Space Layout Randomization means base addresses change each time the game launches. The code section might be at 0x140000000 one session and 0x150000000 the next.

The solution: work with offsets from the PE base rather than absolute addresses. GNames isn't at 0x1512a1c80—it's at base + 0x112a1c80. The base address is easy to find (it's where the PE header is), and offsets remain constant across sessions.

When you discover a useful address, record it as an offset. When you need it next session, add the current base address.

---

## Wine/Proton Considerations

If you're running BL4 through Proton on Linux, memory layouts differ slightly from native Windows. The dump format is ELF rather than MDMP. Address mappings may need translation.

The bl4 tools handle both formats, but be aware that tutorials and tools written for native Windows analysis may need adaptation. The concepts are identical; the mechanics differ slightly.

---

## Exercises

**Exercise 1: Find Your Steam ID**

Your Steam ID is used to encrypt save files. It exists in memory while the game runs.

1. Take a memory dump while logged in
2. Search for your Steam ID as an ASCII string (it's a 17-digit number starting with 7656119)
3. Note the addresses where it appears
4. Consider: why might it appear in multiple places?

**Exercise 2: Count Inventory Items**

Find how many items are in your inventory:

1. Locate the player's inventory structure (hint: it's a TArray)
2. Read the Count field at offset +0x08
3. Compare with your in-game inventory count

**Exercise 3: Track a Value Change**

Pick something easy to change in-game (ammo count, for example):

1. Take a dump with ammo at some value
2. Fire a shot (or reload, depending on direction)
3. Take another dump
4. Search both dumps for the old and new values
5. Compare regions around the hits

---

## What's Next

Memory analysis reveals runtime state, but most players interact with the game through save files. Those files are encrypted, compressed, and structured in a specific format.

Next, we'll crack open BL4 save files—understanding the encryption, decompression, and YAML structure that makes save editing possible.

**Next: [Chapter 4: Save File Format](04-save-files.md)**
