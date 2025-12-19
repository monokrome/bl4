# Chapter 2: Unreal Engine Architecture

Borderlands 4 runs on Unreal Engine 5, and that's great news for reverse engineering. Every Unreal game—from Fortnite to Elden Ring to indie projects—shares the same fundamental architecture. Learn how Unreal organizes data once, and you've learned something applicable to hundreds of games.

This chapter explores how Unreal thinks about the world. Understanding these patterns transforms mysterious byte sequences into recognizable structures.

---

## Everything Is a UObject

Unreal Engine has a single base class for nearly everything: `UObject`. Your character? A UObject. That legendary weapon? A UObject. The class definition that describes what a weapon *is*? Also a UObject.

This design creates a unified system where the engine can manage, serialize, and inspect anything. Need to save the game state? Iterate through UObjects. Need to find all weapons in the world? Query UObjects by class. Need to know what fields a weapon has? Ask the UObject's class definition.

The inheritance hierarchy for a Borderlands 4 player character looks like this:

```
UObject
└── AActor (things that exist in the world)
    └── APawn (things that can be possessed/controlled)
        └── ACharacter (humanoid pawns with movement)
            └── AGbxCharacter (Gearbox's base character)
                └── AOakCharacter (BL4 player/enemy)
```

Every step adds capabilities. UObject provides basic memory management and reflection. AActor adds world position and component attachment. APawn adds controller possession. ACharacter adds a skeletal mesh and movement component. By the time you reach AOakCharacter, you have a fully-featured game entity with health, weapons, skills, and AI hooks.

---

## The 40-Byte Header

Every UObject begins with the same 40-byte (0x28) header. Recognizing this structure in memory dumps is a core skill.

```
Offset  Size  Field          Purpose
------  ----  -----          -------
0x00    8     VTable         Pointer to virtual function table
0x08    4     ObjectFlags    Flags like RF_Transient, RF_Public
0x0C    4     InternalIndex  Position in the global object array
0x10    8     ClassPrivate   Pointer to this object's UClass
0x18    8     NamePrivate    FName (index into name pool)
0x20    8     OuterPrivate   Parent object (package, owner)
```

When you see a pointer in memory and want to know if it's a valid UObject, check if the pointer at offset 0x10 (ClassPrivate) points to something reasonable. If that pointer's object also has a sensible structure, you're probably looking at a real UObject.

The `InternalIndex` at 0x0C is particularly useful—it tells you where this object lives in Unreal's global tracking array, which we'll explore shortly.

---

## How Unreal Stores Names

Storing the string "Damage" every time a weapon references that property would waste enormous amounts of memory. Instead, Unreal uses a global name pool called `GNames` (or `FNamePool`).

An `FName` isn't a string—it's an 8-byte value containing an index into this pool:

```
FName (8 bytes)
├── Bits 0-31:  ComparisonIndex (which name in the pool)
└── Bits 32-63: Number (instance suffix, like "Actor_5")
```

When Unreal needs the actual string, it looks up the index in the name pool. The pool itself is organized as a chunked array—multiple blocks of entries, where each entry stores the actual characters:

```
FNameEntry
├── Header (2 bytes): bit 0 = is_wide, bits 6-15 = length
└── Characters (N bytes): the actual string data
```

The first few names in any Unreal game are always the same: "None" at index 0, then "ByteProperty", "IntProperty", and so on. This predictability helps locate the name pool in memory—search for the byte sequence representing "None\0ByteProperty" and you're close to GNames.

---

## Reflection: How Unreal Knows Itself

The reflection system is what makes Unreal games remarkably inspectable. At runtime, Unreal knows the name, type, and offset of every field in every class. This isn't magic—it's data structures describing data structures.

Every class has a `UClass` object that describes it. UClass inherits from `UStruct`, which contains the property definitions. Each property (`FProperty`) knows its name, its byte offset within the owning struct, its type, and various flags.

The key fields in UStruct:

