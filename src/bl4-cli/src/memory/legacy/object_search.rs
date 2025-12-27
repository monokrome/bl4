//! Object Search Functions
//!
//! Functions for searching UObjects in GUObjectArray by name or pattern.

use crate::memory::constants::*;
use crate::memory::fname::{FNamePool, FNameReader};
use crate::memory::source::MemorySource;
use crate::memory::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

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
