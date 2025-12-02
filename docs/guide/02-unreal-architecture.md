# Chapter 2: Unreal Engine Architecture

Borderlands 4 runs on Unreal Engine 5. Understanding UE's architecture is essential for reverse engineering any Unreal game. This chapter covers the core concepts you'll encounter.

---

## Why Unreal Matters

Every Unreal game shares the same foundational architecture:
- **UObjects** — The base class for everything
- **Reflection** — Runtime type information
- **Pak Files** — Compressed asset archives
- **Serialization** — How data is saved and loaded

Learn these once, and you can reverse engineer *any* Unreal game.

---

## The UObject System

### Everything Is a UObject

In Unreal, virtually every game object inherits from `UObject`:

```
UObject
├── AActor (things in the world)
│   ├── APawn (controllable entities)
│   │   └── ACharacter (humanoid pawns)
│   │       └── AOakCharacter (BL4 player/enemy)
│   └── AWeapon (weapons in BL4)
├── UStruct (data structures)
├── UClass (class definitions)
└── UEnum (enumerations)
```

### UObject Memory Layout

Every UObject starts with a standard header:

```cpp
class UObject {
    void* VTable;           // +0x00: Virtual function table pointer
    int32 ObjectFlags;      // +0x08: Object state flags
    int32 InternalIndex;    // +0x0C: Index in global object array
    UClass* ClassPrivate;   // +0x10: Pointer to this object's class
    FName NamePrivate;      // +0x18: Object's name
    UObject* OuterPrivate;  // +0x20: Parent/container object
    // ... subclass fields follow
};
```

| Offset | Size | Field | Purpose |
|--------|------|-------|---------|
| 0x00 | 8 | VTable | Points to virtual function table |
| 0x08 | 4 | ObjectFlags | RF_* flags (transient, public, etc.) |
| 0x0C | 4 | InternalIndex | Position in GUObjectArray |
| 0x10 | 8 | ClassPrivate | Pointer to UClass describing this object |
| 0x18 | 8 | NamePrivate | FName (index into global name pool) |
| 0x20 | 8 | OuterPrivate | Owning object (package, actor, etc.) |

!!! note
    **Total header size**: 0x28 bytes (40 bytes). All UObjects start with this structure, then add their own fields.

---

## The FName System

Unreal stores strings efficiently using a global name pool.

### How FNames Work

Instead of storing the string "Damage" everywhere, Unreal stores an *index* into a global table:

```
FName Structure (8 bytes):
┌─────────────────────────────┐
│ ComparisonIndex (4 bytes)   │  ← Index into name pool
├─────────────────────────────┤
│ Number (4 bytes)            │  ← Instance number (e.g., "Actor_5")
└─────────────────────────────┘
```

### Decoding an FName

```rust
let fname_raw: u64 = 0x0000000000000938;  // From memory

let comparison_index = (fname_raw & 0xFFFFFFFF) as u32;  // = 0x938
let number = (fname_raw >> 32) as u32;                    // = 0

// Look up in name pool
let name = gnames.lookup(comparison_index);  // = "Damage"

// If number > 0, append it: "Actor_5"
```

### The FNamePool (GNames)

The global name pool is a chunked array:

```
FNamePool
├── Lock (8 bytes)
├── CurrentBlock (4 bytes)     ← Number of allocated blocks
├── CurrentByteCursor (4 bytes)
└── Blocks[8192]               ← Array of block pointers
    ├── Block 0 → [ "None", "ByteProperty", "IntProperty", ... ]
    ├── Block 1 → [ ... more names ... ]
    └── Block N → [ ... ]
```

Each block contains FNameEntry structures:

```
FNameEntry:
┌─────────────────────────┐
│ Header (2 bytes)        │  ← Bit 0: IsWide, Bits 6-15: Length
├─────────────────────────┤
│ Characters (N bytes)    │  ← ASCII or UTF-16 string data
└─────────────────────────┘
```

!!! tip
    Finding GNames is often the first step in UE reverse engineering. Search memory for the pattern `"None\0ByteProperty"` — these are always the first two names.

---

## The Reflection System

Unreal's reflection system lets the engine inspect classes at runtime.

### UClass — Class Definitions

Every class has a `UClass` object describing it:

```cpp
class UClass : public UStruct {
    // ... inherited from UStruct ...
    UObject* DefaultObject;     // +0x110: CDO (Class Default Object)
    // ... function pointers, interfaces, etc.
};
```

### UStruct — Structure Layout

`UStruct` describes the layout of a class or struct:

```cpp
class UStruct : public UField {
    UStruct* Super;             // +0x40: Parent class
    UField* Children;           // +0x48: First child field (legacy)
    FField* ChildProperties;    // +0x50: Property linked list (UE5)
    int32 PropertiesSize;       // +0x58: Total size of all properties
    // ...
};
```

