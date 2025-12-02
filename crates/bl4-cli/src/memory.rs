//! Process injection and memory manipulation for Borderlands 4
//!
//! This module provides functionality to:
//! - Find and attach to the BL4 process (including under Proton/Wine)
//! - Read/write process memory
//! - Read from memory dump files (gcore output)
//! - Locate UE5 structures (GUObjectArray, GNames, etc.)
//! - Generate usmap files from live process or dumps
//! - Read and modify game state (inventory, stats, etc.)

// Allow dead code for SDK documentation constants - these are reference values
// for UE5 structure layouts that may be used in future implementations.
#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LE};
use memmap2::Mmap;
use process_memory::{CopyAddress, ProcessHandle, PutAddress, TryIntoProcessHandle};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use sysinfo::System;

// ============================================================================
// Constants - BL4 UE5.5 Memory Layout
// ============================================================================

/// Windows PE image base address for BL4 executable
pub const PE_IMAGE_BASE: usize = 0x140000000;

/// PE header offset (e_lfanew) within DOS header
const PE_HEADER_OFFSET_LOCATION: usize = 0x3C;

/// Maximum valid PE header offset
const PE_HEADER_MAX_OFFSET: usize = 0x1000;

// -- UObject Layout (VERIFIED from SDK dump) --
// Source: BL4 SDK dump with Denuvo unpacking
//
// class UObject {
//   uint64_t vTable;        // +0x00 (8 bytes)
//   int32_t Flags;          // +0x08 (4 bytes)
//   int32_t InternalIndex;  // +0x0C (4 bytes)
//   class UClass *Class;    // +0x10 (8 bytes) - ClassPrivate
//   class FName Name;       // +0x18 (8 bytes) - NamePrivate (ComparisonIndex + Number)
//   class UObject *Outer;   // +0x20 (8 bytes) - OuterPrivate
// }; // Size: 0x28
//
// This matches standard UE5 layout (not custom like previously suspected).

/// VTable pointer offset in UObjectBase
pub const UOBJECT_VTABLE_OFFSET: usize = 0x00;

/// Object flags offset in UObjectBase
pub const UOBJECT_FLAGS_OFFSET: usize = 0x08;

/// Internal index offset in UObjectBase
pub const UOBJECT_INTERNAL_INDEX_OFFSET: usize = 0x0C;

/// ClassPrivate (UClass*) offset in UObjectBase
/// VERIFIED: Standard UE5 layout at +0x10
pub const UOBJECT_CLASS_OFFSET: usize = 0x10;

/// NamePrivate (FName) offset in UObjectBase
/// The FName ComparisonIndex is the first 4 bytes, Number is next 4 bytes
pub const UOBJECT_NAME_OFFSET: usize = 0x18;

/// OuterPrivate (UObject*) offset in UObjectBase
pub const UOBJECT_OUTER_OFFSET: usize = 0x20;

/// Minimum bytes to read for UObject header
pub const UOBJECT_HEADER_SIZE: usize = 0x28;

// -- SDK Data Pointers (offsets from PE_IMAGE_BASE) --
// These are offsets, add to PE_IMAGE_BASE (0x140000000) to get virtual address
// Updated for latest patch (Nov 2025)

/// GUObjectArray offset from image base (VA = 0x1513878f0)
pub const GOBJECTS_OFFSET: usize = 0x113878f0;

/// GNames (FNamePool) offset from image base (VA = 0x1512a1c80)
pub const GNAMES_OFFSET: usize = 0x112a1c80;

/// GWorld offset from image base (VA = 0x151532cb8)
pub const GWORLD_OFFSET: usize = 0x11532cb8;

/// ProcessEvent function offset from image base
pub const PROCESS_EVENT_OFFSET: usize = 0x14f7010;

/// ProcessEvent vtable index
pub const PROCESS_EVENT_VTABLE_INDEX: usize = 0x49;

// -- UField Layout --
// class UField : public UObject {
//   class UField *Next;  // +0x28
// }; // Size: 0x30

/// UField::Next offset
pub const UFIELD_NEXT_OFFSET: usize = 0x28;

/// UField size
pub const UFIELD_SIZE: usize = 0x30;

// -- UStruct Layout --
// class UStruct : public UField {
//   char pad_0030[16];      // +0x30
//   class UStruct *Super;   // +0x40
//   class UField *Children; // +0x48
//   char pad_0050[8];       // +0x50
//   int32_t Size;           // +0x58
//   int16_t MinAlignment;   // +0x5C
//   char pad_005E[82];      // +0x5E
// }; // Size: 0xB0

/// UStruct::Super offset
pub const USTRUCT_SUPER_OFFSET: usize = 0x40;

/// UStruct::Children offset (UField* linked list for UFunctions, not properties!)
pub const USTRUCT_CHILDREN_OFFSET: usize = 0x48;

/// UStruct::ChildProperties offset (FField* linked list for FProperty - UE5 only)
pub const USTRUCT_CHILDPROPERTIES_OFFSET: usize = 0x50;

/// UStruct::Size offset
pub const USTRUCT_SIZE_OFFSET: usize = 0x58;

/// UStruct::MinAlignment offset
pub const USTRUCT_MINALIGNMENT_OFFSET: usize = 0x5C;

/// UStruct total size
pub const USTRUCT_SIZE: usize = 0xB0;

// -- UClass Layout --
// class UClass : public UStruct {
//   char pad_00B0[96];              // +0xB0
//   class UObject *DefaultObject;  // +0x110
//   char pad_0118[232];             // +0x118
// }; // Size: 0x200

/// UClass::DefaultObject offset
pub const UCLASS_DEFAULT_OBJECT_OFFSET: usize = 0x110;

/// UClass total size
pub const UCLASS_SIZE: usize = 0x200;

// -- FField Layout (base class for FProperty) --
// UE5 changed from UProperty to FProperty (no longer UObject-derived)

/// FField::ClassPrivate offset - pointer to FFieldClass
pub const FFIELD_CLASS_OFFSET: usize = 0x00;

/// FField::Owner offset - FFieldVariant (UObject* or FField*)
pub const FFIELD_OWNER_OFFSET: usize = 0x08;

/// FField::Next offset - pointer to next FField in linked list
pub const FFIELD_NEXT_OFFSET: usize = 0x18;

/// FField::NamePrivate offset - FName
pub const FFIELD_NAME_OFFSET: usize = 0x20;

/// FField::FlagsPrivate offset - EObjectFlags
pub const FFIELD_FLAGS_OFFSET: usize = 0x28;

// -- FProperty Layout (extends FField) --

/// FProperty::ArrayDim offset
pub const FPROPERTY_ARRAYDIM_OFFSET: usize = 0x30;

/// FProperty::ElementSize offset
pub const FPROPERTY_ELEMENTSIZE_OFFSET: usize = 0x34;

/// FProperty::PropertyFlags offset (EPropertyFlags, 8 bytes)
pub const FPROPERTY_PROPERTYFLAGS_OFFSET: usize = 0x38;

/// FProperty::RepIndex offset
pub const FPROPERTY_REPINDEX_OFFSET: usize = 0x40;

/// FProperty::Offset_Internal offset - offset within struct
pub const FPROPERTY_OFFSET_OFFSET: usize = 0x4C;

/// FProperty total size (base, without type-specific data)
pub const FPROPERTY_BASE_SIZE: usize = 0x78;

// -- FFieldClass Layout --

/// FFieldClass::Name offset - FName identifying the property type
pub const FFIELDCLASS_NAME_OFFSET: usize = 0x00;

/// FFieldClass::Id offset - unique ID
pub const FFIELDCLASS_ID_OFFSET: usize = 0x08;

/// FFieldClass::CastFlags offset
pub const FFIELDCLASS_CASTFLAGS_OFFSET: usize = 0x10;

/// FFieldClass::ClassFlags offset
pub const FFIELDCLASS_CLASSFLAGS_OFFSET: usize = 0x18;

/// FFieldClass::SuperClass offset - parent FFieldClass*
pub const FFIELDCLASS_SUPERCLASS_OFFSET: usize = 0x20;

// -- Component Offsets --

/// ComponentToWorld offset in USceneComponent
pub const COMPONENT_TO_WORLD_OFFSET: usize = 0x240;

/// Bones TArray offset in USkinnedMeshComponent
pub const BONES_OFFSET: usize = 0x6A8;

/// Bones2 TArray offset in USkinnedMeshComponent
pub const BONES2_OFFSET: usize = 0x6B8;

// -- Actor Offsets --

/// RootComponent offset in AActor
pub const ACTOR_ROOT_COMPONENT_OFFSET: usize = 0x1C8;

// -- Controller Offsets --

/// PlayerState offset in AController
pub const CONTROLLER_PLAYER_STATE_OFFSET: usize = 0x398;

/// Pawn offset in AController
pub const CONTROLLER_PAWN_OFFSET: usize = 0x3D0;

/// Character offset in AController
pub const CONTROLLER_CHARACTER_OFFSET: usize = 0x3E0;

// -- PlayerController Offsets --

/// AcknowledgedPawn offset in APlayerController
pub const PLAYERCONTROLLER_ACKNOWLEDGED_PAWN_OFFSET: usize = 0x438;

/// PlayerCameraManager offset in APlayerController
pub const PLAYERCONTROLLER_CAMERA_MANAGER_OFFSET: usize = 0x448;

/// CheatManager offset in APlayerController
pub const PLAYERCONTROLLER_CHEAT_MANAGER_OFFSET: usize = 0x4F8;

/// CheatClass offset in APlayerController
pub const PLAYERCONTROLLER_CHEAT_CLASS_OFFSET: usize = 0x500;

// -- World Offsets --

/// PersistentLevel offset in UWorld
pub const WORLD_PERSISTENT_LEVEL_OFFSET: usize = 0x30;

/// GameState offset in UWorld
pub const WORLD_GAME_STATE_OFFSET: usize = 0x178;

/// Levels TArray offset in UWorld
pub const WORLD_LEVELS_OFFSET: usize = 0x190;

/// OwningGameInstance offset in UWorld
pub const WORLD_GAME_INSTANCE_OFFSET: usize = 0x1F0;

// -- ULevel Offsets --

/// Actors TArray offset in ULevel
pub const LEVEL_ACTORS_OFFSET: usize = 0xA0;

// -- Character Offsets --

/// Mesh offset in ACharacter
pub const CHARACTER_MESH_OFFSET: usize = 0x428;

/// CharacterMovement offset in ACharacter
pub const CHARACTER_MOVEMENT_OFFSET: usize = 0x430;

// -- Pawn Offsets --

/// PlayerState offset in APawn
pub const PAWN_PLAYER_STATE_OFFSET: usize = 0x3B0;

/// Controller offset in APawn
pub const PAWN_CONTROLLER_OFFSET: usize = 0x3C0;

// -- PlayerState Offsets --

/// PawnPrivate offset in APlayerState
pub const PLAYERSTATE_PAWN_OFFSET: usize = 0x408;

/// PlayerNamePrivate offset in APlayerState
pub const PLAYERSTATE_NAME_OFFSET: usize = 0x428;

// -- Oak Character Offsets (BL4 specific) --

/// DamageState offset in AOakCharacter
pub const OAK_CHARACTER_DAMAGE_STATE_OFFSET: usize = 0x4038;

/// HealthState offset in AOakCharacter
pub const OAK_CHARACTER_HEALTH_STATE_OFFSET: usize = 0x4640;

/// HealthCondition offset in AOakCharacter
pub const OAK_CHARACTER_HEALTH_CONDITION_OFFSET: usize = 0x5B98;

/// ActiveWeapons offset in AOakCharacter
pub const OAK_CHARACTER_ACTIVE_WEAPONS_OFFSET: usize = 0x5F50;

/// DownState offset in AOakCharacter
pub const OAK_CHARACTER_DOWN_STATE_OFFSET: usize = 0x6F40;

/// AmmoRegenerate offset in AOakCharacter
pub const OAK_CHARACTER_AMMO_REGEN_OFFSET: usize = 0x95E8;

// -- Oak PlayerController Offsets --

/// OakCharacter offset in AOakPlayerController
pub const OAK_PLAYERCONTROLLER_CHARACTER_OFFSET: usize = 0x0F20;

/// PersonalVehicleState offset in AOakPlayerController
pub const OAK_PLAYERCONTROLLER_VEHICLE_STATE_OFFSET: usize = 0x3880;

// -- Currency Manager Offsets --

/// Currencies TArray offset in UGbxCurrencyManager
pub const CURRENCY_MANAGER_CURRENCIES_OFFSET: usize = 0x30;

// -- FName Encoding --

/// FNamePool header address discovered from BL4 dump
pub const FNAMEPOOL_HEADER_ADDR: usize = 0x1513b0c80;

/// Block index shift for FName ComparisonIndex
pub const FNAME_BLOCK_SHIFT: u32 = 16;

/// Block offset mask for FName ComparisonIndex
pub const FNAME_OFFSET_MASK: u32 = 0xFFFF;

/// FName "Class" index (block 0, byte offset 1176 = 0x498, index = 1176/2 = 588)
pub const FNAME_CLASS_INDEX: u32 = 588;

/// Known FName indices for core UE types (block 0)
pub const FNAME_SCRIPTSTRUCT_INDEX: u32 = 92;
pub const FNAME_FUNCTION_INDEX: u32 = 93;
pub const FNAME_PACKAGE_INDEX: u32 = 126;
pub const FNAME_OBJECT_INDEX: u32 = 86;

// -- GUObjectArray --

/// Chunk size for GUObjectArray (objects per chunk)
pub const GUOBJECTARRAY_CHUNK_SIZE: usize = 65536;

// -- UClass Metaclass --
// With the VERIFIED SDK layout (ClassPrivate at +0x10, NamePrivate at +0x18),
// we can now properly search for UClass metaclass using the correct offsets.
// The previous anomaly was due to searching at wrong offsets (+0x08, +0x18).
//
// UClass metaclass characteristics:
// - ClassPrivate (+0x10) points to ITSELF (self-referential)
// - NamePrivate (+0x18) has FName index 588 ("Class")
// - vtable[0] points to code section
//
// TODO: Re-run metaclass scan with correct SDK offsets to find true UClass.

/// UClass metaclass address - PLACEHOLDER (needs re-scan with correct offsets)
/// This was found with wrong offsets; re-scan needed.
pub const UCLASS_METACLASS_ADDR: usize = 0x1514d3ed0;

/// vtable address of this self-referential object (for verification)
pub const UCLASS_METACLASS_VTABLE: usize = 0x14fd8a240;

// -- Pointer Validation --

/// Minimum valid pointer address (Windows user mode)
pub const MIN_VALID_POINTER: usize = 0x10000;

/// Maximum valid pointer address
pub const MAX_VALID_POINTER: usize = 0x800000000000;

/// Minimum vtable address (in executable range)
pub const MIN_VTABLE_ADDR: usize = 0x140000000;

/// Maximum vtable address (in executable data sections)
pub const MAX_VTABLE_ADDR: usize = 0x175000000;

// ============================================================================
// Fast Pattern Scanning (Boyer-Moore style with SIMD acceleration)
// ============================================================================

/// Find the longest contiguous run of non-wildcard bytes in a pattern.
/// Returns (start_offset, bytes) of the best anchor substring.
fn find_best_anchor<'a>(pattern: &'a [u8], mask: &[u8]) -> (usize, &'a [u8]) {
    let mut best_start = 0;
    let mut best_len = 0;
    let mut current_start = 0;
    let mut current_len = 0;

    for (i, &m) in mask.iter().enumerate() {
        if m != 0 {
            // Fixed byte
            if current_len == 0 {
                current_start = i;
            }
            current_len += 1;
        } else {
            // Wildcard - end current run
            if current_len > best_len {
                best_start = current_start;
                best_len = current_len;
            }
            current_len = 0;
        }
    }

    // Check final run
    if current_len > best_len {
        best_start = current_start;
        best_len = current_len;
    }

    if best_len == 0 {
        // All wildcards - return empty anchor
        (0, &[])
    } else {
        (best_start, &pattern[best_start..best_start + best_len])
    }
}

/// Verify a full pattern (with wildcards) at a given position in data.
#[inline]
fn verify_pattern(data: &[u8], pattern: &[u8], mask: &[u8]) -> bool {
    if data.len() < pattern.len() {
        return false;
    }
    for i in 0..pattern.len() {
        if mask[i] != 0 && data[i] != pattern[i] {
            return false;
        }
    }
    true
}

/// Fast pattern scan using memchr's SIMD-accelerated memmem finder.
///
/// This uses Boyer-Moore-style searching on the longest contiguous
/// non-wildcard substring, then verifies the full pattern at each hit.
///
/// For patterns without wildcards, this is equivalent to ripgrep's search.
/// For patterns with wildcards, we get O(n/m) average case on the anchor.
pub fn scan_pattern_fast(data: &[u8], pattern: &[u8], mask: &[u8]) -> Vec<usize> {
    if pattern.is_empty() {
        return vec![];
    }

    // Find the best anchor (longest fixed substring)
    let (anchor_offset, anchor_bytes) = find_best_anchor(pattern, mask);

    if anchor_bytes.is_empty() {
        // All wildcards - fall back to checking every position
        let mut results = Vec::new();
        for i in 0..=data.len().saturating_sub(pattern.len()) {
            if verify_pattern(&data[i..], pattern, mask) {
                results.push(i);
            }
        }
        return results;
    }

    // Use memchr's SIMD-accelerated finder
    let finder = memchr::memmem::Finder::new(anchor_bytes);
    let mut results = Vec::new();

    for anchor_pos in finder.find_iter(data) {
        // Calculate where the full pattern would start
        let pattern_start = anchor_pos.saturating_sub(anchor_offset);

        // Ensure we can fit the full pattern
        if pattern_start + pattern.len() > data.len() {
            continue;
        }

        // Also ensure the anchor position aligns correctly
        // (anchor_pos should equal pattern_start + anchor_offset)
        if anchor_pos != pattern_start + anchor_offset {
            continue;
        }

        // Verify the full pattern including wildcards
        if verify_pattern(&data[pattern_start..], pattern, mask) {
            results.push(pattern_start);
        }
    }

    results
}

// ============================================================================
// Memory Source Abstraction
// ============================================================================

/// Trait for reading memory from various sources (live process, dump file, etc.)
pub trait MemorySource: Send + Sync {
    /// Read bytes from a virtual address
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>>;

    /// Get the list of memory regions
    fn regions(&self) -> &[MemoryRegion];

    /// Check if this is a live (writable) source
    fn is_live(&self) -> bool;

    /// Read a u64 from memory
    fn read_u64(&self, address: usize) -> Result<u64> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes))
    }

    /// Read a u32 from memory
    fn read_u32(&self, address: usize) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        Ok(LE::read_u32(&bytes))
    }

    /// Read a pointer (usize) from memory
    fn read_ptr(&self, address: usize) -> Result<usize> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes) as usize)
    }

    /// Read a null-terminated string from memory
    fn read_cstring(&self, address: usize, max_len: usize) -> Result<String> {
        let bytes = self.read_bytes(address, max_len)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    /// Find a region containing the given address
    fn find_region(&self, address: usize) -> Option<&MemoryRegion> {
        self.regions().iter().find(|r| address >= r.start && address < r.end)
    }

    /// Check if an address is readable
    fn is_readable(&self, address: usize) -> bool {
        self.find_region(address).map(|r| r.is_readable()).unwrap_or(false)
    }
}

/// Memory dump file source
///
/// Supports Linux gcore dumps where file offset ≈ virtual address for
/// Wine/Proton processes with the main executable loaded at 0x140000000.
pub struct DumpFile {
    /// Memory-mapped dump file
    mmap: Mmap,
    /// Virtual address regions parsed from dump or maps file
    regions: Vec<MemoryRegion>,
    /// Base address offset (file_offset = va - base_offset for linear dumps)
    base_offset: usize,
    /// Path to the dump file
    pub path: PathBuf,
}

impl DumpFile {
    /// MDMP signature "MDMP" in little-endian
    const MDMP_SIGNATURE: u32 = 0x504D444D; // "MDMP"

    /// MDMP stream types
    const MEMORY_64_LIST_STREAM: u32 = 9;

