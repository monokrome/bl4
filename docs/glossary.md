# Glossary

Quick reference for terms used throughout this guide. Page references indicate the primary location where each term is explained.

---

## A

**AActor**
: Base class for all objects that can be placed in a level. Size: 912 bytes. See [Appendix A](appendix-a-sdk-layouts.md).

**AES-256-ECB**
: Advanced Encryption Standard with 256-bit key in Electronic Codebook mode. Used by BL4 for save file encryption. See [Chapter 4](04-save-files.md).

**ASLR**
: Address Space Layout Randomization. Security feature that randomizes memory addresses on each launch. See [Chapter 3](03-memory-analysis.md).

**AOakCharacter**
: BL4's main player/enemy character class. Size: 38,800 bytes. Contains health, damage, and weapon state. See [Appendix A](appendix-a-sdk-layouts.md).

---

## B

**Base85**
: Number encoding using 85 printable ASCII characters. BL4 uses a custom alphabet for item serials. See [Chapter 5](05-item-serials.md).

**Big-Endian**
: Byte order where most significant byte comes first. Used in Base85 decoding. See [Chapter 1](01-binary-basics.md).

**Bit Mirroring**
: Reversing the bit order within each byte (e.g., 0b10000111 → 0b11100001). Part of serial decoding. See [Chapter 5](05-item-serials.md).

**Bitstream**
: Sequence of bits read without byte alignment. Used in item serial encoding. See [Chapter 5](05-item-serials.md).

---

## C

**CDO**
: Class Default Object. UE's template object containing default property values. See [Chapter 2](02-unreal-architecture.md).

**ClassPrivate**
: UObject field at offset 0x10 pointing to the object's UClass. See [Chapter 2](02-unreal-architecture.md).

**Comparison Index**
: The 32-bit index portion of an FName, used to look up strings in GNames. See [Chapter 2](02-unreal-architecture.md).

---

## D

**DataTable**
: UE asset type for tabular data. Contains rows of structured data. See [Chapter 6](06-data-extraction.md).

**Dedicated Drop**
: Loot that only drops from specific enemies. See [Appendix C](appendix-c-loot-system.md).

---

## E

**ECB**
: Electronic Codebook. Block cipher mode where identical plaintext blocks produce identical ciphertext. See [Chapter 4](04-save-files.md).

---

## F

**FField**
: UE5's property descriptor base class. Replaces UProperty from UE4. See [Appendix A](appendix-a-sdk-layouts.md).

**FName**
: Unreal's string identifier. 8 bytes containing index and instance number. See [Chapter 2](02-unreal-architecture.md).

**FNamePool**
: Global string pool containing all FName strings. Also called GNames. See [Chapter 2](02-unreal-architecture.md).

**FProperty**
: UE5 property descriptor. Contains offset, size, and type information. See [Appendix A](appendix-a-sdk-layouts.md).

**FString**
: Unreal's dynamic string type. 16 bytes containing pointer, count, and capacity. See [Appendix A](appendix-a-sdk-layouts.md).

**FTransform**
: 96-byte structure containing rotation (FQuat), translation (FVector), and scale. See [Appendix A](appendix-a-sdk-layouts.md).

**FVector**
: 24-byte 3D vector using doubles (not floats like UE4). See [Appendix A](appendix-a-sdk-layouts.md).

---

## G

**GNames**
: Global FName string pool. Located at offset 0x112a1c80 from PE base. See [Chapter 2](02-unreal-architecture.md).

**GUObjectArray**
: Global array containing all UObjects. Located at offset 0x113878f0. See [Chapter 2](02-unreal-architecture.md).

**GWorld**
: Pointer to current UWorld. Located at offset 0x11532cb8. See [Appendix A](appendix-a-sdk-layouts.md).

---

## H

**Heap**
: Memory region for dynamic allocations. Valid range: 0x10000-0x800000000000. See [Chapter 3](03-memory-analysis.md).

**Hexadecimal**
: Base-16 number system using 0-9 and A-F. See [Chapter 1](01-binary-basics.md).

---

## I

**InternalIndex**
: UObject field at offset 0x0C containing position in GUObjectArray. See [Chapter 2](02-unreal-architecture.md).

**IoStore**
: UE5's container format using .utoc (table of contents) and .ucas (data) files. See [Chapter 6](06-data-extraction.md).

**Item Pool**
: Definition of possible loot drops. See [Appendix C](appendix-c-loot-system.md).

**Item Serial**
: Base85-encoded string representing an item's full configuration. See [Chapter 5](05-item-serials.md).

---

## L

**Little-Endian**
: Byte order where least significant byte comes first. Used by x86/x64 and save files. See [Chapter 1](01-binary-basics.md).

**Luck**
: Game system that modifies loot rarity chances. See [Appendix C](appendix-c-loot-system.md).

---

## M

**Magic Header**
: Fixed bit pattern at start of data. Item serials use 0010000 (7 bits). See [Chapter 5](05-item-serials.md).

**MDMP**
: Windows minidump format. Used for memory dump files. See [Chapter 3](03-memory-analysis.md).

---

## N

**NamePrivate**
: UObject field at offset 0x18 containing the object's FName. See [Chapter 2](02-unreal-architecture.md).