### FProperty — Property Metadata

Each property (field) in a class has metadata:

```cpp
class FProperty : public FField {
    int32 ArrayDim;         // +0x30: Array size (1 for non-arrays)
    int32 ElementSize;      // +0x34: Size of one element
    uint64 PropertyFlags;   // +0x38: CPF_* flags
    int32 Offset_Internal;  // +0x4C: Byte offset in owning struct
    // ... type-specific data at +0x78
};
```

!!! note
    **Why this matters**: To parse game data, you need to know where each field is. The reflection system tells you "Damage is at offset 0x48, it's an f32."

---

## Global Arrays

Unreal maintains several global arrays accessible from any thread.

### GUObjectArray

All UObjects are tracked in a global array:

```cpp
struct FUObjectArray {
    FChunkedFixedUObjectArray Objects;  // Chunked array of all objects
};

struct FUObjectItem {
    UObject* Object;        // +0x00: The actual object
    int32 Flags;            // +0x08: Item flags
    int32 ClusterRootIndex; // +0x0C: Clustering info
    int32 SerialNumber;     // +0x10: For weak references
};
```

### GWorld

Pointer to the current world (level):

```cpp
UWorld* GWorld;  // Global pointer to active world
```

### Finding Globals

These globals are accessed via LEA instructions in code:

```asm
; Common pattern for accessing GNames
lea rax, [rip + 0x????????]  ; 48 8D 05 XX XX XX XX
```

BL4-specific offsets (as of Nov 2025):

| Global | Offset from PE Base | Virtual Address |
|--------|---------------------|-----------------|
| GUObjectArray | 0x113878f0 | 0x1513878f0 |
| GNames | 0x112a1c80 | 0x1512a1c80 |
| GWorld | 0x11532cb8 | 0x151532cb8 |

---

## Pak Files and Asset Format

### IoStore Containers

BL4 uses UE5's IoStore format:

| File | Purpose |
|------|---------|
| `.utoc` | Table of contents (asset index) |
| `.ucas` | Container archive (compressed data) |
| `.pak` | Legacy format (some assets) |

### Asset Types

| Extension | Type | Contents |
|-----------|------|----------|
| `.uasset` | Asset file | Object definitions, properties |
| `.uexp` | Export data | Bulk data (textures, meshes) |
| `.ubulk` | Bulk data | Large data split from uasset |

### Zen Package Format

UE5 uses "Zen" packages internally:

```
Zen Package Header:
├── Summary
│   ├── Name (FName of this package)
│   ├── Flags
│   └── Cooked hash
├── Name Map (local FNames)
├── Import Map (external dependencies)
├── Export Map (objects in this package)
└── Export Data (serialized object data)
```

!!! tip
    Use `retoc` to extract from IoStore containers, and our `uextract` tool to parse the Zen packages.

---

## Usmap Files

A `.usmap` file contains serialized reflection data — all the class/struct definitions needed to parse assets.

### Why You Need Usmap

UE5 uses "unversioned" serialization — property data is written without field names or types. To parse it, you need the schema:

```
Without usmap:  [ 0x42 0x48 0x00 0x00 0x40 0x1C 0x00 0x00 ... ]
                  ???

With usmap:     Damage (f32) = 50.0
                Level (u32) = 7200
                ...
```

### Usmap Structure

```
Header:
├── Magic: 0x30C4
├── Version: 3 (LargeEnums)
├── Compression: 0 (None), 1 (Oodle), 2 (Brotli), 3 (ZStd)
├── CompressedSize
└── DecompressedSize

Payload:
├── Names: ["None", "ByteProperty", "Damage", ...]
├── Enums: [{ name: "EWeaponType", values: [...] }, ...]
└── Structs: [{ name: "FWeaponData", properties: [...] }, ...]
```

### Our Generated Usmap

The bl4 project generates a complete usmap from memory:

| Metric | Count |
|--------|-------|
| Names | 64,917 |
| Enums | 2,986 |
| Structs | 16,849 |
| Properties | 58,793 |

---

## Common UE5 Types

### TArray — Dynamic Arrays

```cpp
template<typename T>
struct TArray {
    T* Data;        // +0x00: Pointer to elements
    int32 Count;    // +0x08: Number of elements
    int32 Max;      // +0x0C: Allocated capacity
};  // Size: 0x10 (16 bytes)
```

### FString — Strings

```cpp
struct FString {
    TArray<wchar_t> Data;  // Wide string data
};  // Size: 0x10 (16 bytes)
```

Serialized format:
```
Length (i32, negative = UTF-16, positive = ASCII)
Characters (abs(Length) bytes or chars)
Null terminator
```

### FVector / FRotator

