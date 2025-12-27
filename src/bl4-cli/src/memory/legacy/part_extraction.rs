//! Part Extraction Functions
//!
//! Functions for extracting part definitions from memory.

use super::part_defs::{get_category_for_part, PartDefinition};
use crate::memory::constants::*;
use crate::memory::fname::{FNamePool, FNameReader};
use crate::memory::pattern::scan_pattern_fast;
use crate::memory::source::MemorySource;
use crate::memory::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

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
