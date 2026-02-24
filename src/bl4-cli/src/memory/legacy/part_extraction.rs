//! Part Extraction Functions
//!
//! Extracts part definitions from memory by walking the GUObjectArray.
//! Falls back to FName marker scanning if GUObjectArray walking fails.

use super::part_defs::{get_category_for_part, PartDefinition};
use crate::memory::constants::*;
use crate::memory::discovery::discover_guobject_array;
use crate::memory::fname::{FNamePool, FNameReader};
use crate::memory::pattern::scan_pattern_fast;
use crate::memory::source::MemorySource;
use crate::memory::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// Extract part definitions, trying GUObjectArray walking first then falling back to marker scan.
pub fn extract_parts_from_fname_arrays(source: &dyn MemorySource) -> Result<Vec<PartDefinition>> {
    match extract_parts_via_guobject_walk(source) {
        Ok(parts) if !parts.is_empty() => return Ok(parts),
        Ok(_) => eprintln!("GUObjectArray walk found 0 parts, falling back to marker scan..."),
        Err(e) => eprintln!(
            "GUObjectArray walk failed: {}, falling back to marker scan...",
            e
        ),
    }

    extract_parts_via_marker_scan(source)
}

/// Primary approach: walk GUObjectArray to find InventoryPartDef objects.
///
/// No pre-discovered metaclass address needed. Instead, we resolve each object's
/// class FName and check if it matches inventory part patterns.
fn extract_parts_via_guobject_walk(source: &dyn MemorySource) -> Result<Vec<PartDefinition>> {
    eprintln!("Extracting parts via GUObjectArray walking...");

    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    // Discover GUObjectArray (SDK-first, fast)
    let guobjects = match GUObjectArray::discover(source) {
        Ok(g) => g,
        Err(_) => discover_guobject_array(source, 0)?,
    };

    eprintln!("Walking {} objects...", guobjects.num_elements);

    // First pass: find all part objects by walking GUObjectArray
    let sample_objects = find_part_objects(source, &guobjects, &mut fname_reader)?;

    if sample_objects.is_empty() {
        bail!("No InventoryPartDef objects found in GUObjectArray");
    }

    // Probe for SerialIndex offset using sample objects
    let serial_index_offset = probe_serial_index_offset(source, &sample_objects)?;
    eprintln!("Detected SerialIndex offset: {:#x}", serial_index_offset);

    // Extract parts from the collected objects
    extract_parts_from_objects(source, &sample_objects, &mut fname_reader, serial_index_offset)
}

