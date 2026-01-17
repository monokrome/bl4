//! UE5 unversioned property parsing

use std::collections::HashMap;

use crate::types::{ParsedProperty, ParsedPropertyValue, PropertyTypeInfo};

/// FFragment from UE5 unversioned header - packed into 16 bits
#[derive(Debug, Clone, Default)]
pub struct FFragment {
    pub skip_num: u8,         // 7 bits: properties to skip
    pub has_any_zeroes: bool, // 1 bit: zero mask follows
    pub is_last: bool,        // 1 bit: final fragment marker
    pub value_count: u8,      // 7 bits: property count in this fragment
}

impl FFragment {
    pub fn unpack(packed: u16) -> Self {
        Self {
            skip_num: (packed & 0x7f) as u8,
            has_any_zeroes: (packed & 0x80) != 0,
            is_last: (packed & 0x100) != 0,
            value_count: (packed >> 9) as u8,
        }
    }
}

/// Context for property parsing - holds names and struct definitions
pub struct PropertyParseContext<'a> {
    pub names: &'a [String],
    pub struct_lookup: &'a HashMap<String, &'a usmap::Struct>,
}

/// Parse the FUnversionedHeader from export data
/// Returns (fragments, zero_mask, bytes_consumed)
pub fn parse_unversioned_header(data: &[u8]) -> Option<(Vec<FFragment>, Vec<u8>, usize)> {
    if data.len() < 2 {
        return None;
    }

    let mut pos = 0;
    let mut fragments = Vec::new();
    let mut total_zero_bits = 0;

    loop {
        if pos + 2 > data.len() {
            return None;
        }

        let packed = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;

        let fragment = FFragment::unpack(packed);

        if fragment.has_any_zeroes {
            total_zero_bits += fragment.value_count as usize;
        }

        let is_last = fragment.is_last;
        fragments.push(fragment);

        if is_last {
            break;
        }
    }

    let zero_mask = if total_zero_bits > 0 {
        let num_bytes = total_zero_bits.div_ceil(8);
        if pos + num_bytes > data.len() {
            return None;
        }
        let mask = data[pos..pos + num_bytes].to_vec();
        pos += num_bytes;
        mask
    } else {
        Vec::new()
    };

    Some((fragments, zero_mask, pos))
}

/// Get property indices that should be serialized based on fragments and zero mask
pub fn get_serialized_property_indices(fragments: &[FFragment], zero_mask: &[u8]) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut current_index = 0;
    let mut zero_bit_index = 0;

    for fragment in fragments {
        current_index += fragment.skip_num as usize;

        for _ in 0..fragment.value_count {
            let is_zeroed = if fragment.has_any_zeroes && !zero_mask.is_empty() {
                let byte_idx = zero_bit_index / 8;
                let bit_idx = zero_bit_index % 8;
                zero_bit_index += 1;

                if byte_idx < zero_mask.len() {
                    (zero_mask[byte_idx] & (1 << bit_idx)) != 0
                } else {
                    false
                }
            } else {
                false
            };

            if !is_zeroed {
                indices.push(current_index);
            }
            current_index += 1;
        }
    }

    indices
}

/// Get all properties for a struct, including inherited properties from super_struct
pub fn get_all_struct_properties<'a>(
    struct_name: &str,
    struct_lookup: &'a HashMap<String, &usmap::Struct>,
) -> Vec<&'a usmap::Property> {
    let mut all_props = Vec::new();
    let mut current_name = Some(struct_name.to_string());

    while let Some(name) = current_name {
        if let Some(struct_def) = struct_lookup.get(&name) {
            for prop in struct_def.properties.iter().rev() {
                all_props.push(prop);
            }
            current_name = struct_def.super_struct.clone();
        } else {
            break;
        }
    }

    all_props.reverse();
    all_props.sort_by_key(|p| p.index);
    all_props
}

