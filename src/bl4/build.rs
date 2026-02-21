use std::fs;
use std::path::Path;

fn main() {
    let parts_dir = Path::new("../../share/manifest/parts");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("parts_database.tsv");

    println!("cargo::rerun-if-changed=../../share/manifest/parts/");

    let mut entries: Vec<(u32, String, String)> = Vec::new();

    if parts_dir.is_dir() {
        for entry in fs::read_dir(parts_dir).expect("Failed to read parts directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "tsv") {
                let stem = path.file_stem().unwrap().to_str().unwrap();
                let (category, slot) = match parse_filename(stem) {
                    Some(pair) => pair,
                    None => continue,
                };

                let content = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

                for line in content.lines().skip(1) {
                    if !line.is_empty() {
                        entries.push((category, slot.clone(), line.to_string()));
                    }
                }
            }
        }
    }

    entries.sort_by_key(|(cat, _, _)| *cat);

    let mut out = String::from("category\tindex\tname\tslot\n");
    for (category, slot, line) in &entries {
        // line is "index\tname", append slot as 4th column
        out.push_str(&format!("{}\t{}\t{}\n", category, line, slot));
    }

    fs::write(&out_path, &out)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", out_path.display(), e));
}

/// Extract category ID and slot name from a filename stem.
///
/// Formats: `"barrel-10001"` → `(10001, "barrel")`, `"stat_group2-10001"` → `(10001, "stat_group2")`
fn parse_filename(stem: &str) -> Option<(u32, String)> {
    if let Some(pos) = stem.rfind('-') {
        if let Ok(id) = stem[pos + 1..].parse() {
            let slot = &stem[..pos];
            return Some((id, slot.to_string()));
        }
    }
    // Plain numeric filenames have no slot info
    let id: u32 = stem.parse().ok()?;
    Some((id, "unknown".to_string()))
}