    /// Open a memory dump file
    ///
    /// Supports:
    /// - Windows Minidump (MDMP) format - auto-detected by "MDMP" signature
    /// - Raw/gcore dumps - file offset ≈ virtual address
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)
            .with_context(|| format!("Failed to open dump file: {:?}", path))?;

        let mmap = unsafe { Mmap::map(&file) }
            .with_context(|| format!("Failed to mmap dump file: {:?}", path))?;

        eprintln!("Opened dump file: {:?} ({} MB)", path, mmap.len() / 1_000_000);

        // Check for MDMP signature
        if mmap.len() >= 4 && LE::read_u32(&mmap[0..4]) == Self::MDMP_SIGNATURE {
            eprintln!("Detected Windows Minidump (MDMP) format");
            return Self::parse_mdmp(mmap, path);
        }

        // Try to find an accompanying .maps file
        let maps_path = path.with_extension("maps");
        let regions = if maps_path.exists() {
            Self::parse_maps_file(&maps_path)?
        } else {
            // Create synthetic regions based on typical BL4 layout
            Self::create_default_regions(mmap.len())
        };

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0, // Linear mapping: file_offset == VA
            path,
        })
    }

    /// Parse Windows Minidump format
    fn parse_mdmp(mmap: Mmap, path: PathBuf) -> Result<Self> {
        // MINIDUMP_HEADER structure:
        // 0x00: Signature (4 bytes) - "MDMP"
        // 0x04: Version (4 bytes)
        // 0x08: NumberOfStreams (4 bytes)
        // 0x0C: StreamDirectoryRva (4 bytes)
        // 0x10: CheckSum (4 bytes)
        // 0x14: TimeDateStamp (4 bytes)
        // 0x18: Flags (8 bytes)

        if mmap.len() < 32 {
            bail!("MDMP file too small for header");
        }

        let num_streams = LE::read_u32(&mmap[0x08..0x0C]) as usize;
        let stream_dir_rva = LE::read_u32(&mmap[0x0C..0x10]) as usize;

        eprintln!("MDMP: {} streams, directory at {:#x}", num_streams, stream_dir_rva);

        // Each MINIDUMP_DIRECTORY entry is 12 bytes:
        // 0x00: StreamType (4 bytes)
        // 0x04: DataSize (4 bytes)
        // 0x08: Rva (4 bytes)

        let mut memory_ranges: Vec<(u64, u64, u64)> = Vec::new(); // (base_addr, size, file_offset)

        for i in 0..num_streams {
            let entry_offset = stream_dir_rva + i * 12;
            if entry_offset + 12 > mmap.len() {
                break;
            }

            let stream_type = LE::read_u32(&mmap[entry_offset..entry_offset + 4]);
            let data_size = LE::read_u32(&mmap[entry_offset + 4..entry_offset + 8]) as usize;
            let rva = LE::read_u32(&mmap[entry_offset + 8..entry_offset + 12]) as usize;

            if stream_type == Self::MEMORY_64_LIST_STREAM {
                eprintln!("Found Memory64ListStream at RVA {:#x}, size {}", rva, data_size);

                // MINIDUMP_MEMORY64_LIST structure:
                // 0x00: NumberOfMemoryRanges (8 bytes)
                // 0x08: BaseRva (8 bytes) - where memory data starts
                // 0x10: Array of MINIDUMP_MEMORY_DESCRIPTOR64 (16 bytes each)
                //       - StartOfMemoryRange (8 bytes)
                //       - DataSize (8 bytes)

                if rva + 16 > mmap.len() {
                    bail!("Memory64ListStream header out of bounds");
                }

                let num_ranges = LE::read_u64(&mmap[rva..rva + 8]) as usize;
                let base_rva = LE::read_u64(&mmap[rva + 8..rva + 16]);

                eprintln!("Memory64List: {} ranges, data starts at RVA {:#x}", num_ranges, base_rva);

                let mut current_file_offset = base_rva;

                for j in 0..num_ranges {
                    let desc_offset = rva + 16 + j * 16;
                    if desc_offset + 16 > mmap.len() {
                        break;
                    }

                    let start_addr = LE::read_u64(&mmap[desc_offset..desc_offset + 8]);
                    let range_size = LE::read_u64(&mmap[desc_offset + 8..desc_offset + 16]);

                    memory_ranges.push((start_addr, range_size, current_file_offset));
                    current_file_offset += range_size;
                }

                eprintln!("Parsed {} memory ranges from MDMP", memory_ranges.len());
                break;
            }
        }

        if memory_ranges.is_empty() {
            bail!("No Memory64ListStream found in MDMP - dump may be incomplete");
        }

        // Convert to MemoryRegion format
        let regions: Vec<MemoryRegion> = memory_ranges
            .iter()
            .map(|(base, size, file_offset)| MemoryRegion {
                start: *base as usize,
                end: (*base + *size) as usize,
                perms: "rw-p".to_string(),
                offset: *file_offset as usize,
                path: None,
            })
            .collect();

        // Print some diagnostic info about key regions
        let gobjects_va = PE_IMAGE_BASE + GOBJECTS_OFFSET;
        let gnames_va = PE_IMAGE_BASE + GNAMES_OFFSET;

        // Debug: show all ranges near the SDK offset
        eprintln!("Memory ranges near SDK GObjects offset ({:#x}):", gobjects_va);
        for region in &regions {
            // Show ranges within 1MB of the SDK offset
            if region.end > gobjects_va.saturating_sub(0x100000)
               && region.start < gobjects_va.saturating_add(0x100000) {
                eprintln!("  {:#x}-{:#x} (size {:#x}, file offset {:#x})",
                         region.start, region.end, region.end - region.start, region.offset);
            }
        }

        for region in &regions {
            if gobjects_va >= region.start && gobjects_va < region.end {
                eprintln!("GObjects ({:#x}) found in region {:#x}-{:#x}, file offset {:#x}",
                         gobjects_va, region.start, region.end, region.offset);
            }
            if gnames_va >= region.start && gnames_va < region.end {
                eprintln!("GNames ({:#x}) found in region {:#x}-{:#x}, file offset {:#x}",
                         gnames_va, region.start, region.end, region.offset);
            }
        }

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0,
            path,
        })
    }

    /// Open a dump with an explicit maps file
    pub fn open_with_maps<P: AsRef<Path>>(dump_path: P, maps_path: P) -> Result<Self> {
        let dump_path = dump_path.as_ref().to_path_buf();
        let file = File::open(&dump_path)
            .with_context(|| format!("Failed to open dump file: {:?}", dump_path))?;

        let mmap = unsafe { Mmap::map(&file) }
            .with_context(|| format!("Failed to mmap dump file: {:?}", dump_path))?;

        let regions = Self::parse_maps_file(maps_path.as_ref())?;

        eprintln!("Opened dump file: {:?} ({} MB) with {} regions",
                 dump_path, mmap.len() / 1_000_000, regions.len());

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0,
            path: dump_path,
        })
    }

    /// Parse a maps file (supports both /proc/pid/maps and custom dump format)
    fn parse_maps_file(path: &Path) -> Result<Vec<MemoryRegion>> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open maps file: {:?}", path))?;

        let reader = BufReader::new(file);
        let mut regions = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            // Try to detect format:
            // Custom format: "0xSTART 0xEND SIZE FILE_OFFSET"
            // Linux /proc/pid/maps: "START-END perms offset dev inode path"

            if parts[0].starts_with("0x") {
                // Custom dump format: 0xSTART 0xEND SIZE FILE_OFFSET
                if parts.len() < 4 {
                    continue;
                }

                let start = usize::from_str_radix(parts[0].trim_start_matches("0x"), 16).unwrap_or(0);
                let end = usize::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap_or(0);
                // parts[2] is size (decimal), we can compute it
                let file_offset = usize::from_str_radix(parts[3].trim_start_matches("0x"), 16).unwrap_or(0);

                regions.push(MemoryRegion {
                    start,
                    end,
                    perms: "rw-p".to_string(), // Assume readable/writable for dumps
                    offset: file_offset,
                    path: None,
                });
            } else {
                // Linux /proc/pid/maps format: START-END perms offset dev inode path
                let addr_parts: Vec<&str> = parts[0].split('-').collect();
                if addr_parts.len() != 2 {
                    continue;
                }

                let start = usize::from_str_radix(addr_parts[0], 16).unwrap_or(0);
                let end = usize::from_str_radix(addr_parts[1], 16).unwrap_or(0);
                let perms = parts.get(1).unwrap_or(&"").to_string();
                let offset = parts
                    .get(2)
                    .and_then(|s| usize::from_str_radix(s, 16).ok())
                    .unwrap_or(0);
                let path = parts.get(5).map(|s| s.to_string());

                regions.push(MemoryRegion {
                    start,
                    end,
                    perms,
                    offset,
                    path,
                });
            }
        }

        Ok(regions)
    }

    /// Create default regions for a dump without maps info
    fn create_default_regions(dump_size: usize) -> Vec<MemoryRegion> {
        // Based on typical BL4/Proton memory layout
        vec![
            // PE Header
            MemoryRegion {
                start: 0x140000000,
                end: 0x140001000,
                perms: "r--p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            // Code section
            MemoryRegion {
                start: 0x140001000,
                end: 0x14e61c000,
                perms: "r-xp".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            // Read-only data
            MemoryRegion {
                start: 0x14e61c000,
                end: 0x15120e000,
                perms: "r--p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            // Data section
            MemoryRegion {
                start: 0x15120e000,
                end: 0x15175c000,
                perms: "rw-p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            // Heap/runtime (extend to dump size)
            MemoryRegion {
                start: 0x15175c000,
                end: dump_size.min(0x800000000000),
                perms: "rw-p".to_string(),
                offset: 0,
                path: None,
            },
        ]
    }

    /// Convert virtual address to file offset
    fn va_to_offset(&self, va: usize) -> Option<usize> {
        // Look up the region containing this VA
        for region in &self.regions {
            if va >= region.start && va < region.end {
                // Found the region - calculate file offset
                let region_offset = va - region.start;
                let file_offset = region.offset + region_offset;
                if file_offset < self.mmap.len() {
                    return Some(file_offset);
                }
            }
        }

        // Fallback: for gcore dumps, file offset might equal VA minus base
        let offset = va.checked_sub(self.base_offset)?;
        if offset < self.mmap.len() {
            Some(offset)
        } else {
            None
        }
    }
}

impl MemorySource for DumpFile {
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let offset = self.va_to_offset(address)
            .ok_or_else(|| anyhow::anyhow!("Address {:#x} out of dump range", address))?;

        if offset + size > self.mmap.len() {
            bail!("Read of {} bytes at {:#x} exceeds dump size", size, address);
        }

        Ok(self.mmap[offset..offset + size].to_vec())
    }

    fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    fn is_live(&self) -> bool {
        false
    }
}

/// Process info for an attached BL4 instance
pub struct Bl4Process {
    pub pid: u32,
    pub handle: ProcessHandle,
    pub exe_path: PathBuf,
    pub maps: Vec<MemoryRegion>,
}

impl MemorySource for Bl4Process {
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.handle
            .copy_address(address, &mut buffer)
            .with_context(|| format!("Failed to read {} bytes at {:#x}", size, address))?;
        Ok(buffer)
    }

    fn regions(&self) -> &[MemoryRegion] {
        &self.maps
    }

    fn is_live(&self) -> bool {
        true
    }
}

/// A memory region from /proc/pid/maps
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: usize,
    pub end: usize,
    pub perms: String,
    pub offset: usize,
    pub path: Option<String>,
}

impl MemoryRegion {
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn is_readable(&self) -> bool {
        self.perms.starts_with('r')
    }

    pub fn is_writable(&self) -> bool {
        self.perms.chars().nth(1) == Some('w')
    }

    pub fn is_executable(&self) -> bool {
        self.perms.chars().nth(2) == Some('x')
    }
}

/// Find running Borderlands 4 process
pub fn find_bl4_process() -> Result<u32> {
    let mut system = System::new_all();
    system.refresh_all();

    // Collect all candidate processes with their memory usage
    let mut candidates: Vec<(u32, u64)> = Vec::new();

    for process in system.processes().values() {
        let pid = process.pid().as_u32();
        let memory = process.memory();

        // Check cmdline for Wine/Proton processes running BL4
        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/cmdline", pid)) {
            // Must contain Borderlands4.exe in the cmdline
            if cmdline.contains("Borderlands4.exe") || cmdline.contains("borderlands4.exe") {
                // Get the thread group ID (Tgid) - this is the main process ID
                // Threads have the same Tgid as their parent process
                let tgid = get_tgid(pid).unwrap_or(pid);

                // Check if this is the actual game process (high memory usage)
                // or just a launcher/wrapper (low memory usage)
                // The actual game uses several GB of RAM
                if memory > 1_000_000_000 {
                    // More than 1GB = likely the actual game
                    candidates.push((tgid, memory));
                } else {
                    // Could be a wrapper, but still add as fallback
                    candidates.push((tgid, memory));
                }
            }
        }

        // Also check process name directly
        let name = process.name().to_string_lossy();
        if name.contains("Borderlands4") || name.contains("borderlands4") {
            let tgid = get_tgid(pid).unwrap_or(pid);
            candidates.push((tgid, memory));
        }
    }

    // Deduplicate by PID (multiple threads may have the same Tgid)
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.dedup_by(|a, b| a.0 == b.0);

    if let Some((pid, memory)) = candidates.first() {
        eprintln!(
            "Found BL4 process: PID {} (memory: {} MB)",
            pid,
            memory / 1_000_000
        );
        return Ok(*pid);
    }

    bail!("Borderlands 4 process not found. Is the game running?")
}

/// Get the thread group ID (main process) for a given PID/TID
fn get_tgid(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if line.starts_with("Tgid:") {
            return line.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

/// Parse /proc/pid/maps to get memory regions
fn parse_maps(pid: u32) -> Result<Vec<MemoryRegion>> {
    let maps_path = format!("/proc/{}/maps", pid);
    let file = File::open(&maps_path)
        .with_context(|| format!("Failed to open {}. Do you have permission?", maps_path))?;

    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        // Parse address range: "7f1234000000-7f1234001000"
        let addr_parts: Vec<&str> = parts[0].split('-').collect();
        if addr_parts.len() != 2 {
            continue;
        }

        let start = usize::from_str_radix(addr_parts[0], 16).unwrap_or(0);
        let end = usize::from_str_radix(addr_parts[1], 16).unwrap_or(0);
        let perms = parts.get(1).unwrap_or(&"").to_string();
        let offset = parts
            .get(2)
            .and_then(|s| usize::from_str_radix(s, 16).ok())
            .unwrap_or(0);
        let path = parts.get(5).map(|s| s.to_string());

        regions.push(MemoryRegion {
            start,
            end,
            perms,
            offset,
            path,
        });
    }

    Ok(regions)
}

impl Bl4Process {
    /// Attach to a running BL4 process
    pub fn attach() -> Result<Self> {
        let pid = find_bl4_process()?;
        let handle = (pid as process_memory::Pid)
            .try_into_process_handle()
            .context("Failed to attach to process. Try running with sudo.")?;

        let maps = parse_maps(pid)?;

        // Find the main executable path
        let exe_path = std::fs::read_link(format!("/proc/{}/exe", pid))
            .unwrap_or_else(|_| PathBuf::from("unknown"));

        Ok(Bl4Process {
            pid,
            handle,
            exe_path,
            maps,
        })
    }

    /// Read bytes from process memory
    pub fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.handle
            .copy_address(address, &mut buffer)
            .with_context(|| format!("Failed to read {} bytes at {:#x}", size, address))?;
        Ok(buffer)
    }

    /// Read a u64 from process memory
    pub fn read_u64(&self, address: usize) -> Result<u64> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes))
    }

    /// Read a u32 from process memory
    pub fn read_u32(&self, address: usize) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        Ok(LE::read_u32(&bytes))
    }

    /// Read a pointer (usize) from process memory
    pub fn read_ptr(&self, address: usize) -> Result<usize> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes) as usize)
    }

    /// Read a null-terminated string from process memory
    pub fn read_cstring(&self, address: usize, max_len: usize) -> Result<String> {
        let bytes = self.read_bytes(address, max_len)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    /// Write bytes to process memory
    pub fn write_bytes(&self, address: usize, data: &[u8]) -> Result<()> {
        self.handle
            .put_address(address, data)
            .with_context(|| format!("Failed to write {} bytes at {:#x}", data.len(), address))?;
        Ok(())
    }

    /// Find the main executable module
    pub fn find_main_module(&self) -> Option<&MemoryRegion> {
        self.maps.iter().find(|r| {
            r.path
                .as_ref()
                .map(|p| p.contains("Borderlands4") && p.ends_with(".exe"))
                .unwrap_or(false)
                && r.is_executable()
        })
    }

    /// Scan memory for a byte pattern (SIMD-accelerated Boyer-Moore style)
    pub fn scan_pattern(&self, pattern: &[u8], mask: &[u8]) -> Result<Vec<usize>> {
        let mut results = Vec::new();

        for region in &self.maps {
            if !region.is_readable() || region.size() > 100 * 1024 * 1024 {
                continue; // Skip non-readable or huge regions
            }

            if let Ok(data) = self.read_bytes(region.start, region.size()) {
                // Use fast SIMD-accelerated pattern matching
                for offset in scan_pattern_fast(&data, pattern, mask) {
                    results.push(region.start + offset);
                }
            }
        }

        Ok(results)
    }

    /// Get process info summary
    pub fn info(&self) -> String {
        let main_module = self.find_main_module();
        let module_info = main_module
            .map(|m| format!("Base: {:#x}, Size: {:#x}", m.start, m.size()))
            .unwrap_or_else(|| "Not found".to_string());

        format!(
            "PID: {}\nExecutable: {}\nMain Module: {}\nMemory Regions: {}",
            self.pid,
            self.exe_path.display(),
            module_info,
            self.maps.len()
        )
    }
}

/// UE5 structure offsets
#[derive(Debug)]
pub struct Ue5Offsets {
    pub guobject_array: usize,
    pub gnames: usize,
}

/// Discovered GNames pool
#[derive(Debug)]
pub struct GNamesPool {
    pub address: usize,
    pub sample_names: Vec<(u32, String)>,
}

/// Discovered GUObjectArray
#[derive(Debug)]
pub struct GUObjectArray {
    pub address: usize,
    /// Pointer to the Objects** array (array of chunk pointers)
    pub objects_ptr: usize,
    pub max_elements: i32,
    pub num_elements: i32,
    /// First chunk pointer (for direct access to first 64K items)
    pub first_chunk_ptr: usize,
    /// Size of each FUObjectItem in bytes (16 for UE5.3+, 24 for older)
    pub item_size: usize,
}

/// GUObjectArray virtual address (PE_IMAGE_BASE + GOBJECTS_OFFSET)
pub const GUOBJECTARRAY_VA: usize = PE_IMAGE_BASE + GOBJECTS_OFFSET;

impl GUObjectArray {
    /// Discover GUObjectArray at the known offset
    ///
    /// FUObjectArray structure (UE5):
    /// - Offset 0:  Objects** (8 bytes) - pointer to chunk pointer array
    /// - Offset 8:  PreAllocatedObjects* (8 bytes) - usually NULL
    /// - Offset 16: MaxElements (4 bytes) - typically 0x200000 (2097152)
    /// - Offset 20: NumElements (4 bytes) - current count
    /// - Offset 24: MaxChunks (4 bytes)
    /// - Offset 28: NumChunks (4 bytes)
    pub fn discover(source: &dyn MemorySource) -> Result<Self> {
        let addr = GUOBJECTARRAY_VA;

        // Read the GUObjectArray header (32 bytes)
        let header = source.read_bytes(addr, 32)
            .context("Failed to read GUObjectArray header")?;

        let objects_ptr = LE::read_u64(&header[0..8]) as usize;
        let _preallocated = LE::read_u64(&header[8..16]) as usize;
        let max_elements = LE::read_i32(&header[16..20]);
        let num_elements = LE::read_i32(&header[20..24]);
        let _max_chunks = LE::read_i32(&header[24..28]);
        let num_chunks = LE::read_i32(&header[28..32]);

        // Validate the header
        if objects_ptr == 0 || objects_ptr < MIN_VALID_POINTER || objects_ptr > MAX_VALID_POINTER {
            bail!("GUObjectArray Objects pointer {:#x} is invalid", objects_ptr);
        }

        if max_elements <= 0 || max_elements > 10_000_000 {
            bail!("GUObjectArray MaxElements {} is unreasonable", max_elements);
        }

        if num_elements <= 0 || num_elements > max_elements {
            bail!("GUObjectArray NumElements {} is invalid (max={})", num_elements, max_elements);
        }

        eprintln!("Found GUObjectArray at {:#x}:", addr);
        eprintln!("  Objects ptr: {:#x}", objects_ptr);
        eprintln!("  MaxElements: {}", max_elements);
        eprintln!("  NumElements: {}", num_elements);
        eprintln!("  NumChunks: {}", num_chunks);

        // Read first chunk pointer to validate and detect item size
        let first_chunk_data = source.read_bytes(objects_ptr, 8)?;
        let first_chunk_ptr = LE::read_u64(&first_chunk_data) as usize;

        if first_chunk_ptr == 0 || first_chunk_ptr < MIN_VALID_POINTER {
            bail!("First chunk pointer {:#x} is invalid", first_chunk_ptr);
        }

        eprintln!("  First chunk at: {:#x}", first_chunk_ptr);

        // Detect FUObjectItem size by examining the first few items
        // UE5.0-5.2: 24 bytes (Object* + Flags + ClusterRootIndex + SerialNumber + padding)
        // UE5.3+: 16 bytes (Object* + combined Flags/Serial)
        let item_size = Self::detect_item_size(source, first_chunk_ptr)?;
        eprintln!("  Detected FUObjectItem size: {} bytes", item_size);

        Ok(GUObjectArray {
            address: addr,
            objects_ptr,
            max_elements,
            num_elements,
            first_chunk_ptr,
            item_size,
        })
    }

