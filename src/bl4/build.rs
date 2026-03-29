use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !dir.join("share/manifest").exists() {
        if !dir.pop() {
            panic!(
                "Could not find share/manifest from {}",
                env!("CARGO_MANIFEST_DIR")
            );
        }
    }
    dir.join("share/manifest")
}

fn copy_manifest_file(manifest: &Path, out_dir: &Path, rel_path: &str) {
    let src = manifest.join(rel_path);
    let filename = Path::new(rel_path)
        .file_name()
        .expect("rel_path must have a filename");
    let dest = out_dir.join(filename);

    println!("cargo::rerun-if-changed={}", src.display());

    if src.exists() {
        fs::copy(&src, &dest).unwrap_or_else(|e| panic!("Failed to copy {}: {}", src.display(), e));
    } else {
        fs::write(&dest, "")
            .unwrap_or_else(|e| panic!("Failed to write stub {}: {}", dest.display(), e));
    }
}

fn main() {
    let manifest = manifest_dir();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    copy_manifest_file(&manifest, &out_dir, "category_names.tsv");
    copy_manifest_file(&manifest, &out_dir, "manufacturers.json");
    copy_manifest_file(&manifest, &out_dir, "weapon_types.json");
    copy_manifest_file(&manifest, &out_dir, "drop_pools.tsv");
    copy_manifest_file(&manifest, &out_dir, "part_pools.tsv");
    copy_manifest_file(
        &manifest,
        &out_dir,
        "data_tables/table_bossreplay_costs.tsv",
    );
    copy_manifest_file(&manifest, &out_dir, "item_names.tsv");
    copy_manifest_file(&manifest, &out_dir, "missions/mission_sets.tsv");
    copy_manifest_file(&manifest, &out_dir, "missions/missions.tsv");

    build_parts_database(&manifest, &out_dir);
}

fn build_parts_database(manifest: &Path, out_dir: &Path) {
    let parts_dir = manifest.join("parts");
    let out_path = out_dir.join("parts_database.tsv");

    println!("cargo::rerun-if-changed={}", parts_dir.display());

    let mut entries: Vec<(u32, String, String)> = Vec::new();

    if parts_dir.is_dir() {
        for entry in fs::read_dir(&parts_dir).expect("Failed to read parts directory") {
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
        out.push_str(&format!("{}\t{}\t{}\n", category, line, slot));
    }

    fs::write(&out_path, &out)
        .unwrap_or_else(|e| panic!("Failed to write {}: {}", out_path.display(), e));
}

fn parse_filename(stem: &str) -> Option<(u32, String)> {
    if let Some(pos) = stem.rfind('-') {
        if let Ok(id) = stem[pos + 1..].parse() {
            let slot = &stem[..pos];
            return Some((id, slot.to_string()));
        }
    }
    let id: u32 = stem.parse().ok()?;
    Some((id, "unknown".to_string()))
}