/// Walk GUObjectArray and collect all objects whose class is an inventory part type.
fn find_part_objects(
    source: &dyn MemorySource,
    guobjects: &GUObjectArray,
    fname_reader: &mut FNameReader,
) -> Result<Vec<(usize, usize)>> {
    let mut class_cache: std::collections::HashMap<usize, bool> =
        std::collections::HashMap::new();
    let mut class_names: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut sample_objects: Vec<(usize, usize)> = Vec::new();
    let mut scanned = 0usize;
    let mut fname_errors = 0usize;

    for (_idx, obj_ptr) in guobjects.iter_objects(source) {
        if !(MIN_VALID_POINTER..=MAX_VALID_POINTER).contains(&obj_ptr) {
            continue;
        }

        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        if !(MIN_VALID_POINTER..=MAX_VALID_POINTER).contains(&class_ptr) {
            continue;
        }

        let is_part = match class_cache.get(&class_ptr) {
            Some(&cached) => cached,
            None => {
                // Resolve class name and track it for diagnostics
                let class_header = source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE).ok();
                if let Some(ch) = &class_header {
                    let class_name_idx =
                        LE::read_u32(&ch[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
                    match fname_reader.read_name(source, class_name_idx) {
                        Ok(name) => {
                            *class_names.entry(name).or_insert(0) += 1;
                        }
                        Err(_) => {
                            fname_errors += 1;
                        }
                    }
                }

                let result = resolve_is_part_class(source, class_ptr, fname_reader);
                class_cache.insert(class_ptr, result);
                result
            }
        };

        if is_part {
            sample_objects.push((obj_ptr, class_ptr));
        }

        scanned += 1;
        if scanned % 100_000 == 0 {
            eprintln!(
                "  Scanned {} objects, found {} part instances...",
                scanned,
                sample_objects.len()
            );
        }
    }

    // Diagnostic: show top class names found
    if sample_objects.is_empty() && !class_names.is_empty() {
        let mut top_classes: Vec<_> = class_names.iter().collect();
        top_classes.sort_by(|a, b| b.1.cmp(a.1));
        eprintln!("  Top 20 class names found (of {} unique):", top_classes.len());
        for (name, count) in top_classes.iter().take(20) {
            eprintln!("    {} ({}x)", name, count);
        }
        if fname_errors > 0 {
            eprintln!("  FName resolution errors: {}", fname_errors);
        }

        // Check if any class name contains "part" or "inventory" (case-insensitive)
        let part_classes: Vec<_> = class_names
            .keys()
            .filter(|n| {
                let lower = n.to_lowercase();
                lower.contains("part") || lower.contains("inventory") || lower.contains("weapon")
            })
            .collect();
        if !part_classes.is_empty() {
            eprintln!("  Classes containing 'part/inventory/weapon':");
            for name in &part_classes {
                eprintln!("    {}", name);
            }
        }
    }

    eprintln!(
        "First pass: {} objects scanned, {} part instances",
        scanned,
        sample_objects.len()
    );

    Ok(sample_objects)
}

/// Resolve whether a class pointer represents an inventory part definition class.
fn resolve_is_part_class(
    source: &dyn MemorySource,
    class_ptr: usize,
    fname_reader: &mut FNameReader,
) -> bool {
    let class_header = match source.read_bytes(class_ptr, UOBJECT_HEADER_SIZE) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let class_name_idx =
        LE::read_u32(&class_header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
    let class_name = match fname_reader.read_name(source, class_name_idx) {
        Ok(n) => n,
        Err(_) => return false,
    };

    let is_part = is_inventory_part_class(&class_name);
    if is_part {
        eprintln!("  Found part class: '{}' at {:#x}", class_name, class_ptr);
    }
    is_part
}

/// Extract PartDefinitions from a list of (obj_ptr, class_ptr) pairs.
fn extract_parts_from_objects(
    source: &dyn MemorySource,
    objects: &[(usize, usize)],
    fname_reader: &mut FNameReader,
    serial_index_offset: usize,
) -> Result<Vec<PartDefinition>> {
    let mut parts = Vec::new();
    let mut seen: std::collections::HashSet<(i64, i16)> = std::collections::HashSet::new();

    for &(obj_ptr, _) in objects {
        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
        let name = match fname_reader.read_name(source, name_index) {
            Ok(n) => n,
            Err(_) => continue,
        };

        let serial_data = match source.read_bytes(obj_ptr + serial_index_offset, 12) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let category = LE::read_i64(&serial_data[0..8]);
        let index = LE::read_i16(&serial_data[10..12]);

        if category > 0 && category < 1000 && index >= 0 && index < 1000 {
            let key = (category, index);
            if !seen.insert(key) {
                continue;
            }

            parts.push(PartDefinition {
                name,
                category,
                index,
                object_address: obj_ptr,
            });
        }
    }

    parts.sort_by_key(|p| (p.category, p.index));

    eprintln!(
        "GUObjectArray extraction: {} unique parts from {} instances",
        parts.len(),
        objects.len()
    );

    Ok(parts)
}

/// Check if a UClass name represents an inventory part definition
fn is_inventory_part_class(class_name: &str) -> bool {
    let lower = class_name.to_lowercase();
    lower.contains("inventorypartdef")
        || lower.contains("inventorypart")
        || lower == "gbxinventorypartdef"
        || (lower.contains("part") && lower.contains("def"))
}

/// Probe candidate offsets on sample objects to find GbxSerialNumberIndex
fn probe_serial_index_offset(
    source: &dyn MemorySource,
    sample_objects: &[(usize, usize)],
) -> Result<usize> {
    let candidate_offsets: &[usize] = &[
        0x28, 0x30, 0x38, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80, 0x88, 0x90,
        0x98, 0xA0, 0xA8, 0xB0, 0xB8, 0xC0, 0xC8, 0xD0, 0xD8, 0xE0,
    ];

    let samples = sample_objects.len().min(100);
    let mut offset_scores: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    for &(obj_ptr, _) in sample_objects.iter().take(samples) {
        let obj_data = match source.read_bytes(obj_ptr, 0x100) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for &offset in candidate_offsets {
            if offset + 12 > obj_data.len() {
                continue;
            }

            let category = LE::read_i64(&obj_data[offset..offset + 8]);
            let index = LE::read_i16(&obj_data[offset + 10..offset + 12]);

            let is_valid_category = (2..=30).contains(&category)
                || (244..=250).contains(&category)
                || (279..=350).contains(&category)
                || (400..=420).contains(&category);
            let is_valid_index = (0..500).contains(&index);

            if is_valid_category && is_valid_index {
                *offset_scores.entry(offset).or_insert(0) += 1;
            }
        }
    }

    let best = offset_scores
        .iter()
        .max_by_key(|&(_, score)| score)
        .map(|(&offset, &score)| (offset, score));

    match best {
        Some((offset, score)) => {
            eprintln!(
                "  Best offset {:#x} with score {}/{}",
                offset, score, samples
            );
            Ok(offset)
        }
        None => {
            eprintln!("  Warning: could not detect SerialIndex offset, defaulting to 0x30");
            Ok(0x30)
        }
    }
}

/// Extract InventoryPartDef objects using a pre-discovered class pointer.
#[allow(clippy::cognitive_complexity)]
pub fn extract_part_definitions(
    source: &dyn MemorySource,
    _gnames_addr: usize,
    guobjects: &GUObjectArray,
    inventory_part_def_class: usize,
) -> Result<Vec<PartDefinition>> {
    eprintln!("Extracting InventoryPartDef objects...");

    let pool = FNamePool::discover(source)?;
    let mut fname_reader = FNameReader::new(pool);

    let mut parts = Vec::new();
    let mut scanned = 0;
    let mut sample_objects: Vec<(usize, usize)> = Vec::new();

    // Collect sample objects for offset probing
    for (_idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        if class_ptr != inventory_part_def_class {
            continue;
        }

        sample_objects.push((obj_ptr, class_ptr));
        if sample_objects.len() >= 100 {
            break;
        }
    }

    let serial_index_offset = probe_serial_index_offset(source, &sample_objects)?;
    eprintln!(
        "Detected SerialIndex offset: {:#x}",
        serial_index_offset
    );

    // Full extraction pass
    for (idx, obj_ptr) in guobjects.iter_objects(source) {
        if obj_ptr < MIN_VALID_POINTER || obj_ptr > MAX_VALID_POINTER {
            continue;
        }

        let header = match source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let class_ptr =
            LE::read_u64(&header[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]) as usize;
        if class_ptr != inventory_part_def_class {
            continue;
        }

        scanned += 1;

        let name_index = LE::read_u32(&header[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]);
        let name = match fname_reader.read_name(source, name_index) {
            Ok(n) => n,
            Err(_) => continue,
        };

        if let Ok(obj_data) = source.read_bytes(obj_ptr + serial_index_offset, 12) {
            let category = LE::read_i64(&obj_data[0..8]);
            let index = LE::read_i16(&obj_data[10..12]);

            if category > 0 && category < 1000 && index >= 0 && index < 1000 {
                parts.push(PartDefinition {
                    name,
                    category,
                    index,
                    object_address: obj_ptr,
                });
            }
        }

        if idx % 100_000 == 0 && idx > 0 {
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

/// Fallback: extract parts by scanning for 0xFFFFFFFF markers in memory.
#[allow(clippy::cognitive_complexity)]
fn extract_parts_via_marker_scan(source: &dyn MemorySource) -> Result<Vec<PartDefinition>> {
    eprintln!("Extracting parts via FName array pattern (marker scan)...");

    let pool = FNamePool::discover(source)?;
    let part_fnames = search_fname_pool_for_parts(source, &pool)?;
    eprintln!("Found {} FNames containing '.part_'", part_fnames.len());

    if part_fnames.is_empty() {
        bail!("No part FNames found in FNamePool");
    }

    let fname_to_name: std::collections::HashMap<u32, String> = part_fnames.into_iter().collect();

    let marker_pattern = [0xFF, 0xFF, 0xFF, 0xFF];
    let mask = vec![1u8; 4];

    let mut parts = Vec::new();
    let mut seen_keys: std::collections::HashSet<(i64, i16, String)> =
        std::collections::HashSet::new();
    let mut processed_regions = 0;

    for region in source.regions() {
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

        let region_data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let marker_offsets = scan_pattern_fast(&region_data, &marker_pattern, &mask);

        for &marker_offset in &marker_offsets {
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

            if marker != 0xFFFFFFFF || padding != 0 {
                continue;
            }
            if !(MIN_VALID_POINTER..=MAX_VALID_POINTER).contains(&pointer) {
                continue;
            }

            let name = match fname_to_name.get(&fname_idx) {
                Some(n) => n.clone(),
                None => continue,
            };

            let serial_data = match source.read_bytes(pointer + 0x28, 4) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let index = LE::read_i16(&serial_data[2..4]);
            if !(0..=1000).contains(&index) {
                continue;
            }

            let category = match get_category_for_part(&name) {
                Some(c) => c,
                None => continue,
            };

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
        "Marker scan: {} regions processed, {} parts extracted",
        processed_regions,
        parts.len()
    );

    parts.sort_by_key(|p| (p.category, p.index));
    Ok(parts)
}

/// List all FNames containing ".part_" from the FNamePool
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

        let block_data = match source.read_bytes(block_addr, 64 * 1024) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for (pos, window) in block_data.windows(search_pattern.len()).enumerate() {
            if window != search_pattern {
                continue;
            }

            let mut entry_start = None;
            for back in 1..64 {
                if pos < back + 2 {
                    break;
                }
                let header_pos = pos - back;
                let header = &block_data[header_pos..header_pos + 2];
                let header_val = LE::read_u16(header);
                let len = (header_val >> 6) as usize;

                if len > 0 && len <= 1024 && header_pos + 2 + len <= block_data.len() {
                    let name_bytes = &block_data[header_pos + 2..header_pos + 2 + len];
                    if let Ok(name_str) = std::str::from_utf8(name_bytes) {
                        if name_str.to_lowercase().contains(".part_") {
                            let byte_offset = header_pos;
                            let fname_index =
                                ((block_idx as u32) << 16) | ((byte_offset / 2) as u32);
                            entry_start = Some((fname_index, name_str.to_string()));
                            break;
                        }
                    }
                }
            }

            if let Some((fname_idx, name)) = entry_start {
                if !results.iter().any(|(idx, _)| *idx == fname_idx) {
                    results.push((fname_idx, name));
                }
            }
        }
    }

    Ok(results)
}