/// Read a serialized FName index and resolve to string
pub fn read_fname(data: &[u8], pos: usize, names: &[String]) -> Option<(String, usize)> {
    if pos + 8 > data.len() {
        return None;
    }
    let name_index = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
    let name = names.get(name_index as usize)?.clone();
    Some((name, 8))
}

/// Read a serialized string (length-prefixed)
pub fn read_fstring(data: &[u8], pos: usize) -> Option<(String, usize)> {
    if pos + 4 > data.len() {
        return None;
    }
    let len = i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);

    if len == 0 {
        return Some((String::new(), 4));
    }

    let (str_len, is_utf16) = if len < 0 {
        ((-len) as usize, true)
    } else {
        (len as usize, false)
    };

    let start = pos + 4;
    if is_utf16 {
        let byte_len = str_len * 2;
        if start + byte_len > data.len() {
            return None;
        }
        let utf16: Vec<u16> = (0..str_len)
            .map(|i| u16::from_le_bytes([data[start + i * 2], data[start + i * 2 + 1]]))
            .collect();
        let s = String::from_utf16_lossy(&utf16)
            .trim_end_matches('\0')
            .to_string();
        Some((s, 4 + byte_len))
    } else {
        if start + str_len > data.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&data[start..start + str_len])
            .trim_end_matches('\0')
            .to_string();
        Some((s, 4 + str_len))
    }
}

/// Read a soft/asset object path reference
pub fn read_object_path(data: &[u8], pos: usize, names: &[String]) -> Option<(String, usize)> {
    if pos + 8 > data.len() {
        return None;
    }

    if let Some((path, consumed)) = read_fname(data, pos, names) {
        if !path.is_empty() && (path.starts_with('/') || path.contains('.')) {
            return Some((path, consumed));
        }
    }

    let mut total_consumed = 0;
    let (asset_path, consumed) = read_fname(data, pos, names)?;
    total_consumed += consumed;

    if pos + total_consumed + 4 <= data.len() {
        let (subpath, consumed) = read_fstring(data, pos + total_consumed)?;
        total_consumed += consumed;

        if subpath.is_empty() {
            return Some((asset_path, total_consumed));
        }
        return Some((format!("{}:{}", asset_path, subpath), total_consumed));
    }

    Some((asset_path, total_consumed))
}

/// Convert a ParsedProperty to a ParsedPropertyValue for array/map storage
pub fn property_to_value(prop: &ParsedProperty) -> ParsedPropertyValue {
    if let Some(b) = prop.int_value {
        if prop.value_type.as_deref() == Some("Bool") {
            return ParsedPropertyValue::Bool(b != 0);
        }
        return ParsedPropertyValue::Int(b);
    }
    if let Some(f) = prop.float_value {
        return ParsedPropertyValue::Float(f);
    }
    if let Some(ref s) = prop.string_value {
        return ParsedPropertyValue::String(s.clone());
    }
    if let Some(ref s) = prop.enum_value {
        return ParsedPropertyValue::String(s.clone());
    }
    if let Some(ref p) = prop.object_path {
        return ParsedPropertyValue::Object(p.clone());
    }
    if let Some(ref arr) = prop.array_values {
        return ParsedPropertyValue::Array(arr.clone());
    }
    if let Some(ref st) = prop.struct_values {
        return ParsedPropertyValue::Struct(st.clone());
    }
    ParsedPropertyValue::Null
}

