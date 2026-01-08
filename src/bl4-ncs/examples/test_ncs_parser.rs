#!/usr/bin/env rust-script
//! Test NcsParser on inv4.ncs

use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count};
use bl4_ncs::ncs_parser::{parse_document, extract_serial_indices};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-inv4.ncs>", args[0]);
        std::process::exit(1);
    }

    let inv_path = &args[1];
    println!("=== NcsParser Test ===\n");

    // Read and decompress
    let compressed_data = fs::read(inv_path).expect("Failed to read NCS file");
    let data = decompress_ncs(&compressed_data).expect("Failed to decompress NCS file");

    println!("Decompressed: {} bytes", data.len());

    // Parse header and strings
    let header = parse_header(&data).expect("Failed to parse header");
    let strings = parse_ncs_string_table(&data, &header);

    println!("Type: {}", header.type_name);
    println!("Format: {}", header.format_code);
    println!("Strings: {}", strings.len());

    // Find binary offset
    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary section");

    println!("Binary offset: 0x{:x}\n", binary_offset);

    // Parse document with NcsParser
    println!("Calling NcsParser::parse_document...");
    match parse_document(&data, &strings, binary_offset) {
        Some(doc) => {
            println!("SUCCESS! Parsed document:");
            println!("  Table ID: {}", doc.table_id);
            println!("  Deps: {}", doc.deps.len());
            println!("  Remap A: {} values (width={})", doc.remap_a.count, doc.remap_a.width);
            println!("  Remap B: {} values (width={})", doc.remap_b.count, doc.remap_b.width);
            println!("  Records: {}", doc.records.len());

            // Extract serial indices
            let serial_indices = extract_serial_indices(&doc);
            println!("\nSerial indices extracted: {}", serial_indices.len());

            if !serial_indices.is_empty() {
                println!("\nFirst 5 serial indices:");
                for (i, entry) in serial_indices.iter().take(5).enumerate() {
                    println!("  {}: {} - index={} ({})",
                        i + 1, entry.part_name, entry.index, entry.item_type);
                }
            }
        }
        None => {
            println!("FAILED to parse document");
        }
    }
}