```cpp
class UStruct {
    UStruct* Super;              // Parent class (offset ~0x40)
    FField* ChildProperties;     // First property in linked list (offset ~0x50)
    int32 PropertiesSize;        // Total byte size of all properties
};
```

And each FProperty in the linked list:

```cpp
class FProperty {
    FField* Next;                // Next property in chain (offset ~0x18)
    FName NamePrivate;           // Property name (offset ~0x20)
    int32 Offset_Internal;       // Byte offset in struct (offset ~0x4C)
    int32 ElementSize;           // Size of one element
    // ... type-specific data follows
};
```

To parse a weapon's damage value, you don't hardcode "damage is at offset 0x48." Instead, you find the Weapon class, walk its property chain until you find one named "Damage," read its offset, and use that. This approach survives game patches that shuffle memory layouts.

---

## The Global Object Array

Unreal tracks all live UObjects in a global array called `GUObjectArray`. This is your index to everything in the game's memory.

The array is chunked—multiple blocks of ~65536 entries each. Each entry (`FUObjectItem`) is 24 bytes:

```
FUObjectItem (0x18 bytes)
├── Object pointer (8 bytes): the actual UObject
├── Flags (4 bytes): item-level flags
├── ClusterRootIndex (4 bytes): clustering info
└── SerialNumber (4 bytes): for weak references
```

To enumerate all objects of a specific class:

1. Get the chunk pointer from GUObjectArray
2. Read the element count
3. For each element, read the Object pointer
4. Read the object's ClassPrivate pointer
5. Resolve the class's name via FName lookup
6. If it matches your target class, you found one

In BL4, with the game running, you might find thousands of objects: hundreds of AWeapon instances, dozens of AOakCharacter instances, and tens of thousands of supporting objects like damage components and inventory slots.

---

## Pak Files: Where Assets Live

On disk, Unreal games store assets in `.pak` archives. BL4 uses UE5's IoStore format, which splits the data across multiple files:

**`.utoc` (Table of Contents)**: An index listing every asset, its location, size, and compression info.

**`.ucas` (Container Archive)**: The actual compressed asset data, referenced by the utoc.

**`.pak` (Legacy Format)**: Some assets still use the older format for compatibility.

Inside these archives, individual assets follow the Zen package format:

```
Zen Package
├── Summary (package metadata)
├── Name Map (local FNames used in this package)
├── Import Map (external assets this package references)
├── Export Map (objects defined in this package)
└── Export Data (serialized property data)
```

The export data contains the actual property values, but here's the catch: UE5 uses "unversioned" serialization. Properties are written without their names or types—just raw values in order. To parse them, you need external schema information.

---

## Usmap: The Rosetta Stone

A `.usmap` file contains the schema needed to parse unversioned assets. It's essentially a dump of the reflection system: all class names, property names, types, and offsets.

Without usmap:
```
Raw bytes: 42 48 00 00 40 1C 00 00 ...
Meaning: ???
```

With usmap:
```
Damage (f32): 50.0
Level (u32): 7200
ElementType (enum): Fire
...
```

The usmap format is straightforward:

```
Header
├── Magic: 0x30C4
├── Version: 3 (most recent)
├── Compression: 0=None, 1=Oodle, 2=Brotli, 3=ZStd
├── CompressedSize / DecompressedSize
└── Payload
    ├── Names array (all string names)
    ├── Enums array (enum definitions)
    └── Structs array (class/struct definitions with properties)
```

The bl4 project generates usmap files from memory dumps. Our current usmap contains 64,917 names, 2,986 enums, and 16,849 struct definitions. This covers essentially every data structure BL4 uses.

---

## Common UE5 Data Types

Certain types appear everywhere in Unreal. Recognizing them speeds up analysis.

**TArray<T>** (16 bytes): Dynamic arrays.
```
├── Data pointer (8 bytes): heap allocation
├── Count (4 bytes): current elements
└── Max (4 bytes): allocated capacity
```

**FString** (16 bytes): Dynamic strings (internally a TArray<wchar_t>).
When serialized: length as i32 (negative means UTF-16), then characters, then null terminator.