/// Parse a property value, returning the value and bytes consumed
#[allow(clippy::too_many_lines)]
pub fn parse_property_value_extended(
    data: &[u8],
    pos: usize,
    inner: &usmap::PropertyInner,
    ctx: &PropertyParseContext<'_>,
) -> Option<(ParsedProperty, usize)> {
    match inner {
        usmap::PropertyInner::Bool => {
            if pos >= data.len() {
                return None;
            }
            let mut prop = ParsedProperty::with_type("Bool");
            prop.int_value = Some(if data[pos] != 0 { 1 } else { 0 });
            Some((prop, 1))
        }
        usmap::PropertyInner::Byte => {
            if pos >= data.len() {
                return None;
            }
            let mut prop = ParsedProperty::with_type("Byte");
            prop.int_value = Some(data[pos] as i64);
            Some((prop, 1))
        }
        usmap::PropertyInner::Int8 => {
            if pos >= data.len() {
                return None;
            }
            let mut prop = ParsedProperty::with_type("Int8");
            prop.int_value = Some(data[pos] as i8 as i64);
            Some((prop, 1))
        }
        usmap::PropertyInner::Int16 => {
            if pos + 2 > data.len() {
                return None;
            }
            let val = i16::from_le_bytes([data[pos], data[pos + 1]]);
            let mut prop = ParsedProperty::with_type("Int16");
            prop.int_value = Some(val as i64);
            Some((prop, 2))
        }
        usmap::PropertyInner::UInt16 => {
            if pos + 2 > data.len() {
                return None;
            }
            let val = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let mut prop = ParsedProperty::with_type("UInt16");
            prop.int_value = Some(val as i64);
            Some((prop, 2))
        }
        usmap::PropertyInner::Int => {
            if pos + 4 > data.len() {
                return None;
            }
            let val = i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut prop = ParsedProperty::with_type("Int");
            prop.int_value = Some(val as i64);
            Some((prop, 4))
        }
        usmap::PropertyInner::UInt32 => {
            if pos + 4 > data.len() {
                return None;
            }
            let val = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut prop = ParsedProperty::with_type("UInt32");
            prop.int_value = Some(val as i64);
            Some((prop, 4))
        }
        usmap::PropertyInner::Int64 => {
            if pos + 8 > data.len() {
                return None;
            }
            let val = i64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
            let mut prop = ParsedProperty::with_type("Int64");
            prop.int_value = Some(val);
            Some((prop, 8))
        }
        usmap::PropertyInner::UInt64 => {
            if pos + 8 > data.len() {
                return None;
            }
            let val = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
            let mut prop = ParsedProperty::with_type("UInt64");
            prop.int_value = Some(val as i64);
            Some((prop, 8))
        }
        usmap::PropertyInner::Float => {
            if pos + 4 > data.len() {
                return None;
            }
            let val = f32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut prop = ParsedProperty::with_type("Float");
            prop.float_value = Some(val as f64);
            Some((prop, 4))
        }
        usmap::PropertyInner::Double => {
            if pos + 8 > data.len() {
                return None;
            }
            let val = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
            let mut prop = ParsedProperty::with_type("Double");
            prop.float_value = Some(val);
            Some((prop, 8))
        }
        usmap::PropertyInner::Name => {
            let (name, consumed) = read_fname(data, pos, ctx.names)?;
            let mut prop = ParsedProperty::with_type("Name");
            prop.string_value = Some(name);
            Some((prop, consumed))
        }
        usmap::PropertyInner::Str
        | usmap::PropertyInner::Utf8Str
        | usmap::PropertyInner::AnsiStr => {
            let (s, consumed) = read_fstring(data, pos)?;
            let mut prop = ParsedProperty::with_type("String");
            prop.string_value = Some(s);
            Some((prop, consumed))
        }
        usmap::PropertyInner::Text => {
            if pos + 4 > data.len() {
                return None;
            }
            let (s, consumed) = read_fstring(data, pos + 4).unwrap_or((String::new(), 0));
            let mut prop = ParsedProperty::with_type("Text");
            prop.string_value = Some(s);
            Some((prop, 4 + consumed))
        }
        usmap::PropertyInner::Object
        | usmap::PropertyInner::WeakObject
        | usmap::PropertyInner::LazyObject
        | usmap::PropertyInner::Interface => {
            if pos + 4 > data.len() {
                return None;
            }
            let index =
                i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);

            let path = if index == 0 {
                "None".to_string()
            } else if index > 0 {
                format!("Export:{}", index - 1)
            } else {
                format!("Import:{}", -index - 1)
            };

            let mut prop = ParsedProperty::with_type("Object");
            prop.object_path = Some(path);
            Some((prop, 4))
        }
        usmap::PropertyInner::SoftObject | usmap::PropertyInner::AssetObject => {
            let (path, consumed) = read_object_path(data, pos, ctx.names)?;
            let mut prop = ParsedProperty::with_type("SoftObject");
            prop.object_path = Some(path);
            Some((prop, consumed))
        }
        usmap::PropertyInner::Enum {
            inner: _,
            name: enum_name,
        } => {
            let (value, consumed) = read_fname(data, pos, ctx.names)?;
            let mut prop = ParsedProperty::with_type(&format!("Enum:{}", enum_name));
            prop.enum_value = Some(value);
            Some((prop, consumed))
        }
        usmap::PropertyInner::Array { inner } => {
            if pos + 4 > data.len() {
                return None;
            }
            let count =
                i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut current_pos = pos + 4;
            let mut values = Vec::new();

            for _ in 0..count {
                if let Some((elem, consumed)) =
                    parse_property_value_extended(data, current_pos, inner, ctx)
                {
                    values.push(property_to_value(&elem));
                    current_pos += consumed;
                } else {
                    break;
                }
            }

            let mut prop = ParsedProperty::with_type("Array");
            prop.int_value = Some(count as i64);
            prop.array_values = Some(values);
            Some((prop, current_pos - pos))
        }
        usmap::PropertyInner::Struct { name: struct_name } => {
            if let Some(struct_def) = ctx.struct_lookup.get(struct_name) {
                if let Some((fragments, zero_mask, header_size)) =
                    parse_unversioned_header(&data[pos..])
                {
                    let indices = get_serialized_property_indices(&fragments, &zero_mask);
                    let all_props = get_all_struct_properties(struct_name, ctx.struct_lookup);
                    let index_to_prop: HashMap<usize, &usmap::Property> =
                        all_props.iter().map(|p| (p.index as usize, *p)).collect();

                    let mut struct_props = Vec::new();
                    let mut current_pos = pos + header_size;

                    for prop_index in indices {
                        if let Some(prop_def) = index_to_prop.get(&prop_index) {
                            if let Some((mut parsed, consumed)) =
                                parse_property_value_extended(data, current_pos, &prop_def.inner, ctx)
                            {
                                parsed.name = prop_def.name.clone();
                                struct_props.push(parsed);
                                current_pos += consumed;
                            } else {
                                break;
                            }
                        }
                    }

                    if !struct_props.is_empty() {
                        let mut prop = ParsedProperty::with_type(&format!("Struct:{}", struct_name));
                        prop.struct_values = Some(struct_props);
                        return Some((prop, current_pos - pos));
                    }
                }

                if struct_def.properties.is_empty() {
                    let mut prop = ParsedProperty::with_type(&format!("Struct:{}", struct_name));
                    prop.struct_values = Some(Vec::new());
                    return Some((prop, 0));
                }
            }
            None
        }
        usmap::PropertyInner::Map { key, value } => {
            if pos + 4 > data.len() {
                return None;
            }
            let count =
                i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut current_pos = pos + 4;
            let mut pairs = Vec::new();

            for _ in 0..count {
                let (key_prop, key_consumed) =
                    parse_property_value_extended(data, current_pos, key, ctx)?;
                current_pos += key_consumed;

                let (val_prop, val_consumed) =
                    parse_property_value_extended(data, current_pos, value, ctx)?;
                current_pos += val_consumed;

                pairs.push((property_to_value(&key_prop), property_to_value(&val_prop)));
            }

            let mut prop = ParsedProperty::with_type("Map");
            prop.int_value = Some(count as i64);
            prop.map_values = Some(pairs);
            Some((prop, current_pos - pos))
        }
        usmap::PropertyInner::Set { key } => {
            if pos + 4 > data.len() {
                return None;
            }
            let count =
                i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut current_pos = pos + 4;
            let mut values = Vec::new();

            for _ in 0..count {
                if let Some((elem, consumed)) =
                    parse_property_value_extended(data, current_pos, key, ctx)
                {
                    values.push(property_to_value(&elem));
                    current_pos += consumed;
                } else {
                    break;
                }
            }

            let mut prop = ParsedProperty::with_type("Set");
            prop.int_value = Some(count as i64);
            prop.array_values = Some(values);
            Some((prop, current_pos - pos))
        }
        usmap::PropertyInner::Optional { inner } => {
            if pos >= data.len() {
                return None;
            }
            let has_value = data[pos] != 0;
            if has_value {
                let (mut prop, consumed) =
                    parse_property_value_extended(data, pos + 1, inner, ctx)?;
                prop.value_type = Some(format!(
                    "Optional<{}>",
                    prop.value_type.as_deref().unwrap_or("?")
                ));
                Some((prop, 1 + consumed))
            } else {
                Some((ParsedProperty::with_type("Optional:None"), 1))
            }
        }
        usmap::PropertyInner::Delegate | usmap::PropertyInner::MulticastDelegate => None,
        usmap::PropertyInner::FieldPath => {
            if pos + 4 > data.len() {
                return None;
            }
            let count =
                i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let mut current_pos = pos + 4;
            let mut path_parts = Vec::new();

            for _ in 0..count {
                let (name, consumed) = read_fname(data, current_pos, ctx.names)?;
                path_parts.push(name);
                current_pos += consumed;
            }

            let (_, owner_consumed) =
                read_fname(data, current_pos, ctx.names).unwrap_or((String::new(), 8));
            current_pos += owner_consumed;

            let mut prop = ParsedProperty::with_type("FieldPath");
            prop.string_value = Some(path_parts.join("."));
            Some((prop, current_pos - pos))
        }
        usmap::PropertyInner::Unknown => None,
    }
}

