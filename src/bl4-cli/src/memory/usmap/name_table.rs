//! USMAP name table building

use std::collections::HashMap;

use crate::memory::reflection::{EnumInfo, PropertyInfo, StructInfo};

/// Build the name table from structs and enums
pub fn build_name_table(
    structs: &[StructInfo],
    enums: &[EnumInfo],
) -> (Vec<String>, HashMap<String, u32>) {
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

    (names, name_to_index)
}

/// Collect property names recursively for nested types
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

/// Serialize name table to bytes
pub fn serialize_name_table(names: &[String]) -> Vec<u8> {
    let mut payload = Vec::new();

    payload.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for name in names {
        let bytes = name.as_bytes();
        // Use LongFName format: length as u16
        payload.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        payload.extend_from_slice(bytes);
    }

    payload
}
