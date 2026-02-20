//! Diagnostic tool: dump raw export bytes for assets of a given class.
//!
//! Usage: cargo run --example dump_exports -- <class_name> [max_samples]
//!
//! Reads the BL4 IoStore, finds assets matching the class, and hex-dumps
//! the first 256 bytes of each matching export along with any ASCII strings.

use anyhow::{Context, Result};
use retoc::{
    container_header::EIoContainerHeaderVersion,
    iostore,
    script_objects::FPackageObjectIndexType,
    zen::FZenPackageHeader,
    Config, EIoStoreTocVersion,
};
use std::path::Path;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

const PAK_PATH: &str = "/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks";
const SCRIPTOBJECTS_PATH: &str = "/tmp/scriptobjects.json";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <class_name> [max_samples]", args[0]);
        eprintln!("Example: {} InventoryBodyData 5", args[0]);
        std::process::exit(1);
    }

    let class_name = &args[1];
    let max_samples: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

    eprintln!("Opening IoStore at {PAK_PATH}...");
    let config = Arc::new(Config {
        aes_keys: HashMap::new(),
        container_header_version_override: None,
        toc_version_override: None,
    });
    let store = iostore::open(Path::new(PAK_PATH), config)
        .context("Failed to open IoStore")?;

    let toc_version = store
        .container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store
        .container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    eprintln!("Loading scriptobjects from {SCRIPTOBJECTS_PATH}...");
    let (hash_to_path, name_to_hash) = load_scriptobjects(SCRIPTOBJECTS_PATH)?;

    let target_hash = name_to_hash
        .get(class_name.as_str())
        .with_context(|| format!("Class '{}' not found in scriptobjects ({} classes loaded)", class_name, name_to_hash.len()))?;
    eprintln!("Class '{class_name}' -> hash {target_hash}");

    eprintln!("Scanning for matching assets (max {max_samples})...");
    let mut found = 0usize;

    for chunk in store.chunks() {
        if found >= max_samples {
            break;
        }
        let path = match chunk.path() {
            Some(p) if p.ends_with(".uasset") => p,
            _ => continue,
        };

        let data = match chunk.read() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let mut cursor = Cursor::new(&data);
        let header = match FZenPackageHeader::deserialize(
            &mut cursor,
            None,
            toc_version,
            container_header_version,
            None,
        ) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let header_end = cursor.position() as usize;

        for (ei, export) in header.export_map.iter().enumerate() {
            if export.class_index.kind() != FPackageObjectIndexType::ScriptImport {
                continue;
            }
            let class_hash = format!("{:X}", export.class_index.raw_index());
            if class_hash != *target_hash {
                continue;
            }

            let obj_name = header.name_map.get(export.object_name).to_string();
            let resolved_class = hash_to_path
                .get(&class_hash)
                .map(|p| p.rsplit('.').next().unwrap_or(p).to_string())
                .unwrap_or_else(|| class_hash.clone());

            let offset = header_end + export.cooked_serial_offset as usize;
            let size = export.cooked_serial_size as usize;

            println!("=== {} ===", path);
            println!("  class: {}", resolved_class);
            println!("  export[{}]: {} ({} bytes, offset 0x{:X} in chunk)", ei, obj_name, size, offset);
            println!("  header_end: 0x{:X}, cooked_serial_offset: 0x{:X}", header_end, export.cooked_serial_offset);
            println!("  name_map: {:?}", header.name_map.copy_raw_names());

            if offset + size <= data.len() {
                let export_data = &data[offset..offset + size];
                let dump_len = export_data.len().min(512);
                println!("  hex[0..{}]:", dump_len);
                print_hex(export_data, dump_len);
                println!("  strings: {:?}", find_ascii_strings(export_data, 4));
            } else {
                println!("  ERROR: export data out of bounds (chunk len {})", data.len());
            }
            println!();

            found += 1;
            if found >= max_samples {
                break;
            }
        }
    }

    eprintln!("Found {} matching exports.", found);
    Ok(())
}

fn load_scriptobjects(path: &str) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
    let data = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;

    let hash_to_path: HashMap<String, String> = json
        .get("hash_to_path")
        .and_then(|v| v.as_object())
        .context("Missing hash_to_path")?
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    let mut name_to_hash: HashMap<String, String> = HashMap::new();
    if let Some(objects) = json.get("objects").and_then(|v| v.as_array()) {
        for obj in objects {
            let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let hash = obj.get("hash").and_then(|h| h.as_str()).unwrap_or("");
            if !name.is_empty() && !hash.is_empty() {
                name_to_hash.insert(name.to_string(), hash.to_string());
            }
            if let Some(path) = obj.get("path").and_then(|p| p.as_str()) {
                if let Some(last) = path.rsplit('.').next() {
                    if !name_to_hash.contains_key(last) {
                        name_to_hash.insert(last.to_string(), hash.to_string());
                    }
                }
            }
        }
    }

    Ok((hash_to_path, name_to_hash))
}

fn print_hex(data: &[u8], len: usize) {
    for (i, chunk) in data[..len].chunks(16).enumerate() {
        let offset = i * 16;
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let ascii: String = chunk.iter().map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                *b as char
            } else {
                '.'
            }
        }).collect();

        // Pad hex to full width for alignment
        let hex_str = if chunk.len() < 16 {
            let mut s = hex.join(" ");
            for _ in 0..(16 - chunk.len()) {
                s.push_str("   ");
            }
            s
        } else {
            hex.join(" ")
        };

        println!("    {:04x}: {} |{}|", offset, hex_str, ascii);
    }
}

fn find_ascii_strings(data: &[u8], min_len: usize) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();

    for &b in data {
        if b.is_ascii_graphic() || b == b' ' {
            current.push(b);
        } else {
            if current.len() >= min_len {
                strings.push(String::from_utf8_lossy(&current).to_string());
            }
            current.clear();
        }
    }
    if current.len() >= min_len {
        strings.push(String::from_utf8_lossy(&current).to_string());
    }

    strings
}