/// Parse property values from export serialized data (heuristic approach)
#[allow(clippy::too_many_lines)]
pub fn parse_export_properties(
    data: &[u8],
    offset: usize,
    size: usize,
    names: &[String],
) -> Option<Vec<ParsedProperty>> {
    if offset >= data.len() || size == 0 {
        return None;
    }

    let end = (offset + size).min(data.len());
    let export_data = &data[offset..end];

    if export_data.len() < 8 {
        return None;
    }

    let mut properties = Vec::new();

    let has_double = names.iter().any(|n| n == "DoubleProperty");
    let has_float = names.iter().any(|n| n == "FloatProperty");

    let mut value_props: Vec<&String> = names
        .iter()
        .filter(|n| {
            let parts: Vec<&str> = n.split('_').collect();
            parts.len() >= 3
                && parts
                    .iter()
                    .any(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_hexdigit()))
                && !n.starts_with('/')
                && !n.contains("Property")
        })
        .collect();

    value_props.sort_by_key(|n| {
        n.split('_')
            .filter_map(|s| s.parse::<u32>().ok())
            .next()
            .unwrap_or(9999)
    });

    if value_props.is_empty() {
        return None;
    }

    if has_double {
        let scan_start = export_data.len().saturating_sub(value_props.len() * 8 + 32);
        let mut double_values: Vec<f64> = Vec::new();

        for i in (scan_start..export_data.len().saturating_sub(7)).step_by(8) {
            if let Ok(bytes) = export_data[i..i + 8].try_into() {
                let bytes: [u8; 8] = bytes;
                let val = f64::from_le_bytes(bytes);
                if val.is_finite()
                    && (val == 0.0 || (val.abs() >= 0.0001 && val.abs() <= 1_000_000.0))
                {
                    double_values.push(val);
                }
            }
        }

        let num_to_map = value_props.len().min(double_values.len());
        let value_start = double_values.len().saturating_sub(num_to_map);

        for (i, prop_name) in value_props.iter().take(num_to_map).enumerate() {
            let val_idx = value_start + i;
            if val_idx < double_values.len() {
                let parts: Vec<&str> = prop_name.split('_').collect();
                let base_name = if parts.len() >= 2 {
                    format!("{}_{}", parts[0], parts[1])
                } else {
                    prop_name.to_string()
                };

                let mut prop = ParsedProperty::with_type("Double");
                prop.name = base_name;
                prop.float_value = Some(double_values[val_idx]);
                properties.push(prop);
            }
        }
    }

    if has_float && properties.is_empty() {
        let scan_start = export_data.len().saturating_sub(value_props.len() * 4 + 16);
        let mut float_values: Vec<f32> = Vec::new();

        for i in (scan_start..export_data.len().saturating_sub(3)).step_by(4) {
            if let Ok(bytes) = export_data[i..i + 4].try_into() {
                let bytes: [u8; 4] = bytes;
                let val = f32::from_le_bytes(bytes);
                if val.is_finite()
                    && (val == 0.0 || (val.abs() >= 0.0001 && val.abs() <= 1_000_000.0))
                {
                    float_values.push(val);
                }
            }
        }

        let num_to_map = value_props.len().min(float_values.len());
        let value_start = float_values.len().saturating_sub(num_to_map);

        for (i, prop_name) in value_props.iter().take(num_to_map).enumerate() {
            let val_idx = value_start + i;
            if val_idx < float_values.len() {
                let parts: Vec<&str> = prop_name.split('_').collect();
                let base_name = if parts.len() >= 2 {
                    format!("{}_{}", parts[0], parts[1])
                } else {
                    prop_name.to_string()
                };

                let mut prop = ParsedProperty::with_type("Float");
                prop.name = base_name;
                prop.float_value = Some(float_values[val_idx] as f64);
                properties.push(prop);
            }
        }
    }

    if properties.is_empty() {
        None
    } else {
        Some(properties)
    }
}

