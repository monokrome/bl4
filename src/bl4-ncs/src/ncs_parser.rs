//! NCS binary section parser

use crate::bit_reader::BitReader;
use crate::types::StringTable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed NCS document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub table_id: String,
    pub deps: Vec<String>,
    pub remap_a: FixedWidthArray,
    pub remap_b: FixedWidthArray,
    pub records: Vec<Record>,
}

/// Fixed-width integer array (24-bit count + 8-bit width header)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedWidthArray {
    pub count: u32,
    pub width: u8,
    pub values: Vec<u32>,
}

/// Parsed record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub tags: Vec<Tag>,
    pub entries: HashMap<String, EntryValue>,
    pub dep_entries: Vec<DepEntry>,
}

/// Dependency entry (contains serialindex)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepEntry {
    pub dep_table_name: String,
    pub dep_table_id: usize,
    pub name: String,
    pub fields: HashMap<String, FieldValue>,
}

/// Tag types from tags section
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Tag {
    Pair { value: u32 },
    U32 { value: u32 },
    U32F32 { u32_val: u32, f32_val: f32 },
    List { items: Vec<String> },
    Variant { subtype: u8 },
}

/// Entry value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EntryValue {
    Present,
    String(String),
    Ref(String),
}

/// Field value in dep_entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    String(String),
    Object(HashMap<String, String>),
}

/// Parse FixedWidthIntArray24
pub fn parse_fixed_width_array24(reader: &mut BitReader) -> Option<FixedWidthArray> {
    let count = reader.read_bits(24)?;
    let width = reader.read_bits(8)? as u8;

    eprintln!("DEBUG FixedWidthArray24: count={}, width={}", count, width);

    if width == 0 || width > 32 {
        eprintln!("DEBUG: Invalid width {}", width);
        return None;
    }

    if count > 100000 {
        eprintln!("DEBUG: Count too large: {}", count);
        return None;
    }

    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(reader.read_bits(width)?);
    }

    Some(FixedWidthArray { count, width, values })
}

/// Read Elias gamma coded value
pub fn read_elias_gamma(reader: &mut BitReader) -> Option<u32> {
    let mut zeros = 0;

    // Count leading zeros
    while zeros < 32 {
        match reader.read_bits(1) {
            Some(0) => zeros += 1,
            Some(1) => break,
            Some(_) => return None, // Invalid - should only be 0 or 1
            None => return None,
        }
    }

    if zeros == 0 {
        return Some(1);
    }

    if zeros > 31 {
        return None;
    }

    let remainder = reader.read_bits(zeros as u8)?;
    Some((1 << zeros) | remainder)
}

/// Parse dependencies (Elias gamma coded indices)
pub fn parse_dependencies(reader: &mut BitReader, strings: &StringTable) -> Option<Vec<String>> {
    let mut deps = Vec::new();

    loop {
        let idx = read_elias_gamma(reader)?;
        eprintln!("DEBUG deps: read Elias gamma = {}", idx);

        if idx == 0 || idx > 1024 || idx as usize >= strings.len() {
            eprintln!("DEBUG deps: stopping (idx={}, max={})", idx, strings.len());
            break;
        }

        let s = strings.get(idx as usize)?;
        eprintln!("DEBUG deps: [{}] = {:?}", idx, s);
        deps.push(s.to_string());

        if deps.len() >= 1024 {
            break;
        }
    }

    Some(deps)
}

/// Calculate bit width for indexing
fn bit_width(count: usize) -> u8 {
    if count < 2 {
        return 1;
    }
    let n = (count - 1) as u32;
    (32 - n.leading_zeros()) as u8
}

/// Parse tags section (until 0x7a terminator)
pub fn parse_tags(
    reader: &mut BitReader,
    strings: &StringTable,
    remap_a: &FixedWidthArray,
) -> Option<Vec<Tag>> {
    let mut tags = Vec::new();

    loop {
        let tag_byte = reader.read_bits(8)? as u8;

        if tag_byte == 0x7a {
            break;
        }

        let tag = match tag_byte {
            0x61 => {
                let idx = reader.read_bits(remap_a.width)?;
                let value = *remap_a.values.get(idx as usize)?;
                Tag::Pair { value }
            }
            0x62 => {
                let value = reader.read_bits(32)?;
                Tag::U32 { value }
            }
            0x63 => {
                let bits = reader.read_bits(32)?;
                Tag::U32F32 {
                    u32_val: bits,
                    f32_val: f32::from_bits(bits),
                }
            }
            0x64 | 0x65 | 0x66 => {
                let items = parse_list(reader, strings)?;
                Tag::List { items }
            }
            0x70 => {
                let subtype = reader.read_bits(2)? as u8;
                Tag::Variant { subtype }
            }
            _ => continue,
        };

        tags.push(tag);
    }

    Some(tags)
}

