//! USMAP File Writer
//!
//! Functions for writing usmap files from reflection data.

use crate::memory::reflection::{EPropertyType, EnumInfo, PropertyInfo, StructInfo};

use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

/// Usmap file format constants
pub mod format {
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
    file.write_all(&format::MAGIC.to_le_bytes())?;

    // Version (1 byte) - LargeEnums = 3
    file.write_all(&[format::EUsmapVersion::LargeEnums as u8])?;

    // bHasVersionInfo (1 byte) - false for now
    // Required for version >= PackageVersioning (1)
    file.write_all(&[0u8])?;

    // Compression method (4 bytes as u32)
    file.write_all(&(format::EUsmapCompression::None as u32).to_le_bytes())?;

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
    name_to_index: &HashMap<String, u32>,
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
    name_to_index: &HashMap<String, u32>,
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
    use byteorder::{ByteOrder, LE};
    use tempfile::tempdir;

    #[test]
    fn test_usmap_magic_constant() {
        assert_eq!(format::MAGIC, 0x30C4);
    }

    #[test]
    fn test_usmap_version_enum() {
        assert_eq!(format::EUsmapVersion::Initial as u8, 0);
        assert_eq!(format::EUsmapVersion::PackageVersioning as u8, 1);
        assert_eq!(format::EUsmapVersion::LongFName as u8, 2);
        assert_eq!(format::EUsmapVersion::LargeEnums as u8, 3);
    }

    #[test]
    fn test_usmap_compression_enum() {
        assert_eq!(format::EUsmapCompression::None as u8, 0);
        assert_eq!(format::EUsmapCompression::Oodle as u8, 1);
        assert_eq!(format::EUsmapCompression::Brotli as u8, 2);
        assert_eq!(format::EUsmapCompression::ZStandard as u8, 3);
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
        assert_eq!(LE::read_u16(&data[0..2]), format::MAGIC);

        // Check version (LargeEnums = 3)
        assert_eq!(data[2], format::EUsmapVersion::LargeEnums as u8);
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
        assert_eq!(LE::read_u16(&data[0..2]), format::MAGIC);
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
