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

            if obj_ptr == 0 {
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

            // Debug: print first few FName indices we see
            if scanned <= 5 {
                eprintln!(
                    "  UObject[{}] at {:#x}: class={:#x}, name_idx={} ({:#x})",
                    scanned - 1,
                    obj_ptr,
                    class_ptr,
                    name_index,
                    name_index
                );
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
