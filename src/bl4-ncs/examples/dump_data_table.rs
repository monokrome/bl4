//! Diagnostic: parse gbx_ue_data_table.bin and dump extracted data tables.
//!
//! Usage:
//!   dump_data_table [path] [filter]
//!   dump_data_table --json [path]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let json_mode = args.iter().any(|a| a == "--json");
    let positional: Vec<&str> = args[1..].iter().filter(|a| !a.starts_with('-')).map(|s| s.as_str()).collect();

    let path = positional.first().copied().unwrap_or("share/manifest/ncs/gbx_ue_data_table.bin");
    let filter = positional.get(1).copied();

    let data = std::fs::read(path).expect("Failed to read data table");
    eprintln!("Read {} bytes from {}", data.len(), path);

    let manifest = bl4_ncs::extract_data_tables(&data).expect("Failed to extract data tables");
    eprintln!("{} tables, {} total rows", manifest.len(), manifest.total_rows());

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
        println!("=== {} ({}) ===", table.name, table.rows.len());
        if !table.row_struct.is_empty() {
            println!("  schema: {}", table.row_struct);
        }
        for row in &table.rows {
            print!("  {}", row.row_name);
            if row.fields.is_empty() {
                println!(" (no fields)");
            } else {
                println!();
                let mut field_keys: Vec<&str> = row.fields.keys().map(|s| s.as_str()).collect();
                field_keys.sort();
                for fk in &field_keys {
                    println!("    {}: {}", fk, row.fields[*fk]);
                }
            }
        }
        println!();
    }
}
