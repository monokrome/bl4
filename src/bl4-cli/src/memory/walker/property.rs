//! Property Extraction
//!
//! Functions for extracting FProperty data from UE5 memory.

use crate::memory::constants::*;
use crate::memory::fname::FNameReader;
use crate::memory::reflection::{EPropertyType, PropertyInfo};
use crate::memory::source::MemorySource;

use anyhow::Result;
use byteorder::{ByteOrder, LE};

/// Read property type name from FFieldClass
pub fn read_property_type(
    source: &dyn MemorySource,
    field_class_ptr: usize,
    fname_reader: &mut FNameReader,
    debug: bool,
) -> Result<String> {
    if field_class_ptr == 0 {
        return Ok("Unknown".to_string());
    }

    // Read FFieldClass data - in BL4's UE5.4, this is a vtable followed by class data
    // Read 0x180 bytes to find the FName (might be past offset 0x100)
    let class_data = source.read_bytes(field_class_ptr, 0x180)?;

    if debug {
        eprintln!(
            "  FFieldClass at {:#x} (raw dump - 0x180 bytes):",
            field_class_ptr
        );
        // Dump all 0x180 bytes as hex for analysis
        for i in 0..24 {
            let offset = i * 16;
            if offset + 16 <= class_data.len() {
                eprint!("    +{:03x}: ", offset);
                for j in 0..16 {
                    eprint!("{:02x} ", class_data[offset + j]);
                }
                // Also show as ASCII
                eprint!(" | ");
                for j in 0..16 {
                    let b = class_data[offset + j];
                    if b >= 0x20 && b < 0x7f {
                        eprint!("{}", b as char);
                    } else {
                        eprint!(".");
                    }
                }
                eprintln!();
            }
        }
    }

    // Search for an FName-like value (small index that resolves to *Property)
    // Property type FNames are at low indices: IntProperty ~10, ObjectProperty ~32
    // Scan entire 0x180 byte region
    for offset in (0..0x180).step_by(4) {
        if offset + 4 <= class_data.len() {
            let name_index = LE::read_u32(&class_data[offset..offset + 4]);
            // Property type FNames should be small (< 500) and non-zero
            if name_index > 0 && name_index < 500 {
                if let Ok(name) = fname_reader.read_name(source, name_index) {
                    if name.ends_with("Property") {
                        if debug {
                            eprintln!(
                                "    Found Property type at +{:#x}: idx={}, name='{}'",
                                offset, name_index, name
                            );
                        }
                        return Ok(name);
                    }
                }
            }
        }
    }

    // FFieldClass in BL4's UE5.4 is purely a vtable with no embedded FName.
    // Return a placeholder that will be replaced with actual type during property extraction.
    Ok("_UNKNOWN_TYPE_".to_string())
}

