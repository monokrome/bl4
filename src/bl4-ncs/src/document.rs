//! Unified document types for parsed NCS data
//!
//! These types represent the output of the NCS table data decoder.
//! Each NCS file contains one or more named tables, each containing
//! records with entries and optional dependency entries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed NCS document containing all tables from a single NCS file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub tables: HashMap<String, Table>,
}

/// A single table with dependency references and records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub deps: Vec<String>,
    pub records: Vec<Record>,
}

/// A record containing entries decoded from the binary section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    pub entries: Vec<Entry>,
}

/// An entry with a key, fields map, and optional dependency entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub key: String,
    pub value: Value,
    pub dep_entries: Vec<DepEntry>,
}

/// A dependency entry linking to another table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepEntry {
    pub dep_table_name: String,
    pub dep_index: u32,
    pub key: String,
    pub value: Value,
}

/// Value types produced by decode_node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Leaf(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Ref { r#ref: String },
}

/// Record tag metadata from the tags section preceding entries
///
/// Tags carry per-record metadata like key names, numeric values,
/// name lists, and inline variant nodes. Tags are stored separately
/// from entries for round-trip fidelity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__tag")]
pub enum Tag {
    #[serde(rename = "a")]
    KeyName { pair: String },
    #[serde(rename = "b")]
    U32 { value: u32 },
    #[serde(rename = "c")]
    F32 { u32_value: u32, f32_value: f32 },
    #[serde(rename = "d")]
    NameListD { list: Vec<String> },
    #[serde(rename = "e")]
    NameListE { list: Vec<String> },
    #[serde(rename = "f")]
    NameListF { list: Vec<String> },
    #[serde(rename = "p")]
    Variant { variant: Value },
}

/// Serial index entry extracted from parsed data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialIndexEntry {
    pub table_name: String,
    pub dep_table: String,
    pub part_name: String,
    pub index: u32,
}

/// A part entry with category context for the parts database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizedPart {
    pub category: u32,
    pub index: u32,
    pub name: String,
}

/// Extract parts grouped by category from a parsed document
///
/// Each entry with a serialindex defines a category (the serialindex IS the
/// category ID). The entry's dep_entries are the parts in that category,
/// with each dep_entry's serialindex as the part index.
pub fn extract_categorized_parts(doc: &Document) -> Vec<CategorizedPart> {
    let mut results = Vec::new();

    for table in doc.tables.values() {
        for record in &table.records {
            for entry in &record.entries {
                let Some(category) = extract_index_from_value(&entry.value) else {
                    continue;
                };

                for dep_entry in &entry.dep_entries {
                    if let Some(index) = extract_index_from_value(&dep_entry.value) {
                        results.push(CategorizedPart {
                            category,
                            index,
                            name: dep_entry.key.clone(),
                        });
                    }
                }
            }
        }
    }

    results
}

/// Extract category ID → NCS entry key name mapping from a parsed document
///
/// Only includes entries that have dep_entries with serial indices (actual
/// parts), avoiding cosmetic/metadata entries that reuse the same ID space.
/// First-seen name wins when multiple entries share a category ID.
pub fn extract_category_names(doc: &Document) -> HashMap<u32, String> {
    let mut names = HashMap::new();

    for table in doc.tables.values() {
        for record in &table.records {
            for entry in &record.entries {
                let Some(category) = extract_index_from_value(&entry.value) else {
                    continue;
                };

                let has_parts = entry.dep_entries.iter().any(|de| {
                    extract_index_from_value(&de.value).is_some()
                });
                if !has_parts {
                    continue;
                }

                names.entry(category).or_insert_with(|| entry.key.clone());
            }
        }
    }

    names
}

/// Extract serial indices from a parsed document
///
/// Looks for entries and dep_entries containing "serialindex" fields
/// with numeric index values.
pub fn extract_serial_indices(doc: &Document) -> Vec<SerialIndexEntry> {
    let mut results = Vec::new();

    for (table_name, table) in &doc.tables {
        for record in &table.records {
            for entry in &record.entries {
                if let Some(index) = extract_index_from_value(&entry.value) {
                    results.push(SerialIndexEntry {
                        table_name: table_name.clone(),
                        dep_table: String::new(),
                        part_name: entry.key.clone(),
                        index,
                    });
                }

                for dep_entry in &entry.dep_entries {
                    if let Some(index) = extract_index_from_value(&dep_entry.value) {
                        results.push(SerialIndexEntry {
                            table_name: table_name.clone(),
                            dep_table: dep_entry.dep_table_name.clone(),
                            part_name: dep_entry.key.clone(),
                            index,
                        });
                    }
                }
            }
        }
    }

    results
}