    /// Detect FUObjectItem size by examining the object array
    fn detect_item_size(source: &dyn MemorySource, chunk_ptr: usize) -> Result<usize> {
        // Read enough data to check both 16 and 24 byte item sizes
        let test_data = source.read_bytes(chunk_ptr, 24 * 10)?;

        // Try 16-byte items first (UE5.3+)
        let mut valid_16 = 0;
        for i in 0..10 {
            let ptr = LE::read_u64(&test_data[i * 16..i * 16 + 8]) as usize;
            if ptr == 0 || (ptr >= MIN_VALID_POINTER && ptr < MAX_VALID_POINTER) {
                valid_16 += 1;
            }
        }

        // Try 24-byte items (UE5.0-5.2)
        let mut valid_24 = 0;
        for i in 0..10 {
            let ptr = LE::read_u64(&test_data[i * 24..i * 24 + 8]) as usize;
            if ptr == 0 || (ptr >= MIN_VALID_POINTER && ptr < MAX_VALID_POINTER) {
                valid_24 += 1;
            }
        }

        eprintln!("  Item size detection: 16-byte validity={}/10, 24-byte validity={}/10",
                 valid_16, valid_24);

        // Prefer 24-byte if both seem valid (UE5.5 likely uses 24)
        if valid_24 >= 8 {
            Ok(24)
        } else if valid_16 >= 8 {
            Ok(16)
        } else {
            // Default to 24 for BL4/UE5.5
            eprintln!("  Warning: Could not reliably detect item size, defaulting to 24");
            Ok(24)
        }
    }

    /// Iterate over all UObject pointers in the array
    pub fn iter_objects<'a>(&'a self, source: &'a dyn MemorySource) -> UObjectIterator<'a> {
        UObjectIterator {
            source,
            array: self,
            chunk_idx: 0,
            item_idx: 0,
            chunk_data: Vec::new(),
            chunk_ptr: 0,
        }
    }
}

/// Iterator over UObject pointers in GUObjectArray
pub struct UObjectIterator<'a> {
    source: &'a dyn MemorySource,
    array: &'a GUObjectArray,
    chunk_idx: usize,
    item_idx: usize,
    chunk_data: Vec<u8>,
    chunk_ptr: usize,
}

impl<'a> Iterator for UObjectIterator<'a> {
    type Item = (usize, usize); // (index, object_ptr)

    fn next(&mut self) -> Option<Self::Item> {
        let num_chunks = ((self.array.num_elements as usize) + GUOBJECTARRAY_CHUNK_SIZE - 1)
            / GUOBJECTARRAY_CHUNK_SIZE;

        loop {
            if self.chunk_idx >= num_chunks {
                return None;
            }

            let items_in_chunk = if self.chunk_idx == num_chunks - 1 {
                let remainder = (self.array.num_elements as usize) % GUOBJECTARRAY_CHUNK_SIZE;
                if remainder == 0 { GUOBJECTARRAY_CHUNK_SIZE } else { remainder }
            } else {
                GUOBJECTARRAY_CHUNK_SIZE
            };

            // Load chunk if needed
            if self.chunk_data.is_empty() || self.item_idx >= items_in_chunk {
                self.chunk_idx += if self.chunk_data.is_empty() { 0 } else { 1 };
                self.item_idx = 0;

                if self.chunk_idx >= num_chunks {
                    return None;
                }

                // Read chunk pointer
                let chunk_ptr_offset = self.array.objects_ptr + self.chunk_idx * 8;
                let chunk_ptr_data = self.source.read_bytes(chunk_ptr_offset, 8).ok()?;
                self.chunk_ptr = LE::read_u64(&chunk_ptr_data) as usize;

                if self.chunk_ptr == 0 {
                    self.chunk_data.clear();
                    continue;
                }

                // Read chunk data
                let items_to_read = if self.chunk_idx == num_chunks - 1 {
                    let remainder = (self.array.num_elements as usize) % GUOBJECTARRAY_CHUNK_SIZE;
                    if remainder == 0 { GUOBJECTARRAY_CHUNK_SIZE } else { remainder }
                } else {
                    GUOBJECTARRAY_CHUNK_SIZE
                };

                self.chunk_data = self.source
                    .read_bytes(self.chunk_ptr, items_to_read * self.array.item_size)
                    .ok()?;
            }

            // Get object pointer from current item
            let item_offset = self.item_idx * self.array.item_size;
            let obj_ptr = LE::read_u64(&self.chunk_data[item_offset..item_offset + 8]) as usize;

            let global_idx = self.chunk_idx * GUOBJECTARRAY_CHUNK_SIZE + self.item_idx;
            self.item_idx += 1;

            if obj_ptr != 0 {
                return Some((global_idx, obj_ptr));
            }
            // Skip null entries
        }
    }
}

/// PE executable section information
#[derive(Debug, Clone)]
pub struct PeSection {
    pub name: String,
    pub virtual_address: usize,
    pub virtual_size: usize,
    pub characteristics: u32,
}

impl PeSection {
    /// Check if this section is executable (contains code)
    pub fn is_executable(&self) -> bool {
        // IMAGE_SCN_MEM_EXECUTE = 0x20000000
        // IMAGE_SCN_CNT_CODE = 0x00000020
        (self.characteristics & 0x20000020) != 0
    }
}

/// Code section bounds for vtable validation
/// Holds multiple ranges since there can be gaps between code sections
#[derive(Debug, Clone)]
pub struct CodeBounds {
    pub ranges: Vec<(usize, usize)>, // (start, end) pairs
}

impl CodeBounds {
    /// Check if an address is within any code section
    pub fn contains(&self, addr: usize) -> bool {
        self.ranges.iter().any(|(start, end)| addr >= *start && addr < *end)
    }
}

/// Parse PE header to find code section bounds
/// Works with both live processes and memory dumps
pub fn find_code_bounds(source: &dyn MemorySource) -> Result<CodeBounds> {
    // Find PE image base by looking for MZ header in typical locations
    let pe_bases = [
        0x140000000usize, // Windows x64 default image base
        0x400000,         // Windows x86 default
        0x10000,          // Alternative
    ];

    for &base in &pe_bases {
        if let Ok(bounds) = parse_pe_code_section(source, base) {
            for (start, end) in &bounds.ranges {
                eprintln!("Found code range: {:#x}-{:#x} (from PE at {:#x})", start, end, base);
            }
            return Ok(bounds);
        }
    }

    // Fallback: scan for MZ header in memory regions
    for region in source.regions() {
        if region.start < 0x100000 || region.size() < 0x1000 {
            continue;
        }

        // Check for MZ header
        if let Ok(header) = source.read_bytes(region.start, 2) {
            if header == b"MZ" {
                if let Ok(bounds) = parse_pe_code_section(source, region.start) {
                    for (start, end) in &bounds.ranges {
                        eprintln!("Found code range: {:#x}-{:#x} (from PE at {:#x})", start, end, region.start);
                    }
                    return Ok(bounds);
                }
            }
        }
    }

    // Ultimate fallback: use hardcoded values for BL4
    eprintln!("Warning: Could not parse PE header, using fallback code bounds");
    Ok(CodeBounds {
        ranges: vec![(0x140001000, 0x14f000000)], // Conservative range for .ecode only
    })
}

/// Discover the "Class" UClass by scanning for self-referential pattern
/// In UE5, the UClass for "Class" has ClassPrivate pointing to itself
/// This doesn't rely on GUObjectArray being correct
pub fn discover_class_uclass(source: &dyn MemorySource) -> Result<usize> {
    let code_bounds = find_code_bounds(source)?;

    eprintln!("Scanning for Class UClass (self-referential pattern)...");

    // Scan writable data sections for the pattern:
    // - Valid vtable at +0x00 (first entry points to code)
    // - ClassPrivate at +0x10 points back to the object itself
    // - NamePrivate at +0x18 contains an FName index for "Class"

    let mut candidates: Vec<usize> = Vec::new();

    for region in source.regions() {
        if !region.is_readable() || !region.is_writable() {
            continue;
        }

        // Focus on data sections in the executable's address space
        if region.start < 0x151000000 || region.start > 0x175000000 {
            continue;
        }

        eprintln!("  Scanning region {:#x}-{:#x} for Class UClass...", region.start, region.end);

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Scan for self-referential pattern at 8-byte aligned addresses
        for i in (0..data.len().saturating_sub(0x28)).step_by(8) {
            let obj_addr = region.start + i;

            // Read potential vtable pointer
            let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;

            // vtable should be in a valid range (not null, not too low)
            if vtable_ptr < 0x140000000 || vtable_ptr > 0x160000000 {
                continue;
            }

            // ClassPrivate is at +0x10 - check if it's self-referential
            let class_private = LE::read_u64(&data[i + 0x10..i + 0x18]) as usize;

            if class_private != obj_addr {
                continue; // Not self-referential
            }

            // Verify vtable is valid (first entry points to code)
            if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                let first_func = LE::read_u64(&vtable_data) as usize;
                if !code_bounds.contains(first_func) {
                    continue;
                }
            } else {
                continue;
            }

            // Found a candidate!
            eprintln!("  Found self-referential object at {:#x} (vtable={:#x})", obj_addr, vtable_ptr);
            candidates.push(obj_addr);
        }
    }

    if candidates.is_empty() {
        // Try alternative offsets - maybe ClassPrivate is at a different offset
        eprintln!("  No self-referential UClass found at offset 0x10, trying alternative offsets...");

        for class_offset in [0x08, 0x18, 0x20, 0x28] {
            for region in source.regions() {
                if !region.is_readable() || !region.is_writable() {
                    continue;
                }

                if region.start < 0x151000000 || region.start > 0x175000000 {
                    continue;
                }

                let data = match source.read_bytes(region.start, region.size()) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                for i in (0..data.len().saturating_sub(0x30)).step_by(8) {
                    let obj_addr = region.start + i;
                    let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;

                    if vtable_ptr < 0x140000000 || vtable_ptr > 0x160000000 {
                        continue;
                    }

                    if i + class_offset + 8 > data.len() {
                        continue;
                    }

                    let class_private = LE::read_u64(&data[i + class_offset..i + class_offset + 8]) as usize;

                    if class_private != obj_addr {
                        continue;
                    }

                    if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                        let first_func = LE::read_u64(&vtable_data) as usize;
                        if !code_bounds.contains(first_func) {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    eprintln!("  Found self-referential at {:#x} with class_offset={:#x}", obj_addr, class_offset);
                    candidates.push(obj_addr);

                    if candidates.len() >= 3 {
                        break;
                    }
                }

                if candidates.len() >= 3 {
                    break;
                }
            }

            if !candidates.is_empty() {
                eprintln!("  Class UClass likely at offset {:#x}", class_offset);
                break;
            }
        }
    }

    if candidates.is_empty() {
        bail!("Could not find Class UClass (self-referential pattern not found)");
    }

    // Return the first candidate
    Ok(candidates[0])
}

/// Parse PE header at given base address to find code section
fn parse_pe_code_section(source: &dyn MemorySource, base: usize) -> Result<CodeBounds> {
    // Read DOS header
    let dos_header = source.read_bytes(base, 64)?;

    // Check MZ signature
    if dos_header[0] != b'M' || dos_header[1] != b'Z' {
        bail!("Invalid DOS signature at {:#x}", base);
    }

    // Get PE header offset (e_lfanew)
    let pe_offset = LE::read_u32(&dos_header[PE_HEADER_OFFSET_LOCATION..PE_HEADER_OFFSET_LOCATION + 4]) as usize;
    if pe_offset == 0 || pe_offset > PE_HEADER_MAX_OFFSET {
        bail!("Invalid PE offset: {:#x}", pe_offset);
    }

    // Read PE header
    let pe_header = source.read_bytes(base + pe_offset, 264)?; // PE sig + COFF header + Optional header

    // Check PE signature
    if &pe_header[0..4] != b"PE\0\0" {
        bail!("Invalid PE signature at {:#x}", base + pe_offset);
    }

    // Parse COFF header (starts at offset 4)
    let number_of_sections = LE::read_u16(&pe_header[6..8]) as usize;
    let size_of_optional_header = LE::read_u16(&pe_header[20..22]) as usize;

    if number_of_sections == 0 || number_of_sections > 100 {
        bail!("Invalid section count: {}", number_of_sections);
    }

    // Section headers start after optional header
    // COFF header is 20 bytes, optional header follows
    let sections_offset = pe_offset + 24 + size_of_optional_header;

    // Read all section headers (40 bytes each)
    let sections_data = source.read_bytes(base + sections_offset, number_of_sections * 40)?;

    let mut code_ranges: Vec<(usize, usize)> = Vec::new();

    for i in 0..number_of_sections {
        let section_offset = i * 40;
        let section_data = &sections_data[section_offset..section_offset + 40];

        // Section name (8 bytes, null-padded)
        let name_bytes = &section_data[0..8];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        // Virtual size, virtual address, characteristics
        let virtual_size = LE::read_u32(&section_data[8..12]) as usize;
        let virtual_address = LE::read_u32(&section_data[12..16]) as usize;
        let characteristics = LE::read_u32(&section_data[36..40]);

        let section = PeSection {
            name: name.clone(),
            virtual_address,
            virtual_size,
            characteristics,
        };

        // Check if this is an actual code section (not just executable metadata)
        // Only include sections that actually contain code, not .pdata/.reloc/etc
        let is_code_section = section.is_executable() &&
            (name.contains("text") || name.contains("code") || name == ".ecode" ||
             name.starts_with(".text") || name.starts_with(".code"));

        let section_start = base + virtual_address;
        let section_end = section_start + virtual_size;

        if is_code_section {
            code_ranges.push((section_start, section_end));
            eprintln!("  Found code section '{}': {:#x}-{:#x}",
                     name, section_start, section_end);
        } else if section.is_executable() {
            eprintln!("  Skipping executable non-code '{}': {:#x}-{:#x}",
                     name, section_start, section_end);
        } else {
            // Print non-executable sections for debugging (like .rdata where vtables live)
            if name.contains("data") || name.contains("rdata") {
                eprintln!("  Found data section '{}': {:#x}-{:#x} (chars: {:#x})",
                         name, section_start, section_end, characteristics);
            }
        }
    }

    if code_ranges.is_empty() {
        bail!("No code sections found in PE at {:#x}", base);
    }

    Ok(CodeBounds { ranges: code_ranges })
}

// ============================================================================
// Discovery Functions (work with any MemorySource)
// ============================================================================

/// Scan memory for a byte pattern with mask (SIMD-accelerated Boyer-Moore style)
pub fn scan_pattern(source: &dyn MemorySource, pattern: &[u8], mask: &[u8]) -> Result<Vec<usize>> {
    let mut results = Vec::new();

    for region in source.regions() {
        if !region.is_readable() || region.size() > 100 * 1024 * 1024 {
            continue; // Skip non-readable or huge regions
        }

        if let Ok(data) = source.read_bytes(region.start, region.size()) {
            // Use fast SIMD-accelerated pattern matching
            for offset in scan_pattern_fast(&data, pattern, mask) {
                results.push(region.start + offset);
            }
        }
    }

    Ok(results)
}

/// Discover GNames pool by searching for the characteristic "None" + "ByteProperty" pattern
pub fn discover_gnames(source: &dyn MemorySource) -> Result<GNamesPool> {
    // GNames starts with FNameEntry for "None" followed by "ByteProperty"
    // FNameEntry format in UE5: length_byte (low 6 bits + flags), string bytes
    // "None" with typical flags: 1e 01 4e 6f 6e 65 (length=4, flags, "None")
    // Then "ByteProperty": 10 03 42 79 74 65 50 72 6f 70 65 72 74 79

    // Search for "None" followed by "ByteProperty"
    let pattern = b"\x1e\x01None\x10\x03ByteProperty";
    let mask = vec![1u8; pattern.len()];

    let results = scan_pattern(source, pattern, &mask)?;

    if results.is_empty() {
        // Try alternative pattern without exact length bytes
        let alt_pattern: &[u8] = b"None";
        let alt_mask = vec![1u8; alt_pattern.len()];
        let alt_results = scan_pattern(source, alt_pattern, &alt_mask)?;

        // Filter to find ones followed by ByteProperty
        for addr in alt_results {
            if addr < 2 {
                continue;
            }
            // Check if "ByteProperty" follows within ~20 bytes
            if let Ok(data) = source.read_bytes(addr.saturating_sub(2), 64) {
                if let Some(_pos) = data.windows(12).position(|w| w == b"ByteProperty") {
                    // Found it! The pool starts before "None"
                    let gnames_addr = addr - 2; // Account for length/flags bytes

                    // Read some sample names
                    let mut sample_names = Vec::new();
                    sample_names.push((0, "None".to_string()));
                    sample_names.push((1, "ByteProperty".to_string()));

                    // Try to read more names from the pool
                    if let Ok(pool_data) = source.read_bytes(gnames_addr, 4096) {
                        let mut offset = 0;
                        let mut index = 0u32;
                        while offset < pool_data.len() - 2 && sample_names.len() < 20 {
                            // FNameEntry: length_byte (6 bits len, 2 bits flags), string
                            let len_byte = pool_data[offset];
                            let string_len = (len_byte >> 1) & 0x3F;
                            if string_len == 0 || string_len > 60 {
                                offset += 1;
                                continue;
                            }
                            let start = offset + 2; // Skip length byte and flags byte
                            let end = start + string_len as usize;
                            if end <= pool_data.len() {
                                if let Ok(name) = String::from_utf8(pool_data[start..end].to_vec()) {
                                    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                                        sample_names.push((index, name));
                                    }
                                }
                            }
                            offset = end;
                            index += 1;
                        }
                    }

                    return Ok(GNamesPool {
                        address: gnames_addr,
                        sample_names,
                    });
                }
            }
        }

        bail!("GNames pool not found. The game may use a different FName format.");
    }

    let gnames_addr = results[0];

    // Read sample names
    let sample_names = vec![
        (0, "None".to_string()),
        (1, "ByteProperty".to_string()),
    ];

    Ok(GNamesPool {
        address: gnames_addr,
        sample_names,
    })
}

/// Check if a pointer value looks like a valid heap/data pointer for this dump
/// Windows heap is typically in the range 0x00010000-0x7FFFFFFFFFFF
fn is_valid_pointer(ptr: usize) -> bool {
    // Accept both low heap (Windows user mode) and high addresses (executable sections)
    ptr >= MIN_VALID_POINTER && ptr < MAX_VALID_POINTER
}

