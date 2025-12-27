//! Legacy memory module - remaining functionality to be further modularized.
//!
//! Contains GUObjectArray, FName reading, UClass discovery,
//! reflection data extraction, and part definitions.

#![allow(dead_code)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::single_match)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::needless_borrow)]
#![allow(unused_comparisons)]

use super::binary::find_code_bounds;
use super::constants::*;
use super::discovery::{discover_gnames, discover_guobject_array};
use super::fname::{FNamePool, FNameReader};
use super::pattern::scan_pattern_fast;
use super::reflection::{
    EPropertyType, EnumInfo, PropertyInfo, StructInfo, UClassMetaclassInfo, UObjectInfo,
    UObjectOffsets, discover_uclass_metaclass_exhaustive, find_all_uclasses,
};
use super::source::MemorySource;
use super::ue5::{GNamesPool, GUObjectArray, Ue5Offsets};
use super::walker::extract_property;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

// Note: Many functions have been extracted to separate modules:
// - discovery.rs: discover_class_uclass, discover_gnames, etc.
// - reflection.rs: EPropertyType, PropertyInfo, etc.
// - walker.rs: analyze_dump, walk_guobject_array
// - usmap.rs: extract_struct_properties, extract_enum_values, write_usmap

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
                let class_ptr =
                    LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
                let class_name = if class_ptr != 0
                    && class_ptr >= MIN_VALID_POINTER
                    && class_ptr < MAX_VALID_POINTER
                {
                    if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                        let class_name_idx = LE::read_u32(
                            &class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4],
                        );
                        fname_reader
                            .read_name(source, class_name_idx)
                            .unwrap_or_else(|_| "Unknown".to_string())
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
            eprintln!(
                "  Scanned {} objects, found {} matches...",
                idx,
                results.len()
            );
        }
    }

    eprintln!(
        "Found {} objects matching '{}'",
        results.len(),
        name_pattern
    );
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

    let mut map: std::collections::BTreeMap<String, Vec<ObjectMapEntry>> =
        std::collections::BTreeMap::new();
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

        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        let class_name =
            if class_ptr != 0 && class_ptr >= MIN_VALID_POINTER && class_ptr < MAX_VALID_POINTER {
                if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                    let class_name_idx =
                        LE::read_u32(&class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
                    fname_reader
                        .read_name(source, class_name_idx)
                        .unwrap_or_else(|_| "Unknown".to_string())
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

    eprintln!(
        "Object map complete: {} objects across {} classes",
        total,
        map.len()
    );
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
        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;

        // Get name index
        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);

        // Try to resolve name using FNameReader (supports all blocks)
        if let Ok(name) = fname_reader.read_name(source, name_index) {
            if name == class_name {
                // Verify this is actually a UClass by checking its Class is "Class" or "BlueprintGeneratedClass"
                if class_ptr != 0 && class_ptr >= MIN_VALID_POINTER && class_ptr < MAX_VALID_POINTER
                {
                    if let Ok(class_header) = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
                        let class_name_idx = LE::read_u32(
                            &class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4],
                        );
                        if let Ok(class_class_name) = fname_reader.read_name(source, class_name_idx)
                        {
                            // Accept both native Class and Blueprint-generated classes
                            if class_class_name == "Class"
                                || class_class_name == "BlueprintGeneratedClass"
                            {
                                eprintln!(
                                    "Found UClass '{}' at {:#x} (index {}, type={})",
                                    class_name, obj_ptr, idx, class_class_name
                                );
                                return Ok(obj_ptr);
                            } else {
                                eprintln!(
                                    "  Partial match: '{}' at {:#x} is a '{}', not a UClass",
                                    class_name, obj_ptr, class_class_name
                                );
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

    bail!(
        "UClass '{}' not found (checked {} partial matches)",
        class_name,
        found_count
    )
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
        0x28, // Right after UObject
        0x30, // Common first property offset
        0x38, //
        0x40, //
        0x48, // After some padding
        0x50, //
        0x58, //
        0x60, //
        0x68, //
        0x70, //
        0x78, //
        0x80, //
        0x88, //
        0x90, //
        0x98, //
        0xA0, //
        0xA8, //
        0xB0, //
        0xB8, //
        0xC0, //
        0xC8, //
        0xD0, //
        0xD8, //
        0xE0, //
    ];

    // First pass: find the correct offset by looking for valid Category patterns
    let mut offset_scores: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
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
        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
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
                let is_valid_category = (category >= 2 && category <= 30)
                    || (category >= 244 && category <= 250)
                    || (category >= 279 && category <= 350)
                    || (category >= 400 && category <= 420);

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
    let best_offset = offset_scores
        .iter()
        .max_by_key(|&(_, score)| score)
        .map(|(&offset, _)| offset);

    let serial_index_offset = match best_offset {
        Some(offset) => {
            eprintln!(
                "Detected SerialIndex offset: {:#x} (score: {})",
                offset,
                offset_scores.get(&offset).unwrap_or(&0)
            );
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
        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
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

    eprintln!(
        "Extraction complete: scanned {} InventoryPartDef objects, extracted {} parts",
        scanned,
        parts.len()
    );

    Ok(parts)
}

/// Map part name prefix to Part Group ID (category)
fn get_category_for_part(name: &str) -> Option<i64> {
    // Extract prefix (everything before ".part_")
    let prefix = name.split(".part_").next()?.to_lowercase();

    // Map prefixes to Part Group IDs (derived from reference data)
    match prefix.as_str() {
        // Pistols (2-6)
        "dad_ps" => Some(2),
        "jak_ps" => Some(3),
        "ted_ps" => Some(4),
        "tor_ps" => Some(5),
        "ord_ps" => Some(6),

        // Shotguns (8-12)
        "dad_sg" => Some(8),
        "jak_sg" => Some(9),
        "ted_sg" => Some(10),
        "tor_sg" => Some(11),
        "bor_sg" => Some(12),

        // Assault Rifles (13-18)
        "dad_ar" => Some(13),
        "jak_ar" => Some(14),
        "ted_ar" => Some(15),
        "tor_ar" => Some(16),
        "vla_ar" => Some(17),
        "ord_ar" => Some(18),

        // SMGs (19-24)
        "mal_sg" => Some(19), // Maliwan SG is actually an SMG category
        "dad_sm" => Some(20),
        "bor_sm" => Some(21),
        "vla_sm" => Some(22),
        "mal_sm" => Some(23),

        // Snipers (25-29)
        "bor_sr" => Some(25),
        "jak_sr" => Some(26),
        "ord_sr" => Some(28),
        "mal_sr" => Some(29),

        // Class mods
        "classmod_gravitar" | "classmod" => Some(97),

        // Heavy Weapons (244-247)
        "vla_hw" => Some(244),
        "tor_hw" => Some(245),
        "bor_hw" => Some(246),
        "mal_hw" => Some(247),

        // Shields (279-288)
        "energy_shield" => Some(279),
        "bor_shield" => Some(280),
        "dad_shield" => Some(281),
        "jak_shield" => Some(282),
        "armor_shield" => Some(283),
        "mal_shield" => Some(284),
        "ord_shield" => Some(285),
        "ted_shield" => Some(286),
        "tor_shield" => Some(287),

        // Gadgets (300-330)
        "grenade_gadget" | "mal_grenade_gadget" => Some(300),
        "turret_gadget" | "weapon_turret" => Some(310),
        "repair_kit" | "dad_repair_kit" => Some(320),
        "terminal_gadget" | "dad_terminal" | "mal_terminal" | "ord_terminal" | "ted_terminal" => {
            Some(330)
        }

        // Enhancements (400-409)
        "dad_enhancement" | "enhancement" => Some(400),
        "bor_enhancement" => Some(401),
        "jak_enhancement" => Some(402),
        "mal_enhancement" => Some(403),
        "ord_enhancement" => Some(404),
        "ted_enhancement" => Some(405),
        "tor_enhancement" => Some(406),
        "vla_enhancement" => Some(407),
        "cov_enhancement" => Some(408),
        "atl_enhancement" => Some(409),

        // Shield parts
        "shield" => Some(279),

        // Weapon parts for special weapons
        "weapon_brute" | "weapon_ripperturret" => Some(310),

        // Fallback: try to match partial prefixes
        other => {
            if other.ends_with("_ps") {
                Some(2)
            } else if other.ends_with("_sg") {
                Some(8)
            } else if other.ends_with("_ar") {
                Some(13)
            } else if other.ends_with("_sm") {
                Some(20)
            } else if other.ends_with("_sr") {
                Some(25)
            } else if other.ends_with("_hw") {
                Some(244)
            } else if other.contains("shield") {
                Some(279)
            } else if other.contains("gadget") {
                Some(300)
            } else if other.contains("enhancement") {
                Some(400)
            } else if other.contains("terminal") {
                Some(330)
            } else if other.contains("turret") {
                Some(310)
            } else if other.contains("repair") {
                Some(320)
            } else if other.contains("grenade") {
                Some(300)
            } else {
                None
            }
        }
    }
}

/// Extract part definitions using the discovered FName array pattern.
///
/// The game registers parts in internal arrays with a specific structure:
/// - Part Array Entry (24 bytes):
///   - FName Index (4 bytes) - References the part name in FNamePool
///   - Padding (4 bytes) - Always zero
///   - Pointer (8 bytes) - Address of the part's UObject
///   - Marker (4 bytes) - 0xFFFFFFFF sentinel value
///   - Priority (4 bytes) - Selection priority (not the serial index!)
///
/// - UObject at Pointer (offset +0x28):
///   - Scope (1 byte) - EGbxSerialNumberIndexScope (Root=1, Sub=2)
///   - (reserved 1 byte)
///   - Index (2 bytes, Int16) - THE SERIAL INDEX we need!
///
/// The Part Group ID (category) is derived from the part name prefix, as it's not
/// stored directly in the UObject structure at a fixed offset.
pub fn extract_parts_from_fname_arrays(source: &dyn MemorySource) -> Result<Vec<PartDefinition>> {
    eprintln!("Extracting parts via FName array pattern...");

    // Discover FNamePool for name resolution
    let pool = FNamePool::discover(source)?;

    // Step 1: Build a set of all part FName indices for quick lookup
    eprintln!("Step 1: Building FName lookup table from FNamePool...");
    let part_fnames = search_fname_pool_for_parts(source, &pool)?;
    eprintln!("Found {} FNames containing '.part_'", part_fnames.len());

    if part_fnames.is_empty() {
        bail!("No part FNames found in FNamePool");
    }

    // Create a HashMap for quick FName index -> name lookup
    let fname_to_name: std::collections::HashMap<u32, String> = part_fnames.into_iter().collect();

    // Step 2: Scan for 0xFFFFFFFF markers in targeted memory regions
    eprintln!("Step 2: Scanning for part array entries...");

    let marker_pattern = [0xFF, 0xFF, 0xFF, 0xFF];
    let mask = vec![1u8; 4];

    let mut parts = Vec::new();
    let mut seen_keys: std::collections::HashSet<(i64, i16, String)> =
        std::collections::HashSet::new();

    let mut processed_regions = 0;
    let mut total_markers = 0;

    // Process memory regions in chunks
    for region in source.regions() {
        // Skip very small or very large regions
        if region.size() < 1024 || region.size() > 500 * 1024 * 1024 {
            continue;
        }
        if !region.is_readable() {
            continue;
        }

        processed_regions += 1;
        if processed_regions % 100 == 0 {
            eprintln!(
                "  Processed {} regions, found {} parts so far...",
                processed_regions,
                parts.len()
            );
        }

        // Read the region
        let region_data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Scan for markers using SIMD-accelerated search
        let marker_offsets = scan_pattern_fast(&region_data, &marker_pattern, &mask);
        total_markers += marker_offsets.len();

        for &marker_offset in &marker_offsets {
            // Entry structure: FName(4) + padding(4) + pointer(8) + marker(4) + priority(4)
            if marker_offset < 16 {
                continue;
            }
            let entry_offset = marker_offset - 16;
            if entry_offset + 24 > region_data.len() {
                continue;
            }

            let entry_data = &region_data[entry_offset..entry_offset + 24];

            let fname_idx = LE::read_u32(&entry_data[0..4]);
            let padding = LE::read_u32(&entry_data[4..8]);
            let pointer = LE::read_u64(&entry_data[8..16]) as usize;
            let marker = LE::read_u32(&entry_data[16..20]);

            // Validate entry structure
            if marker != 0xFFFFFFFF {
                continue;
            }
            if padding != 0 {
                continue;
            }
            if pointer < MIN_VALID_POINTER || pointer > MAX_VALID_POINTER {
                continue;
            }

            // Check if this FName is a known part name
            let name = match fname_to_name.get(&fname_idx) {
                Some(n) => n.clone(),
                None => continue,
            };

            // Read index at UObject+0x2A (skipping scope bytes at +0x28-0x29)
            let serial_data = match source.read_bytes(pointer + 0x28, 4) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let index = LE::read_i16(&serial_data[2..4]);

            // Validate index
            if index < 0 || index > 1000 {
                continue;
            }

            // Derive category from part name prefix
            let category = match get_category_for_part(&name) {
                Some(c) => c,
                None => continue, // Unknown prefix, skip
            };

            // Skip duplicates
            let key = (category, index, name.clone());
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key);

            parts.push(PartDefinition {
                name,
                category,
                index,
                object_address: pointer,
            });
        }
    }

    eprintln!(
        "Extraction complete: processed {} regions, {} markers, extracted {} parts",
        processed_regions,
        total_markers,
        parts.len()
    );

    // Sort by category and index
    parts.sort_by_key(|p| (p.category, p.index));

    Ok(parts)
}

/// List all FNames containing ".part_" from the FNamePool (public wrapper for debugging)
pub fn list_all_part_fnames(source: &dyn MemorySource) -> Result<Vec<String>> {
    let pool = FNamePool::discover(source)?;
    let fnames = search_fname_pool_for_parts(source, &pool)?;
    Ok(fnames.into_iter().map(|(_, name)| name).collect())
}

/// Search FNamePool for all names containing ".part_"
fn search_fname_pool_for_parts(
    source: &dyn MemorySource,
    pool: &FNamePool,
) -> Result<Vec<(u32, String)>> {
    let search_pattern = b".part_";
    let mut results = Vec::new();

    for (block_idx, &block_addr) in pool.blocks.iter().enumerate() {
        if block_addr == 0 {
            continue;
        }

        // Read block data (64KB per block)
        let block_data = match source.read_bytes(block_addr, 64 * 1024) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Search for ".part_" pattern within the block
        for (pos, window) in block_data.windows(search_pattern.len()).enumerate() {
            if window == search_pattern {
                // Found potential match - try to find the entry start
                // Walk backwards to find the header (length byte)
                let mut entry_start = None;
                for back in 1..64 {
                    if pos < back + 2 {
                        break;
                    }
                    let header_pos = pos - back;
                    let header = &block_data[header_pos..header_pos + 2];
                    let header_val = LE::read_u16(header);
                    let len = (header_val >> 6) as usize;

                    // Check if this looks like a valid header
                    if len > 0 && len <= 1024 && header_pos + 2 + len <= block_data.len() {
                        // Verify the string contains our pattern
                        let name_bytes = &block_data[header_pos + 2..header_pos + 2 + len];
                        if let Ok(name_str) = std::str::from_utf8(name_bytes) {
                            if name_str.to_lowercase().contains(".part_") {
                                // Valid entry found
                                let byte_offset = header_pos;
                                // FName index = (block_idx << 16) | (byte_offset / 2)
                                let fname_index =
                                    ((block_idx as u32) << 16) | ((byte_offset / 2) as u32);
                                entry_start = Some((fname_index, name_str.to_string()));
                                break;
                            }
                        }
                    }
                }

                if let Some((fname_idx, name)) = entry_start {
                    // Avoid duplicates from overlapping pattern matches
                    if !results.iter().any(|(idx, _)| *idx == fname_idx) {
                        results.push((fname_idx, name));
                    }
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::super::source::find_bl4_process;

    #[test]
    fn test_find_process() {
        // This will fail if BL4 isn't running, which is expected
        let result = find_bl4_process();
        println!("Find process result: {:?}", result);
    }
}