**FVector** (24 bytes in UE5): 3D coordinates.
```
├── X (8 bytes, double)
├── Y (8 bytes, double)
└── Z (8 bytes, double)
```

Note: UE4 used 12-byte vectors with floats. UE5 switched to doubles. This is a common source of parsing errors when adapting UE4 tools.

**FTransform** (96 bytes): Position + rotation + scale.
```
├── Rotation (32 bytes, FQuat)
├── Translation (32 bytes, FVector + padding)
└── Scale3D (32 bytes, FVector + padding)
```

---

## BL4's Class Structure

The Gearbox-specific classes follow predictable patterns. AOakCharacter, the player/enemy base class, is about 38KB (0x9790 bytes) and contains:

```
AOakCharacter (inherits AGbxCharacter)
├── ~0x4038: FOakDamageState (0x608 bytes)
├── ~0x4640: FOakCharacterHealthState (0x1E8 bytes)
├── ~0x5F50: FOakActiveWeaponsState (0x210 bytes)
└── ... hundreds more fields
```

AWeapon, the weapon class, runs about 3.4KB (0xD48 bytes):

```
AWeapon (inherits AInventory)
├── ~0xC40: FDamageModifierData (0x6C bytes)
├── ~0xCB8: FGbxAttributeFloat ZoomTimeScale
└── ... damage calculation fields, fire modes, etc.
```

These offsets shift between game patches. The reflection system (or a current usmap) is the authoritative source.

---

## Walking Memory: A Preview

We'll cover memory analysis properly in Chapter 3, but here's a taste of how Unreal's architecture enables systematic exploration.

To find all weapons in memory:

1. Locate GUObjectArray (in BL4: base + 0x113878f0)
2. Read the chunk pointer and element count
3. For each object in the array:
   - Skip if the object pointer is null
   - Read ClassPrivate at object + 0x10
   - Read the class's FName at class + 0x18
   - Resolve the name through GNames
   - If it's "Weapon" or a subclass, record the address

Once you have weapon addresses, use reflection to find properties:

1. Read ChildProperties from the UClass
2. Walk the linked list of FProperty
3. For each property, read its name and offset
4. Use those offsets to read actual values from the weapon object

This two-phase approach—find objects, then decode them—works for any Unreal game.

---

## Exercises

**Exercise 1: Decode a UObject Header**

Given this memory dump:
```
00000000: 50 3A 4F 14 01 00 00 00  00 00 00 02 38 04 00 00
00000010: E0 51 8B 90 01 00 00 00  38 09 00 00 00 00 00 00
00000020: 80 25 6E 91 01 00 00 00
```

What is:
1. The VTable pointer?
2. The InternalIndex?
3. The FName comparison index?

**Exercise 2: Trace a Class Hierarchy**

Starting from an AOakCharacter object:
1. Read ClassPrivate (offset 0x10) to get the UClass
2. Read Super (offset ~0x40) to get the parent class
3. Continue until Super is null

What classes would you encounter?

<details>
<summary>Answers</summary>

**Exercise 1:**
1. VTable: 0x00000001144F3A50 (bytes 0-7, little-endian)
2. InternalIndex: 0x00000438 = 1080 (bytes 0x0C-0x0F)
3. FName index: 0x00000938 = 2360 (bytes 0x18-0x1B, lower 32 bits)

**Exercise 2:**
```
AOakCharacter
└── AGbxCharacter
    └── ACharacter
        └── APawn
            └── AActor
                └── UObject
                    └── (Super = null, stop)
```

</details>

---

## What's Next

You now understand how Unreal organizes its world—UObjects with reflectable properties, tracked in global arrays, stored in pak files with usmap schemas. This knowledge transforms memory analysis from random exploration into systematic discovery.

Next, we'll put these concepts into practice by analyzing live game memory and extracting data directly from a running instance.

**Next: [Chapter 3: Memory Analysis](03-memory-analysis.md)**

