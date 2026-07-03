//! List classes command

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use retoc::{
    container_header::EIoContainerHeaderVersion, iostore, zen::FZenPackageHeader, AesKey, Config,
    EIoStoreTocVersion, FGuid,
};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// List all unique class hashes found in pak files
#[allow(clippy::too_many_lines)]
pub fn list_classes(
    input: &Path,
    scriptobjects_path: &Path,
    aes_key: Option<&str>,
    samples: usize,
) -> Result<()> {
    use retoc::script_objects::FPackageObjectIndexType;

    // Load scriptobjects for name resolution
    let so_data = std::fs::read_to_string(scriptobjects_path)
        .with_context(|| format!("Failed to read scriptobjects file {:?}", scriptobjects_path))?;
    let so_json: serde_json::Value = serde_json::from_str(&so_data)?;

    let hash_to_path: HashMap<String, String> = so_json
        .get("hash_to_path")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Build retoc config
    let mut aes_keys = HashMap::new();
    if let Some(key) = aes_key {
        let parsed_key: AesKey = key.parse()?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    // Open IoStore
    let store = iostore::open(input, config)?;

    // Get container versions
    let toc_version = store
        .container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store
        .container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    // Scan all .uasset files
    let uasset_entries: Vec<_> = store
        .chunks()
        .filter_map(|chunk| {
            chunk.path().and_then(|path| {
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
        })
        .collect();

    eprintln!("Scanning {} .uasset files...", uasset_entries.len());

    // Collect classes: hash -> (class_name, count, sample_paths)
    type ClassInfo = (String, usize, Vec<String>);
    let class_map: Arc<Mutex<BTreeMap<String, ClassInfo>>> = Arc::new(Mutex::new(BTreeMap::new()));

    let pb = ProgressBar::new(uasset_entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("#>-"),
    );

    uasset_entries.par_iter().for_each(|(chunk, path)| {
        pb.inc(1);

        if let Ok(data) = chunk.read() {
            let mut cursor = Cursor::new(&data);
            if let Ok(header) = FZenPackageHeader::deserialize(
                &mut cursor,
                None,
                toc_version,
                container_header_version,
                None,
            ) {
                for export in &header.export_map {
                    if export.class_index.kind() == FPackageObjectIndexType::ScriptImport {
                        let class_hash = format!("{:X}", export.class_index.raw_index());
                        let mut map = class_map.lock().unwrap();
                        let entry = map.entry(class_hash.clone()).or_insert_with(|| {
                            let name = hash_to_path
                                .get(&class_hash)
                                .cloned()
                                .unwrap_or_else(|| "UNKNOWN".to_string());
                            (name, 0, Vec::new())
                        });
                        entry.1 += 1;
                        if entry.2.len() < samples {
                            entry.2.push(path.clone());
                        }
                    }
                }
            }
        }
    });

    pb.finish_and_clear();

    // Print results sorted by count
    let map = class_map.lock().unwrap();
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|b| std::cmp::Reverse(b.1 .1));

    eprintln!("\n{} unique class types found:", entries.len());
    println!("{:<20} {:<60} Count", "Hash", "Class Name");
    println!("{:-<100}", "");

    for (hash, (name, count, sample_paths)) in entries {
        println!("{:<20} {:<60} {}", hash, name, count);
        for path in sample_paths {
            println!("  -> {}", path);
        }
    }

    Ok(())
}
