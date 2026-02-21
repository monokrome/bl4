//! Diagnostic: parse gbx_ue_data_table.bin and dump extracted data tables.
//!
//! Usage:
//!   dump_data_table [path] [filter]             # TSV to stdout (default)
//!   dump_data_table --json [path]                # JSON to stdout
//!   dump_data_table --write-dir DIR [path]       # per-table TSVs to DIR/

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let json_mode = args.iter().any(|a| a == "--json");
    let write_dir = args
        .iter()
        .position(|a| a == "--write-dir")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    let positional: Vec<&str> = args[1..]
        .iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| write_dir.map_or(true, |d| a.as_str() != d))
        .map(|s| s.as_str())
        .collect();

    let path = positional
        .first()
        .copied()
        .unwrap_or("share/manifest/ncs/gbx_ue_data_table.bin");
    let filter = positional.get(1).copied();

    let data = std::fs::read(path).expect("Failed to read data table");
    eprintln!("Read {} bytes from {}", data.len(), path);

    let manifest = bl4_ncs::extract_data_tables(&data).expect("Failed to extract data tables");
    eprintln!("{} tables, {} total rows", manifest.len(), manifest.total_rows());

    if let Some(dir) = write_dir {
        bl4_ncs::write_data_tables(&manifest, dir).expect("Failed to write TSVs");
        eprintln!("Wrote {} TSVs to {}", manifest.len(), dir);
        return;
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
        return;
    }

    let mut keys = manifest.keys();
    if let Some(f) = filter {
        keys.retain(|k| k.contains(f));
    }

    for key in &keys {
        let table = manifest.get(key).unwrap();
        eprintln!("--- {} ({} rows) ---", table.name, table.rows.len());
        print!("{}", bl4_ncs::table_to_tsv(table));
    }
}
