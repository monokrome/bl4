//! Property extraction tests
//!
//! Tests for FProperty data extraction from UE5 memory.

#[cfg(test)]
pub mod tests {
    use crate::memory::constants::*;
    use crate::memory::reflection::{
        EPropertyType, EnumInfo, PropertyInfo, StructInfo, UObjectInfo, UObjectOffsets,
    };

    use byteorder::{ByteOrder, LE};

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

        assert_eq!(
            LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]),
            100
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]),
            1
        );
        assert_eq!(
            LE::read_i32(
                &prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]
            ),
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
        let obj_data = create_mock_uobject(0x14f000000, 0x01, 42, 0x200000000, 100, 0x300000000);

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
            element_size: 12,
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
            element_size: 16,
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
            element_size: 80,
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

        assert_eq!(offsets.class_offset, UOBJECT_CLASS_OFFSET);
        assert_eq!(offsets.name_offset, UOBJECT_NAME_OFFSET);
    }
}
