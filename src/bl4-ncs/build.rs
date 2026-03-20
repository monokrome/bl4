use std::fs;
use std::path::PathBuf;

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

fn main() {
    let manifest = manifest_dir();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let src = manifest.join("data_tables/table_bossreplay_costs.tsv");
    let dest = out_dir.join("table_bossreplay_costs.tsv");

    println!("cargo::rerun-if-changed={}", src.display());

    if src.exists() {
        fs::copy(&src, &dest).expect("Failed to copy boss replay costs TSV");
    } else {
        fs::write(&dest, "row_name\tcomment\n").expect("Failed to write stub TSV");
    }
}