/// Discover GUObjectArray by scanning code for the access pattern
///
/// Uses the code pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1
/// This is: mov rax, [rip+offset]; mov rcx, [rax+rcx*8]; lea rax, [rcx+rdx*8]
/// The RIP-relative offset in the first instruction points to GUObjectArray
pub fn discover_guobject_array(source: &dyn MemorySource, _gnames_addr: usize) -> Result<GUObjectArray> {
    // First, try to find GUObjectArray via code pattern scanning
    // Pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1 EB ??
    // This is: mov rax, [rip+disp32]; mov rcx, [rax+rcx*8]; lea rax, [rcx+rdx*8]; jmp

    eprintln!("Searching for GUObjectArray via code pattern...");

    // Try two approaches:
    // 1. Specific pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1
    // 2. Generic: any 48 8B 05 (mov rax, [rip+disp]) pointing to valid GUObjectArray

    let pattern_suffix: &[u8] = &[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1];
    let mut found_candidates: Vec<(usize, usize)> = Vec::new(); // (code_addr, guobj_addr)

    // Scan code sections for this pattern
    for region in source.regions() {
        // Look for regions in the main executable range (covers both .ecode and .code sections)
        // .ecode: 0x140001000-0x14e61c000
        // .code:  0x15218c000-0x15f273720
        if region.start < 0x140000000 || region.start > 0x175000000 {
            continue;
        }
        // Skip small regions (likely not code)
        if region.size() < 1024 * 1024 {
            continue;
        }

        eprintln!("  Scanning {:#x}-{:#x} ({} MB)...",
                 region.start, region.end, region.size() / (1024 * 1024));

        // Read the region in chunks to avoid huge allocations
        let chunk_size = 16 * 1024 * 1024; // 16MB chunks
        let mut offset = 0usize;

        while offset < region.size() {
            let read_size = chunk_size.min(region.size() - offset);
            let data = match source.read_bytes(region.start + offset, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            // Search for pattern: 48 8B 05 [4 bytes disp] 48 8B 0C C8 48 8D 04 D1
            for i in 0..data.len().saturating_sub(20) {
                // Check for mov rax, [rip+disp32] prefix
                if data[i] == 0x48 && data[i + 1] == 0x8B && data[i + 2] == 0x05 {
                    // Check suffix after the 4-byte displacement
                    if data[i + 7..].starts_with(pattern_suffix) {
                        // Found the pattern! Extract the RIP-relative displacement
                        let disp = LE::read_i32(&data[i + 3..i + 7]);
                        let instruction_addr = region.start + offset + i;
                        let next_instruction = instruction_addr + 7; // RIP points to next instruction
                        let guobject_addr = (next_instruction as i64 + disp as i64) as usize;

                        eprintln!("Found GObjects access pattern at {:#x}", instruction_addr);
                        eprintln!("  Displacement: {:#x} ({})", disp, disp);
                        eprintln!("  Calculated GUObjectArray address: {:#x}", guobject_addr);

                        // Validate by reading the structure
                        if let Ok(header) = source.read_bytes(guobject_addr, 32) {
                            let objects_ptr = LE::read_u64(&header[0..8]) as usize;
                            let max_elements = LE::read_i32(&header[16..20]);
                            let num_elements = LE::read_i32(&header[20..24]);
                            let num_chunks = LE::read_i32(&header[28..32]);

                            eprintln!("  Objects**: {:#x}", objects_ptr);
                            eprintln!("  MaxElements: {}", max_elements);
                            eprintln!("  NumElements: {}", num_elements);
                            eprintln!("  NumChunks: {}", num_chunks);

                            // Validate structure
                            if objects_ptr > MIN_VALID_POINTER && objects_ptr < MAX_VALID_POINTER
                                && max_elements > 0 && max_elements <= 10_000_000
                                && num_elements > 0 && num_elements <= max_elements
                                && num_chunks > 0 && num_chunks <= 100
                            {
                                // Read first chunk pointer
                                if let Ok(chunk_data) = source.read_bytes(objects_ptr, 8) {
                                    let first_chunk = LE::read_u64(&chunk_data) as usize;

                                    if first_chunk > MIN_VALID_POINTER && first_chunk < MAX_VALID_POINTER {
                                        // Detect item size
                                        let item_size = GUObjectArray::detect_item_size(source, first_chunk)
                                            .unwrap_or(24);

                                        eprintln!("*** Found valid GUObjectArray at {:#x}! ***", guobject_addr);
                                        return Ok(GUObjectArray {
                                            address: guobject_addr,
                                            objects_ptr,
                                            max_elements,
                                            num_elements,
                                            first_chunk_ptr: first_chunk,
                                            item_size,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            offset += chunk_size - 20; // Overlap to catch patterns at chunk boundaries
        }
    }

    eprintln!("Specific pattern not found, trying generic RIP-relative load scan...");

    // Generic search: find any 48 8B 05 that points to valid GUObjectArray
    // This is slower but more flexible
    for region in source.regions() {
        if region.start < 0x140000000 || region.start > 0x160000000 {
            continue;
        }
        if region.size() < 1024 * 1024 {
            continue;
        }

        let chunk_size = 16 * 1024 * 1024;
        let mut offset = 0usize;

        while offset < region.size() {
            let read_size = chunk_size.min(region.size() - offset);
            let data = match source.read_bytes(region.start + offset, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            // Search for mov rax, [rip+disp32]: 48 8B 05 xx xx xx xx
            for i in 0..data.len().saturating_sub(7) {
                if data[i] == 0x48 && data[i + 1] == 0x8B && data[i + 2] == 0x05 {
                    let disp = LE::read_i32(&data[i + 3..i + 7]);
                    let instruction_addr = region.start + offset + i;
                    let next_instruction = instruction_addr + 7;
                    let target_addr = (next_instruction as i64 + disp as i64) as usize;

                    // Quick filter: target must be in data section range
                    if target_addr < 0x15100000 || target_addr > 0x15200000 {
                        continue;
                    }

                    // Read and validate as GUObjectArray
                    if let Ok(header) = source.read_bytes(target_addr, 32) {
                        let objects_ptr = LE::read_u64(&header[0..8]) as usize;
                        let max_elements = LE::read_i32(&header[16..20]);
                        let num_elements = LE::read_i32(&header[20..24]);
                        let num_chunks = LE::read_i32(&header[28..32]);

                        // Check for valid GUObjectArray signature
                        if objects_ptr > MIN_VALID_POINTER && objects_ptr < MAX_VALID_POINTER
                            && max_elements >= 0x100000 && max_elements <= 0x400000  // 1M-4M range
                            && num_elements > 100_000 && num_elements <= max_elements
                            && num_chunks > 0 && num_chunks <= 100
                        {
                            let expected_chunks = (num_elements + 65535) / 65536;
                            if (num_chunks - expected_chunks).abs() <= 2 {
                                eprintln!("Generic scan found candidate at {:#x} -> {:#x}", instruction_addr, target_addr);
                                eprintln!("  Objects**: {:#x}, MaxElem: {}, NumElem: {}, Chunks: {}",
                                         objects_ptr, max_elements, num_elements, num_chunks);
                                found_candidates.push((instruction_addr, target_addr));
                            }
                        }
                    }
                }
            }

            offset += chunk_size - 7;
        }
    }

    // Validate candidates
    for (code_addr, guobj_addr) in &found_candidates {
        eprintln!("Validating candidate from {:#x} -> {:#x}", code_addr, guobj_addr);

        if let Ok(header) = source.read_bytes(*guobj_addr, 32) {
            let objects_ptr = LE::read_u64(&header[0..8]) as usize;
            let max_elements = LE::read_i32(&header[16..20]);
            let num_elements = LE::read_i32(&header[20..24]);

            if let Ok(chunk_data) = source.read_bytes(objects_ptr, 8) {
                let first_chunk = LE::read_u64(&chunk_data) as usize;

                if first_chunk > MIN_VALID_POINTER && first_chunk < MAX_VALID_POINTER {
                    let item_size = GUObjectArray::detect_item_size(source, first_chunk).unwrap_or(24);

                    eprintln!("*** FOUND GUObjectArray at {:#x}! ***", guobj_addr);
                    return Ok(GUObjectArray {
                        address: *guobj_addr,
                        objects_ptr,
                        max_elements,
                        num_elements,
                        first_chunk_ptr: first_chunk,
                        item_size,
                    });
                }
            }
        }
    }

    eprintln!("Code pattern search complete, trying heap structure scan...");

    // Try scanning heap for GUObjectArray structure pattern
    // Look for: ptr(8) [ptr or 0](8) MaxElements(4) NumElements(4) MaxChunks(4) NumChunks(4)
    // Where NumChunks == ceil(NumElements / 65536)

    let code_bounds = find_code_bounds(source)?;

    // Candidate struct for scoring
    #[derive(Debug)]
    struct Candidate {
        address: usize,
        objects_ptr: usize,
        max_elements: i32,
        num_elements: i32,
        first_chunk_ptr: usize,
        item_size: usize,
        score: i32,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    const MAX_VALID_FNAME_INDEX: u32 = 20_000_000; // ~305 blocks worth

    // Count heap regions for debug
    let heap_regions: Vec<_> = source.regions().iter()
        .filter(|r| r.start < 0x140000000 && r.size() >= 1024 * 1024 && r.size() <= 1024 * 1024 * 1024)
        .collect();
    eprintln!("Found {} heap regions to scan", heap_regions.len());

    for region in source.regions() {
        // Focus on heap regions (lower addresses, not in exe range)
        if region.start >= 0x140000000 {
            continue;
        }
        if region.size() < 1024 * 1024 || region.size() > 1024 * 1024 * 1024 {
            continue;
        }

        eprintln!("  Scanning heap {:#x}-{:#x} ({} MB)...",
                 region.start, region.end, region.size() / (1024 * 1024));

        let data = match source.read_bytes(region.start, region.size().min(100 * 1024 * 1024)) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for i in (0..data.len().saturating_sub(32)).step_by(8) {
            let ptr = LE::read_u64(&data[i..i + 8]) as usize;
            let prealloc = LE::read_u64(&data[i + 8..i + 16]) as usize;
            let max_elem = LE::read_i32(&data[i + 16..i + 20]);
            let num_elem = LE::read_i32(&data[i + 20..i + 24]);
            let max_chunks = LE::read_i32(&data[i + 24..i + 28]);
            let num_chunks = LE::read_i32(&data[i + 28..i + 32]);

            // Quick filters
            if ptr == 0 || ptr < MIN_VALID_POINTER || ptr > MAX_VALID_POINTER {
                continue;
            }
            if prealloc != 0 && (prealloc < MIN_VALID_POINTER || prealloc > MAX_VALID_POINTER) {
                continue;
            }
            if num_elem < 100_000 || num_elem > 3_000_000 {
                continue;
            }
            let expected_chunks = (num_elem + 65535) / 65536;
            // Relax the chunks check - allow ±2 difference
            if (num_chunks - expected_chunks).abs() > 2 || num_chunks <= 0 {
                continue;
            }
            if max_chunks < num_chunks || max_chunks > 200 {
                continue;
            }
            if max_elem < num_elem || max_elem > 10_000_000 {
                continue;
            }

            eprintln!("Heap candidate at {:#x}:", region.start + i);
            eprintln!("  ptr={:#x}, prealloc={:#x}", ptr, prealloc);
            eprintln!("  max_elem={}, num_elem={}, max_chunks={}, num_chunks={}",
                     max_elem, num_elem, max_chunks, num_chunks);

            // Validate by reading chunk pointers
            if let Ok(chunk_data) = source.read_bytes(ptr, 8 * num_chunks as usize) {
                let first_chunk = LE::read_u64(&chunk_data[0..8]) as usize;
                eprintln!("  first_chunk={:#x}", first_chunk);

                if first_chunk > MIN_VALID_POINTER && first_chunk < MAX_VALID_POINTER {
                    let expected_chunks = (num_elem + 65535) / 65536;
                    // Try to validate objects with FName check
                    for item_size in [16usize, 24] {
                        if let Ok(items) = source.read_bytes(first_chunk, item_size * 10) {
                            let mut vtable_valid = 0;
                            let mut fname_valid = 0;
                            for j in 0..10 {
                                let obj_ptr = LE::read_u64(&items[j * item_size..]) as usize;
                                if obj_ptr == 0 {
                                    continue;
                                }
                                if obj_ptr > MIN_VALID_POINTER && obj_ptr < MAX_VALID_POINTER {
                                    if let Ok(obj) = source.read_bytes(obj_ptr, 0x20) {
                                        let vtable = LE::read_u64(&obj) as usize;
                                        if vtable > MIN_VALID_POINTER && vtable < MAX_VALID_POINTER {
                                            if let Ok(vt) = source.read_bytes(vtable, 8) {
                                                let func = LE::read_u64(&vt) as usize;
                                                if code_bounds.contains(func) {
                                                    vtable_valid += 1;
                                                }
                                            }
                                        }
                                        // Check FName index at offset 0x18
                                        let fname_idx = LE::read_u32(&obj[0x18..0x1C]);
                                        if fname_idx < MAX_VALID_FNAME_INDEX {
                                            fname_valid += 1;
                                        }
                                    }
                                }
                            }
                            if vtable_valid >= 3 && fname_valid >= 3 {
                                let mut score = 0i32;
                                // Bonus for first_chunk in PE data section
                                if first_chunk >= 0x150000000 && first_chunk < 0x160000000 {
                                    score += 100;
                                }
                                score += fname_valid as i32 * 10;
                                if num_chunks == expected_chunks {
                                    score += 5;
                                }
                                score += (num_elem / 10000) as i32;

                                let detected_item_size = GUObjectArray::detect_item_size(source, first_chunk)
                                    .unwrap_or(item_size);

                                eprintln!("Heap candidate at {:#x}: vtable_valid={}, fname_valid={}, score={}, first_chunk={:#x}",
                                         region.start + i, vtable_valid, fname_valid, score, first_chunk);

                                candidates.push(Candidate {
                                    address: region.start + i,
                                    objects_ptr: ptr,
                                    max_elements: max_elem,
                                    num_elements: num_elem,
                                    first_chunk_ptr: first_chunk,
                                    item_size: detected_item_size,
                                    score,
                                });
                                break; // Break out of item_size loop
                            }
                        }
                    }
                }
            }
        }
    }

    eprintln!("Heap scan complete, trying comprehensive memory scan...");

    // First, do a targeted debug scan of the .srdata section (where SDK says GObjects should be)
    // SDK offset 0x1513878f0 is in .srdata (0x15120e000-0x15175c000)
    let srdata_start = 0x15120e000usize;
    let srdata_end = 0x15175c000usize;
    let sdk_gobjects = 0x1513878f0usize;

    eprintln!("DEBUG: Targeted scan of .srdata section ({:#x}-{:#x})...", srdata_start, srdata_end);
    eprintln!("DEBUG: SDK says GObjects at {:#x}", sdk_gobjects);

    if let Ok(data) = source.read_bytes(srdata_start, srdata_end - srdata_start) {
        // Check what's at the SDK offset
        let sdk_offset_in_section = sdk_gobjects - srdata_start;
        if sdk_offset_in_section + 32 <= data.len() {
            eprintln!("DEBUG: Data at SDK GObjects offset ({:#x}):", sdk_gobjects);
            let ptr = LE::read_u64(&data[sdk_offset_in_section..sdk_offset_in_section + 8]) as usize;
            let prealloc = LE::read_u64(&data[sdk_offset_in_section + 8..sdk_offset_in_section + 16]) as usize;
            let max_elem = LE::read_i32(&data[sdk_offset_in_section + 16..sdk_offset_in_section + 20]);
            let num_elem = LE::read_i32(&data[sdk_offset_in_section + 20..sdk_offset_in_section + 24]);
            let max_chunks = LE::read_i32(&data[sdk_offset_in_section + 24..sdk_offset_in_section + 28]);
            let num_chunks = LE::read_i32(&data[sdk_offset_in_section + 28..sdk_offset_in_section + 32]);
            eprintln!("  ptr={:#x}, prealloc={:#x}", ptr, prealloc);
            eprintln!("  max_elem={}, num_elem={}, max_chunks={}, num_chunks={}", max_elem, num_elem, max_chunks, num_chunks);
        }

        // Find anything that looks like a count in expected range (100k-3M)
        let mut count_candidates = Vec::new();
        for i in (0..data.len().saturating_sub(32)).step_by(4) {
            let val = LE::read_i32(&data[i..i + 4]);
            if val >= 100_000 && val <= 3_000_000 {
                count_candidates.push((i, val));
            }
        }
        eprintln!("DEBUG: Found {} values in 100k-3M range in .srdata", count_candidates.len());

        // For the first few, show context
        for (idx, (offset, val)) in count_candidates.iter().take(10).enumerate() {
            // Check 16 bytes before - should be a valid pointer (Objects**)
            if *offset >= 20 {
                let ptr_offset = offset - 20; // num_elem is at +20 in the struct
                let ptr = LE::read_u64(&data[ptr_offset..ptr_offset + 8]) as usize;
                let prealloc = LE::read_u64(&data[ptr_offset + 8..ptr_offset + 16]) as usize;
                let max_elem = LE::read_i32(&data[ptr_offset + 16..ptr_offset + 20]);
                let num_chunks = LE::read_i32(&data[*offset + 4..offset + 8]);

                let expected_chunks = (*val + 65535) / 65536;

                if ptr > 0x10000 && ptr < 0x800000000000 && ptr % 8 == 0 {
                    eprintln!("DEBUG[{}]: count={} at .srdata+{:#x} (VA {:#x})",
                             idx, val, offset, srdata_start + offset);
                    eprintln!("  ptr={:#x}, prealloc={:#x}, max_elem={}, num_chunks={} (expected={})",
                             ptr, prealloc, max_elem, num_chunks, expected_chunks);

                    // Try to read the chunk pointer
                    if let Ok(chunk_data) = source.read_bytes(ptr, 8) {
                        let first_chunk = LE::read_u64(&chunk_data) as usize;
                        eprintln!("  first_chunk={:#x}", first_chunk);
                    }
                }
            }
        }
    } else {
        eprintln!("DEBUG: Could not read .srdata section");
    }

    // Comprehensive scan: search ALL readable regions for GUObjectArray pattern
    // Structure: ptr(8) prealloc(8) max_elem(4) num_elem(4) max_chunks(4) num_chunks(4)
    eprintln!("Scanning all memory regions for GUObjectArray structure...");

    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }
        // Skip tiny regions
        if region.size() < 4096 {
            continue;
        }
        // Skip huge regions (>1GB) - scan in chunks if needed
        let scan_size = region.size().min(512 * 1024 * 1024);

        let data = match source.read_bytes(region.start, scan_size) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for i in (0..data.len().saturating_sub(32)).step_by(8) {
            let ptr = LE::read_u64(&data[i..i + 8]) as usize;
            let prealloc = LE::read_u64(&data[i + 8..i + 16]) as usize;
            let max_elem = LE::read_i32(&data[i + 16..i + 20]);
            let num_elem = LE::read_i32(&data[i + 20..i + 24]);
            let max_chunks = LE::read_i32(&data[i + 24..i + 28]);
            let num_chunks = LE::read_i32(&data[i + 28..i + 32]);

            // Looser validation
            if ptr == 0 || ptr < 0x10000 || ptr > 0x800000000000 || ptr % 8 != 0 {
                continue;
            }
            if prealloc != 0 && (prealloc < 0x10000 || prealloc > 0x800000000000 || prealloc % 8 != 0) {
                continue;
            }
            // NumElements should be reasonably large (100k-3M for loaded game)
            if num_elem < 100_000 || num_elem > 3_000_000 {
                continue;
            }
            // MaxElements should be >= NumElements
            if max_elem < num_elem || max_elem > 10_000_000 {
                continue;
            }
            // NumChunks should approximately match (allow ±1)
            let expected_chunks = (num_elem + 65535) / 65536;
            if (num_chunks - expected_chunks).abs() > 1 {
                continue;
            }
            // MaxChunks sanity
            if max_chunks < num_chunks || max_chunks > 100 {
                continue;
            }

            // Found a candidate - try to validate by following pointers
            if let Ok(chunk_data) = source.read_bytes(ptr, 8) {
                let first_chunk = LE::read_u64(&chunk_data) as usize;
                if first_chunk > 0x10000 && first_chunk < 0x800000000000 && first_chunk % 8 == 0 {
                    eprintln!("Candidate at {:#x}: ptr={:#x}, num_elem={}, chunks={}, first_chunk={:#x}",
                             region.start + i, ptr, num_elem, num_chunks, first_chunk);

                    // Validate objects and compute score
                    for item_size in [24usize, 16] {
                        if let Ok(items) = source.read_bytes(first_chunk, item_size * 10) {
                            let mut vtable_valid = 0;
                            let mut fname_valid = 0;
                            for j in 0..10 {
                                let obj_ptr = LE::read_u64(&items[j * item_size..]) as usize;
                                if obj_ptr == 0 {
                                    continue;
                                }
                                if obj_ptr > 0x10000 && obj_ptr < 0x800000000000 {
                                    // Read UObject header to check vtable and FName
                                    if let Ok(obj) = source.read_bytes(obj_ptr, 0x20) {
                                        let vtable = LE::read_u64(&obj) as usize;
                                        if vtable > 0x140000000 && vtable < 0x160000000 {
                                            if let Ok(vt) = source.read_bytes(vtable, 8) {
                                                let func = LE::read_u64(&vt) as usize;
                                                if func > 0x140000000 && func < 0x160000000 {
                                                    vtable_valid += 1;
                                                }
                                            }
                                        }
                                        // Check FName index at offset 0x18
                                        let fname_idx = LE::read_u32(&obj[0x18..0x1C]);
                                        if fname_idx < MAX_VALID_FNAME_INDEX {
                                            fname_valid += 1;
                                        }
                                    }
                                }
                            }

                            // Only consider candidates with at least 3 valid vtables AND 3 valid FNames
                            if vtable_valid >= 3 && fname_valid >= 3 {
                                let mut score = 0i32;

                                // Bonus for first_chunk in PE data section
                                if first_chunk >= 0x150000000 && first_chunk < 0x160000000 {
                                    score += 100;
                                }

                                // Bonus for each valid FName
                                score += fname_valid as i32 * 10;

                                // Bonus for exact chunk count match
                                if num_chunks == expected_chunks {
                                    score += 5;
                                }

                                // Bonus for more objects
                                score += (num_elem / 10000) as i32;

                                let detected_item_size = GUObjectArray::detect_item_size(source, first_chunk)
                                    .unwrap_or(item_size);

                                eprintln!("Candidate at {:#x}: vtable_valid={}, fname_valid={}, score={}, first_chunk={:#x}",
                                         region.start + i, vtable_valid, fname_valid, score, first_chunk);

                                candidates.push(Candidate {
                                    address: region.start + i,
                                    objects_ptr: ptr,
                                    max_elements: max_elem,
                                    num_elements: num_elem,
                                    first_chunk_ptr: first_chunk,
                                    item_size: detected_item_size,
                                    score,
                                });

                                // No early return - collect all candidates
                                break; // But break out of item_size loop
                            }
                        }
                    }
                }
            }
        }
    }

    // Select the best candidate based on score
    if candidates.is_empty() {
        bail!("GUObjectArray not found in any memory region (no candidates passed validation)");
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.score.cmp(&a.score));

    eprintln!("\n=== Top GUObjectArray candidates ===");
    for (i, c) in candidates.iter().take(5).enumerate() {
        eprintln!("[{}] {:#x}: score={}, num_elem={}, first_chunk={:#x}, item_size={}",
                 i, c.address, c.score, c.num_elements, c.first_chunk_ptr, c.item_size);
    }

    let best = &candidates[0];
    eprintln!("\n*** Selected GUObjectArray at {:#x} (score={}) ***", best.address, best.score);

    Ok(GUObjectArray {
        address: best.address,
        objects_ptr: best.objects_ptr,
        max_elements: best.max_elements,
        num_elements: best.num_elements,
        first_chunk_ptr: best.first_chunk_ptr,
        item_size: best.item_size,
    })
}

/// Read an FName string from the GNames pool
pub fn read_fname(source: &dyn MemorySource, gnames_addr: usize, index: u32) -> Result<String> {
    // This is a simplified implementation
    // Real UE5 FNamePool uses chunked blocks

    // For now, scan forward from gnames_addr to find the indexed name
    // This is slow but works for testing

    if index == 0 {
        return Ok("None".to_string());
    }

    let data = source.read_bytes(gnames_addr, 64 * 1024)?; // Read 64KB of pool

    let mut offset = 0;
    let mut current_index = 0u32;

    while offset < data.len() - 2 && current_index < index {
        let len_byte = data[offset];
        let string_len = ((len_byte >> 1) & 0x3F) as usize;
        if string_len == 0 {
            offset += 1;
            continue;
        }
        offset += 2 + string_len; // Skip length byte, flags byte, and string
        current_index += 1;
    }

    if current_index == index && offset < data.len() - 2 {
        let len_byte = data[offset];
        let string_len = ((len_byte >> 1) & 0x3F) as usize;
        if string_len > 0 && offset + 2 + string_len <= data.len() {
            let name_bytes = &data[offset + 2..offset + 2 + string_len];
            return Ok(String::from_utf8_lossy(name_bytes).to_string());
        }
    }

    bail!("FName index {} not found", index)
}

/// Find UE5 global structures by pattern scanning
pub fn find_ue5_offsets(source: &dyn MemorySource) -> Result<Ue5Offsets> {
    let gnames = discover_gnames(source)?;

    // Try to find GUObjectArray
    let guobject_array = match discover_guobject_array(source, gnames.address) {
        Ok(arr) => arr.address,
        Err(_) => 0, // Not found yet
    };

    Ok(Ue5Offsets {
        gnames: gnames.address,
        guobject_array,
    })
}

/// UObject class type information
#[derive(Debug, Clone)]
pub struct UObjectInfo {
    pub address: usize,
    pub class_ptr: usize,
    pub name_index: u32,
    pub name: String,
    pub class_name: String,
}

/// Property type enumeration for usmap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EPropertyType {
    ByteProperty,
    BoolProperty,
    IntProperty,
    FloatProperty,
    ObjectProperty,
    NameProperty,
    DelegateProperty,
    DoubleProperty,
    ArrayProperty,
    StructProperty,
    StrProperty,
    TextProperty,
    InterfaceProperty,
    MulticastDelegateProperty,
    WeakObjectProperty,
    LazyObjectProperty,
    AssetObjectProperty,
    SoftObjectProperty,
    UInt64Property,
    UInt32Property,
    UInt16Property,
    Int64Property,
    Int16Property,
    Int8Property,
    MapProperty,
    SetProperty,
    EnumProperty,
    FieldPathProperty,
    OptionalProperty,
    Unknown,
}

impl EPropertyType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "ByteProperty" => Self::ByteProperty,
            "BoolProperty" => Self::BoolProperty,
            "IntProperty" => Self::IntProperty,
            "FloatProperty" => Self::FloatProperty,
            "ObjectProperty" => Self::ObjectProperty,
            "NameProperty" => Self::NameProperty,
            "DelegateProperty" => Self::DelegateProperty,
            "DoubleProperty" => Self::DoubleProperty,
            "ArrayProperty" => Self::ArrayProperty,
            "StructProperty" => Self::StructProperty,
            "StrProperty" => Self::StrProperty,
            "TextProperty" => Self::TextProperty,
            "InterfaceProperty" => Self::InterfaceProperty,
            "MulticastDelegateProperty" | "MulticastInlineDelegateProperty" | "MulticastSparseDelegateProperty" => Self::MulticastDelegateProperty,
            "WeakObjectProperty" => Self::WeakObjectProperty,
            "LazyObjectProperty" => Self::LazyObjectProperty,
            "AssetObjectProperty" => Self::AssetObjectProperty,
            "SoftObjectProperty" => Self::SoftObjectProperty,
            "UInt64Property" => Self::UInt64Property,
            "UInt32Property" => Self::UInt32Property,
            "UInt16Property" => Self::UInt16Property,
            "Int64Property" => Self::Int64Property,
            "Int16Property" => Self::Int16Property,
            "Int8Property" => Self::Int8Property,
            "MapProperty" => Self::MapProperty,
            "SetProperty" => Self::SetProperty,
            "EnumProperty" => Self::EnumProperty,
            "FieldPathProperty" => Self::FieldPathProperty,
            "OptionalProperty" => Self::OptionalProperty,
            "ClassProperty" => Self::ObjectProperty, // ClassProperty is a subtype of ObjectProperty
            "SoftClassProperty" => Self::SoftObjectProperty,
            _ => Self::Unknown,
        }
    }

    /// Get the usmap type ID
    pub fn to_usmap_id(&self) -> u8 {
        match self {
            Self::ByteProperty => 0,
            Self::BoolProperty => 1,
            Self::IntProperty => 2,
            Self::FloatProperty => 3,
            Self::ObjectProperty => 4,
            Self::NameProperty => 5,
            Self::DelegateProperty => 6,
            Self::DoubleProperty => 7,
            Self::ArrayProperty => 8,
            Self::StructProperty => 9,
            Self::StrProperty => 10,
            Self::TextProperty => 11,
            Self::InterfaceProperty => 12,
            Self::MulticastDelegateProperty => 13,
            Self::WeakObjectProperty => 14,
            Self::LazyObjectProperty => 15,
            Self::AssetObjectProperty => 16,
            Self::SoftObjectProperty => 17,
            Self::UInt64Property => 18,
            Self::UInt32Property => 19,
            Self::UInt16Property => 20,
            Self::Int64Property => 21,
            Self::Int16Property => 22,
            Self::Int8Property => 23,
            Self::MapProperty => 24,
            Self::SetProperty => 25,
            Self::EnumProperty => 26,
            Self::FieldPathProperty => 27,
            Self::OptionalProperty => 28,
            Self::Unknown => 255,
        }
    }
}

/// Property information extracted from FProperty
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// Property name
    pub name: String,
    /// Property type (e.g., "IntProperty", "StructProperty")
    pub property_type: EPropertyType,
    /// Property type name string
    pub type_name: String,
    /// Array dimension (1 for regular, >1 for fixed arrays)
    pub array_dim: i32,
    /// Element size in bytes
    pub element_size: i32,
    /// Property flags (EPropertyFlags)
    pub property_flags: u64,
    /// Offset within struct
    pub offset: i32,
    /// For StructProperty: the struct type name
    pub struct_type: Option<String>,
    /// For EnumProperty: the enum type name
    pub enum_type: Option<String>,
    /// For ArrayProperty/SetProperty/MapProperty: inner property type
    pub inner_type: Option<Box<PropertyInfo>>,
    /// For MapProperty: value property type
    pub value_type: Option<Box<PropertyInfo>>,
}

/// UStruct/UClass with extracted properties
#[derive(Debug, Clone)]
pub struct StructInfo {
    /// Address of the UStruct in memory
    pub address: usize,
    /// Name of the struct/class
    pub name: String,
    /// Super class/struct name (if any)
    pub super_name: Option<String>,
    /// Properties in this struct
    pub properties: Vec<PropertyInfo>,
    /// Size of the struct in bytes
    pub struct_size: i32,
    /// Whether this is a UClass (vs UScriptStruct)
    pub is_class: bool,
}

/// Enum information
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Address of the UEnum in memory
    pub address: usize,
    /// Name of the enum
    pub name: String,
    /// Enum values (name, value)
    pub values: Vec<(String, i64)>,
}

/// FNamePool structure discovered in BL4 UE5.5
/// The pool header is at a fixed location, with blocks stored in an array
#[derive(Debug, Clone)]
pub struct FNamePool {
    /// Address of the FNamePool header
    pub header_addr: usize,
    /// Current block count
    pub current_block: u32,
    /// Current byte cursor in the current block
    pub current_cursor: u32,
    /// Cached block addresses
    pub blocks: Vec<usize>,
}

impl FNamePool {
    /// Discover the FNamePool dynamically by searching for header pointing to known GNames
    ///
    /// The FNamePool header layout (UE5.5):
    /// +0x00: Lock (8 bytes) - should be 0 or small value
    /// +0x08: CurrentBlock (4 bytes)
    /// +0x0C: CurrentByteCursor (4 bytes)
    /// +0x10: Blocks[] - array of block pointers (8 bytes each)
    pub fn discover(source: &dyn MemorySource) -> Result<Self> {
        // First try known SDK location
        if let Ok(pool) = Self::discover_at_address(source, FNAMEPOOL_HEADER_ADDR) {
            return Ok(pool);
        }

        eprintln!("SDK FNamePool location invalid, searching dynamically...");

        // Search for FNamePool header in data sections
        // Look for structures where Block0 points to a valid FNameEntry
        for region in source.regions() {
            if !region.is_readable() {
                continue;
            }
            // Focus on PE data sections
            if region.start < 0x150000000 || region.start > 0x160000000 {
                continue;
            }

            let data = match source.read_bytes(region.start, region.size().min(16 * 1024 * 1024)) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for i in (0..data.len().saturating_sub(32)).step_by(8) {
                let lock = LE::read_u64(&data[i..i + 8]);
                let current_block = LE::read_u32(&data[i + 8..i + 12]);
                let current_cursor = LE::read_u32(&data[i + 12..i + 16]);
                let block0 = LE::read_u64(&data[i + 16..i + 24]) as usize;

                // Validate pattern
                if lock > 100 {
                    continue;
                }
                if current_block == 0 || current_block > 1000 {
                    continue;
                }
                if current_cursor == 0 || current_cursor > 0x100000 {
                    continue;
                }
                if block0 < 0x1000000 || block0 > 0x800000000000 || block0 % 8 != 0 {
                    continue;
                }

                // Try to read Block0 and validate it contains FName entries
                // FName entry starts with 2-byte header where bits 6-15 are length
                if let Ok(entry_data) = source.read_bytes(block0, 64) {
                    // First entry at offset 0 is usually "None" (len 4)
                    let header0 = LE::read_u16(&entry_data[0..2]);
                    let len0 = (header0 >> 6) as usize;
                    if len0 == 4 && &entry_data[2..6] == b"None" {
                        let header_addr = region.start + i;
                        eprintln!(
                            "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
                            header_addr, lock, current_block, current_cursor, block0
                        );

                        // Read all block pointers
                        let num_blocks = (current_block + 1) as usize;
                        let blocks_data = source.read_bytes(header_addr + 16, num_blocks * 8)?;
                        let blocks: Vec<usize> = blocks_data
                            .chunks_exact(8)
                            .map(|c| LE::read_u64(c) as usize)
                            .collect();

                        return Ok(FNamePool {
                            header_addr,
                            current_block,
                            current_cursor,
                            blocks,
                        });
                    }
                }
            }
        }

        bail!("FNamePool header not found")
    }

    /// Try to discover FNamePool at a specific address
    fn discover_at_address(source: &dyn MemorySource, addr: usize) -> Result<Self> {
        let header_data = source.read_bytes(addr, 24)?;
        let lock = LE::read_u64(&header_data[0..8]);
        let current_block = LE::read_u32(&header_data[8..12]);
        let current_cursor = LE::read_u32(&header_data[12..16]);
        let block0 = LE::read_u64(&header_data[16..24]) as usize;

        // Validate header
        if current_block == 0 || current_block > 1000 {
            bail!("FNamePool current_block {} invalid", current_block);
        }
        if block0 == 0 || block0 < MIN_VALID_POINTER || block0 > MAX_VALID_POINTER {
            bail!("FNamePool block0 pointer {:#x} is invalid", block0);
        }

        // Verify block0 contains "None" at offset 0
        let entry_data = source.read_bytes(block0, 8)?;
        let header0 = LE::read_u16(&entry_data[0..2]);
        let len0 = (header0 >> 6) as usize;
        if len0 != 4 || &entry_data[2..6] != b"None" {
            bail!("Block0 doesn't start with 'None' entry");
        }

        eprintln!(
            "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
            addr, lock, current_block, current_cursor, block0
        );

        // Read all block pointers
        let num_blocks = (current_block + 1) as usize;
        let blocks_data = source.read_bytes(addr + 16, num_blocks * 8)?;
        let blocks: Vec<usize> = blocks_data
            .chunks_exact(8)
            .map(|c| LE::read_u64(c) as usize)
            .collect();

        Ok(FNamePool {
            header_addr: addr,
            current_block,
            current_cursor,
            blocks,
        })
    }

    /// Discover FNamePool using the known GNames pool address
    /// Searches for header structures that have Block0 == gnames_addr
    pub fn discover_with_gnames(source: &dyn MemorySource, gnames_addr: usize) -> Result<Self> {
        eprintln!("Searching for FNamePool header with Block0 = {:#x}...", gnames_addr);

        // Search in PE data sections for a header pointing to gnames_addr
        for region in source.regions() {
            if !region.is_readable() {
                continue;
            }
            // Focus on PE data sections
            if region.start < 0x140000000 || region.start > 0x160000000 {
                continue;
            }

            let data = match source.read_bytes(region.start, region.size().min(32 * 1024 * 1024)) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for i in (0..data.len().saturating_sub(32)).step_by(8) {
                let block0 = LE::read_u64(&data[i + 16..i + 24]) as usize;
                if block0 != gnames_addr {
                    continue;
                }

                let lock = LE::read_u64(&data[i..i + 8]);
                let current_block = LE::read_u32(&data[i + 8..i + 12]);
                let current_cursor = LE::read_u32(&data[i + 12..i + 16]);

                // Validate
                if lock > 100 || current_block == 0 || current_block > 1000 {
                    continue;
                }

                let header_addr = region.start + i;
                eprintln!(
                    "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
                    header_addr, lock, current_block, current_cursor, block0
                );

                // Read all block pointers
                let num_blocks = (current_block + 1) as usize;
                let blocks_data = source.read_bytes(header_addr + 16, num_blocks * 8)?;
                let blocks: Vec<usize> = blocks_data
                    .chunks_exact(8)
                    .map(|c| LE::read_u64(c) as usize)
                    .collect();

                return Ok(FNamePool {
                    header_addr,
                    current_block,
                    current_cursor,
                    blocks,
                });
            }
        }

        bail!("FNamePool header with Block0={:#x} not found", gnames_addr)
    }
}

/// FNamePool reader for UE5
/// UE5 uses a chunked FNamePool with block-based storage
pub struct FNameReader {
    /// The FNamePool structure
    pub pool: FNamePool,
    /// Cached name entries: index -> name
    cache: std::collections::HashMap<u32, String>,
}

impl FNameReader {
    pub fn new(pool: FNamePool) -> Self {
        Self {
            pool,
            cache: std::collections::HashMap::new(),
        }
    }

    /// Legacy constructor for compatibility
    pub fn new_legacy(pool_base: usize) -> Self {
        Self {
            pool: FNamePool {
                header_addr: 0,
                current_block: 0,
                current_cursor: 0,
                blocks: vec![pool_base],
            },
            cache: std::collections::HashMap::new(),
        }
    }

    /// Read an FName entry from the pool
    /// FName index encoding in UE5:
    /// - ComparisonIndex = (BlockIndex << 16) | (BlockOffset >> 1)
    /// - BlockOffset is the byte offset within the block, divided by 2
    pub fn read_name(&mut self, source: &dyn MemorySource, fname_index: u32) -> Result<String> {
        if fname_index == 0 {
            return Ok("None".to_string());
        }

        // Check cache first
        if let Some(name) = self.cache.get(&fname_index) {
            return Ok(name.clone());
        }

        // Extract block index and offset from ComparisonIndex
        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        // Get block address
        let block_addr = if block_index < self.pool.blocks.len() {
            self.pool.blocks[block_index]
        } else {
            bail!("FName block {} out of range (have {} blocks)", block_index, self.pool.blocks.len());
        };

        if block_addr == 0 {
            bail!("FName block {} is null", block_index);
        }

        // Read the FNameEntry at block + offset
        let entry_addr = block_addr + block_offset;
        let header = source.read_bytes(entry_addr, 2)?;
        let header_val = LE::read_u16(&header);

        // FNameEntry header format (UE5):
        // - bIsWide: bit 0
        // - ProbeHashBits: bits 1-5 (5 bits)
        // - Len: bits 6-15 (10 bits)
        let is_wide = (header_val & 1) != 0;
        let len = (header_val >> 6) as usize;

        if len == 0 || len > 1024 {
            // Try alternative BL4-specific format: Len in bits 1-6 of first byte
            let alt_len = ((header[0] >> 1) & 0x3F) as usize;
            if alt_len > 0 && alt_len <= 63 {
                let bytes = source.read_bytes(entry_addr + 2, alt_len)?;
                let name = String::from_utf8_lossy(&bytes).to_string();
                self.cache.insert(fname_index, name.clone());
                return Ok(name);
            }
            bail!(
                "Invalid FName length {} at index {} (block={}, offset={:#x}, header={:#x})",
                len, fname_index, block_index, block_offset, header_val
            );
        }

        let name = if is_wide {
            // UTF-16
            let bytes = source.read_bytes(entry_addr + 2, len * 2)?;
            let chars: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| LE::read_u16(c))
                .collect();
            String::from_utf16_lossy(&chars)
        } else {
            // ASCII/UTF-8
            let bytes = source.read_bytes(entry_addr + 2, len)?;
            String::from_utf8_lossy(&bytes).to_string()
        };

        self.cache.insert(fname_index, name.clone());
        Ok(name)
    }

    /// Debug: dump information about an FName index
    pub fn debug_read(&self, source: &dyn MemorySource, fname_index: u32) -> Result<()> {
        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        eprintln!("FName {} -> block={}, offset={:#x}", fname_index, block_index, block_offset);

        if block_index >= self.pool.blocks.len() {
            eprintln!("  Block out of range!");
            return Ok(());
        }

        let block_addr = self.pool.blocks[block_index];
        let entry_addr = block_addr + block_offset;
        eprintln!("  Block addr: {:#x}, Entry addr: {:#x}", block_addr, entry_addr);

        let data = source.read_bytes(entry_addr, 32)?;
        eprint!("  Data: ");
        for b in &data {
            eprint!("{:02x} ", b);
        }
        eprintln!();

        // Try to interpret as string
        eprint!("  ASCII: ");
        for b in &data {
            let c = *b as char;
            if c.is_ascii_graphic() || c == ' ' {
                eprint!("{}", c);
            } else {
                eprint!(".");
            }
        }
        eprintln!();

        Ok(())
    }

    /// Search for a string in the FNamePool and return its index
    /// This walks through entries to find the matching string
    pub fn search_name(&mut self, source: &dyn MemorySource, target: &str) -> Result<Option<u32>> {
        // Search through each block
        for (block_idx, &block_addr) in self.pool.blocks.iter().enumerate() {
            if block_addr == 0 {
                continue;
            }

            // Read a chunk of the block (FName blocks are typically 64KB)
            let block_size = 64 * 1024;
            let data = match source.read_bytes(block_addr, block_size) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mut offset = 0usize;
            while offset + 2 < data.len() {
                let header_val = LE::read_u16(&data[offset..offset + 2]);
                let is_wide = (header_val & 1) != 0;
                let len = (header_val >> 6) as usize;

                if len == 0 || len > 1024 || offset + 2 + len > data.len() {
                    break;
                }

                // Read the string
                let name = if is_wide {
                    let end = (offset + 2 + len * 2).min(data.len());
                    let chars: Vec<u16> = data[offset + 2..end]
                        .chunks_exact(2)
                        .map(|c| LE::read_u16(c))
                        .collect();
                    String::from_utf16_lossy(&chars)
                } else {
                    String::from_utf8_lossy(&data[offset + 2..offset + 2 + len]).to_string()
                };

                // Calculate FName index: (block_idx << 16) | (byte_offset / 2)
                let fname_index = ((block_idx as u32) << 16) | ((offset as u32) / 2);

                // Cache this entry
                self.cache.insert(fname_index, name.clone());

                if name == target {
                    return Ok(Some(fname_index));
                }

                // Move to next entry (header + string, aligned to 2 bytes)
                let entry_size = 2 + if is_wide { len * 2 } else { len };
                offset += (entry_size + 1) & !1; // Align to 2-byte boundary
            }
        }

        Ok(None)
    }

    /// Find "Class" FName index dynamically
    pub fn find_class_index(&mut self, source: &dyn MemorySource) -> Result<u32> {
        // First try the SDK constant
        if let Ok(name) = self.read_name(source, FNAME_CLASS_INDEX) {
            if name == "Class" {
                return Ok(FNAME_CLASS_INDEX);
            }
        }

        // Search for it
        if let Some(idx) = self.search_name(source, "Class")? {
            eprintln!("Found 'Class' FName at index {} (SDK said {})", idx, FNAME_CLASS_INDEX);
            return Ok(idx);
        }

        bail!("Could not find 'Class' FName in pool")
    }

    /// Find "Object" FName index dynamically
    pub fn find_object_index(&mut self, source: &dyn MemorySource) -> Result<u32> {
        // First try the SDK constant
        if let Ok(name) = self.read_name(source, FNAME_OBJECT_INDEX) {
            if name == "Object" {
                return Ok(FNAME_OBJECT_INDEX);
            }
        }

        // Search for it
        if let Some(idx) = self.search_name(source, "Object")? {
            eprintln!("Found 'Object' FName at index {} (SDK said {})", idx, FNAME_OBJECT_INDEX);
            return Ok(idx);
        }

        bail!("Could not find 'Object' FName in pool")
    }
}


/// Find all UClass instances by scanning for objects with ClassPrivate == UCLASS_METACLASS_ADDR
/// This is more reliable than walking GUObjectArray when the array location is uncertain
pub fn find_all_uclasses(
    source: &dyn MemorySource,
    fname_reader: &mut FNameReader,
) -> Result<Vec<UObjectInfo>> {
    let code_bounds = find_code_bounds(source)?;
    let mut results = Vec::new();
    let mut scanned_bytes = 0usize;

    eprintln!("Scanning for UClass instances (ClassPrivate == {:#x})...", UCLASS_METACLASS_ADDR);

    // Scan all readable regions in the executable's data space
    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Focus on PE + heap regions where UObjects live
        let in_pe = region.start >= 0x140000000 && region.start <= 0x175000000;
        let in_heap = region.start >= 0x1000000 && region.start < 0x140000000;
        if !in_pe && !in_heap {
            continue;
        }

        // Skip very large regions (heap can be huge)
        if region.size() > 100 * 1024 * 1024 {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        scanned_bytes += data.len();

        // Scan for 8-byte aligned pointers to the UClass metaclass
        for i in (0..data.len().saturating_sub(UOBJECT_HEADER_SIZE)).step_by(8) {
            // Check ClassPrivate at offset 0x18
            if i + UOBJECT_CLASS_OFFSET + 8 > data.len() {
                continue;
            }

            let class_ptr = LE::read_u64(&data[i + UOBJECT_CLASS_OFFSET..i + UOBJECT_CLASS_OFFSET + 8]) as usize;

            if class_ptr != UCLASS_METACLASS_ADDR {
                continue;
            }

            let obj_addr = region.start + i;

            // Validate vtable
            let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;
            if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
                continue;
            }

            // Verify vtable[0] points to code
            if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                let first_func = LE::read_u64(&vtable_data) as usize;
                if !code_bounds.contains(first_func) {
                    continue;
                }
            } else {
                continue;
            }

            // Read FName
            let name_index = LE::read_u32(&data[i + UOBJECT_NAME_OFFSET..i + UOBJECT_NAME_OFFSET + 4]);

            // Resolve name
            let name = match fname_reader.read_name(source, name_index) {
                Ok(n) => n,
                Err(_) => format!("FName_{}", name_index),
            };

            results.push(UObjectInfo {
                address: obj_addr,
                class_ptr,
                name_index,
                name,
                class_name: "Class".to_string(),
            });
        }
    }

    eprintln!("Scanned {} MB, found {} UClass instances",
             scanned_bytes / 1_000_000, results.len());

    // Sort by name for easier reading
    results.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(results)
}

/// UE5 UObject offsets
/// These vary by engine version but are consistent within a build
pub struct UObjectOffsets {
    /// Offset of ClassPrivate (UClass*) in UObject
    pub class_offset: usize,
    /// Offset of NamePrivate (FName) in UObject
    pub name_offset: usize,
    /// Offset of OuterPrivate (UObject*) in UObject
    pub outer_offset: usize,
}

impl Default for UObjectOffsets {
    fn default() -> Self {
        // Uses constants defined at top of file - see UOBJECT_* for documentation
        Self {
            class_offset: UOBJECT_CLASS_OFFSET,
            name_offset: UOBJECT_NAME_OFFSET,
            outer_offset: UOBJECT_OUTER_OFFSET,
        }
    }
}

/// Result of UClass metaclass discovery
#[derive(Debug, Clone)]
pub struct UClassMetaclassInfo {
    /// Address of the UClass metaclass
    pub address: usize,
    /// Vtable address
    pub vtable: usize,
    /// Offset where ClassPrivate was found
    pub class_offset: usize,
    /// Offset where NamePrivate was found
    pub name_offset: usize,
    /// FName index
    pub fname_index: u32,
    /// Resolved name
    pub name: String,
}

/// Find the UClass metaclass by exhaustively searching for self-referential objects
/// with FName "Class" (index 588) at various layout offsets.
///
/// This function tries different combinations of ClassPrivate and NamePrivate offsets
/// to handle UE5 version differences.
pub fn discover_uclass_metaclass_exhaustive(
    source: &dyn MemorySource,
    fname_reader: &mut FNameReader,
) -> Result<UClassMetaclassInfo> {
    let code_bounds = find_code_bounds(source)?;

    eprintln!("=== Exhaustive UClass Metaclass Discovery ===");

    // Find the actual FName index for "Class" dynamically
    let class_fname_idx = fname_reader.find_class_index(source)
        .unwrap_or(FNAME_CLASS_INDEX);
    eprintln!("Looking for self-referential object with FName 'Class' ({})...", class_fname_idx);

    // Possible offsets to try
    let class_offsets = [0x08, 0x10, 0x18, 0x20, 0x28];
    let name_offsets = [0x18, 0x20, 0x28, 0x30, 0x38, 0x40];

    // Build the 4-byte pattern for the Class FName index (little-endian)
    let class_fname_bytes = class_fname_idx.to_le_bytes();

    // Scan ALL readable memory for objects with FName "Class"
    eprintln!("Scanning all memory for objects with FName {} ('Class')...", class_fname_idx);

    let mut scanned_mb = 0usize;
    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Scan in chunks for large regions
        let chunk_size = 256 * 1024 * 1024; // 256MB chunks
        let mut offset = 0usize;

        while offset < region.size() {
            let read_size = (region.size() - offset).min(chunk_size);
            let chunk_start = region.start + offset;

            let data = match source.read_bytes(chunk_start, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            scanned_mb += data.len() / (1024 * 1024);
            if scanned_mb % 1000 == 0 && scanned_mb > 0 {
                eprint!("\r  Scanned {} MB...", scanned_mb);
            }

            // Boyer-Moore style: search for the FName index bytes first
            let mut pos = 0;
            while pos + 64 < data.len() {
                // Try each name_offset to find the FName pattern
                for &name_offset in &name_offsets {
                    if pos + name_offset + 4 > data.len() {
                        continue;
                    }

                    // Check if FName at this offset matches
                    if &data[pos + name_offset..pos + name_offset + 4] != class_fname_bytes {
                        continue;
                    }

                    // Found potential match - validate structure
                    for &class_offset in &class_offsets {
                        if class_offset == name_offset {
                            continue;
                        }

                        let max_offset = class_offset.max(name_offset) + 8;
                        if pos + max_offset > data.len() {
                            continue;
                        }

                        let obj_addr = chunk_start + pos;

                        // Check vtable
                        let vtable_ptr = LE::read_u64(&data[pos..pos + 8]) as usize;
                        if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
                            continue;
                        }

                        // Verify vtable[0] points to code
                        if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                            let first_func = LE::read_u64(&vtable_data) as usize;
                            if !code_bounds.contains(first_func) {
                                continue;
                            }
                        } else {
                            continue;
                        }

                        // Check if ClassPrivate == self (self-referential)
                        let class_ptr = LE::read_u64(&data[pos + class_offset..pos + class_offset + 8]) as usize;
                        if class_ptr == obj_addr {
                            eprintln!("\rFound UClass metaclass at {:#x}!", obj_addr);
                            eprintln!("  vtable: {:#x}", vtable_ptr);
                            eprintln!("  ClassPrivate offset: {:#x}", class_offset);
                            eprintln!("  NamePrivate offset: {:#x}", name_offset);

                            let fname_idx = LE::read_u32(&data[pos + name_offset..pos + name_offset + 4]);
                            let name = fname_reader.read_name(source, fname_idx)
                                .unwrap_or_else(|_| format!("FName_{}", fname_idx));

                            return Ok(UClassMetaclassInfo {
                                address: obj_addr,
                                vtable: vtable_ptr,
                                class_offset,
                                name_offset,
                                fname_index: fname_idx,
                                name,
                            });
                        }
                    }
                }
                pos += 8; // Align to 8-byte boundary
            }

            offset += chunk_size;
        }
    }
    eprintln!("\r  Scanned {} MB total", scanned_mb);

    // Second approach: find any self-referential object and check its FName
    eprintln!("\nNo self-referential object with FName {} found.", class_fname_idx);
    eprintln!("Searching all memory for any self-referential objects...");

    let mut self_refs: Vec<(usize, usize, usize, usize, u32, String)> = Vec::new();
    let mut scanned_mb2 = 0usize;

    'outer: for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Scan in chunks for large regions
        let chunk_size = 256 * 1024 * 1024;
        let mut offset = 0usize;

        while offset < region.size() && self_refs.len() < 50 {
            let read_size = (region.size() - offset).min(chunk_size);
            let chunk_start = region.start + offset;

            let data = match source.read_bytes(chunk_start, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            scanned_mb2 += data.len() / (1024 * 1024);
            if scanned_mb2 % 2000 == 0 && scanned_mb2 > 0 {
                eprint!("\r  Scanned {} MB for self-refs...", scanned_mb2);
            }

            for &class_offset in &class_offsets {
                for &name_offset in &name_offsets {
                    if class_offset == name_offset {
                        continue;
                    }

                    let max_offset = class_offset.max(name_offset) + 8;

                    for i in (0..data.len().saturating_sub(max_offset)).step_by(8) {
                        let obj_addr = chunk_start + i;

                        // Check ClassPrivate == self first (fast filter)
                        let class_ptr = LE::read_u64(&data[i + class_offset..i + class_offset + 8]) as usize;
                        if class_ptr != obj_addr {
                            continue;
                        }

                        // Validate vtable
                        let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;
                        if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
                            continue;
                        }

                        if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                            let first_func = LE::read_u64(&vtable_data) as usize;
                            if !code_bounds.contains(first_func) {
                                continue;
                            }
                        } else {
                            continue;
                        }

                        // Read FName
                        let fname_idx = LE::read_u32(&data[i + name_offset..i + name_offset + 4]);
                        let name = fname_reader.read_name(source, fname_idx)
                            .unwrap_or_else(|_| format!("FName_{}", fname_idx));

                        self_refs.push((obj_addr, vtable_ptr, class_offset, name_offset, fname_idx, name));

                        if self_refs.len() >= 50 {
                            break 'outer;
                        }
                    }
                }
            }

            offset += chunk_size;
        }
    }
    eprintln!("\r  Scanned {} MB for self-refs", scanned_mb2);

    eprintln!("Found {} self-referential objects with valid vtables:", self_refs.len());
    for (addr, vt, cls_off, name_off, idx, name) in &self_refs {
        let marker = if *idx == class_fname_idx || name == "Class" { " <-- METACLASS!" } else { "" };
        eprintln!("  {:#x}: vt={:#x}, cls@+{:#x}, name@+{:#x}, FName={}(\"{}\"){}",
                 addr, vt, cls_off, name_off, idx, name, marker);
    }

    // Check if any is "Class"
    if let Some((addr, vt, cls_off, name_off, idx, name)) = self_refs.iter()
        .find(|(_, _, _, _, idx, name)| *idx == class_fname_idx || name == "Class") {
        return Ok(UClassMetaclassInfo {
            address: *addr,
            vtable: *vt,
            class_offset: *cls_off,
            name_offset: *name_off,
            fname_index: *idx,
            name: name.clone(),
        });
    }

    bail!("UClass metaclass not found in dump. The dump may be incomplete or the FName format is different.")
}

