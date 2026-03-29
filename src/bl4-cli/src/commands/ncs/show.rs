//! NCS show command

use anyhow::{Context, Result};
use bl4_ncs::document::{DepEntry, Document, Entry, Record, Tag, Value};
use bl4_ncs::{decompress_ncs, is_ncs, parse_ncs_binary_from_reader, NcsContent};
use std::fs;
use std::path::Path;

use super::format::output_tsv;
use super::types::FileInfo;
use super::util::print_hex;

pub enum ShowMode {
    Document,
    Raw { all_strings: bool },
    Hex,
    Json,
    Tsv,
}

pub fn show_file(path: &Path, mode: ShowMode) -> Result<()> {
    let data = fs::read(path).context("Failed to read file")?;

    if matches!(mode, ShowMode::Hex) {
        print_hex(&data);
        return Ok(());
    }

    let decompressed = if is_ncs(&data) {
        decompress_ncs(&data).context("Failed to decompress NCS data")?
    } else {
        data
    };

    match &mode {
        ShowMode::Hex => unreachable!(),
        ShowMode::Json => {
            if let Some(doc) =
                parse_ncs_binary_from_reader(&mut std::io::Cursor::new(&decompressed))
            {
                println!("{}", serde_json::to_string_pretty(&doc)?);
                return Ok(());
            }
            show_raw(path, &decompressed, false, true)
        }
        ShowMode::Tsv => {
            if let Some(doc) =
                parse_ncs_binary_from_reader(&mut std::io::Cursor::new(&decompressed))
            {
                output_tsv(&doc);
                return Ok(());
            }
            show_raw(path, &decompressed, false, false)
        }
        ShowMode::Raw { all_strings } => show_raw(path, &decompressed, *all_strings, false),
        ShowMode::Document => {
            if let Some(doc) =
                parse_ncs_binary_from_reader(&mut std::io::Cursor::new(&decompressed))
            {
                print_document(path, &doc);
                return Ok(());
            }
            show_raw(path, &decompressed, false, false)
        }
    }
}

fn show_raw(path: &Path, data: &[u8], all_strings: bool, json: bool) -> Result<()> {
    let content = NcsContent::parse(data).context("Failed to parse NCS content")?;

    let info = FileInfo {
        path: path.to_string_lossy().to_string(),
        type_name: content.type_name().to_string(),
        format_code: content.format_code().to_string(),
        entry_names: if all_strings {
            content.strings.clone()
        } else {
            content.entry_names().map(|s| s.to_string()).collect()
        },
        guids: content.guids().map(|s| s.to_string()).collect(),
        numeric_values: content
            .numeric_values()
            .map(|(s, v)| (s.to_string(), v))
            .collect(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("File: {}", info.path);
        println!("Type: {}", info.type_name);
        println!("Format: {}", info.format_code);

        println!("\nEntry Names ({}):", info.entry_names.len());
        for name in &info.entry_names {
            println!("  - {}", name);
        }

        if !info.guids.is_empty() {
            println!("\nGUIDs ({}):", info.guids.len());
            for guid in &info.guids {
                println!("  - {}", guid);
            }
        }

        if !info.numeric_values.is_empty() {
            println!("\nNumeric Values ({}):", info.numeric_values.len());
            for (s, v) in &info.numeric_values {
                println!("  - {} = {}", s, v);
            }
        }
    }

    Ok(())
}

fn print_document(path: &Path, doc: &Document) {
    println!("File: {}", path.display());
    println!("Tables: {}", doc.tables.len());

    let mut table_names: Vec<&String> = doc.tables.keys().collect();
    table_names.sort();

    for table_name in table_names {
        let table = &doc.tables[table_name];
        println!("\n--- Table: {} ---", table_name);
        if !table.deps.is_empty() {
            println!("  Deps: {}", table.deps.join(", "));
        }
        println!("  Records: {}", table.records.len());

        for (ri, record) in table.records.iter().enumerate() {
            print_record(ri, record);
        }
    }
}

fn print_record(index: usize, record: &Record) {
    println!("\n  Record {}:", index);
    print_tags(&record.tags);

    for entry in &record.entries {
        print_entry(entry, 4);
    }
}

fn print_tags(tags: &[Tag]) {
    if tags.is_empty() {
        return;
    }
    print!("    Tags:");
    for tag in tags {
        match tag {
            Tag::KeyName { pair } => print!(" a={}", pair),
            Tag::U32 { value } => print!(" b={}", value),
            Tag::F32 {
                u32_value,
                f32_value,
            } => print!(" c={}({:.4})", u32_value, f32_value),
            Tag::NameListD { list } => print!(" d=[{}]", list.join(", ")),
            Tag::NameListE { list } => print!(" e=[{}]", list.join(", ")),
            Tag::NameListF { list } => print!(" f=[{}]", list.join(", ")),
            Tag::Variant { variant } => print!(" p={}", format_value_inline(variant)),
        }
    }
    println!();
}

fn print_entry(entry: &Entry, indent: usize) {
    let pad = " ".repeat(indent);
    let serial_index = extract_serialindex(&entry.value);
    let formatted = format_value_expanded(&entry.value, indent + 2);

    if let Some(idx) = serial_index {
        println!("{}[{}] {} = {}", pad, idx, entry.key, formatted);
    } else {
        println!("{}{} = {}", pad, entry.key, formatted);
    }

    for dep in &entry.dep_entries {
        print_dep_entry(dep, indent + 2);
    }
}

fn print_dep_entry(dep: &DepEntry, indent: usize) {
    let pad = " ".repeat(indent);
    let serial_index = extract_serialindex(&dep.value);

    if let Some(idx) = serial_index {
        if dep.dep_table_name.is_empty() {
            println!("{}  [{:>3}] {}", pad, idx, dep.key);
        } else {
            println!(
                "{}  [{:>3}] {} (dep: {})",
                pad, idx, dep.key, dep.dep_table_name
            );
        }
    } else if dep.dep_table_name.is_empty() {
        println!("{}  {}", pad, dep.key);
    } else {
        println!("{}  {} (dep: {})", pad, dep.key, dep.dep_table_name);
    }
}

fn extract_serialindex(value: &Value) -> Option<u32> {
    if let Value::Map(map) = value {
        if let Some(Value::Leaf(s)) = map.get("serialindex") {
            return s.parse().ok();
        }
    }
    None
}

fn format_value_expanded(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Leaf(s) => s.clone(),
        Value::Ref { r#ref } => format!("-> {}", r#ref),
        Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| format_value_expanded(v, indent + 2))
                .collect();
            format!("[{}]", items.join(", "))
        }
        Value::Map(map) => {
            let pad = " ".repeat(indent);
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let pairs: Vec<String> = keys
                .iter()
                .map(|k| {
                    let v = &map[*k];
                    let formatted = format_value_expanded(v, indent + 2);
                    format!("{}{}: {}", pad, k, formatted)
                })
                .collect();
            format!("{{\n{}\n{}}}", pairs.join("\n"), " ".repeat(indent.saturating_sub(2)))
        }
    }
}

fn format_value_inline(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Leaf(s) => s.clone(),
        Value::Ref { r#ref } => format!("-> {}", r#ref),
        Value::Array(arr) => format!("[{}]", arr.len()),
        Value::Map(map) => format!("{{{} keys}}", map.len()),
    }
}