**Nibble**
: Half a byte (4 bits). Used in VarInt encoding. See [Chapter 5](05-item-serials.md).

---

## O

**ObjectFlags**
: UObject field at offset 0x08 containing RF_* state flags. See [Chapter 2](02-unreal-architecture.md).

**Oodle**
: Compression algorithm used in UE5 IoStore containers. See [Chapter 6](06-data-extraction.md).

**OuterPrivate**
: UObject field at offset 0x20 pointing to parent/container object. See [Chapter 2](02-unreal-architecture.md).

---

## P

**Pak File**
: Legacy UE archive format (.pak). See [Chapter 6](06-data-extraction.md).

**Part**
: Token type in item serials representing weapon components. See [Chapter 5](05-item-serials.md).

**PE Base**
: Base address of Windows executable (0x140000000 for BL4). See [Chapter 3](03-memory-analysis.md).

**PKCS7**
: Padding scheme for block ciphers. See [Chapter 4](04-save-files.md).

**Pointer Chain**
: Sequence of pointer dereferences to reach target data. See [Chapter 3](03-memory-analysis.md).

---

## R

**Rarity**
: Item quality tier (Common, Uncommon, Rare, Epic, Legendary). See [Appendix B](appendix-b-weapon-parts.md).

**Reflection**
: UE's runtime type information system. See [Chapter 2](02-unreal-architecture.md).

**retoc**
: Tool for extracting IoStore containers. See [Chapter 6](06-data-extraction.md).

**RIP-Relative**
: x64 addressing mode relative to instruction pointer. See [Chapter 3](03-memory-analysis.md).

---

## S

**Separator**
: Token type in item serials marking section boundaries. See [Chapter 5](05-item-serials.md).

**Serial**
: See Item Serial.

**Steam ID**
: Unique identifier for Steam accounts. Used to derive encryption key. See [Chapter 4](04-save-files.md).

**SuperStruct**
: UStruct field at offset 0x40 pointing to parent class. See [Appendix A](appendix-a-sdk-layouts.md).

---

## T

**TArray**
: Unreal's dynamic array template. 16 bytes containing pointer, count, max. See [Appendix A](appendix-a-sdk-layouts.md).

**Token**
: Discrete data element in item serial bitstream. Types: VarInt, VarBit, Part, String, Separator. See [Chapter 5](05-item-serials.md).

---

## U

**uasset**
: Unreal asset file containing object definitions and properties. See [Chapter 6](06-data-extraction.md).

**UClass**
: UE's class definition object. Size: 512 bytes. See [Chapter 2](02-unreal-architecture.md).

**ucas**
: IoStore container archive file containing compressed asset data. See [Chapter 6](06-data-extraction.md).

**uexp**
: Bulk data file accompanying .uasset. See [Chapter 6](06-data-extraction.md).

**UObject**
: Base class for all Unreal objects. Size: 40 bytes. See [Chapter 2](02-unreal-architecture.md).

**Usmap**
: Mapping file containing UE reflection data for parsing unversioned assets. See [Chapter 2](02-unreal-architecture.md).

**UStruct**
: UE's structure/class layout descriptor. Size: 176 bytes. See [Chapter 2](02-unreal-architecture.md).

**utoc**
: IoStore table of contents file. See [Chapter 6](06-data-extraction.md).

---

## V

**VarBit**
: Token type with 5-bit length prefix followed by N data bits. See [Chapter 5](05-item-serials.md).

**VarInt**
: Variable-length integer using 4-bit nibbles with continuation bits. See [Chapter 5](05-item-serials.md).

**Virtual Address (VA)**
: Memory address in process's virtual address space. See [Chapter 3](03-memory-analysis.md).

**VTable**
: Virtual function table pointer at offset 0x00 of UObjects. See [Chapter 2](02-unreal-architecture.md).

---

## W

**WASM**
: WebAssembly. Binary format for running code in browsers. See [Chapter 7](07-bl4-tools.md).

---

## Z

**Zen Package**
: UE5's internal package format used in IoStore containers. See [Chapter 6](06-data-extraction.md).

**zlib**
: Compression library. Used for save file compression after encryption. See [Chapter 4](04-save-files.md).

---

## Symbols

**@Ug**
: Prefix for all BL4 item serials. See [Chapter 5](05-item-serials.md).

**0x**
: Prefix indicating hexadecimal number. See [Chapter 1](01-binary-basics.md).

---

## Quick Reference: Key Offsets

| Symbol | Offset | Description |
|--------|--------|-------------|
| GUObjectArray | 0x113878f0 | All UObjects |
| GNames | 0x112a1c80 | FName pool |
| GWorld | 0x11532cb8 | Current world |
| ClassPrivate | +0x10 | UObject's class |
| NamePrivate | +0x18 | UObject's name |
| OuterPrivate | +0x20 | UObject's parent |
| SuperStruct | +0x40 | UStruct's parent |

---

## Quick Reference: File Extensions

| Extension | Description |
|-----------|-------------|
| .sav | Encrypted save file |
| .pak | Legacy archive |
| .utoc | IoStore index |
| .ucas | IoStore data |
| .uasset | Asset file |
| .uexp | Bulk data |
| .usmap | Mapping file |

---

*This glossary covers terms from all chapters and appendices.*
