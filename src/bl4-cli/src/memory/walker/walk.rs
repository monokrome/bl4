//! GUObjectArray Walking
//!
//! Iterator over all UObjects with class information.

use crate::memory::constants::*;
use crate::memory::fname::FNameReader;
use crate::memory::reflection::{UObjectInfo, UObjectOffsets};
use crate::memory::source::MemorySource;
use crate::memory::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

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

            // Skip null entries
            if obj_ptr == 0 {
                continue;
            }

            // Validate object pointer is in reasonable range
            if obj_ptr < MIN_VALID_POINTER || obj_ptr >= MAX_VALID_POINTER {
                continue;
            }

            scanned += 1;

            // Read UObject header (need at least name_offset + 4 = 0x34 bytes)
            let obj_data = match source.read_bytes(obj_ptr, 0x40) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr =
                LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
            let name_index = LE::read_u32(&obj_data[offsets.name_offset..offsets.name_offset + 4]);

            // Skip entries with invalid class pointers (freed/garbage objects)
            if class_ptr != 0 && (class_ptr < MIN_VALID_POINTER || class_ptr >= MAX_VALID_POINTER) {
                continue;
            }

            // Skip entries with obviously invalid FName indices
            // FName index: upper 16 bits = block, lower 16 bits = offset
            // Block should be reasonable (< 1000 blocks is generous)
            let block = (name_index >> 16) as usize;
            if block > 1000 {
                continue;
            }

            // Debug: print first few valid objects with hex dump to find actual layout
            if scanned <= 5 {
                eprintln!(
                    "  UObject[{}] at {:#x}: class={:#x}, name_idx={} ({:#x})",
                    scanned - 1,
                    obj_ptr,
                    class_ptr,
                    name_index,
                    name_index
                );
                // Hex dump first 64 bytes to understand actual layout
                eprintln!("    Raw bytes:");
                for row in 0..4 {
                    let start = row * 16;
                    let hex: String = obj_data[start..start + 16]
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!("    +{:#04x}: {}", start, hex);
                }
                let _ = fname_reader.debug_read(source, name_index);
            }

            // Read the name
            match fname_reader.read_name(source, name_index) {
                Ok(name) => {
                    // Self-referential class: UClass for "Class" has ClassPrivate pointing to itself
                    if name == "Class" && class_ptr == obj_ptr {
                        class_class_ptr = Some(obj_ptr);
                        class_candidate = Some((obj_ptr, class_ptr));
                        eprintln!(
                            "  Found UClass 'Class' at {:#x} (self-referential)",
                            obj_ptr
                        );
                    } else if name == "ScriptStruct" {
                        scriptstruct_candidate = Some((obj_ptr, class_ptr));
                    } else if name == "Enum" {
                        enum_candidate = Some((obj_ptr, class_ptr));
                    }

                    // Stop early if we found all three candidates
                    if class_candidate.is_some()
                        && scriptstruct_candidate.is_some()
                        && enum_candidate.is_some()
                    {
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
                eprint!(
                    "\r  Scanned {}/{} objects...",
                    scanned, guobj_array.num_elements
                );
            }
        }

        // Stop early if we found all three candidates
        if class_candidate.is_some() && scriptstruct_candidate.is_some() && enum_candidate.is_some()
        {
            break;
        }
    }
    eprintln!("\r  First pass complete: scanned {} objects", scanned);

    // Debug: if we didn't find Class, try to find it dynamically
    if class_class_ptr.is_none() {
        eprintln!("  DEBUG: Did not find UClass 'Class' via name reading.");
        eprintln!("  Searching FNamePool for actual 'Class' index...");

        // Search FNamePool for "Class"
        if let Ok(Some(class_fname_idx)) = fname_reader.search_name(source, "Class") {
            eprintln!("  Found 'Class' at FName index {}", class_fname_idx);

            // Count how many objects have this name_idx
            let mut objects_with_class_name = 0;

            // Now scan for objects with this index that are self-referential
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
                    let obj_ptr =
                        LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;
                    if obj_ptr == 0
                        || obj_ptr < MIN_VALID_POINTER
                        || obj_ptr >= MAX_VALID_POINTER
                    {
                        continue;
                    }
                    if let Ok(obj_data) = source.read_bytes(obj_ptr, 0x40) {
                        let name_index = LE::read_u32(
                            &obj_data[offsets.name_offset..offsets.name_offset + 4],
                        );
                        if name_index == class_fname_idx {
                            objects_with_class_name += 1;
                            let class_ptr = LE::read_u64(
                                &obj_data[offsets.class_offset..offsets.class_offset + 8],
                            ) as usize;
                            if objects_with_class_name <= 5 {
                                eprintln!(
                                    "  Object with FName 'Class' at {:#x}, class={:#x}, self-ref={}",
                                    obj_ptr,
                                    class_ptr,
                                    class_ptr == obj_ptr
                                );
                            }
                            if class_ptr == obj_ptr {
                                class_class_ptr = Some(obj_ptr);
                                class_candidate = Some((obj_ptr, class_ptr));
                                eprintln!("  -> This is UClass 'Class'!");
                                break;
                            }
                        }
                    }
                }
                if class_class_ptr.is_some() {
                    break;
                }
            }
            eprintln!(
                "  Total objects with FName index {}: {}",
                class_fname_idx, objects_with_class_name
            );
        } else {
            eprintln!("  Could not find 'Class' in FNamePool!");
        }
    }

    // If still not found, the UObject layout might be different - try alternate offsets
    if class_class_ptr.is_none() {
        eprintln!("  Trying alternate UObject layouts...");
        // Try reading name from different offsets
        if let Ok(Some(class_fname_idx)) = fname_reader.search_name(source, "Class") {
            for alt_name_offset in [0x28usize, 0x20, 0x30, 0x08] {
                let mut found_at_alt = 0;
                for (chunk_idx, &chunk_ptr) in chunk_ptrs.iter().enumerate() {
                    if chunk_ptr == 0 {
                        continue;
                    }
                    let items_in_chunk = if chunk_idx == num_chunks - 1 {
                        (guobj_array.num_elements as usize) % CHUNK_SIZE
                    } else {
                        CHUNK_SIZE
                    };
                    let chunk_data = match source.read_bytes(chunk_ptr, items_in_chunk * item_size)
                    {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    for item_idx in 0..items_in_chunk {
                        let item_offset = item_idx * item_size;
                        let obj_ptr =
                            LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;
                        if obj_ptr == 0
                            || obj_ptr < MIN_VALID_POINTER
                            || obj_ptr >= MAX_VALID_POINTER
                        {
                            continue;
                        }
                        if let Ok(obj_data) = source.read_bytes(obj_ptr, 0x48) {
                            if alt_name_offset + 4 <= obj_data.len() {
                                let name_index = LE::read_u32(
                                    &obj_data[alt_name_offset..alt_name_offset + 4],
                                );
                                if name_index == class_fname_idx {
                                    found_at_alt += 1;
                                    // Check for self-referential at different class offsets
                                    for alt_class_offset in [0x10usize, 0x18, 0x20, 0x08] {
                                        if alt_class_offset + 8 <= obj_data.len() {
                                            let class_ptr = LE::read_u64(
                                                &obj_data
                                                    [alt_class_offset..alt_class_offset + 8],
                                            )
                                                as usize;
                                            if class_ptr == obj_ptr {
                                                eprintln!(
                                                    "  FOUND! UClass 'Class' at {:#x} (name@+{:#x}, class@+{:#x})",
                                                    obj_ptr, alt_name_offset, alt_class_offset
                                                );
                                                // Hex dump
                                                eprintln!("    Raw bytes:");
                                                for row in 0..4 {
                                                    let start = row * 16;
                                                    let hex: String = obj_data
                                                        [start..start + 16]
                                                        .iter()
                                                        .map(|b| format!("{:02x}", b))
                                                        .collect::<Vec<_>>()
                                                        .join(" ");
                                                    eprintln!("    +{:#04x}: {}", start, hex);
                                                }
                                                class_class_ptr = Some(obj_ptr);
                                                break;
                                            }
                                        }
                                    }
                                    if found_at_alt <= 3 && class_class_ptr.is_none() {
                                        eprintln!(
                                            "  Object {:#x} has 'Class' at +{:#x}, checking class ptrs...",
                                            obj_ptr, alt_name_offset
                                        );
                                        for off in [0x08usize, 0x10, 0x18, 0x20] {
                                            if off + 8 <= obj_data.len() {
                                                let ptr = LE::read_u64(&obj_data[off..off + 8])
                                                    as usize;
                                                eprintln!(
                                                    "    +{:#04x}: {:#x} (self-ref={})",
                                                    off,
                                                    ptr,
                                                    ptr == obj_ptr
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if class_class_ptr.is_some() {
                            break;
                        }
                    }
                    if class_class_ptr.is_some() {
                        break;
                    }
                }
                if found_at_alt > 0 {
                    eprintln!(
                        "  Offset +{:#x}: {} objects have 'Class' FName",
                        alt_name_offset, found_at_alt
                    );
                }
                if class_class_ptr.is_some() {
                    break;
                }
            }
        }
    }

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

            // Skip null or invalid entries
            if obj_ptr == 0 || obj_ptr < MIN_VALID_POINTER || obj_ptr >= MAX_VALID_POINTER {
                continue;
            }

            scanned += 1;

            let obj_data = match source.read_bytes(obj_ptr, 0x38) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr =
                LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
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
                eprint!(
                    "\r  Scanned {}/{} objects, found {} reflection types...",
                    scanned,
                    guobj_array.num_elements,
                    results.len()
                );
            }
        }
    }

    eprintln!(
        "\r  Second pass complete: {} reflection objects found",
        results.len()
    );

    // Summary
    let class_count = results.iter().filter(|o| o.class_name == "Class").count();
    let struct_count = results
        .iter()
        .filter(|o| o.class_name == "ScriptStruct")
        .count();
    let enum_count = results.iter().filter(|o| o.class_name == "Enum").count();

    eprintln!(
        "Found {} UClass, {} UScriptStruct, {} UEnum",
        class_count, struct_count, enum_count
    );

    Ok(results)
}