/// Parse string list (until "none" terminator)
fn parse_list(reader: &mut BitReader, strings: &StringTable) -> Option<Vec<String>> {
    let string_bits = bit_width(strings.len());
    let mut items = Vec::new();

    for _ in 0..4095 {
        let idx = reader.read_bits(string_bits)?;
        let s = strings.get(idx as usize)?;

        if s.eq_ignore_ascii_case("none") || s.is_empty() {
            break;
        }

        items.push(s.to_string());
    }

    Some(items)
}

/// Parse entries section (2-bit type codes)
pub fn parse_entries(
    reader: &mut BitReader,
    strings: &StringTable,
) -> Option<HashMap<String, EntryValue>> {
    let string_bits = bit_width(strings.len());
    let mut entries = HashMap::new();

    loop {
        let entry_type = reader.read_bits(2)?;

        match entry_type {
            0 => break,
            1 => {
                let idx = reader.read_bits(string_bits)?;
                let name = strings.get(idx as usize)?;
                entries.insert(name.to_string(), EntryValue::Present);
            }
            2 => {
                let idx = reader.read_bits(string_bits)?;
                let name = strings.get(idx as usize)?;
                // Variant - skip for now
                entries.insert(name.to_string(), EntryValue::Present);
            }
            3 => {
                let idx = reader.read_bits(string_bits)?;
                let name = strings.get(idx as usize)?;
                let ref_idx = reader.read_bits(string_bits)?;
                let ref_name = strings.get(ref_idx as usize)?;
                entries.insert(name.to_string(), EntryValue::Ref(ref_name.to_string()));
            }
            _ => return None,
        }
    }

    Some(entries)
}

/// Parse dep_entries (WHERE SERIALINDEX IS)
pub fn parse_dep_entries(
    reader: &mut BitReader,
    strings: &StringTable,
    deps: &[String],
) -> Option<Vec<DepEntry>> {
    let string_bits = bit_width(strings.len());
    let mut all_entries = Vec::new();

    for (dep_idx, dep_name) in deps.iter().enumerate() {
        loop {
            let entry_type = reader.read_bits(2)?;

            if entry_type == 0 {
                break;
            }

            let name_idx = reader.read_bits(string_bits)?;
            let name = strings.get(name_idx as usize)?;

            if name.eq_ignore_ascii_case("none") || name.is_empty() {
                break;
            }

            let mut fields = HashMap::new();

            match entry_type {
                1 => {
                    // Simple entry - just name
                }
                2 => {
                    // Nested fields - THIS IS WHERE SERIALINDEX IS
                    fields = parse_nested_fields(reader, strings)?;
                }
                3 => {
                    // Reference
                    let ref_idx = reader.read_bits(string_bits)?;
                    let ref_val = strings.get(ref_idx as usize)?;
                    fields.insert("ref".to_string(), FieldValue::String(ref_val.to_string()));
                }
                _ => {}
            }

            all_entries.push(DepEntry {
                dep_table_name: dep_name.clone(),
                dep_table_id: dep_idx,
                name: name.to_string(),
                fields,
            });
        }
    }

    Some(all_entries)
}

/// Parse nested fields (contains serialindex structure)
fn parse_nested_fields(
    reader: &mut BitReader,
    strings: &StringTable,
) -> Option<HashMap<String, FieldValue>> {
    let string_bits = bit_width(strings.len());
    let mut fields = HashMap::new();

    loop {
        let field_idx = reader.read_bits(string_bits)?;
        let field_name = strings.get(field_idx as usize)?;

        if field_name.eq_ignore_ascii_case("none") || field_name.is_empty() {
            break;
        }

        // Special handling for serialindex - it's a nested object
        if field_name == "serialindex" {
            let mut si_obj = HashMap::new();

            // serialindex has 4 fields: status, index, _category, _scope
            for _ in 0..4 {
                let key_idx = reader.read_bits(string_bits)?;
                let key = strings.get(key_idx as usize)?;

                if key.eq_ignore_ascii_case("none") || key.is_empty() {
                    break;
                }

                let val_idx = reader.read_bits(string_bits)?;
                let val = strings.get(val_idx as usize)?;

                si_obj.insert(key.to_string(), val.to_string());
            }

            fields.insert("serialindex".to_string(), FieldValue::Object(si_obj));
        } else {
            // Regular field
            let val_idx = reader.read_bits(string_bits)?;
            let val = strings.get(val_idx as usize)?;
            fields.insert(field_name.to_string(), FieldValue::String(val.to_string()));
        }
    }

    Some(fields)
}

