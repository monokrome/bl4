use std::fs;
use std::path::Path;

fn main() {
    let parts_dir = Path::new("../../share/manifest/parts");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("parts_database.tsv");

    println!("cargo::rerun-if-changed=../../share/manifest/parts/");

    let mut entries: Vec<(u32, String)> = Vec::new();

    if parts_dir.is_dir() {
        for entry in fs::read_dir(parts_dir).expect("Failed to read parts directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "tsv") {
                let stem = path.file_stem().unwrap().to_str().unwrap();
                let category: u32 = match parse_category_id(stem) {
                    Some(id) => id,
                    None => continue,
                };

                let content = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

                for line in content.lines().skip(1) {
                    if !line.is_empty() {
                        entries.push((category, line.to_string()));
                    }
                }
            }
        }
    }

    entries.sort_by_key(|(cat, _)| *cat);

    let mut out = String::from("category\tindex\tname\n");
    for (category, line) in &entries {
        out.push_str(&format!("{}\t{}\n", category, line));
    }

    fs::write(&out_path, &out)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", out_path.display(), e));
}

/// Extract category ID from a filename stem like "jakobs_pistol-3" or "10001"
fn parse_category_id(stem: &str) -> Option<u32> {
    // Try "{slug}-{id}" format first
    if let Some(pos) = stem.rfind('-') {
        if let Ok(id) = stem[pos + 1..].parse() {
            return Some(id);
        }
    }
    // Fall back to plain numeric
    stem.parse().ok()
}