/// Analyze a dump file to discover UObject layout and UClass metaclass
pub fn analyze_dump(source: &dyn MemorySource) -> Result<()> {
    eprintln!("=== BL4 Dump Analysis ===\n");

    // Step 1: Find code bounds
    eprintln!("Step 1: Finding code bounds from PE header...");
    let code_bounds = find_code_bounds(source)?;
    eprintln!("  Found {} code ranges", code_bounds.ranges.len());

    // Step 2: Discover FNamePool
    eprintln!("\nStep 2: Discovering FNamePool...");

    let pool = match FNamePool::discover(source) {
        Ok(p) => {
            eprintln!("  FNamePool at {:#x}", p.header_addr);
            eprintln!("  {} blocks, cursor at {}", p.current_block + 1, p.current_cursor);
            p
        }
        Err(e) => {
            eprintln!("  ERROR: Could not discover FNamePool: {}", e);
            bail!("FNamePool discovery failed - cannot continue analysis");
        }
    };

    let mut fname_reader = FNameReader::new(pool);

    // Verify FName resolution by finding "Class" and "Object" dynamically
    eprintln!("\nStep 3: Verifying FName resolution...");

    // Find "Class" FName dynamically
    let class_idx = match fname_reader.find_class_index(source) {
        Ok(idx) => {
            eprintln!("  FName 'Class' found at index {} (SDK constant was {})", idx, FNAME_CLASS_INDEX);
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Class' FName: {}", e);
            FNAME_CLASS_INDEX // Fall back to SDK constant
        }
    };

    // Find "Object" FName dynamically
    let object_idx = match fname_reader.find_object_index(source) {
        Ok(idx) => {
            eprintln!("  FName 'Object' found at index {} (SDK constant was {})", idx, FNAME_OBJECT_INDEX);
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Object' FName: {}", e);
            FNAME_OBJECT_INDEX // Fall back to SDK constant
        }
    };

    // Verify the indices work
    for (idx, expected) in [(class_idx, "Class"), (object_idx, "Object")] {
        match fname_reader.read_name(source, idx) {
            Ok(name) => {
                let status = if name == expected { "OK" } else { "MISMATCH" };
                eprintln!("  FName {} = \"{}\" (expected \"{}\") [{}]", idx, name, expected, status);
            }
            Err(e) => {
                eprintln!("  FName {} = ERROR: {}", idx, e);
            }
        }
    }

    // Step 4: Find UClass metaclass
    eprintln!("\nStep 4: Finding UClass metaclass...");
    match discover_uclass_metaclass_exhaustive(source, &mut fname_reader) {
        Ok(info) => {
            eprintln!("\n=== UClass Metaclass Found ===");
            eprintln!("  Address: {:#x}", info.address);
            eprintln!("  Vtable: {:#x}", info.vtable);
            eprintln!("  ClassPrivate offset: {:#x}", info.class_offset);
            eprintln!("  NamePrivate offset: {:#x}", info.name_offset);
            eprintln!("  FName: {} (\"{}\")", info.fname_index, info.name);

            // Update the constants for future use
            eprintln!("\nRecommended constant updates:");
            eprintln!("  pub const UCLASS_METACLASS_ADDR: usize = {:#x};", info.address);
            eprintln!("  pub const UCLASS_METACLASS_VTABLE: usize = {:#x};", info.vtable);
            eprintln!("  pub const UOBJECT_CLASS_OFFSET: usize = {:#x};", info.class_offset);
            eprintln!("  pub const UOBJECT_NAME_OFFSET: usize = {:#x};", info.name_offset);
        }
        Err(e) => {
            eprintln!("  Failed: {}", e);
        }
    }

    Ok(())
}

