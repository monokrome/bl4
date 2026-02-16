//! NCS debug command

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::util::print_hex;

#[allow(clippy::cognitive_complexity)]
pub fn debug_file(path: &Path, show_hex: bool, do_parse: bool, _show_offsets: bool) -> Result<()> {
    use bl4_ncs::{parse_ncs_binary, NcsContent};

    let data = fs::read(path).context("Failed to read file")?;
    println!("File: {}", path.display());
    println!("Size: {} bytes", data.len());

    // Basic content info
    if let Some(content) = NcsContent::parse(&data) {
        println!("\nType: {}", content.type_name());
        println!("Format: {}", content.format_code());
        println!("Strings: {}", content.strings.len());

        if show_hex {
            println!("\nFirst 64 bytes:");
            print_hex(&data[..data.len().min(64)]);
        }

        println!("\nFirst 20 strings:");
        for (i, s) in content.strings.iter().enumerate().take(20) {
            println!("  {:3}: {}", i, s);
        }
        if content.strings.len() > 20 {
            println!("  ... and {} more", content.strings.len() - 20);
        }
    }

    // New pipeline parse
    if do_parse {
        println!("\n=== New Pipeline Parse ===");

        // Blob header
        if let Some(blob) = bl4_ncs::parse::blob::BlobHeader::parse(&data) {
            println!(
                "BlobHeader: entry_count={}, flags={}, string_bytes={}",
                blob.entry_count, blob.flags, blob.string_bytes
            );

            let header_strings = bl4_ncs::parse::blob::extract_header_strings(&data, &blob);
            println!(
                "Header strings ({}): {:?}",
                header_strings.len(),
                &header_strings[..header_strings.len().min(10)]
            );

            let body_offset = blob.body_offset();
            if body_offset < data.len() {
                let body = &data[body_offset..];

                // Type code table
                if let Some(tct) = bl4_ncs::parse::typecodes::parse_type_code_table(body) {
                    println!("\nTypeCodeTable:");
                    println!("  type_codes: {:?}", tct.header.type_codes);
                    println!("  type_index_count: {}", tct.header.type_index_count);
                    println!(
                        "  value_strings: {} (declared {})",
                        tct.value_strings.len(),
                        tct.value_strings_declared_count
                    );
                    println!(
                        "  value_kinds: {} (declared {})",
                        tct.value_kinds.len(),
                        tct.value_kinds_declared_count
                    );
                    println!(
                        "  key_strings: {} (declared {})",
                        tct.key_strings.len(),
                        tct.key_strings_declared_count
                    );
                    println!(
                        "  row_flags: {:?}",
                        &tct.header.row_flags[..tct.header.row_flags.len().min(10)]
                    );
                    println!("  data_offset: {}", tct.data_offset);
                }
            }
        }

        // Full document parse
        match parse_ncs_binary(&data) {
            Some(doc) => {
                println!("\nParsed {} tables:", doc.tables.len());
                for (name, table) in &doc.tables {
                    let total_entries: usize =
                        table.records.iter().map(|r| r.entries.len()).sum();
                    println!(
                        "  '{}': {} deps, {} records, {} entries",
                        name,
                        table.deps.len(),
                        table.records.len(),
                        total_entries
                    );
                    if !table.deps.is_empty() {
                        println!("    deps: {:?}", table.deps);
                    }
                }

                // Show first few entries from first table
                if let Some((name, table)) = doc.tables.iter().next() {
                    println!("\nFirst entries from '{}':", name);
                    for (ri, record) in table.records.iter().take(3).enumerate() {
                        for (ei, entry) in record.entries.iter().take(5).enumerate() {
                            println!("  record[{}].entry[{}]: key={:?}", ri, ei, entry.key);
                        }
                    }
                }
            }
            None => {
                println!("Failed to parse with new pipeline");
            }
        }
    }

    Ok(())
}
