//! USMAP struct and enum serialization

use std::collections::HashMap;

use anyhow::Result;

use crate::memory::reflection::{EPropertyType, EnumInfo, PropertyInfo, StructInfo};

/// Serialize enums to bytes
pub fn serialize_enums(enums: &[EnumInfo], name_to_index: &HashMap<String, u32>) -> Vec<u8> {
    let mut payload = Vec::new();

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

    payload
}

/// Serialize structs to bytes
pub fn serialize_structs(
    structs: &[StructInfo],
    name_to_index: &HashMap<String, u32>,
) -> Result<Vec<u8>> {
    let mut payload = Vec::new();

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
            write_property(&mut payload, prop, name_to_index, i as u16)?;
        }
    }

    Ok(payload)
}

/// Write a property to the payload
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
pub fn write_property_type(
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
