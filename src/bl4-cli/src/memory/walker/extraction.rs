//! Property extraction from UE5 memory
//!
//! Functions for extracting FProperty data from UE5 memory.

#![allow(unused_comparisons)]

use crate::memory::constants::*;
use crate::memory::fname::FNameReader;
use crate::memory::reflection::{EPropertyType, PropertyInfo};
use crate::memory::source::MemorySource;

use super::validation::{is_valid_property, is_valid_uobject};

use anyhow::Result;
use byteorder::{ByteOrder, LE};

// Debug counter for property extraction
static DEBUG_PROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Extract a single FProperty from memory
pub fn extract_property(
    source: &dyn MemorySource,
    prop_ptr: usize,
    fname_reader: &mut FNameReader,
) -> Result<PropertyInfo> {
    let prop_data = source.read_bytes(prop_ptr, 0x80)?;
    let base_fields = read_base_fields(&prop_data);

    let count = DEBUG_PROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    #[allow(clippy::absurd_extreme_comparisons)]
    let debug = count < 0; // Disabled - always false for usize

    if debug {
        dump_property_debug(&prop_data, prop_ptr, count, source, fname_reader);
    }

    let name = fname_reader.read_name(source, base_fields.name_index)?;
    let type_info = infer_property_type(source, prop_ptr, &prop_data, fname_reader)?;

    if debug {
        eprintln!(
            "  Resolved: name='{}', inferred_type='{}'",
            name, type_info.type_name
        );
    }

    Ok(PropertyInfo {
        name,
        property_type: type_info.property_type,
        type_name: type_info.type_name,
        array_dim: base_fields.array_dim,
        element_size: base_fields.element_size,
        property_flags: base_fields.property_flags,
        offset: base_fields.offset,
        struct_type: type_info.struct_type,
        enum_type: type_info.enum_type,
        inner_type: type_info.inner_type,
        value_type: type_info.value_type,
    })
}

/// Base property fields read from FProperty data
struct BaseFields {
    name_index: u32,
    array_dim: i32,
    element_size: i32,
    property_flags: u64,
    offset: i32,
}

/// Read base FProperty fields from raw data
fn read_base_fields(prop_data: &[u8]) -> BaseFields {
    BaseFields {
        name_index: LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]),
        array_dim: LE::read_i32(
            &prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4],
        ),
        element_size: LE::read_i32(
            &prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4],
        ),
        property_flags: LE::read_u64(
            &prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8],
        ),
        offset: LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]),
    }
}

/// Inferred type information from property analysis
struct TypeInfo {
    property_type: EPropertyType,
    type_name: String,
    struct_type: Option<String>,
    enum_type: Option<String>,
    inner_type: Option<Box<PropertyInfo>>,
    value_type: Option<Box<PropertyInfo>>,
}

/// Infer property type from type-specific data pointers
fn infer_property_type(
    source: &dyn MemorySource,
    prop_ptr: usize,
    prop_data: &[u8],
    fname_reader: &mut FNameReader,
) -> Result<TypeInfo> {
    let ptr_at_78 = LE::read_u64(&prop_data[0x78..0x80]) as usize;

    // Try to infer from type-specific data at offset 0x78
    if ptr_at_78 != 0 {
        if let Some(info) = try_infer_container_type(source, prop_ptr, ptr_at_78, fname_reader)? {
            return Ok(info);
        }
        if let Some(info) = try_infer_object_type(source, ptr_at_78, fname_reader)? {
            return Ok(info);
        }
    }

    // Fall back to element size inference
    Ok(infer_from_element_size(prop_data))
}