fn extract_index_from_value(value: &Value) -> Option<u32> {
    match value {
        Value::Map(map) => {
            if let Some(si_value) = map.get("serialindex") {
                return extract_index_from_serialindex(si_value);
            }
            for v in map.values() {
                if let Some(idx) = extract_index_from_value(v) {
                    return Some(idx);
                }
            }
            None
        }
        Value::Array(arr) => {
            for v in arr {
                if let Some(idx) = extract_index_from_value(v) {
                    return Some(idx);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_index_from_serialindex(value: &Value) -> Option<u32> {
    match value {
        Value::Map(map) => {
            if let Some(Value::Leaf(idx_str)) = map.get("index") {
                let clean = if let Some(pos) = idx_str.find('\'') {
                    let end = idx_str.rfind('\'').unwrap_or(idx_str.len());
                    &idx_str[pos + 1..end]
                } else {
                    idx_str.as_str()
                };
                clean.parse().ok()
            } else {
                None
            }
        }
        Value::Leaf(s) => {
            let clean = if let Some(pos) = s.find('\'') {
                let end = s.rfind('\'').unwrap_or(s.len());
                &s[pos + 1..end]
            } else {
                s.as_str()
            };
            clean.parse().ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_serialization() {
        let leaf = Value::Leaf("hello".to_string());
        let json = serde_json::to_string(&leaf).unwrap();
        assert_eq!(json, "\"hello\"");

        let null = Value::Null;
        let json = serde_json::to_string(&null).unwrap();
        assert_eq!(json, "null");

        let arr = Value::Array(vec![
            Value::Leaf("a".to_string()),
            Value::Leaf("b".to_string()),
        ]);
        let json = serde_json::to_string(&arr).unwrap();
        assert_eq!(json, "[\"a\",\"b\"]");
    }

    #[test]
    fn test_extract_serial_index() {
        let mut si_map = HashMap::new();
        si_map.insert("index".to_string(), Value::Leaf("42".to_string()));
        si_map.insert("status".to_string(), Value::Leaf("Active".to_string()));

        let mut entry_map = HashMap::new();
        entry_map.insert("serialindex".to_string(), Value::Map(si_map));

        let result = extract_index_from_value(&Value::Map(entry_map));
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_extract_typed_serial_index() {
        let mut si_map = HashMap::new();
        si_map.insert("index".to_string(), Value::Leaf("int'237'".to_string()));

        let mut entry_map = HashMap::new();
        entry_map.insert("serialindex".to_string(), Value::Map(si_map));

        let result = extract_index_from_value(&Value::Map(entry_map));
        assert_eq!(result, Some(237));
    }

    #[test]
    fn test_tag_serialization() {
        let tag_a = Tag::KeyName {
            pair: "test_key".to_string(),
        };
        let json = serde_json::to_string(&tag_a).unwrap();
        assert!(json.contains("\"__tag\":\"a\""));
        assert!(json.contains("\"pair\":\"test_key\""));

        let tag_b = Tag::U32 { value: 42 };
        let json = serde_json::to_string(&tag_b).unwrap();
        assert!(json.contains("\"__tag\":\"b\""));
        assert!(json.contains("\"value\":42"));

        let tag_c = Tag::F32 {
            u32_value: 1065353216,
            f32_value: 1.0,
        };
        let json = serde_json::to_string(&tag_c).unwrap();
        assert!(json.contains("\"__tag\":\"c\""));
        assert!(json.contains("\"u32_value\":1065353216"));
        assert!(json.contains("\"f32_value\":1.0"));

        let tag_d = Tag::NameListD {
            list: vec!["name1".to_string(), "name2".to_string()],
        };
        let json = serde_json::to_string(&tag_d).unwrap();
        assert!(json.contains("\"__tag\":\"d\""));
        assert!(json.contains("\"list\":[\"name1\",\"name2\"]"));
    }

    #[test]
    fn test_tag_deserialization_roundtrip() {
        let tags = vec![
            Tag::KeyName {
                pair: "test".to_string(),
            },
            Tag::U32 { value: 99 },
            Tag::F32 {
                u32_value: 0x3F800000,
                f32_value: 1.0,
            },
            Tag::NameListD {
                list: vec!["a".to_string(), "b".to_string()],
            },
            Tag::NameListE {
                list: vec!["x".to_string()],
            },
            Tag::NameListF { list: vec![] },
            Tag::Variant {
                variant: Value::Leaf("val".to_string()),
            },
        ];

        for tag in &tags {
            let json = serde_json::to_string(tag).unwrap();
            let roundtrip: Tag = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&roundtrip).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_value_deserialization_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Leaf("hello".to_string()),
            Value::Array(vec![Value::Leaf("a".to_string()), Value::Null]),
            Value::Ref {
                r#ref: "some_ref".to_string(),
            },
        ];

        for val in &values {
            let json = serde_json::to_string(val).unwrap();
            let roundtrip: Value = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&roundtrip).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_extract_serial_indices_from_dep_entries() {
        let mut si_map = HashMap::new();
        si_map.insert("index".to_string(), Value::Leaf("5".to_string()));

        let doc = Document {
            tables: HashMap::from([(
                "test_table".to_string(),
                Table {
                    name: "test_table".to_string(),
                    deps: vec!["dep_table".to_string()],
                    records: vec![Record {
                        tags: vec![],
                        entries: vec![Entry {
                            key: "main_key".to_string(),
                            value: Value::Null,
                            dep_entries: vec![DepEntry {
                                dep_table_name: "dep_table".to_string(),
                                dep_index: 0,
                                key: "dep_key".to_string(),
                                value: Value::Map({
                                    let mut m = HashMap::new();
                                    m.insert("serialindex".to_string(), Value::Map(si_map.clone()));
                                    m
                                }),
                            }],
                        }],
                    }],
                },
            )]),
        };

        let indices = extract_serial_indices(&doc);
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0].part_name, "dep_key");
        assert_eq!(indices[0].index, 5);
        assert_eq!(indices[0].dep_table, "dep_table");
    }

    #[test]
    fn test_record_tags_skip_empty() {
        let record = Record {
            tags: vec![],
            entries: vec![],
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("tags"), "empty tags should be omitted");

        let record_with_tags = Record {
            tags: vec![Tag::U32 { value: 1 }],
            entries: vec![],
        };
        let json = serde_json::to_string(&record_with_tags).unwrap();
        assert!(json.contains("\"tags\""), "non-empty tags should be present");
    }
}