```cpp
struct FVector {
    double X, Y, Z;
};  // Size: 0x18 (24 bytes)

struct FRotator {
    double Pitch, Yaw, Roll;
};  // Size: 0x18 (24 bytes)
```

!!! warning
    UE5 uses `double` for vectors (24 bytes), not `float` like UE4 (12 bytes). This is a common source of parsing errors.

### FTransform

```cpp
struct FTransform {
    FQuat Rotation;     // +0x00 (32 bytes)
    FVector Translation; // +0x20 (24 bytes + 8 padding)
    FVector Scale3D;     // +0x40 (24 bytes + 8 padding)
};  // Size: 0x60 (96 bytes)
```

---

## BL4-Specific Classes

### AOakCharacter

The player/enemy character class:

```cpp
class AOakCharacter : public AGbxCharacter {
    // Offset 0x4038
    FOakDamageState DamageState;       // Size: 0x608

    // Offset 0x4640
    FOakCharacterHealthState HealthState;  // Size: 0x1E8

    // Offset 0x5F50
    FOakActiveWeaponsState ActiveWeapons;  // Size: 0x210

    // ... many more fields
};  // Total size: ~0x9790
```

### AWeapon

```cpp
class AWeapon : public AInventory {
    // Offset 0xC40
    FDamageModifierData DamageModifierData;  // Size: 0x6C

    // Offset 0xCB8
    FGbxAttributeFloat ZoomTimeScale;

    // ...
};  // Size: ~0xD48
```

---

## Practical: Finding a Class in Memory

Let's trace how to find weapon data in memory:

### Step 1: Find GUObjectArray

```bash
# The global object array contains all UObjects
# In BL4, it's at offset 0x113878f0 from PE base (0x140000000)
bl4 memory --dump share/dumps/game.raw read 0x1513878f0 --size 32
```

### Step 2: Walk the Array

```rust
// Each chunk holds ~65536 objects
let chunk_ptr = read_u64(guobjectarray + 0x00);
let num_elements = read_u32(guobjectarray + 0x08);

// Each FUObjectItem is 24 bytes
for i in 0..num_elements {
    let item_ptr = chunk_ptr + (i * 24);
    let object_ptr = read_u64(item_ptr);

    // Read class pointer
    let class_ptr = read_u64(object_ptr + 0x10);
    let class_name = resolve_fname(read_u64(class_ptr + 0x18));

    if class_name == "Weapon" {
        println!("Found weapon at {:#x}", object_ptr);
    }
}
```

### Step 3: Read Object Properties

Once you have an object, use the reflection system to find field offsets:

```rust
// UClass contains property chain
let child_props = read_u64(class_ptr + 0x50);  // ChildProperties

// Walk the linked list
let mut prop = child_props;
while prop != 0 {
    let name_idx = read_u32(prop + 0x20);
    let offset = read_u32(prop + 0x4C);

    println!("{}: offset 0x{:X}", resolve_fname(name_idx), offset);

    prop = read_u64(prop + 0x18);  // Next property
}
```

---

## Exercises

### Exercise 1: UObject Header

Given this memory dump of a UObject:
```
00000000: 50 3A 4F 14 01 00 00 00  00 00 00 02 38 04 00 00
00000010: E0 51 8B 90 01 00 00 00  38 09 00 00 00 00 00 00
00000020: 80 25 6E 91 01 00 00 00
```

1. What's the VTable pointer?
2. What's the InternalIndex?
3. What's the FName comparison index?

<details>
<summary>Answers</summary>

1. VTable: `0x00000001144F3A50` (bytes 0-7, little-endian)
2. InternalIndex: `0x00000438` (bytes 12-15) = 1080
3. FName index: `0x00000938` (bytes 24-27) = 2360

</details>

### Exercise 2: Class Hierarchy

In Unreal, to find a class's parent:

1. Read `ClassPrivate` at object + 0x10
2. Read `Super` at class + 0x40
3. Read the `Super`'s name

Describe what you'd find for an `AOakCharacter`.

<details>
<summary>Answer</summary>

```
AOakCharacter
└── Super → AGbxCharacter
    └── Super → ACharacter
        └── Super → APawn
            └── Super → AActor
                └── Super → UObject
                    └── Super → nullptr
```

</details>

---

## Key Takeaways

1. **UObjects are everywhere** — Learn the 0x28-byte header
2. **FNames are indices** — Look up strings in the global name pool
3. **Reflection is your friend** — UClass/UStruct tell you where data lives
4. **Usmap files decode assets** — Without them, pak data is opaque

---

## Next Chapter

Now that you understand Unreal's architecture, let's dive into analyzing game memory directly.

**Next: [Chapter 3: Memory Analysis](03-memory-analysis.md)**