/// Walk the GUObjectArray and collect all UClass/UScriptStruct/UEnum objects
pub fn walk_guobject_array(
    source: &dyn MemorySource,
    guobj_array: &GUObjectArray,
    fname_reader: &mut FNameReader,
) -> Result<Vec<UObjectInfo>> {
    let offsets = UObjectOffsets::default();
    let mut results = Vec::new();

    // First, we need to find the UClass for "Class", "ScriptStruct", and "Enum"
    // to identify which objects are reflection types
    let mut class_class_ptr: Option<usize> = None;
    let mut scriptstruct_class_ptr: Option<usize> = None;
    let mut enum_class_ptr: Option<usize> = None;

    // FUObjectItem size - use detected size from discovery
    let item_size = guobj_array.item_size;
    const CHUNK_SIZE: usize = GUOBJECTARRAY_CHUNK_SIZE;

    let num_chunks = ((guobj_array.num_elements as usize) + CHUNK_SIZE - 1) / CHUNK_SIZE;

    eprintln!(
        "Walking GUObjectArray: {} elements in {} chunks",
        guobj_array.num_elements, num_chunks
    );

    // Read chunk pointer array
    let chunk_ptrs_data = source.read_bytes(guobj_array.objects_ptr, num_chunks * 8)?;
    let chunk_ptrs: Vec<usize> = chunk_ptrs_data
        .chunks_exact(8)
        .map(|c| LE::read_u64(c) as usize)
        .collect();

    // First pass: find the self-referential UClass for "Class"
    // Then find ScriptStruct and Enum UClasses
    eprintln!("First pass: finding UClass 'Class' (self-referential)...");

    // Collect candidate objects with names "Class", "ScriptStruct", "Enum"
    let mut class_candidate: Option<(usize, usize)> = None; // (obj_ptr, class_ptr)
    let mut scriptstruct_candidate: Option<(usize, usize)> = None;
    let mut enum_candidate: Option<(usize, usize)> = None;

    let mut scanned = 0;
    for (chunk_idx, &chunk_ptr) in chunk_ptrs.iter().enumerate() {
        if chunk_ptr == 0 {
            continue;
        }

        let items_in_chunk = if chunk_idx == num_chunks - 1 {
            (guobj_array.num_elements as usize) % CHUNK_SIZE
        } else {
            CHUNK_SIZE
        };

        // Read entire chunk at once for efficiency
        let chunk_data = match source.read_bytes(chunk_ptr, items_in_chunk * item_size) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for item_idx in 0..items_in_chunk {
            let item_offset = item_idx * item_size;
            let obj_ptr = LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;

            if obj_ptr == 0 {
                continue;
            }

            scanned += 1;

            // Read UObject header (need at least name_offset + 4 = 0x34 bytes)
            let obj_data = match source.read_bytes(obj_ptr, 0x40) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr = LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
            let name_index = LE::read_u32(&obj_data[offsets.name_offset..offsets.name_offset + 4]);

            // Debug: print first few FName indices we see
            if scanned <= 5 {
                eprintln!("  UObject[{}] at {:#x}: class={:#x}, name_idx={} ({:#x})",
                         scanned - 1, obj_ptr, class_ptr, name_index, name_index);
                let _ = fname_reader.debug_read(source, name_index);
            }

            // Read the name
            match fname_reader.read_name(source, name_index) {
                Ok(name) => {
                    // Self-referential class: UClass for "Class" has ClassPrivate pointing to itself
                    if name == "Class" && class_ptr == obj_ptr {
                        class_class_ptr = Some(obj_ptr);
                        class_candidate = Some((obj_ptr, class_ptr));
                        eprintln!("  Found UClass 'Class' at {:#x} (self-referential)", obj_ptr);
                    } else if name == "ScriptStruct" {
                        scriptstruct_candidate = Some((obj_ptr, class_ptr));
                    } else if name == "Enum" {
                        enum_candidate = Some((obj_ptr, class_ptr));
                    }

                    // Stop early if we found all three candidates
                    if class_candidate.is_some() && scriptstruct_candidate.is_some() && enum_candidate.is_some() {
                        break;
                    }
                }
                Err(e) => {
                    if scanned <= 5 {
                        eprintln!("    FName read error: {}", e);
                    }
                }
            }

            // Progress indicator
            if scanned % 50000 == 0 {
                eprint!("\r  Scanned {}/{} objects...", scanned, guobj_array.num_elements);
            }
        }

        // Stop early if we found all three candidates
        if class_candidate.is_some() && scriptstruct_candidate.is_some() && enum_candidate.is_some() {
            break;
        }
    }
    eprintln!("\r  First pass complete: scanned {} objects", scanned);

    // Validate candidates: ScriptStruct and Enum should have class_ptr == class_class_ptr
    if let Some(class_ptr) = class_class_ptr {
        if let Some((obj_ptr, cptr)) = scriptstruct_candidate {
            if cptr == class_ptr {
                scriptstruct_class_ptr = Some(obj_ptr);
                eprintln!("  Found UClass 'ScriptStruct' at {:#x}", obj_ptr);
            }
        }
        if let Some((obj_ptr, cptr)) = enum_candidate {
            if cptr == class_ptr {
                enum_class_ptr = Some(obj_ptr);
                eprintln!("  Found UClass 'Enum' at {:#x}", obj_ptr);
            }
        }
    }

    if class_class_ptr.is_none() {
        bail!("Could not find UClass 'Class' - FName reading may be broken");
    }

    eprintln!(
        "Core classes found:\n  Class={:#x}\n  ScriptStruct={:?}\n  Enum={:?}",
        class_class_ptr.unwrap(),
        scriptstruct_class_ptr,
        enum_class_ptr
    );

    // Second pass: collect all UClass, UScriptStruct, UEnum objects
    eprintln!("Second pass: collecting reflection objects...");

    scanned = 0;
    for (chunk_idx, &chunk_ptr) in chunk_ptrs.iter().enumerate() {
        if chunk_ptr == 0 {
            continue;
        }

        let items_in_chunk = if chunk_idx == num_chunks - 1 {
            (guobj_array.num_elements as usize) % CHUNK_SIZE
        } else {
            CHUNK_SIZE
        };

        let chunk_data = match source.read_bytes(chunk_ptr, items_in_chunk * item_size) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for item_idx in 0..items_in_chunk {
            let item_offset = item_idx * item_size;
            let obj_ptr = LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;

            if obj_ptr == 0 {
                continue;
            }

            scanned += 1;

            let obj_data = match source.read_bytes(obj_ptr, 0x38) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr = LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
            let name_index = LE::read_u32(&obj_data[offsets.name_offset..offsets.name_offset + 4]);

            // Check if this object is a UClass, UScriptStruct, or UEnum
            let class_name = if Some(class_ptr) == class_class_ptr {
                "Class"
            } else if Some(class_ptr) == scriptstruct_class_ptr {
                "ScriptStruct"
            } else if Some(class_ptr) == enum_class_ptr {
                "Enum"
            } else {
                continue; // Not a reflection type we care about
            };

            if let Ok(name) = fname_reader.read_name(source, name_index) {
                results.push(UObjectInfo {
                    address: obj_ptr,
                    class_ptr,
                    name_index,
                    name,
                    class_name: class_name.to_string(),
                });
            }

            if scanned % 100000 == 0 {
                eprint!("\r  Scanned {}/{} objects, found {} reflection types...",
                       scanned, guobj_array.num_elements, results.len());
            }
        }
    }

    eprintln!("\r  Second pass complete: {} reflection objects found", results.len());

    // Summary
    let class_count = results.iter().filter(|o| o.class_name == "Class").count();
    let struct_count = results.iter().filter(|o| o.class_name == "ScriptStruct").count();
    let enum_count = results.iter().filter(|o| o.class_name == "Enum").count();

    eprintln!(
        "Found {} UClass, {} UScriptStruct, {} UEnum",
        class_count, struct_count, enum_count
    );

    Ok(results)
}