/// Parse embedded UserDefinedStruct schema to extract per-property type info
pub fn parse_embedded_schema(_data: &[u8], names: &[String]) -> Option<Vec<PropertyTypeInfo>> {
    let name_to_idx: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    let double_idx = name_to_idx.get("DoubleProperty").copied();
    let float_idx = name_to_idx.get("FloatProperty").copied();
    let int_idx = name_to_idx.get("IntProperty").copied();

    let prop_names: Vec<(usize, String, u32)> = names
        .iter()
        .enumerate()
        .filter_map(|(idx, name)| {
            if name.starts_with('/') || name.contains("Property") || name == "None" {
                return None;
            }
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                if let Some(guid_pos) = parts
                    .iter()
                    .position(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_hexdigit()))
                {
                    if guid_pos >= 2 {
                        if let Ok(schema_idx) = parts[guid_pos - 1].parse::<u32>() {
                            let prop_name = parts[..guid_pos - 1].join("_");
                            return Some((idx, prop_name, schema_idx));
                        }
                    }
                }
            }
            None
        })
        .collect();

    if prop_names.is_empty() {
        return None;
    }

    let has_double = double_idx.is_some();
    let has_float = float_idx.is_some();
    let has_int = int_idx.is_some();

    let (default_type, default_size) = if has_double && !has_float && !has_int {
        ("Double", 8)
    } else if has_float && !has_double && !has_int {
        ("Float", 4)
    } else if has_int && !has_double && !has_float {
        ("Int", 4)
    } else if has_float && !has_double {
        ("Float", 4)
    } else {
        ("Double", 8)
    };

    let prop_types: HashMap<usize, (String, usize)> = prop_names
        .iter()
        .map(|(idx, _, _)| (*idx, (default_type.to_string(), default_size)))
        .collect();

    let mut result: Vec<PropertyTypeInfo> = prop_names
        .into_iter()
        .map(|(name_idx, prop_name, schema_idx)| {
            let (type_name, size) = prop_types
                .get(&name_idx)
                .cloned()
                .unwrap_or_else(|| ("Double".to_string(), 8));
            PropertyTypeInfo {
                name: prop_name,
                type_name,
                size,
                schema_index: schema_idx,
            }
        })
        .collect();

    result.sort_by_key(|p| p.schema_index);
    Some(result)
}

