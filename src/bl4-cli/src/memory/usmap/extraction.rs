//! Reflection Data Extraction
//!
//! Functions for extracting UE5 reflection data from memory:
//! - extract_struct_properties - Extract properties from UStruct/UClass
//! - extract_enum_values - Extract enum values
//! - extract_reflection_data - Full reflection data extraction

use crate::memory::constants::*;
use crate::memory::fname::FNameReader;
use crate::memory::reflection::{EnumInfo, PropertyInfo, StructInfo, UObjectInfo};
use crate::memory::source::MemorySource;
use crate::memory::walker::extract_property;

use anyhow::Result;
use byteorder::{ByteOrder, LE};

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
    let children_ptr =
        LE::read_u64(&header[USTRUCT_CHILDPROPERTIES_OFFSET..USTRUCT_CHILDPROPERTIES_OFFSET + 8])
            as usize;

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
                    eprintln!(
                        "    Warning: Failed to read property at {:#x}: {}",
                        prop_ptr, e
                    );
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
            eprintln!(
                "  Trying offset +{:#x}: data_ptr={:#x}, count={}",
                names_offset, data_ptr, count
            );
        }

        // Data pointer should be in heap range (0x7ff4... for this dump) or reasonable heap (> 0x1000000)
        // and count should be small (enum shouldn't have millions of values)
        let is_heap_ptr = (data_ptr >= 0x1000000 && data_ptr < 0x140000000)
            || (data_ptr >= 0x7ff000000000 && data_ptr < 0x800000000000);

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
                        let name_idx = LE::read_u32(&pairs_data[off..off + 4]);
                        let name_extra = LE::read_u32(&pairs_data[off + 4..off + 8]);
                        let val = LE::read_i64(&pairs_data[off + 8..off + 16]);
                        eprintln!(
                            " name_idx={}, extra={}, value={}",
                            name_idx, name_extra, val
                        );
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
                    eprintln!(
                        "  Found {} values at offset +{:#x}",
                        values.len(),
                        names_offset
                    );
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
    let script_structs: Vec<_> = objects
        .iter()
        .filter(|o| o.class_name == "ScriptStruct")
        .collect();
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

    eprintln!(
        "Extracting properties from {} structs...",
        script_structs.len()
    );
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
        structs.len(),
        total_props,
        enums.len(),
        total_enum_vals
    );
    eprintln!(
        "Found {} unique FFieldClass pointers (property types)",
        ffc_pointers.len()
    );

    // List unique FFieldClass pointers for debugging
    if ffc_pointers.len() < 50 {
        eprintln!("Unique FFieldClass pointers:");
        for ptr in &ffc_pointers {
            eprintln!("  {:#x}", ptr);
        }
    }

    Ok((structs, enums))
}
