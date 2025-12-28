//! USMAP File Writer
//!
//! Functions for writing usmap files from reflection data.

use crate::memory::reflection::{EnumInfo, StructInfo};

use anyhow::Result;
use std::io::Write;

use super::format::{EUsmapCompression, EUsmapVersion, MAGIC};
use super::name_table::{build_name_table, serialize_name_table};
use super::serializer::{serialize_enums, serialize_structs};

/// Write usmap file from extracted reflection data
pub fn write_usmap(
    path: &std::path::Path,
    structs: &[StructInfo],
    enums: &[EnumInfo],
) -> Result<()> {
    eprintln!("Writing usmap to: {}", path.display());

    // Build name table
    let (names, name_to_index) = build_name_table(structs, enums);
    eprintln!("  Name table: {} unique names", names.len());

    // Build payload buffer (uncompressed)
    let mut payload = serialize_name_table(&names);
    payload.extend(serialize_enums(enums, &name_to_index));
    payload.extend(serialize_structs(structs, &name_to_index)?);

    eprintln!("  Payload size: {} bytes (uncompressed)", payload.len());

    // Write file header + payload
    let mut file = std::fs::File::create(path)?;

    // Magic (2 bytes)
    file.write_all(&MAGIC.to_le_bytes())?;

    // Version (1 byte) - LargeEnums = 3
    file.write_all(&[EUsmapVersion::LargeEnums as u8])?;

    // bHasVersionInfo (1 byte) - false for now
    file.write_all(&[0u8])?;

    // Compression method (4 bytes as u32)
    file.write_all(&(EUsmapCompression::None as u32).to_le_bytes())?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::reflection::{EPropertyType, PropertyInfo};
    use crate::memory::usmap::serializer::write_property_type;
    use byteorder::{ByteOrder, LE};
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_usmap_magic_constant() {
        assert_eq!(MAGIC, 0x30C4);
    }

    #[test]
    fn test_usmap_version_enum() {
        assert_eq!(EUsmapVersion::Initial as u8, 0);
        assert_eq!(EUsmapVersion::PackageVersioning as u8, 1);
        assert_eq!(EUsmapVersion::LongFName as u8, 2);
        assert_eq!(EUsmapVersion::LargeEnums as u8, 3);
    }

    #[test]
    fn test_usmap_compression_enum() {
        assert_eq!(EUsmapCompression::None as u8, 0);
        assert_eq!(EUsmapCompression::Oodle as u8, 1);
        assert_eq!(EUsmapCompression::Brotli as u8, 2);
        assert_eq!(EUsmapCompression::ZStandard as u8, 3);
    }

    #[test]
    fn test_write_usmap_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.usmap");

        let structs: Vec<StructInfo> = vec![];
        let enums: Vec<EnumInfo> = vec![];

        let result = write_usmap(&path, &structs, &enums);
        assert!(result.is_ok());

        let data = std::fs::read(&path).unwrap();
        assert!(data.len() >= 16);
        assert_eq!(LE::read_u16(&data[0..2]), MAGIC);
        assert_eq!(data[2], EUsmapVersion::LargeEnums as u8);
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

        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 16);
        assert_eq!(LE::read_u16(&data[0..2]), MAGIC);
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

        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 16);
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
        assert!(payload.len() >= 5);
    }

    #[test]
    fn test_write_property_type_array() {
        let name_to_index: HashMap<String, u32> =
            [("".to_string(), 0), ("IntArray".to_string(), 1)]
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
                ("Max".to_string(), -1),
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