/// Read property type name from FFieldClass
fn read_property_type(
    source: &dyn MemorySource,
    field_class_ptr: usize,
    fname_reader: &mut FNameReader,
    debug: bool,
) -> Result<String> {
    if field_class_ptr == 0 {
        return Ok("Unknown".to_string());
    }

    // Read FFieldClass data - in BL4's UE5.4, this is a vtable followed by class data
    // Read 0x180 bytes to find the FName (might be past offset 0x100)
    let class_data = source.read_bytes(field_class_ptr, 0x180)?;

    if debug {
        eprintln!("  FFieldClass at {:#x} (raw dump - 0x180 bytes):", field_class_ptr);
        // Dump all 0x180 bytes as hex for analysis
        for i in 0..24 {
            let offset = i * 16;
            if offset + 16 <= class_data.len() {
                eprint!("    +{:03x}: ", offset);
                for j in 0..16 {
                    eprint!("{:02x} ", class_data[offset + j]);
                }
                // Also show as ASCII
                eprint!(" | ");
                for j in 0..16 {
                    let b = class_data[offset + j];
                    if b >= 0x20 && b < 0x7f {
                        eprint!("{}", b as char);
                    } else {
                        eprint!(".");
                    }
                }
                eprintln!();
            }
        }
    }

    // Search for an FName-like value (small index that resolves to *Property)
    // Property type FNames are at low indices: IntProperty ~10, ObjectProperty ~32
    // Scan entire 0x180 byte region
    for offset in (0..0x180).step_by(4) {
        if offset + 4 <= class_data.len() {
            let name_index = LE::read_u32(&class_data[offset..offset + 4]);
            // Property type FNames should be small (< 500) and non-zero
            if name_index > 0 && name_index < 500 {
                if let Ok(name) = fname_reader.read_name(source, name_index) {
                    if name.ends_with("Property") {
                        if debug {
                            eprintln!("    Found Property type at +{:#x}: idx={}, name='{}'", offset, name_index, name);
                        }
                        return Ok(name);
                    }
                }
            }
        }
    }

    // FFieldClass in BL4's UE5.4 is purely a vtable with no embedded FName.
    // Return a placeholder that will be replaced with actual type during property extraction.
    Ok("_UNKNOWN_TYPE_".to_string())
}

