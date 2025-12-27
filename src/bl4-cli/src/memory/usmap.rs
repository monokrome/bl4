//! USMAP File Generation
//!
//! Functions for extracting UE5 reflection data and generating usmap files:
//! - extract_struct_properties - Extract properties from UStruct/UClass
//! - extract_enum_values - Extract enum values
//! - extract_reflection_data - Full reflection data extraction
//! - write_usmap - Write usmap file from reflection data

use super::constants::*;
use super::fname::FNameReader;
use super::reflection::{EPropertyType, EnumInfo, PropertyInfo, StructInfo, UObjectInfo};
use super::source::MemorySource;
use super::walker::extract_property;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};
use std::collections::HashMap;
use std::io::Write;

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
        4 => "Int32Property".to_string(), // Could also be FloatProperty
        8 => {
            if is_object_like {
                "ObjectProperty".to_string()
            } else {
                "Int64Property".to_string() // Could also be DoubleProperty
            }
        }
        12 => "StructProperty".to_string(), // Likely FVector (3 floats)
        16 => "StrProperty".to_string(),    // FString or FVector4/FQuat
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
        let super_idx = st
            .super_name
            .as_ref()
            .and_then(|s| name_to_index.get(s).copied())
            .unwrap_or(0);
        payload.extend_from_slice(&super_idx.to_le_bytes());

        // Property count (sum of array_dim values - accounts for static arrays)
        let prop_count: u16 = st.properties.iter().map(|p| p.array_dim as u16).sum();
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
            let enum_idx = prop
                .enum_type
                .as_ref()
                .and_then(|s| name_to_index.get(s).copied())
                .unwrap_or(0);
            payload.extend_from_slice(&enum_idx.to_le_bytes());
        }
        EPropertyType::StructProperty => {
            // Struct type name
            let struct_idx = prop
                .struct_type
                .as_ref()
                .and_then(|s| name_to_index.get(s).copied())
                .unwrap_or(0);
            payload.extend_from_slice(&struct_idx.to_le_bytes());
        }
        EPropertyType::ArrayProperty
        | EPropertyType::SetProperty
        | EPropertyType::OptionalProperty => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_usmap_magic_constant() {
        assert_eq!(usmap::MAGIC, 0x30C4);
    }

    #[test]
    fn test_usmap_version_enum() {
        assert_eq!(usmap::EUsmapVersion::Initial as u8, 0);
        assert_eq!(usmap::EUsmapVersion::PackageVersioning as u8, 1);
        assert_eq!(usmap::EUsmapVersion::LongFName as u8, 2);
        assert_eq!(usmap::EUsmapVersion::LargeEnums as u8, 3);
    }

    #[test]
    fn test_usmap_compression_enum() {
        assert_eq!(usmap::EUsmapCompression::None as u8, 0);
        assert_eq!(usmap::EUsmapCompression::Oodle as u8, 1);
        assert_eq!(usmap::EUsmapCompression::Brotli as u8, 2);
        assert_eq!(usmap::EUsmapCompression::ZStandard as u8, 3);
    }

    #[test]
    fn test_infer_property_type_from_size() {
        let cache = HashMap::new();

        // Size 1 -> ByteProperty
        assert_eq!(infer_property_type(0, 1, 0, &cache), "ByteProperty");

        // Size 2 -> Int16Property
        assert_eq!(infer_property_type(0, 2, 0, &cache), "Int16Property");

        // Size 4 -> Int32Property
        assert_eq!(infer_property_type(0, 4, 0, &cache), "Int32Property");

        // Size 8 -> Int64Property
        assert_eq!(infer_property_type(0, 8, 0, &cache), "Int64Property");

        // Size 12 -> StructProperty (likely FVector)
        assert_eq!(infer_property_type(0, 12, 0, &cache), "StructProperty");

        // Size 16 -> StrProperty
        assert_eq!(infer_property_type(0, 16, 0, &cache), "StrProperty");

        // Unknown size
        assert!(infer_property_type(0, 99, 0, &cache).starts_with("UnknownProperty"));
    }

    #[test]
    fn test_infer_property_type_with_cache() {
        let mut cache = HashMap::new();
        cache.insert(0x14f000100, "IntProperty".to_string());

        // Should use cached value
        assert_eq!(
            infer_property_type(0x14f000100, 4, 0, &cache),
            "IntProperty"
        );
    }

    #[test]
    fn test_write_usmap_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.usmap");

        let structs: Vec<StructInfo> = vec![];
        let enums: Vec<EnumInfo> = vec![];

        let result = write_usmap(&path, &structs, &enums);
        assert!(result.is_ok());

        // Verify file exists and has correct header
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() >= 16); // At least header size

        // Check magic
        assert_eq!(LE::read_u16(&data[0..2]), usmap::MAGIC);

        // Check version (LargeEnums = 3)
        assert_eq!(data[2], usmap::EUsmapVersion::LargeEnums as u8);
    }

    #[test]
    fn test_write_usmap_with_structs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_structs.usmap");

        let structs = vec![StructInfo {
            address: 0x200000000,
            name: "TestStruct".to_string(),
            super_name: None,
            properties: vec![PropertyInfo {
                name: "IntProp".to_string(),
                property_type: EPropertyType::IntProperty,
                type_name: "IntProperty".to_string(),
                array_dim: 1,
                element_size: 4,
                property_flags: 0,
                offset: 0,
                struct_type: None,
                enum_type: None,
                inner_type: None,
                value_type: None,
            }],
            struct_size: 4,
            is_class: false,
        }];
        let enums: Vec<EnumInfo> = vec![];

        let result = write_usmap(&path, &structs, &enums);
        assert!(result.is_ok());

        // Verify file exists
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 16);

        // Check magic
        assert_eq!(LE::read_u16(&data[0..2]), usmap::MAGIC);
    }

    #[test]
    fn test_write_usmap_with_enums() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_enums.usmap");

        let structs: Vec<StructInfo> = vec![];
        let enums = vec![EnumInfo {
            address: 0x200000000,
            name: "ETestEnum".to_string(),
            values: vec![
                ("Value1".to_string(), 0),
                ("Value2".to_string(), 1),
                ("MAX".to_string(), 255),
            ],
        }];

        let result = write_usmap(&path, &structs, &enums);
        assert!(result.is_ok());

        // Verify file exists
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 16);
    }

    #[test]
    fn test_write_property_basic_types() {
        let name_to_index: HashMap<String, u32> = [
            ("".to_string(), 0),
            ("IntProp".to_string(), 1),
            ("ByteProp".to_string(), 2),
        ]
        .into_iter()
        .collect();

        // Test IntProperty
        let mut payload = Vec::new();
        let int_prop = PropertyInfo {
            name: "IntProp".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let result = write_property(&mut payload, &int_prop, &name_to_index, 0);
        assert!(result.is_ok());
        assert!(!payload.is_empty());
    }

    #[test]
    fn test_write_property_type_struct() {
        let name_to_index: HashMap<String, u32> = [
            ("".to_string(), 0),
            ("VectorProp".to_string(), 1),
            ("Vector".to_string(), 2),
        ]
        .into_iter()
        .collect();

        let mut payload = Vec::new();
        let struct_prop = PropertyInfo {
            name: "VectorProp".to_string(),
            property_type: EPropertyType::StructProperty,
            type_name: "StructProperty".to_string(),
            array_dim: 1,
            element_size: 12,
            property_flags: 0,
            offset: 0,
            struct_type: Some("Vector".to_string()),
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let result = write_property_type(&mut payload, &struct_prop, &name_to_index);
        assert!(result.is_ok());

        // StructProperty should write type ID + struct name index
        assert!(payload.len() >= 5); // 1 byte type + 4 bytes name index
    }

    #[test]
    fn test_write_property_type_array() {
        let name_to_index: HashMap<String, u32> = [("".to_string(), 0), ("IntArray".to_string(), 1)]
            .into_iter()
            .collect();

        let inner = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let array_prop = PropertyInfo {
            name: "IntArray".to_string(),
            property_type: EPropertyType::ArrayProperty,
            type_name: "ArrayProperty".to_string(),
            array_dim: 1,
            element_size: 16,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(inner)),
            value_type: None,
        };

        let mut payload = Vec::new();
        let result = write_property_type(&mut payload, &array_prop, &name_to_index);
        assert!(result.is_ok());

        // ArrayProperty should write type ID + inner type ID
        assert!(payload.len() >= 2);
    }

    #[test]
    fn test_write_property_type_map() {
        let name_to_index: HashMap<String, u32> = [("".to_string(), 0), ("TestMap".to_string(), 1)]
            .into_iter()
            .collect();

        let key = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::StrProperty,
            type_name: "StrProperty".to_string(),
            array_dim: 1,
            element_size: 16,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let value = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let map_prop = PropertyInfo {
            name: "TestMap".to_string(),
            property_type: EPropertyType::MapProperty,
            type_name: "MapProperty".to_string(),
            array_dim: 1,
            element_size: 80,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(key)),
            value_type: Some(Box::new(value)),
        };

        let mut payload = Vec::new();
        let result = write_property_type(&mut payload, &map_prop, &name_to_index);
        assert!(result.is_ok());

        // MapProperty should write type ID + key type ID + value type ID
        assert!(payload.len() >= 3);
    }

    #[test]
    fn test_enum_info_values() {
        let enum_info = EnumInfo {
            address: 0x200000000,
            name: "ETestEnum".to_string(),
            values: vec![
                ("None".to_string(), 0),
                ("Value1".to_string(), 1),
                ("Value2".to_string(), 2),
                ("Max".to_string(), -1), // Often -1 for MAX values
            ],
        };

        assert_eq!(enum_info.values.len(), 4);
        assert_eq!(enum_info.values[0].0, "None");
        assert_eq!(enum_info.values[0].1, 0);
        assert_eq!(enum_info.values[3].1, -1);
    }

    #[test]
    fn test_struct_with_super_class() {
        let child = StructInfo {
            address: 0x200000000,
            name: "ChildClass".to_string(),
            super_name: Some("ParentClass".to_string()),
            properties: vec![],
            struct_size: 0x100,
            is_class: true,
        };

        assert_eq!(child.super_name.as_ref().unwrap(), "ParentClass");
        assert!(child.is_class);
    }

    #[test]
    fn test_property_with_enum_type() {
        let prop = PropertyInfo {
            name: "EnumProp".to_string(),
            property_type: EPropertyType::EnumProperty,
            type_name: "EnumProperty".to_string(),
            array_dim: 1,
            element_size: 1,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: Some("ETestEnum".to_string()),
            inner_type: Some(Box::new(PropertyInfo {
                name: "".to_string(),
                property_type: EPropertyType::ByteProperty,
                type_name: "ByteProperty".to_string(),
                array_dim: 1,
                element_size: 1,
                property_flags: 0,
                offset: 0,
                struct_type: None,
                enum_type: None,
                inner_type: None,
                value_type: None,
            })),
            value_type: None,
        };

        assert_eq!(prop.enum_type, Some("ETestEnum".to_string()));
        assert!(prop.inner_type.is_some());
    }
}
