use std::fs;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let src = Path::new("../../share/manifest/data_tables/table_bossreplay_costs.tsv");

    println!("cargo::rerun-if-changed=../../share/manifest/data_tables/table_bossreplay_costs.tsv");

    let dest = Path::new(&out_dir).join("table_bossreplay_costs.tsv");
    if src.exists() {
        fs::copy(src, &dest).expect("Failed to copy boss replay costs TSV");
    } else {
        fs::write(&dest, "row_name\tcomment\n").expect("Failed to write stub TSV");
    }
}
