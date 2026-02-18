# Glossary {#sec-glossary}

Quick reference for terms used throughout this guide. Page references indicate the primary location where each term is explained.

---

## A

**AActor**
: Base class for all objects that can be placed in a level. Size: 912 bytes. See [Appendix A](#sec-sdk-layouts).

**AES-256-ECB**
: Advanced Encryption Standard with 256-bit key in Electronic Codebook mode. Used by BL4 for save file encryption. See [Chapter 4](#sec-save-files).

**ASLR**
: Address Space Layout Randomization. Security feature that randomizes memory addresses on each launch. See [Chapter 3](#sec-memory-analysis).

**AOakCharacter**
: BL4's main player/enemy character class. Size: 38,800 bytes. Contains health, damage, and weapon state. See [Appendix A](#sec-sdk-layouts).

---

## B

**bl4-community**
: Axum-based REST API server for sharing verified item data between users. Part of the bl4 monorepo. See [Chapter 9](#sec-bl4-tools).

**Base85**
: Number encoding using 85 printable ASCII characters. BL4 uses a custom alphabet for item serials. See [Chapter 5](#sec-item-serials).

**Big-Endian**
: Byte order where most significant byte comes first. Used in Base85 decoding. See [Chapter 1](#sec-binary-basics).

**Bit Mirroring**
: Reversing the bit order within each byte (e.g., 0b10000111 → 0b11100001). Part of serial decoding. See [Chapter 5](#sec-item-serials).

**Bitstream**
: Sequence of bits read without byte alignment. Used in item serial encoding. See [Chapter 5](#sec-item-serials).

---

## C

**CDO**
: Class Default Object. UE's template object containing default property values. See [Chapter 2](#sec-unreal-architecture).

**ClassPrivate**
: UObject field at offset 0x10 pointing to the object's UClass. See [Chapter 2](#sec-unreal-architecture).

**Comparison Index**
: The 32-bit index portion of an FName, used to look up strings in GNames. See [Chapter 2](#sec-unreal-architecture).

---

## D

**Differential Encoding**
: NCS string compression where subsequent strings store only their difference from the first. The prefix `1airship` means "replace first character and append 'airship'". See [Chapter 6](#sec-ncs-format).

**DataTable**
: UE asset type for tabular data. Contains rows of structured data. See [Chapter 7](#sec-data-extraction).

**Dedicated Drop**
: Loot that only drops from specific enemies. See [Appendix C](#sec-loot-system).

---

## E

**ECB**
: Electronic Codebook. Block cipher mode where identical plaintext blocks produce identical ciphertext. See [Chapter 4](#sec-save-files).

---

## F

**FField**
: UE5's property descriptor base class. Replaces UProperty from UE4. See [Appendix A](#sec-sdk-layouts).

**FName**
: Unreal's string identifier. 8 bytes containing index and instance number. See [Chapter 2](#sec-unreal-architecture).

**FNV-1a**
: Fowler-Noll-Vo hash function (variant 1a). Used by NCS format for field name lookups. 64-bit version with offset basis 0xcbf29ce484222325 and prime 0x100000001b3. See [Chapter 6](#sec-ncs-format).

**FOD (Fog of Discovery)**
: The map fog overlay that clears as you explore. Stored in save files as a 128x128 grayscale alpha map per zone (0=fogged, 255=revealed, intermediate values for soft edges). See [Chapter 4](#sec-save-files).

**FNamePool**
: Global string pool containing all FName strings. Also called GNames. See [Chapter 2](#sec-unreal-architecture).

**FProperty**
: UE5 property descriptor. Contains offset, size, and type information. See [Appendix A](#sec-sdk-layouts).

**FString**
: Unreal's dynamic string type. 16 bytes containing pointer, count, and capacity. See [Appendix A](#sec-sdk-layouts).

**FTransform**
: 96-byte structure containing rotation (FQuat), translation (FVector), and scale. See [Appendix A](#sec-sdk-layouts).

**FVector**
: 24-byte 3D vector using doubles (not floats like UE4). See [Appendix A](#sec-sdk-layouts).

---

## G

**gBx**
: Magic header for NCS files (0x67 0x42 0x78). Followed by variant byte and Oodle-compressed payload. See [Chapter 6](#sec-ncs-format).

**GNames**
: Global FName string pool. Located at offset 0x112a1c80 from PE base. See [Chapter 2](#sec-unreal-architecture).

**GUObjectArray**
: Global array containing all UObjects. Located at offset 0x113878f0. See [Chapter 2](#sec-unreal-architecture).

**GWorld**
: Pointer to current UWorld. Located at offset 0x11532cb8. See [Appendix A](#sec-sdk-layouts).

---

## H

**Heap**
: Memory region for dynamic allocations. Valid range: 0x10000-0x800000000000. See [Chapter 3](#sec-memory-analysis).

**Hexadecimal**
: Base-16 number system using 0-9 and A-F. See [Chapter 1](#sec-binary-basics).

---

## I

**InternalIndex**
: UObject field at offset 0x0C containing position in GUObjectArray. See [Chapter 2](#sec-unreal-architecture).

**IoStore**
: UE5's container format using .utoc (table of contents) and .ucas (data) files. See [Chapter 7](#sec-data-extraction).

**Item Pool**
: Definition of possible loot drops. See [Appendix C](#sec-loot-system).

**Item Serial**
: Base85-encoded string representing an item's full configuration. See [Chapter 5](#sec-item-serials).

---

## L

**Licensed Parts**
: BL4's cross-manufacturer part system. A weapon can gain abilities from other manufacturers (e.g., Jakobs Ricochet on a Vladof rifle). Level-gated via `Att_MinGameStage_LicensedPart_*` attributes. See [Chapter 8](#sec-parts-system).

**Little-Endian**
: Byte order where least significant byte comes first. Used by x86/x64 and save files. See [Chapter 1](#sec-binary-basics).

**Luck**
: Game system that modifies loot rarity chances. See [Appendix C](#sec-loot-system).

---

## M

**Magic Header**
: Fixed bit pattern at start of data. Item serials use 0010000 (7 bits). See [Chapter 5](#sec-item-serials).

**MDMP**
: Windows minidump format. Used for memory dump files. See [Chapter 3](#sec-memory-analysis).

---

## N

**NCS (Nexus Config Store)**
: Gearbox's format for storing item pools, part data, and game configuration. Uses gBx header with Oodle compression. Contains data not found in standard PAK assets. See [Chapter 6](#sec-ncs-format).

**NamePrivate**
: UObject field at offset 0x18 containing the object's FName. See [Chapter 2](#sec-unreal-architecture).

**Nibble**
: Half a byte (4 bits). Used in VarInt encoding. See [Chapter 5](#sec-item-serials).

---

## O

**ObjectFlags**
: UObject field at offset 0x08 containing RF_* state flags. See [Chapter 2](#sec-unreal-architecture).

**Oodle**
: Compression algorithm used in UE5 IoStore containers and NCS files. BL4 uses version 9 (oo2core_9_win64.dll). See [Chapter 6](#sec-ncs-format) and [Appendix D](#sec-game-files).

**OuterPrivate**
: UObject field at offset 0x20 pointing to parent/container object. See [Chapter 2](#sec-unreal-architecture).

---

## P

**Parts System**
: BL4's item assembly system where weapons are composed of individual parts (barrel, grip, scope, etc.) drawn from per-manufacturer pools. Defined in NCS `inv.bin`, not PAK assets. See [Chapter 8](#sec-parts-system).

**Pak File**
: Legacy UE archive format (.pak). See [Chapter 7](#sec-data-extraction).

**Part**
: Token type in item serials representing weapon components. See [Chapter 5](#sec-item-serials).

**PE Base**
: Base address of Windows executable (0x140000000 for BL4). See [Chapter 3](#sec-memory-analysis).

**PKCS7**
: Padding scheme for block ciphers. See [Chapter 4](#sec-save-files).

**Pointer Chain**
: Sequence of pointer dereferences to reach target data. See [Chapter 3](#sec-memory-analysis).

---

## R

**Root/Sub Scope**
: In BL4's serial format, bit 7 of a part token index encodes whether a part is Root scope (core item type, bit 7 = 0) or Sub scope (attachment part, bit 7 = 1). The actual index is `token & 0x7F`. See [Chapter 8](#sec-parts-system).

**Rarity**
: Item quality tier (Common, Uncommon, Rare, Epic, Legendary). See [Appendix B](#sec-weapon-parts).

**Reflection**
: UE's runtime type information system. See [Chapter 2](#sec-unreal-architecture).

**retoc**
: Tool for extracting IoStore containers. See [Chapter 7](#sec-data-extraction).

**RIP-Relative**
: x64 addressing mode relative to instruction pointer. See [Chapter 3](#sec-memory-analysis).

---

## S

**Separator**
: Token type in item serials marking section boundaries. See [Chapter 5](#sec-item-serials).

**Serial**
: See Item Serial.

**Steam ID**
: Unique identifier for Steam accounts. Used to derive encryption key. See [Chapter 4](#sec-save-files).

**SuperStruct**
: UStruct field at offset 0x40 pointing to parent class. See [Appendix A](#sec-sdk-layouts).

---

## T

**Tag-Based Encoding**
: The most complex NCS binary format, used by `inv.bin` and `gbxactor.bin`. Each byte acts as a type tag (0x61=pair, 0x62=u32, 0x63=u32f32, 0x64-0x66=list, 0x70=variant, 0x7a=end) determining how to interpret following data. See [Chapter 6](#sec-ncs-format).

**TArray**
: Unreal's dynamic array template. 16 bytes containing pointer, count, max. See [Appendix A](#sec-sdk-layouts).

**Token**
: Discrete data element in item serial bitstream. Types: VarInt, VarBit, Part, String, Separator. See [Chapter 5](#sec-item-serials).

---

## U

**uasset**
: Unreal asset file containing object definitions and properties. See [Chapter 7](#sec-data-extraction).

**UClass**
: UE's class definition object. Size: 512 bytes. See [Chapter 2](#sec-unreal-architecture).

**ucas**
: IoStore container archive file containing compressed asset data. See [Chapter 7](#sec-data-extraction).

**uexp**
: Bulk data file accompanying .uasset. See [Chapter 7](#sec-data-extraction).

**UObject**
: Base class for all Unreal objects. Size: 40 bytes. See [Chapter 2](#sec-unreal-architecture).

**Usmap**
: Mapping file containing UE reflection data for parsing unversioned assets. See [Chapter 2](#sec-unreal-architecture).

**UStruct**
: UE's structure/class layout descriptor. Size: 176 bytes. See [Chapter 2](#sec-unreal-architecture).

**utoc**
: IoStore table of contents file. See [Chapter 7](#sec-data-extraction).

---

## V

**VarBit**
: Token type with 5-bit length prefix followed by N data bits. See [Chapter 5](#sec-item-serials).

**VarInt**
: Variable-length integer using 4-bit nibbles with continuation bits. See [Chapter 5](#sec-item-serials).

**Virtual Address (VA)**
: Memory address in process's virtual address space. See [Chapter 3](#sec-memory-analysis).

**VTable**
: Virtual function table pointer at offset 0x00 of UObjects. See [Chapter 2](#sec-unreal-architecture).

---

## W

**WASM**
: WebAssembly. Binary format for running code in browsers. See [Chapter 9](#sec-bl4-tools).

---

## Z

**Zen Package**
: UE5's internal package format used in IoStore containers. See [Chapter 7](#sec-data-extraction).

**zlib**
: Compression library. Used for save file compression after encryption. See [Chapter 4](#sec-save-files).

---

## Symbols

**@Ug**
: Prefix for all BL4 item serials. See [Chapter 5](#sec-item-serials).

**0x**
: Prefix indicating hexadecimal number. See [Chapter 1](#sec-binary-basics).

---

## Quick Reference: Playable Characters

| Character Name | Internal Class Name | Class Mod Label |
|----------------|---------------------|-----------------|
| Amon | Char_Paladin | Paladin Class Mod |
| Rafa | Char_ExoSoldier | Exo Soldier Class Mod |
| Harlowe | Char_Gravitar | Gravitar Class Mod |
| Vex | Char_DarkSiren | Dark Siren Class Mod |

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