/// Parse single record
pub fn parse_record(
    reader: &mut BitReader,
    strings: &StringTable,
    deps: &[String],
    remap_a: &FixedWidthArray,
) -> Option<Record> {
    // Read 32-bit byte count
    let byte_count = reader.read_bits(32)?;
    let _record_bits = byte_count * 8;

    // Parse tags until 0x7a
    let tags = parse_tags(reader, strings, remap_a)?;

    // Parse entries (2-bit type codes)
    let entries = parse_entries(reader, strings)?;

    // Parse dep_entries if deps exist
    let dep_entries = if !deps.is_empty() {
        parse_dep_entries(reader, strings, deps)?
    } else {
        Vec::new()
    };

    Some(Record {
        tags,
        entries,
        dep_entries,
    })
}

/// Parse full NCS document
pub fn parse_document(data: &[u8], strings: &StringTable, binary_offset: usize) -> Option<Document> {
    let binary_data = &data[binary_offset..];
    let mut reader = BitReader::new(binary_data);

    let string_bits = bit_width(strings.len());

    eprintln!("DEBUG: string_bits={}, strings={}", string_bits, strings.len());

    // CORRECTED: Binary section starts with remap_a, NOT table_id or deps!
    // Deps are in the header, not the binary section
    eprintln!("DEBUG: Parsing remap_a (first thing in binary section)");
    let remap_a = parse_fixed_width_array24(&mut reader)?;
    eprintln!("DEBUG: remap_a count={} width={}", remap_a.count, remap_a.width);

    // TODO: Extract deps from header instead of binary section
    let table_id = String::from("inv"); // From header
    let deps = Vec::new(); // From header (not implemented yet)

    // 4. Parse remap_b
    let remap_b = parse_fixed_width_array24(&mut reader)?;
    eprintln!("DEBUG: remap_b count={} width={}", remap_b.count, remap_b.width);

    // 5. Parse records
    let mut records = Vec::new();
    while reader.has_bits(32) {
        eprintln!("DEBUG: Parsing record {}", records.len());
        match parse_record(&mut reader, strings, &deps, &remap_a) {
            Some(record) => {
                eprintln!("DEBUG:   -> dep_entries={}", record.dep_entries.len());
                records.push(record);
            }
            None => {
                eprintln!("DEBUG:   -> parse failed");
                break;
            }
        }
        if records.len() > 100 {
            eprintln!("DEBUG: Stopping at 100 records");
            break;
        }
    }

    Some(Document {
        table_id,
        deps,
        remap_a,
        remap_b,
        records,
    })
}

/// Extract serial indices from parsed document
pub fn extract_serial_indices(doc: &Document) -> Vec<SerialIndexEntry> {
    let mut entries = Vec::new();

    for record in &doc.records {
        // Get item type from first entry
        let item_type = record
            .entries
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        // Check root-level for serialindex (rare)
        for dep_entry in &record.dep_entries {
            if let Some(FieldValue::Object(si_obj)) = dep_entry.fields.get("serialindex") {
                if let Some(index_str) = si_obj.get("index") {
                    if let Ok(index) = index_str.parse::<u32>() {
                        entries.push(SerialIndexEntry {
                            item_type: item_type.clone(),
                            part_name: dep_entry.name.clone(),
                            index,
                            scope: si_obj.get("_scope").cloned().unwrap_or_else(|| "Unknown".to_string()),
                            category: si_obj.get("_category").cloned().unwrap_or_else(|| "Unknown".to_string()),
                            slot: Some(dep_entry.dep_table_name.clone()),
                        });
                    }
                }
            }
        }
    }

    entries
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialIndexEntry {
    pub item_type: String,
    pub part_name: String,
    pub index: u32,
    pub scope: String,
    pub category: String,
    pub slot: Option<String>,
}