// Debug counter for property extraction
static DEBUG_PROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Extract a single FProperty from memory
fn extract_property(
    source: &dyn MemorySource,
    prop_ptr: usize,
    fname_reader: &mut FNameReader,
) -> Result<PropertyInfo> {
    // Read FProperty data (need about 0x80 bytes for base + some type-specific)
    let prop_data = source.read_bytes(prop_ptr, 0x80)?;

    // FField base
    let field_class_ptr = LE::read_u64(&prop_data[FFIELD_CLASS_OFFSET..FFIELD_CLASS_OFFSET + 8]) as usize;
    let name_index = LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]);

    // FProperty fields
    let array_dim = LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]);
    let element_size = LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]);
    let property_flags = LE::read_u64(&prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8]);
    let offset = LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]);

    // Debug first few properties (disabled for production)
    let count = DEBUG_PROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let debug = count < 0; // Disabled

    if debug {
        eprintln!("\nDEBUG Property {} at {:#x}:", count, prop_ptr);
        eprintln!("  FProperty raw dump (0x80 bytes):");
        for i in 0..8 {
            let off = i * 16;
            eprint!("    +{:03x}: ", off);
            for j in 0..16 {
                eprint!("{:02x} ", prop_data[off + j]);
            }
            eprintln!();
        }

        // Scan for small values that could be FName indices (property type names are < 100)
        eprintln!("  Small values that could be FName indices:");
        for off in (0..0x80).step_by(4) {
            let val = LE::read_u32(&prop_data[off..off + 4]);
            if val > 0 && val < 100 {
                if let Ok(name) = fname_reader.read_name(source, val) {
                    eprintln!("    +{:#04x}: idx={} -> '{}'", off, val, name);
                }
            }
        }
    }

    // Get property name
    let name = fname_reader.read_name(source, name_index)?;

    // Extract type-specific information and infer property type
    let mut struct_type = None;
    let mut enum_type = None;
    let mut inner_type = None;
    let mut value_type = None;
    let mut inferred_type = EPropertyType::Unknown;
    let mut inferred_type_name = "Unknown".to_string();

    // Read type-specific data at offset 0x78+
    let ptr_at_78 = LE::read_u64(&prop_data[0x78..0x80]) as usize;

    // Helper: check if a pointer looks like a valid UObject (has vtable in code section)
    let is_valid_uobject = |addr: usize| -> bool {
        if addr < 0x7ff000000000 || addr > 0x7fff00000000 {
            return false; // Not in heap range for this dump
        }
        if let Ok(vtable_data) = source.read_bytes(addr, 8) {
            let vtable = LE::read_u64(&vtable_data) as usize;
            // Vtable should be in code section (0x140... - 0x15f...)
            vtable >= 0x140000000 && vtable < 0x160000000
        } else {
            false
        }
    };

    // Helper: check if pointer looks like a property (has FFieldClass in data section)
    let is_valid_property = |addr: usize| -> bool {
        if addr == 0 {
            return false;
        }
        if let Ok(ffc_data) = source.read_bytes(addr, 8) {
            let ffc = LE::read_u64(&ffc_data) as usize;
            // FFieldClass should be in .didata section (0x14e... - 0x151...)
            ffc >= 0x14e000000 && ffc < 0x152000000
        } else {
            false
        }
    };

    // Try to infer type by probing type-specific data
    if ptr_at_78 != 0 {
        // Check if it's an inner property (ArrayProperty, SetProperty, MapProperty)
        if is_valid_property(ptr_at_78) {
            // Check for MapProperty first (has second property at 0x80)
            let mut is_map = false;
            if let Ok(extra) = source.read_bytes(prop_ptr + 0x80, 8) {
                let ptr_at_80 = LE::read_u64(&extra) as usize;
                if is_valid_property(ptr_at_80) {
                    if let Ok(key) = extract_property(source, ptr_at_78, fname_reader) {
                        if let Ok(val) = extract_property(source, ptr_at_80, fname_reader) {
                            inferred_type = EPropertyType::MapProperty;
                            inferred_type_name = "MapProperty".to_string();
                            inner_type = Some(Box::new(key));
                            value_type = Some(Box::new(val));
                            is_map = true;
                        }
                    }
                }
            }
            if !is_map {
                // It's ArrayProperty or SetProperty
                if let Ok(inner) = extract_property(source, ptr_at_78, fname_reader) {
                    inferred_type = EPropertyType::ArrayProperty;
                    inferred_type_name = "ArrayProperty".to_string();
                    inner_type = Some(Box::new(inner));
                }
            }
        }
        // Check if it's a UStruct* (StructProperty)
        else if is_valid_uobject(ptr_at_78) {
            if let Ok(struct_data) = source.read_bytes(ptr_at_78 + UOBJECT_NAME_OFFSET, 4) {
                let struct_name_idx = LE::read_u32(&struct_data);
                if let Ok(sname) = fname_reader.read_name(source, struct_name_idx) {
                    // Could be StructProperty or ObjectProperty - distinguish by class
                    if let Ok(class_data) = source.read_bytes(ptr_at_78 + UOBJECT_CLASS_OFFSET, 8) {
                        let class_ptr = LE::read_u64(&class_data) as usize;
                        if let Ok(class_name_data) = source.read_bytes(class_ptr + UOBJECT_NAME_OFFSET, 4) {
                            let class_name_idx = LE::read_u32(&class_name_data);
                            if let Ok(class_name) = fname_reader.read_name(source, class_name_idx) {
                                if class_name == "ScriptStruct" {
                                    inferred_type = EPropertyType::StructProperty;
                                    inferred_type_name = "StructProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Class" {
                                    inferred_type = EPropertyType::ObjectProperty;
                                    inferred_type_name = "ObjectProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Enum" {
                                    inferred_type = EPropertyType::ByteProperty;
                                    inferred_type_name = "ByteProperty".to_string();
                                    enum_type = Some(sname);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // If still unknown, infer from element size
    if inferred_type == EPropertyType::Unknown {
        inferred_type_name = match element_size {
            1 => { inferred_type = EPropertyType::ByteProperty; "ByteProperty" }
            2 => { inferred_type = EPropertyType::Int16Property; "Int16Property" }
            4 => { inferred_type = EPropertyType::IntProperty; "IntProperty" }
            8 => { inferred_type = EPropertyType::Int64Property; "Int64Property" }
            12 => { inferred_type = EPropertyType::StructProperty; "StructProperty" } // Likely FVector
            16 => { inferred_type = EPropertyType::StrProperty; "StrProperty" } // FString is 16 bytes
            24 => { inferred_type = EPropertyType::StructProperty; "StructProperty" } // Likely FRotator or Transform
            _ => "Unknown"
        }.to_string();
    }

    if debug {
        eprintln!("  Resolved: name='{}', inferred_type='{}'", name, inferred_type_name);
    }

    Ok(PropertyInfo {
        name,
        property_type: inferred_type,
        type_name: inferred_type_name,
        array_dim,
        element_size,
        property_flags,
        offset,
        struct_type,
        enum_type,
        inner_type,
        value_type,
    })
}

/// Extract all properties from a UStruct/UClass
pub fn extract_struct_properties(
    source: &dyn MemorySource,
    struct_addr: usize,
    struct_name: &str,
    is_class: bool,
    fname_reader: &mut FNameReader,
) -> Result<StructInfo> {
    // Read UStruct header
    let header = source.read_bytes(struct_addr, 0x60)?;

    // Get super struct
    let super_ptr = LE::read_u64(&header[USTRUCT_SUPER_OFFSET..USTRUCT_SUPER_OFFSET + 8]) as usize;
    let super_name = if super_ptr != 0 {
        if let Ok(super_data) = source.read_bytes(super_ptr + UOBJECT_NAME_OFFSET, 4) {
            let super_name_idx = LE::read_u32(&super_data);
            fname_reader.read_name(source, super_name_idx).ok()
        } else {
            None
        }
    } else {
        None
    };

    // Get struct size
    let struct_size = LE::read_i32(&header[USTRUCT_SIZE_OFFSET..USTRUCT_SIZE_OFFSET + 4]);

    // Get ChildProperties pointer (linked list of FProperty)
    // Note: USTRUCT_CHILDREN_OFFSET (0x48) points to UField* (UFunctions)
    // USTRUCT_CHILDPROPERTIES_OFFSET (0x50) points to FField* (FProperties)
    let children_ptr = LE::read_u64(&header[USTRUCT_CHILDPROPERTIES_OFFSET..USTRUCT_CHILDPROPERTIES_OFFSET + 8]) as usize;

    // Walk property linked list
    let mut properties = Vec::new();
    let mut prop_ptr = children_ptr;
    let mut safety_counter = 0;
    const MAX_PROPERTIES: usize = 10000;

    while prop_ptr != 0 && safety_counter < MAX_PROPERTIES {
        safety_counter += 1;

        match extract_property(source, prop_ptr, fname_reader) {
            Ok(prop) => {
                properties.push(prop);
            }
            Err(e) => {
                // Log but continue - some properties may be unreadable
                if safety_counter <= 3 {
                    eprintln!("    Warning: Failed to read property at {:#x}: {}", prop_ptr, e);
                }
                break;
            }
        }

        // Get next property
        if let Ok(next_data) = source.read_bytes(prop_ptr + FFIELD_NEXT_OFFSET, 8) {
            prop_ptr = LE::read_u64(&next_data) as usize;
        } else {
            break;
        }
    }

    Ok(StructInfo {
        address: struct_addr,
        name: struct_name.to_string(),
        super_name,
        properties,
        struct_size,
        is_class,
    })
}

// Debug counter for enum extraction
static DEBUG_ENUM_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Extract enum values from a UEnum
pub fn extract_enum_values(
    source: &dyn MemorySource,
    enum_addr: usize,
    enum_name: &str,
    fname_reader: &mut FNameReader,
) -> Result<EnumInfo> {
    // UEnum layout varies by UE version. Try different offsets.
    // Common layouts:
    // UE5: Names at +0x60 or +0x68
    // Each entry: FName (4-8 bytes) + int64 (8 bytes)

    let debug = false; // Debug disabled - extraction working

    if debug {
        eprintln!("\nDEBUG Enum '{}' at {:#x}:", enum_name, enum_addr);
        use std::io::Write;
        let _ = std::io::stderr().flush();
        // Dump header
        if let Ok(header) = source.read_bytes(enum_addr, 0x80) {
            eprintln!("  UEnum header (0x80 bytes):");
            for i in 0..8 {
                let off = i * 16;
                eprint!("    +{:03x}: ", off);
                for j in 0..16 {
                    eprint!("{:02x} ", header[off + j]);
                }
                eprintln!();
            }
            let _ = std::io::stderr().flush();
        }
    }

    // Try multiple offsets for Names TArray
    // The TArray should have a heap pointer (0x7ff4... range) and reasonable count
    let offsets_to_try = [0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70];
    let mut values = Vec::new();

    for &names_offset in &offsets_to_try {
        let tarray_data = source.read_bytes(enum_addr + names_offset, 16)?;
        let data_ptr = LE::read_u64(&tarray_data[0..8]) as usize;
        let count = LE::read_i32(&tarray_data[8..12]) as usize;

        if debug {
            eprintln!("  Trying offset +{:#x}: data_ptr={:#x}, count={}", names_offset, data_ptr, count);
        }

        // Data pointer should be in heap range (0x7ff4... for this dump) or reasonable heap (> 0x1000000)
        // and count should be small (enum shouldn't have millions of values)
        let is_heap_ptr = (data_ptr >= 0x1000000 && data_ptr < 0x140000000) ||
                          (data_ptr >= 0x7ff000000000 && data_ptr < 0x800000000000);

        if data_ptr != 0 && is_heap_ptr && count > 0 && count < 1000 {
            // Read all pairs at once
            let pair_size = 16; // FName (8) + int64 (8)
            if let Ok(pairs_data) = source.read_bytes(data_ptr, count * pair_size) {
                if debug && !pairs_data.is_empty() {
                    eprintln!("  Raw pairs at {:#x}:", data_ptr);
                    for i in 0..std::cmp::min(3, count) {
                        let off = i * pair_size;
                        eprint!("    [{}] ", i);
                        for j in 0..16 {
                            eprint!("{:02x} ", pairs_data[off + j]);
                        }
                        let name_idx = LE::read_u32(&pairs_data[off..off+4]);
                        let name_extra = LE::read_u32(&pairs_data[off+4..off+8]);
                        let val = LE::read_i64(&pairs_data[off+8..off+16]);
                        eprintln!(" name_idx={}, extra={}, value={}", name_idx, name_extra, val);
                    }
                }
                for i in 0..count {
                    let offset = i * pair_size;
                    let name_index = LE::read_u32(&pairs_data[offset..offset + 4]);
                    let value = LE::read_i64(&pairs_data[offset + 8..offset + 16]);

                    if let Ok(name) = fname_reader.read_name(source, name_index) {
                        // Strip enum prefix (e.g., "EMyEnum::Value" -> "Value")
                        let short_name = if let Some(pos) = name.find("::") {
                            name[pos + 2..].to_string()
                        } else {
                            name
                        };
                        if debug && i < 3 {
                            eprintln!("    Resolved: '{}' = {}", short_name, value);
                        }
                        values.push((short_name, value));
                    }
                }
            }
            // If we found values, stop trying other offsets
            if !values.is_empty() {
                if debug {
                    eprintln!("  Found {} values at offset +{:#x}", values.len(), names_offset);
                }
                break;
            }
        }
    }

    Ok(EnumInfo {
        address: enum_addr,
        name: enum_name.to_string(),
        values,
    })
}

/// Infer property type from FFieldClass address using known mappings
/// This builds a cache by examining known structs and their property types
fn infer_property_type(
    ffc_ptr: usize,
    element_size: i32,
    property_flags: u64,
    type_cache: &std::collections::HashMap<usize, String>,
) -> String {
    // Check cache first
    if let Some(cached) = type_cache.get(&ffc_ptr) {
        return cached.clone();
    }

    // Fallback: infer from element size and flags
    // CPF_DisableEditOnInstance = 0x0400
    // CPF_ObjectPtr = bit somewhere
    let is_object_like = property_flags & 0x4000 != 0; // CPF_ReferenceParm or similar

    match element_size {
        1 => "ByteProperty".to_string(),
        2 => "Int16Property".to_string(),
        4 => "Int32Property".to_string(),  // Could also be FloatProperty
        8 => {
            if is_object_like {
                "ObjectProperty".to_string()
            } else {
                "Int64Property".to_string() // Could also be DoubleProperty
            }
        }
        12 => "StructProperty".to_string(), // Likely FVector (3 floats)
        16 => "StrProperty".to_string(), // FString or FVector4/FQuat
        _ => format!("UnknownProperty(size={})", element_size),
    }
}

/// Extract all reflection data (structs, classes, enums) from discovered UObjects
pub fn extract_reflection_data(
    source: &dyn MemorySource,
    objects: &[UObjectInfo],
    fname_reader: &mut FNameReader,
) -> Result<(Vec<StructInfo>, Vec<EnumInfo>)> {
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut ffc_pointers: std::collections::HashSet<usize> = std::collections::HashSet::new();

    let _total = objects.len();
    let classes: Vec<_> = objects.iter().filter(|o| o.class_name == "Class").collect();
    let script_structs: Vec<_> = objects.iter().filter(|o| o.class_name == "ScriptStruct").collect();
    let enum_objects: Vec<_> = objects.iter().filter(|o| o.class_name == "Enum").collect();

    eprintln!("Extracting properties from {} classes...", classes.len());
    for (i, obj) in classes.iter().enumerate() {
        if i % 500 == 0 {
            eprint!("\r  Processing class {}/{}...", i, classes.len());
        }
        match extract_struct_properties(source, obj.address, &obj.name, true, fname_reader) {
            Ok(info) => structs.push(info),
            Err(_) => {} // Skip errors silently
        }
    }
    eprintln!("\r  Processed {} classes", classes.len());

    eprintln!("Extracting properties from {} structs...", script_structs.len());
    for (i, obj) in script_structs.iter().enumerate() {
        if i % 500 == 0 {
            eprint!("\r  Processing struct {}/{}...", i, script_structs.len());
        }
        match extract_struct_properties(source, obj.address, &obj.name, false, fname_reader) {
            Ok(info) => structs.push(info),
            Err(_) => {} // Skip errors silently
        }
    }
    eprintln!("\r  Processed {} structs", script_structs.len());

    eprintln!("Extracting values from {} enums...", enum_objects.len());
    for (i, obj) in enum_objects.iter().enumerate() {
        if i % 500 == 0 {
            eprint!("\r  Processing enum {}/{}...", i, enum_objects.len());
        }
        match extract_enum_values(source, obj.address, &obj.name, fname_reader) {
            Ok(info) => enums.push(info),
            Err(_) => {} // Skip errors silently
        }
    }
    eprintln!("\r  Processed {} enums", enum_objects.len());

    // Collect unique FFieldClass pointers from property type names
    for st in &structs {
        for prop in &st.properties {
            if prop.type_name.starts_with("FFieldClass@") {
                if let Ok(addr) = usize::from_str_radix(&prop.type_name[12..], 16) {
                    ffc_pointers.insert(addr);
                }
            }
        }
    }

    // Summary
    let total_props: usize = structs.iter().map(|s| s.properties.len()).sum();
    let total_enum_vals: usize = enums.iter().map(|e| e.values.len()).sum();
    eprintln!(
        "Extracted {} structs/classes with {} properties, {} enums with {} values",
        structs.len(), total_props, enums.len(), total_enum_vals
    );
    eprintln!("Found {} unique FFieldClass pointers (property types)", ffc_pointers.len());

    // List unique FFieldClass pointers for debugging
    if ffc_pointers.len() < 50 {
        eprintln!("Unique FFieldClass pointers:");
        for ptr in &ffc_pointers {
            eprintln!("  {:#x}", ptr);
        }
    }

    Ok((structs, enums))
}

/// Usmap file format constants
pub mod usmap {
    /// Magic number for usmap files
    pub const MAGIC: u16 = 0x30C4;

    /// Usmap version enum
    #[repr(u8)]
    #[derive(Clone, Copy)]
    pub enum EUsmapVersion {
        Initial = 0,
        PackageVersioning = 1,
        LongFName = 2,
        LargeEnums = 3,
        // Add newer versions as needed
    }

    /// Compression method
    #[repr(u8)]
    pub enum EUsmapCompression {
        None = 0,
        Oodle = 1,
        Brotli = 2,
        ZStandard = 3,
    }
}

/// Write usmap file from extracted reflection data
pub fn write_usmap(
    path: &std::path::Path,
    structs: &[StructInfo],
    enums: &[EnumInfo],
) -> Result<()> {
    use std::collections::HashMap;
    use std::io::Write;

    eprintln!("Writing usmap to: {}", path.display());

    // Step 1: Build name table
    let mut names: Vec<String> = Vec::new();
    let mut name_to_index: HashMap<String, u32> = HashMap::new();

    let mut add_name = |name: &str| -> u32 {
        if let Some(&idx) = name_to_index.get(name) {
            return idx;
        }
        let idx = names.len() as u32;
        names.push(name.to_string());
        name_to_index.insert(name.to_string(), idx);
        idx
    };

    // Add empty string as index 0 (used for "no super")
    add_name("");

    // Collect all names from structs
    for st in structs {
        add_name(&st.name);
        if let Some(ref super_name) = st.super_name {
            add_name(super_name);
        }
        for prop in &st.properties {
            add_name(&prop.name);
            if let Some(ref struct_type) = prop.struct_type {
                add_name(struct_type);
            }
            if let Some(ref enum_type) = prop.enum_type {
                add_name(enum_type);
            }
            // Add inner/value types recursively
            fn collect_prop_names(prop: &PropertyInfo, names: &mut Vec<String>) {
                if let Some(ref struct_type) = prop.struct_type {
                    names.push(struct_type.clone());
                }
                if let Some(ref enum_type) = prop.enum_type {
                    names.push(enum_type.clone());
                }
                if let Some(ref inner) = prop.inner_type {
                    collect_prop_names(inner, names);
                }
                if let Some(ref value) = prop.value_type {
                    collect_prop_names(value, names);
                }
            }
            let mut nested_names = Vec::new();
            collect_prop_names(prop, &mut nested_names);
            for n in nested_names {
                add_name(&n);
            }
        }
    }

    // Collect all enum names
    for e in enums {
        add_name(&e.name);
        for (val_name, _) in &e.values {
            add_name(val_name);
        }
    }

    eprintln!("  Name table: {} unique names", names.len());

    // Step 2: Build payload buffer (uncompressed)
    let mut payload = Vec::new();

    // Write name table
    payload.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for name in &names {
        let bytes = name.as_bytes();
        // Use LongFName format: length as u16
        payload.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        payload.extend_from_slice(bytes);
    }

    // Write enums (before structs per format spec)
    // Using version 3 format: just name indices (not ExplicitEnumValues which is version 4)
    payload.extend_from_slice(&(enums.len() as u32).to_le_bytes());
    for e in enums {
        // Enum name index
        let name_idx = name_to_index.get(&e.name).copied().unwrap_or(0);
        payload.extend_from_slice(&name_idx.to_le_bytes());

        // Entry count (as u16 for LargeEnums version)
        payload.extend_from_slice(&(e.values.len() as u16).to_le_bytes());

        // Version 3: just name indices (values are sequential 0, 1, 2, ...)
        for (val_name, _val) in &e.values {
            let val_name_idx = name_to_index.get(val_name).copied().unwrap_or(0);
            payload.extend_from_slice(&val_name_idx.to_le_bytes());
        }
    }

    // Write structs
    payload.extend_from_slice(&(structs.len() as u32).to_le_bytes());
    for st in structs {
        // Struct name index
        let name_idx = name_to_index.get(&st.name).copied().unwrap_or(0);
        payload.extend_from_slice(&name_idx.to_le_bytes());

        // Super type name index (0 for none - empty string)
        let super_idx = st.super_name.as_ref()
            .and_then(|s| name_to_index.get(s).copied())
            .unwrap_or(0);
        payload.extend_from_slice(&super_idx.to_le_bytes());

        // Property count (sum of array_dim values - accounts for static arrays)
        let prop_count: u16 = st.properties.iter()
            .map(|p| p.array_dim as u16)
            .sum();
        payload.extend_from_slice(&prop_count.to_le_bytes());

        // Serializable property count (number of property entries)
        payload.extend_from_slice(&(st.properties.len() as u16).to_le_bytes());

        // Write properties
        for (i, prop) in st.properties.iter().enumerate() {
            write_property(&mut payload, prop, &name_to_index, i as u16)?;
        }
    }

    eprintln!("  Payload size: {} bytes (uncompressed)", payload.len());

    // Step 3: Write file header + payload
    // Using LongFName + LargeEnums version (version 3)
    let mut file = std::fs::File::create(path)?;

    // Magic (2 bytes)
    file.write_all(&usmap::MAGIC.to_le_bytes())?;

    // Version (1 byte) - LargeEnums = 3
    file.write_all(&[usmap::EUsmapVersion::LargeEnums as u8])?;

    // bHasVersionInfo (1 byte) - false for now
    // Required for version >= PackageVersioning (1)
    file.write_all(&[0u8])?;

    // Compression method (4 bytes as u32)
    file.write_all(&(usmap::EUsmapCompression::None as u32).to_le_bytes())?;

    // Compressed size (same as decompressed when uncompressed)
    file.write_all(&(payload.len() as u32).to_le_bytes())?;

    // Decompressed size
    file.write_all(&(payload.len() as u32).to_le_bytes())?;

    // Payload
    file.write_all(&payload)?;

    let header_size = 2 + 1 + 1 + 4 + 4 + 4;
    eprintln!("  Wrote {} bytes total", header_size + payload.len());

    Ok(())
}

/// Write a property type to the payload
fn write_property(
    payload: &mut Vec<u8>,
    prop: &PropertyInfo,
    name_to_index: &std::collections::HashMap<String, u32>,
    index: u16,
) -> Result<()> {
    // Index
    payload.extend_from_slice(&index.to_le_bytes());

    // Array dimension
    payload.push(prop.array_dim as u8);

    // Property name index
    let name_idx = name_to_index.get(&prop.name).copied().unwrap_or(0);
    payload.extend_from_slice(&name_idx.to_le_bytes());

    // Property type
    write_property_type(payload, prop, name_to_index)?;

    Ok(())
}

/// Write property type recursively
fn write_property_type(
    payload: &mut Vec<u8>,
    prop: &PropertyInfo,
    name_to_index: &std::collections::HashMap<String, u32>,
) -> Result<()> {
    // Type ID
    payload.push(prop.property_type.to_usmap_id());

    match prop.property_type {
        EPropertyType::EnumProperty => {
            // Inner type (usually ByteProperty)
            if let Some(ref inner) = prop.inner_type {
                write_property_type(payload, inner, name_to_index)?;
            } else {
                // Default to ByteProperty
                payload.push(EPropertyType::ByteProperty.to_usmap_id());
            }
            // Enum name
            let enum_idx = prop.enum_type.as_ref()
                .and_then(|s| name_to_index.get(s).copied())
                .unwrap_or(0);
            payload.extend_from_slice(&enum_idx.to_le_bytes());
        }
        EPropertyType::StructProperty => {
            // Struct type name
            let struct_idx = prop.struct_type.as_ref()
                .and_then(|s| name_to_index.get(s).copied())
                .unwrap_or(0);
            payload.extend_from_slice(&struct_idx.to_le_bytes());
        }
        EPropertyType::ArrayProperty | EPropertyType::SetProperty | EPropertyType::OptionalProperty => {
            // Inner type
            if let Some(ref inner) = prop.inner_type {
                write_property_type(payload, inner, name_to_index)?;
            } else {
                // Default to Unknown
                payload.push(EPropertyType::Unknown.to_usmap_id());
            }
        }
        EPropertyType::MapProperty => {
            // Key type
            if let Some(ref inner) = prop.inner_type {
                write_property_type(payload, inner, name_to_index)?;
            } else {
                payload.push(EPropertyType::Unknown.to_usmap_id());
            }
            // Value type
            if let Some(ref value) = prop.value_type {
                write_property_type(payload, value, name_to_index)?;
            } else {
                payload.push(EPropertyType::Unknown.to_usmap_id());
            }
        }
        // All other types have no additional data
        _ => {}
    }

    Ok(())
}

// ============================================================================
// Part Definition Extraction
// ============================================================================

/// GbxSerialNumberIndex structure layout
/// From usmap: Category (Int64), scope (Byte), status (Byte), Index (Int16)
#[derive(Debug, Clone)]
pub struct GbxSerialNumberIndex {
    pub category: i64,
    pub scope: u8,
    pub status: u8,
    pub index: i16,
}

/// Extracted part definition with its serial number index
#[derive(Debug, Clone)]
pub struct PartDefinition {
    pub name: String,
    pub category: i64,
    pub index: i16,
    pub object_address: usize,
}

/// Find objects by name pattern and return their class info
/// Used for discovering what class part definitions belong to
pub fn find_objects_by_pattern(
    source: &dyn MemorySource,
    guobjects: &GUObjectArray,
    name_pattern: &str,
    limit: usize,
) -> Result<Vec<(String, String, usize)>> {
    eprintln!("Searching for objects matching '{}'...", name_pattern);

    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    let mut results = Vec::new();

    for (idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
        if let Ok(name) = fname_reader.read_name(source, name_index) {
            if name.contains(name_pattern) {
                let class_ptr = LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
                let class_name = if class_ptr != 0 && class_ptr >= MIN_VALID_POINTER && class_ptr < MAX_VALID_POINTER {
                    if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                        let class_name_idx = LE::read_u32(&class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
                        fname_reader.read_name(source, class_name_idx).unwrap_or_else(|_| "Unknown".to_string())
                    } else {
                        "Unknown".to_string()
                    }
                } else {
                    "Unknown".to_string()
                };

                results.push((name, class_name, class_ptr));
                if results.len() >= limit {
                    break;
                }
            }
        }

        if idx % 100000 == 0 && idx > 0 {
            eprintln!("  Scanned {} objects, found {} matches...", idx, results.len());
        }
    }

    eprintln!("Found {} objects matching '{}'", results.len(), name_pattern);
    Ok(results)
}

/// Object map entry for fast lookups
#[derive(Debug, Clone)]
pub struct ObjectMapEntry {
    pub name: String,
    pub class_name: String,
    pub address: usize,
    pub class_address: usize,
}

/// Generate an object map for all objects in GUObjectArray
/// Returns a map of class_name -> list of (object_name, address)
pub fn generate_object_map(
    source: &dyn MemorySource,
    guobjects: &GUObjectArray,
) -> Result<std::collections::BTreeMap<String, Vec<ObjectMapEntry>>> {
    eprintln!("Generating object map...");

    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    let mut map: std::collections::BTreeMap<String, Vec<ObjectMapEntry>> = std::collections::BTreeMap::new();
    let mut total = 0;

    for (idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
        let name = match fname_reader.read_name(source, name_index) {
            Ok(n) => n,
            Err(_) => continue,
        };

        let class_ptr = LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        let class_name = if class_ptr != 0 && class_ptr >= MIN_VALID_POINTER && class_ptr < MAX_VALID_POINTER {
            if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                let class_name_idx = LE::read_u32(&class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
                fname_reader.read_name(source, class_name_idx).unwrap_or_else(|_| "Unknown".to_string())
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        map.entry(class_name.clone())
            .or_default()
            .push(ObjectMapEntry {
                name,
                class_name,
                address: obj_ptr,
                class_address: class_ptr,
            });

        total += 1;

        if idx % 100000 == 0 && idx > 0 {
            eprintln!("  Processed {} objects ({} valid)...", idx, total);
        }
    }

    eprintln!("Object map complete: {} objects across {} classes", total, map.len());
    Ok(map)
}

/// Find UClass by name in GUObjectArray
/// Returns the address of the UClass object
pub fn find_uclass_by_name(
    source: &dyn MemorySource,
    _gnames_addr: usize,
    guobjects: &GUObjectArray,
    class_name: &str,
) -> Result<usize> {
    eprintln!("Searching for UClass '{}'...", class_name);

    // Use FNameReader for proper multi-block FName resolution
    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    let mut found_count = 0;

    for (idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        // Read UObject header
        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // Get ClassPrivate - for a UClass, this points to the "Class" UClass
        let class_ptr = LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;

        // Get name index
        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);

        // Try to resolve name using FNameReader (supports all blocks)
        if let Ok(name) = fname_reader.read_name(source, name_index) {
            if name == class_name {
                // Verify this is actually a UClass by checking its Class is "Class" or "BlueprintGeneratedClass"
                if class_ptr != 0 && class_ptr >= MIN_VALID_POINTER && class_ptr < MAX_VALID_POINTER {
                    if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                        let class_name_idx = LE::read_u32(&class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
                        if let Ok(class_class_name) = fname_reader.read_name(source, class_name_idx) {
                            // Accept both native Class and Blueprint-generated classes
                            if class_class_name == "Class" || class_class_name == "BlueprintGeneratedClass" {
                                eprintln!("Found UClass '{}' at {:#x} (index {}, type={})",
                                         class_name, obj_ptr, idx, class_class_name);
                                return Ok(obj_ptr);
                            } else {
                                eprintln!("  Partial match: '{}' at {:#x} is a '{}', not a UClass",
                                         class_name, obj_ptr, class_class_name);
                            }
                        }
                    }
                }
                found_count += 1;
            }
        }

        // Progress indicator every 100k objects
        if idx % 100000 == 0 && idx > 0 {
            eprintln!("  Scanned {} objects...", idx);
        }
    }

    bail!("UClass '{}' not found (checked {} partial matches)", class_name, found_count)
}

/// Extract InventoryPartDef objects and their SerialIndex values
///
/// The SerialIndex is a GbxSerialNumberIndex struct embedded in the object.
/// We need to find its offset by examining the class properties or empirically.
pub fn extract_part_definitions(
    source: &dyn MemorySource,
    _gnames_addr: usize,
    guobjects: &GUObjectArray,
    inventory_part_def_class: usize,
) -> Result<Vec<PartDefinition>> {
    eprintln!("Extracting InventoryPartDef objects...");

    // Use FNameReader for proper multi-block FName resolution
    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    let mut parts = Vec::new();
    let mut scanned = 0;

    // For empirical offset discovery, we'll look for Category values that
    // match known patterns (small positive integers in the 2-500 range)
    // GbxSerialNumberIndex is typically at a fixed offset from the UObject base

    // Try common offsets for the SerialIndex property
    // UObject base is 0x28 bytes, then class-specific data follows
    // GbxSerialNumberAwareDef likely adds the SerialIndex early in its layout
    let candidate_offsets = [
        0x28,  // Right after UObject
        0x30,  // Common first property offset
        0x38,  //
        0x40,  //
        0x48,  // After some padding
        0x50,  //
        0x58,  //
        0x60,  //
        0x68,  //
        0x70,  //
        0x78,  //
        0x80,  //
        0x88,  //
        0x90,  //
        0x98,  //
        0xA0,  //
        0xA8,  //
        0xB0,  //
        0xB8,  //
        0xC0,  //
        0xC8,  //
        0xD0,  //
        0xD8,  //
        0xE0,  //
    ];

    // First pass: find the correct offset by looking for valid Category patterns
    let mut offset_scores: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut sample_count = 0;

    for (_idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        // Read UObject header
        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // Check if this object's class matches InventoryPartDef
        let class_ptr = LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        if class_ptr != inventory_part_def_class {
            continue;
        }

        sample_count += 1;

        // Read extended object data to check candidate offsets
        if let Ok(obj_data) = source.read_bytes(obj_ptr, 0x100) {
            for &offset in &candidate_offsets {
                if offset + 12 > obj_data.len() {
                    continue;
                }

                // GbxSerialNumberIndex layout:
                // - Category: i64 (8 bytes)
                // - scope: u8 (1 byte)
                // - status: u8 (1 byte)
                // - Index: i16 (2 bytes)
                let category = LE::read_i64(&obj_data[offset..offset + 8]);
                let index = LE::read_i16(&obj_data[offset + 10..offset + 12]);

                // Valid category values are typically small positive integers
                // Weapons: 2-29, Heavy: 244-247, Shields: 279-288, Gadgets: 300-330, Enhancements: 400-409
                let is_valid_category =
                    (category >= 2 && category <= 30) ||
                    (category >= 244 && category <= 250) ||
                    (category >= 279 && category <= 350) ||
                    (category >= 400 && category <= 420);

                // Valid index values are typically 0-300
                let is_valid_index = index >= 0 && index < 500;

                if is_valid_category && is_valid_index {
                    *offset_scores.entry(offset).or_insert(0) += 1;
                }
            }
        }

        if sample_count >= 100 {
            break; // Enough samples to determine offset
        }
    }

    // Find the best offset
    let best_offset = offset_scores.iter()
        .max_by_key(|&(_, score)| score)
        .map(|(&offset, _)| offset);

    let serial_index_offset = match best_offset {
        Some(offset) => {
            eprintln!("Detected SerialIndex offset: {:#x} (score: {})",
                     offset, offset_scores.get(&offset).unwrap_or(&0));
            offset
        }
        None => {
            eprintln!("Warning: Could not detect SerialIndex offset, trying 0x30");
            0x30
        }
    };

    eprintln!("Offset scores: {:?}", offset_scores);

    // Second pass: extract all parts with the detected offset
    for (idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        // Read UObject header
        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // Check if this object's class matches InventoryPartDef
        let class_ptr = LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        if class_ptr != inventory_part_def_class {
            continue;
        }

        scanned += 1;

        // Get object name using FNameReader (supports all blocks)
        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
        let name = match fname_reader.read_name(source, name_index) {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Read SerialIndex at the detected offset
        if let Ok(obj_data) = source.read_bytes(obj_ptr + serial_index_offset, 12) {
            let category = LE::read_i64(&obj_data[0..8]);
            let _scope = obj_data[8];
            let _status = obj_data[9];
            let index = LE::read_i16(&obj_data[10..12]);

            // Filter out invalid entries
            if category > 0 && category < 1000 && index >= 0 && index < 1000 {
                parts.push(PartDefinition {
                    name,
                    category,
                    index,
                    object_address: obj_ptr,
                });
            }
        }

        // Progress indicator
        if idx % 100000 == 0 && idx > 0 {
            eprintln!("  Scanned {} objects, found {} parts...", idx, parts.len());
        }
    }

    eprintln!("Extraction complete: scanned {} InventoryPartDef objects, extracted {} parts",
             scanned, parts.len());

    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_process() {
        // This will fail if BL4 isn't running, which is expected
        let result = find_bl4_process();
        println!("Find process result: {:?}", result);
    }
}