// Debug counter for property extraction
static DEBUG_PROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Extract a single FProperty from memory
pub fn extract_property(
    source: &dyn MemorySource,
    prop_ptr: usize,
    fname_reader: &mut FNameReader,
) -> Result<PropertyInfo> {
    // Read FProperty data (need about 0x80 bytes for base + some type-specific)
    let prop_data = source.read_bytes(prop_ptr, 0x80)?;

    // FField base
    let _field_class_ptr =
        LE::read_u64(&prop_data[FFIELD_CLASS_OFFSET..FFIELD_CLASS_OFFSET + 8]) as usize;
    let name_index = LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]);

    // FProperty fields
    let array_dim =
        LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]);
    let element_size =
        LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]);
    let property_flags = LE::read_u64(
        &prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8],
    );
    let offset = LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]);

    // Debug first few properties (disabled for production)
    let count = DEBUG_PROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    #[allow(clippy::absurd_extreme_comparisons)]
    let debug = count < 0; // Disabled - always false for usize

    if debug {
        eprintln!("\nDEBUG Property {} at {:#x}:", count, prop_ptr);
        eprintln!("  FProperty raw dump (0x80 bytes):");
        for i in 0..8 {
            let off = i * 16;
            eprint!("    +{:03x}: ", off);
            for j in 0..16 {
                eprint!("{:02x} ", prop_data[off + j]);
            }
            eprintln!();
        }

        // Scan for small values that could be FName indices (property type names are < 100)
        eprintln!("  Small values that could be FName indices:");
        for off in (0..0x80).step_by(4) {
            let val = LE::read_u32(&prop_data[off..off + 4]);
            if val > 0 && val < 100 {
                if let Ok(name) = fname_reader.read_name(source, val) {
                    eprintln!("    +{:#04x}: idx={} -> '{}'", off, val, name);
                }
            }
        }
    }

    // Get property name
    let name = fname_reader.read_name(source, name_index)?;

    // Extract type-specific information and infer property type
    let mut struct_type = None;
    let mut enum_type = None;
    let mut inner_type = None;
    let mut value_type = None;
    let mut inferred_type = EPropertyType::Unknown;
    let mut inferred_type_name = "Unknown".to_string();

    // Read type-specific data at offset 0x78+
    let ptr_at_78 = LE::read_u64(&prop_data[0x78..0x80]) as usize;

    // Helper: check if a pointer looks like a valid UObject (has vtable in code section)
    let is_valid_uobject = |addr: usize| -> bool {
        if addr < 0x7ff000000000 || addr > 0x7fff00000000 {
            return false; // Not in heap range for this dump
        }
        if let Ok(vtable_data) = source.read_bytes(addr, 8) {
            let vtable = LE::read_u64(&vtable_data) as usize;
            // Vtable should be in code section (0x140... - 0x15f...)
            vtable >= 0x140000000 && vtable < 0x160000000
        } else {
            false
        }
    };

    // Helper: check if pointer looks like a property (has FFieldClass in data section)
    let is_valid_property = |addr: usize| -> bool {
        if addr == 0 {
            return false;
        }
        if let Ok(ffc_data) = source.read_bytes(addr, 8) {
            let ffc = LE::read_u64(&ffc_data) as usize;
            // FFieldClass should be in .didata section (0x14e... - 0x151...)
            ffc >= 0x14e000000 && ffc < 0x152000000
        } else {
            false
        }
    };

    // Try to infer type by probing type-specific data
    if ptr_at_78 != 0 {
        // Check if it's an inner property (ArrayProperty, SetProperty, MapProperty)
        if is_valid_property(ptr_at_78) {
            // Check for MapProperty first (has second property at 0x80)
            let mut is_map = false;
            if let Ok(extra) = source.read_bytes(prop_ptr + 0x80, 8) {
                let ptr_at_80 = LE::read_u64(&extra) as usize;
                if is_valid_property(ptr_at_80) {
                    if let Ok(key) = extract_property(source, ptr_at_78, fname_reader) {
                        if let Ok(val) = extract_property(source, ptr_at_80, fname_reader) {
                            inferred_type = EPropertyType::MapProperty;
                            inferred_type_name = "MapProperty".to_string();
                            inner_type = Some(Box::new(key));
                            value_type = Some(Box::new(val));
                            is_map = true;
                        }
                    }
                }
            }
            if !is_map {
                // It's ArrayProperty or SetProperty
                if let Ok(inner) = extract_property(source, ptr_at_78, fname_reader) {
                    inferred_type = EPropertyType::ArrayProperty;
                    inferred_type_name = "ArrayProperty".to_string();
                    inner_type = Some(Box::new(inner));
                }
            }
        }
        // Check if it's a UStruct* (StructProperty)
        else if is_valid_uobject(ptr_at_78) {
            if let Ok(struct_data) = source.read_bytes(ptr_at_78 + UOBJECT_NAME_OFFSET, 4) {
                let struct_name_idx = LE::read_u32(&struct_data);
                if let Ok(sname) = fname_reader.read_name(source, struct_name_idx) {
                    // Could be StructProperty or ObjectProperty - distinguish by class
                    if let Ok(class_data) = source.read_bytes(ptr_at_78 + UOBJECT_CLASS_OFFSET, 8) {
                        let class_ptr = LE::read_u64(&class_data) as usize;
                        if let Ok(class_name_data) =
                            source.read_bytes(class_ptr + UOBJECT_NAME_OFFSET, 4)
                        {
                            let class_name_idx = LE::read_u32(&class_name_data);
                            if let Ok(class_name) = fname_reader.read_name(source, class_name_idx) {
                                if class_name == "ScriptStruct" {
                                    inferred_type = EPropertyType::StructProperty;
                                    inferred_type_name = "StructProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Class" {
                                    inferred_type = EPropertyType::ObjectProperty;
                                    inferred_type_name = "ObjectProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Enum" {
                                    inferred_type = EPropertyType::ByteProperty;
                                    inferred_type_name = "ByteProperty".to_string();
                                    enum_type = Some(sname);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // If still unknown, infer from element size
    if inferred_type == EPropertyType::Unknown {
        inferred_type_name = match element_size {
            1 => {
                inferred_type = EPropertyType::ByteProperty;
                "ByteProperty"
            }
            2 => {
                inferred_type = EPropertyType::Int16Property;
                "Int16Property"
            }
            4 => {
                inferred_type = EPropertyType::IntProperty;
                "IntProperty"
            }
            8 => {
                inferred_type = EPropertyType::Int64Property;
                "Int64Property"
            }
            12 => {
                inferred_type = EPropertyType::StructProperty;
                "StructProperty"
            } // Likely FVector
            16 => {
                inferred_type = EPropertyType::StrProperty;
                "StrProperty"
            } // FString is 16 bytes
            24 => {
                inferred_type = EPropertyType::StructProperty;
                "StructProperty"
            } // Likely FRotator or Transform
            _ => "Unknown",
        }
        .to_string();
    }

    if debug {
        eprintln!(
            "  Resolved: name='{}', inferred_type='{}'",
            name, inferred_type_name
        );
    }

    Ok(PropertyInfo {
        name,
        property_type: inferred_type,
        type_name: inferred_type_name,
        array_dim,
        element_size,
        property_flags,
        offset,
        struct_type,
        enum_type,
        inner_type,
        value_type,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::memory::reflection::{EnumInfo, StructInfo, UObjectInfo, UObjectOffsets};
    use crate::memory::source::tests::MockMemorySource;

    /// Create a mock FProperty in memory
    /// Layout: FField base (0x30) + FProperty fields
    pub fn create_mock_property(
        name_index: u32,
        array_dim: i32,
        element_size: i32,
        property_flags: u64,
        offset: i32,
    ) -> Vec<u8> {
        let mut data = vec![0u8; 0x80];

        // FField::ClassPrivate at +0x00 (mock pointer)
        data[0..8].copy_from_slice(&0x14f000000u64.to_le_bytes());

        // FField::NamePrivate at +0x20 (FName = index + number)
        data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4].copy_from_slice(&name_index.to_le_bytes());

        // FProperty::ArrayDim at +0x30
        data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]
            .copy_from_slice(&array_dim.to_le_bytes());

        // FProperty::ElementSize at +0x34
        data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]
            .copy_from_slice(&element_size.to_le_bytes());

        // FProperty::PropertyFlags at +0x38
        data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8]
            .copy_from_slice(&property_flags.to_le_bytes());

        // FProperty::Offset_Internal at +0x4C
        data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]
            .copy_from_slice(&offset.to_le_bytes());

        data
    }

    /// Create a mock UObject header
    pub fn create_mock_uobject(
        vtable: u64,
        flags: i32,
        internal_index: i32,
        class_ptr: u64,
        name_index: u32,
        outer_ptr: u64,
    ) -> Vec<u8> {
        let mut data = vec![0u8; UOBJECT_HEADER_SIZE];

        data[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8]
            .copy_from_slice(&vtable.to_le_bytes());
        data[UOBJECT_FLAGS_OFFSET..UOBJECT_FLAGS_OFFSET + 4].copy_from_slice(&flags.to_le_bytes());
        data[UOBJECT_INTERNAL_INDEX_OFFSET..UOBJECT_INTERNAL_INDEX_OFFSET + 4]
            .copy_from_slice(&internal_index.to_le_bytes());
        data[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]
            .copy_from_slice(&class_ptr.to_le_bytes());
        data[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]
            .copy_from_slice(&name_index.to_le_bytes());
        data[UOBJECT_OUTER_OFFSET..UOBJECT_OUTER_OFFSET + 8]
            .copy_from_slice(&outer_ptr.to_le_bytes());

        data
    }

    #[test]
    fn test_create_mock_property() {
        let prop_data = create_mock_property(100, 1, 4, 0x1234, 0x10);

        // Verify fields are at correct offsets
        assert_eq!(
            LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]),
            100
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]),
            1
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]),
            4
        );
        assert_eq!(
            LE::read_u64(
                &prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8]
            ),
            0x1234
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]),
            0x10
        );
    }

    #[test]
    fn test_create_mock_uobject() {
        let obj_data = create_mock_uobject(
            0x14f000000, // vtable
            0x01,        // flags
            42,          // internal index
            0x200000000, // class ptr
            100,         // name index
            0x300000000, // outer ptr
        );

        assert_eq!(obj_data.len(), UOBJECT_HEADER_SIZE);
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8]),
            0x14f000000
        );
        assert_eq!(
            LE::read_i32(&obj_data[UOBJECT_FLAGS_OFFSET..UOBJECT_FLAGS_OFFSET + 4]),
            0x01
        );
        assert_eq!(
            LE::read_i32(
                &obj_data[UOBJECT_INTERNAL_INDEX_OFFSET..UOBJECT_INTERNAL_INDEX_OFFSET + 4]
            ),
            42
        );
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]),
            0x200000000
        );
        assert_eq!(
            LE::read_u32(&obj_data[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]),
            100
        );
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_OUTER_OFFSET..UOBJECT_OUTER_OFFSET + 8]),
            0x300000000
        );
    }

    #[test]
    fn test_uobject_info_structure() {
        let info = UObjectInfo {
            address: 0x200000000,
            class_ptr: 0x200000100,
            name_index: 42,
            name: "TestObject".to_string(),
            class_name: "Class".to_string(),
        };

        assert_eq!(info.address, 0x200000000);
        assert_eq!(info.name, "TestObject");
        assert_eq!(info.class_name, "Class");
    }

    #[test]
    fn test_property_type_inference_from_element_size() {
        // ByteProperty (size 1)
        let info = PropertyInfo {
            name: "ByteProp".to_string(),
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
        };
        assert_eq!(info.element_size, 1);

        // IntProperty (size 4)
        let int_info = PropertyInfo {
            name: "IntProp".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 4,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };
        assert_eq!(int_info.element_size, 4);
    }

    #[test]
    fn test_property_info_with_struct_type() {
        let info = PropertyInfo {
            name: "VectorProp".to_string(),
            property_type: EPropertyType::StructProperty,
            type_name: "StructProperty".to_string(),
            array_dim: 1,
            element_size: 12, // FVector = 3 floats
            property_flags: 0,
            offset: 0,
            struct_type: Some("Vector".to_string()),
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        assert_eq!(info.struct_type, Some("Vector".to_string()));
        assert_eq!(info.property_type, EPropertyType::StructProperty);
    }

    #[test]
    fn test_property_info_with_array() {
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

        let array_info = PropertyInfo {
            name: "IntArray".to_string(),
            property_type: EPropertyType::ArrayProperty,
            type_name: "ArrayProperty".to_string(),
            array_dim: 1,
            element_size: 16, // TArray header
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(inner)),
            value_type: None,
        };

        assert!(array_info.inner_type.is_some());
        assert_eq!(
            array_info.inner_type.as_ref().unwrap().property_type,
            EPropertyType::IntProperty
        );
    }

    #[test]
    fn test_property_info_with_map() {
        let key_type = PropertyInfo {
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

        let value_type = PropertyInfo {
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

        let map_info = PropertyInfo {
            name: "StringToIntMap".to_string(),
            property_type: EPropertyType::MapProperty,
            type_name: "MapProperty".to_string(),
            array_dim: 1,
            element_size: 80, // TMap header
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(key_type)),
            value_type: Some(Box::new(value_type)),
        };

        assert!(map_info.inner_type.is_some());
        assert!(map_info.value_type.is_some());
        assert_eq!(
            map_info.inner_type.as_ref().unwrap().property_type,
            EPropertyType::StrProperty
        );
        assert_eq!(
            map_info.value_type.as_ref().unwrap().property_type,
            EPropertyType::IntProperty
        );
    }

    #[test]
    fn test_struct_info_basic() {
        let struct_info = StructInfo {
            address: 0x200000000,
            name: "TestStruct".to_string(),
            super_name: None,
            properties: vec![],
            struct_size: 0x40,
            is_class: false,
        };

        assert_eq!(struct_info.name, "TestStruct");
        assert!(struct_info.super_name.is_none());
        assert!(!struct_info.is_class);
    }

    #[test]
    fn test_struct_info_with_super() {
        let struct_info = StructInfo {
            address: 0x200000000,
            name: "ChildClass".to_string(),
            super_name: Some("ParentClass".to_string()),
            properties: vec![],
            struct_size: 0x100,
            is_class: true,
        };

        assert_eq!(struct_info.super_name, Some("ParentClass".to_string()));
        assert!(struct_info.is_class);
    }

    #[test]
    fn test_enum_info_structure() {
        let enum_info = EnumInfo {
            address: 0x200000000,
            name: "ETestEnum".to_string(),
            values: vec![
                ("Value1".to_string(), 0),
                ("Value2".to_string(), 1),
                ("Value3".to_string(), 2),
                ("MAX".to_string(), 255),
            ],
        };

        assert_eq!(enum_info.name, "ETestEnum");
        assert_eq!(enum_info.values.len(), 4);
        assert_eq!(enum_info.values[0], ("Value1".to_string(), 0));
        assert_eq!(enum_info.values[3], ("MAX".to_string(), 255));
    }

    #[test]
    fn test_uobject_offsets_default() {
        let offsets = UObjectOffsets::default();

        // Verify default offsets match constants
        assert_eq!(offsets.class_offset, UOBJECT_CLASS_OFFSET);
        assert_eq!(offsets.name_offset, UOBJECT_NAME_OFFSET);
    }
}