/// Try to infer container types (Array, Set, Map) from inner property pointers
fn try_infer_container_type(
    source: &dyn MemorySource,
    prop_ptr: usize,
    ptr_at_78: usize,
    fname_reader: &mut FNameReader,
) -> Result<Option<TypeInfo>> {
    if !is_valid_property(source, ptr_at_78) {
        return Ok(None);
    }

    // Check for MapProperty (has second property at 0x80)
    if let Ok(extra) = source.read_bytes(prop_ptr + 0x80, 8) {
        let ptr_at_80 = LE::read_u64(&extra) as usize;
        if is_valid_property(source, ptr_at_80) {
            if let Ok(key) = extract_property(source, ptr_at_78, fname_reader) {
                if let Ok(val) = extract_property(source, ptr_at_80, fname_reader) {
                    return Ok(Some(TypeInfo {
                        property_type: EPropertyType::MapProperty,
                        type_name: "MapProperty".to_string(),
                        struct_type: None,
                        enum_type: None,
                        inner_type: Some(Box::new(key)),
                        value_type: Some(Box::new(val)),
                    }));
                }
            }
        }
    }

    // It's ArrayProperty or SetProperty
    if let Ok(inner) = extract_property(source, ptr_at_78, fname_reader) {
        return Ok(Some(TypeInfo {
            property_type: EPropertyType::ArrayProperty,
            type_name: "ArrayProperty".to_string(),
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(inner)),
            value_type: None,
        }));
    }

    Ok(None)
}

/// Try to infer object types (Struct, Object, Enum) from UObject pointer
fn try_infer_object_type(
    source: &dyn MemorySource,
    ptr_at_78: usize,
    fname_reader: &mut FNameReader,
) -> Result<Option<TypeInfo>> {
    if !is_valid_uobject(source, ptr_at_78) {
        return Ok(None);
    }

    let struct_data = source.read_bytes(ptr_at_78 + UOBJECT_NAME_OFFSET, 4)?;
    let struct_name_idx = LE::read_u32(&struct_data);
    let sname = fname_reader.read_name(source, struct_name_idx)?;

    let class_data = source.read_bytes(ptr_at_78 + UOBJECT_CLASS_OFFSET, 8)?;
    let class_ptr = LE::read_u64(&class_data) as usize;
    let class_name_data = source.read_bytes(class_ptr + UOBJECT_NAME_OFFSET, 4)?;
    let class_name_idx = LE::read_u32(&class_name_data);
    let class_name = fname_reader.read_name(source, class_name_idx)?;

    match class_name.as_str() {
        "ScriptStruct" => Ok(Some(TypeInfo {
            property_type: EPropertyType::StructProperty,
            type_name: "StructProperty".to_string(),
            struct_type: Some(sname),
            enum_type: None,
            inner_type: None,
            value_type: None,
        })),
        "Class" => Ok(Some(TypeInfo {
            property_type: EPropertyType::ObjectProperty,
            type_name: "ObjectProperty".to_string(),
            struct_type: Some(sname),
            enum_type: None,
            inner_type: None,
            value_type: None,
        })),
        "Enum" => Ok(Some(TypeInfo {
            property_type: EPropertyType::ByteProperty,
            type_name: "ByteProperty".to_string(),
            struct_type: None,
            enum_type: Some(sname),
            inner_type: None,
            value_type: None,
        })),
        _ => Ok(None),
    }
}

/// Infer property type from element size
fn infer_from_element_size(prop_data: &[u8]) -> TypeInfo {
    let element_size =
        LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]);

    let (property_type, type_name) = match element_size {
        1 => (EPropertyType::ByteProperty, "ByteProperty"),
        2 => (EPropertyType::Int16Property, "Int16Property"),
        4 => (EPropertyType::IntProperty, "IntProperty"),
        8 => (EPropertyType::Int64Property, "Int64Property"),
        12 => (EPropertyType::StructProperty, "StructProperty"), // Likely FVector
        16 => (EPropertyType::StrProperty, "StrProperty"),       // FString is 16 bytes
        24 => (EPropertyType::StructProperty, "StructProperty"), // Likely FRotator
        _ => (EPropertyType::Unknown, "Unknown"),
    };

    TypeInfo {
        property_type,
        type_name: type_name.to_string(),
        struct_type: None,
        enum_type: None,
        inner_type: None,
        value_type: None,
    }
}

/// Dump property data for debugging
fn dump_property_debug(
    prop_data: &[u8],
    prop_ptr: usize,
    count: usize,
    source: &dyn MemorySource,
    fname_reader: &mut FNameReader,
) {
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

    // Scan for small values that could be FName indices
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
