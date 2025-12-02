# Usmap Generation

**Status**: COMPLETE

This document describes the usmap generation capability for BL4.

---

## What is a .usmap file?

A usmap file contains serialized type information:
- All UClass/UStruct definitions with their property chains
- Property names, types, offsets, sizes, and flags
- Enum definitions with their values
- Used by asset parsing tools (FModel, CUE4Parse, UAssetGUI) to read .uasset files

---

## Generated Usmap Stats

| Metric | Count |
|--------|-------|
| Names | 64,917 |
| Enums | 2,986 |
| Enum values | 17,291 |
| Structs/Classes | 16,849 |
| Properties | 58,793 |
| File size | 2.2 MB |

Output: `share/manifest/mappings.usmap`

---

## Usage

### Generate from Memory Dump

```bash
# Create memory dump while game is running (requires procdump or similar)
# Then generate usmap:
bl4 memory --dump ./share/dumps/your_dump.raw dump-usmap

# Output: BL4.usmap in current directory
```

### Inspect Usmap File

```bash
bl4 usmap-info ./share/manifest/mappings.usmap
```

Output:
```
=== ./share/manifest/mappings.usmap ===
Magic: 0x30c4
Version: 3
HasVersionInfo: false
Compression: 0 (None)
CompressedSize: 2199074 bytes
DecompressedSize: 2199074 bytes

Names: 64917
Enums: 2986
Enum values: 17291
Structs: 16849
Properties: 58793

File size: 2199090 bytes
```

---

## Implementation Details

### Data Sources

All reflection data is extracted from a Windows memory dump (MDMP format):

1. **GUObjectArray** - Global object array containing all UObjects
2. **FNamePool** - String pool for all FNames
3. **UClass/UScriptStruct** - Type definitions with property chains
4. **UEnum** - Enum definitions with values

### Key Offsets (UE5.4)

```rust
// UObject layout
UOBJECT_CLASS_OFFSET: 0x10
UOBJECT_NAME_OFFSET: 0x18
UOBJECT_OUTER_OFFSET: 0x20

// UStruct layout
USTRUCT_SUPER_OFFSET: 0x40
USTRUCT_CHILDPROPERTIES_OFFSET: 0x50  // FProperty linked list
USTRUCT_SIZE_OFFSET: 0x58

// FField/FProperty layout
FFIELD_NEXT_OFFSET: 0x18
FFIELD_NAME_OFFSET: 0x20
FPROPERTY_ARRAYDIM_OFFSET: 0x30
FPROPERTY_ELEMENTSIZE_OFFSET: 0x34
FPROPERTY_FLAGS_OFFSET: 0x38
FPROPERTY_OFFSET_OFFSET: 0x4C

// UEnum layout
UENUM_NAMES_OFFSET: 0x40  // TArray<TPair<FName, int64>>
```

### Property Type Inference

Since FFieldClass in UE5.4 is purely a vtable (no embedded type name), property types are inferred by:

1. **Probing type-specific data at offset 0x78**:
   - StructProperty: Points to UScriptStruct
   - ObjectProperty: Points to UClass
   - ArrayProperty: Points to inner FProperty
   - MapProperty: Points to key FProperty (value at 0x80)

2. **Fallback to element size**:
   - 1 byte → ByteProperty/BoolProperty
   - 4 bytes → IntProperty/FloatProperty
   - 8 bytes → Int64Property/DoubleProperty/ObjectProperty
   - 16 bytes → NameProperty/StrProperty

### Usmap Format (Version 3)

```
Header (16 bytes):
  Magic: u16 (0x30C4)
  Version: u8 (3 = LargeEnums)
  HasVersionInfo: u8 (0 = false)
  Compression: u32 (0 = None)
  CompressedSize: u32
  DecompressedSize: u32

Payload:
  NameCount: u32
  Names: [length: u16, data: utf8]...

  EnumCount: u32
  Enums: [name_idx: u32, count: u16, values: [name_idx: u32]...]...

  StructCount: u32
  Structs: [name_idx: u32, super_idx: u32, prop_count: u16,
            serializable_count: u16, properties: [...]...]...
```

---

## Comparison with Reference

Our generated usmap compared to the reference `share/borderlands.usmap`:

| Metric | Reference | Generated | Difference |
|--------|-----------|-----------|------------|
| Names | 64,506 | 64,917 | +0.6% |
| Enums | 2,979 | 2,986 | +0.2% |
| Enum values | 17,134 | 17,291 | +0.9% |
| Structs | 16,731 | 16,849 | +0.7% |
| Properties | 55,467 | 58,793 | +6.0% |

Our extraction captures **more** reflection data than the reference.

---

*Last updated: 2025-12-02*