/// Extract property info from names table
pub fn extract_property_info_from_names(names: &[String]) -> Vec<(String, u32, String)> {
    let mut props = Vec::new();

    for name in names {
        if name.starts_with('/') || name.contains("Property") || name == "None" {
            continue;
        }

        let parts: Vec<&str> = name.split('_').collect();
        if parts.len() >= 3 {
            if let Some(guid_idx) = parts
                .iter()
                .position(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_hexdigit()))
            {
                if guid_idx >= 2 {
                    if let Ok(index) = parts[guid_idx - 1].parse::<u32>() {
                        let prop_name = parts[..guid_idx - 1].join("_");
                        let guid = parts[guid_idx].to_string();
                        props.push((prop_name, index, guid));
                    }
                }
            }
        }
    }

    props.sort_by_key(|(_, idx, _)| *idx);
    props
}

/// Parse properties using usmap schema for proper field names and types
#[allow(clippy::too_many_lines)]
pub fn parse_export_properties_with_schema(
    data: &[u8],
    offset: usize,
    size: usize,
    names: &[String],
    struct_lookup: &HashMap<String, &usmap::Struct>,
    resolved_class_name: Option<&str>,
    _verbose: bool,
) -> Option<Vec<ParsedProperty>> {
    if offset >= data.len() || size == 0 {
        return None;
    }

    let end = (offset + size).min(data.len());
    let export_data = &data[offset..end];

    if export_data.len() < 2 {
        return None;
    }

    let has_double = names.iter().any(|n| n == "DoubleProperty");
    let has_float = names.iter().any(|n| n == "FloatProperty");

    let struct_type = resolved_class_name
        .and_then(|name| {
            if struct_lookup.contains_key(name) {
                return Some(name.to_string());
            }
            let prefixed = format!("F{}", name);
            if struct_lookup.contains_key(&prefixed) {
                return Some(prefixed);
            }
            None
        })
        .or_else(|| {
            names
                .iter()
                .find(|n| struct_lookup.contains_key(*n))
                .cloned()
                .or_else(|| {
                    names
                        .iter()
                        .find(|n| {
                            let prefixed = format!("F{}", n);
                            struct_lookup.contains_key(&prefixed)
                        })
                        .map(|n| format!("F{}", n))
                })
        });

    let ctx = PropertyParseContext {
        names,
        struct_lookup,
    };

    if let Some(ref type_name) = struct_type {
        if struct_lookup.contains_key(type_name) {
            if let Some((fragments, zero_mask, header_size)) = parse_unversioned_header(export_data)
            {
                let serialized_indices = get_serialized_property_indices(&fragments, &zero_mask);

                if !serialized_indices.is_empty() {
                    let all_props = get_all_struct_properties(type_name, struct_lookup);
                    let index_to_prop: HashMap<usize, &usmap::Property> =
                        all_props.iter().map(|p| (p.index as usize, *p)).collect();

                    let mut properties = Vec::new();
                    let mut pos = header_size;

                    for prop_index in serialized_indices {
                        if let Some(prop_def) = index_to_prop.get(&prop_index) {
                            if let Some((mut parsed, consumed)) =
                                parse_property_value_extended(export_data, pos, &prop_def.inner, &ctx)
                            {
                                parsed.name = prop_def.name.clone();
                                properties.push(parsed);
                                pos += consumed;
                            } else {
                                break;
                            }
                        }
                    }

                    if !properties.is_empty() {
                        return Some(properties);
                    }
                }
            }
        }
    }

    if let Some(prop_info) = parse_embedded_schema(export_data, names) {
        let expected_data_size: usize = prop_info.iter().map(|p| p.size).sum();

        if export_data.len() >= expected_data_size {
            let values_start = export_data.len() - expected_data_size;
            let mut properties = Vec::new();
            let mut pos = values_start;
            let mut garbage_count = 0;

            for prop in &prop_info {
                if pos + prop.size > export_data.len() {
                    break;
                }

                match prop.type_name.as_str() {
                    "Double" => {
                        if let Ok(bytes) = export_data[pos..pos + 8].try_into() {
                            let val: f64 = f64::from_le_bytes(bytes);
                            let is_reasonable = val.is_finite()
                                && (val == 0.0 || val.abs() > 1e-100 && val.abs() < 1e100);
                            if is_reasonable {
                                let mut parsed = ParsedProperty::with_type("Double");
                                parsed.name = prop.name.clone();
                                parsed.float_value = Some(val);
                                properties.push(parsed);
                            } else {
                                garbage_count += 1;
                            }
                        }
                    }
                    "Float" => {
                        if let Ok(bytes) = export_data[pos..pos + 4].try_into() {
                            let val: f32 = f32::from_le_bytes(bytes);
                            let is_reasonable =
                                val.is_finite() && (val == 0.0 || val.abs() > 1e-30 && val.abs() < 1e30);
                            if is_reasonable {
                                let mut parsed = ParsedProperty::with_type("Float");
                                parsed.name = prop.name.clone();
                                parsed.float_value = Some(val as f64);
                                properties.push(parsed);
                            } else {
                                garbage_count += 1;
                            }
                        }
                    }
                    "Int" => {
                        if let Ok(bytes) = export_data[pos..pos + 4].try_into() {
                            let val: i32 = i32::from_le_bytes(bytes);
                            let mut parsed = ParsedProperty::with_type("Int");
                            parsed.name = prop.name.clone();
                            parsed.int_value = Some(val as i64);
                            properties.push(parsed);
                        }
                    }
                    _ => {}
                }

                pos += prop.size;
            }

            if !properties.is_empty() && garbage_count < prop_info.len() / 2 {
                return Some(properties);
            }
        }
    }

    let prop_info = extract_property_info_from_names(names);
    if prop_info.is_empty() {
        return parse_export_properties(data, offset, size, names);
    }

    let value_size = if has_double {
        8
    } else if has_float {
        4
    } else {
        8
    };
    let expected_data_size = prop_info.len() * value_size;

    if export_data.len() >= expected_data_size {
        let values_start = export_data.len() - expected_data_size;
        let mut properties = Vec::new();
        let mut pos = values_start;

        for (prop_name, _, _) in &prop_info {
            if pos + value_size > export_data.len() {
                break;
            }

            if value_size == 8 {
                if let Ok(bytes) = export_data[pos..pos + 8].try_into() {
                    let val: f64 = f64::from_le_bytes(bytes);
                    if val.is_finite() {
                        let mut parsed = ParsedProperty::with_type("Double");
                        parsed.name = prop_name.clone();
                        parsed.float_value = Some(val);
                        properties.push(parsed);
                    }
                }
            } else if let Ok(bytes) = export_data[pos..pos + 4].try_into() {
                let val: f32 = f32::from_le_bytes(bytes);
                if val.is_finite() {
                    let mut parsed = ParsedProperty::with_type("Float");
                    parsed.name = prop_name.clone();
                    parsed.float_value = Some(val as f64);
                    properties.push(parsed);
                }
            }

            pos += value_size;
        }

        if properties.len() == prop_info.len() {
            return Some(properties);
        }
    }

    parse_export_properties(data, offset, size, names)
}
