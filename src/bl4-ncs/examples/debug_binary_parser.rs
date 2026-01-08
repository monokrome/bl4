#!/usr/bin/env rust-script
//! Debug why BinaryParser::parse_records returns 0 records

use bl4_ncs::{
    parse_header, parse_ncs_string_table, find_binary_section_with_count,
    BinaryParser, TagValueParser, BitReader, bit_width, decompress_ncs,
};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-inv4.ncs>", args[0]);
        std::process::exit(1);
    }

    let inv_path = &args[1];
    let compressed_data = fs::read(inv_path).expect("Failed to read NCS file");

    println!("=== Binary Parser Debug ===\n");
    println!("File: {}", inv_path);
    println!("Compressed size: {} bytes", compressed_data.len());

    // Decompress NCS file
    println!("\nDecompressing...");
    let data = decompress_ncs(&compressed_data).expect("Failed to decompress NCS file");
    println!("Decompressed size: {} bytes\n", data.len());

    // Step 1: Parse header
    let header = parse_header(&data).expect("Failed to parse header");
    println!("✓ Header parsed:");
    println!("  Type: {}", header.type_name);
    println!("  Format code: {}", header.format_code);
    println!("  String table offset: 0x{:x}", header.string_table_offset);
    println!();

    // Step 2: Parse string table
    let strings = parse_ncs_string_table(&data, &header);
    println!("✓ String table parsed:");
    println!("  Total strings: {}", strings.len());
    if let Some(first_string) = strings.get(0) {
        println!("  First string: {:?}", first_string);
    }
    println!();

    // Step 3: Calculate string index bit width
    let string_bits = bit_width(strings.len() as u32);
    println!("✓ String index bit width: {} bits (for {} strings)", string_bits, strings.len());
    println!();

    // Step 4: Find binary offset (working method)
    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary section");
    println!("✓ Binary section offset (working method):");
    println!("  Offset: 0x{:x} ({} bytes from start)", binary_offset, binary_offset);
    println!("  Remaining: {} bytes", data.len() - binary_offset);
    println!();

    // Step 5: Inspect binary section start
    let binary_data = &data[binary_offset..];
    println!("✓ Binary section first 64 bytes (hex):");
    for i in 0..64.min(binary_data.len()) {
        if i % 16 == 0 {
            print!("  {:04x}: ", i);
        }
        print!("{:02x} ", binary_data[i]);
        if (i + 1) % 16 == 0 {
            println!();
        }
    }
    println!("\n");

    // Step 6: Try to read first record manually
    println!("✓ Manual record parsing attempt:");
    let mut reader = BitReader::new(binary_data);

    println!("  Reading first string index ({} bits)...", string_bits);
    if let Some(name_index) = reader.read_bits(string_bits) {
        println!("  First index: {} (0x{:x})", name_index, name_index);

        if (name_index as usize) < strings.len() {
            let name = strings.get(name_index as usize).unwrap_or("???");
            println!("  String at index {}: {:?}", name_index, name);

            if name.is_empty() || name.eq_ignore_ascii_case("none") {
                println!("  ⚠ WARNING: First string is empty/none - may indicate wrong offset!");
            } else {
                println!("  ✓ First string looks valid!");
            }
        } else {
            println!("  ✗ ERROR: Index {} out of range (max {})", name_index, strings.len() - 1);
            println!("  This means:");
            println!("    - Offset is wrong, OR");
            println!("    - Bit width is wrong, OR");
            println!("    - Data is not bit-packed at this offset");
        }
    } else {
        println!("  ✗ ERROR: Failed to read {} bits", string_bits);
    }
    println!();

    // Step 7: Test BitReader at different bit positions
    println!("✓ Testing BitReader alignment:");
    for bit_offset in [0, 1, 2, 3, 4, 5, 6, 7] {
        let mut reader = BitReader::new(binary_data);

        // Skip bit_offset bits
        if bit_offset > 0 {
            let _ = reader.read_bits(bit_offset);
        }

        if let Some(index) = reader.read_bits(string_bits) {
            if (index as usize) < strings.len() {
                let s = strings.get(index as usize).unwrap_or("???");
                let valid = !s.is_empty() && !s.eq_ignore_ascii_case("none") && s.len() < 100;
                println!("  Bit offset {}: index={:<5} valid={} string={:?}",
                    bit_offset, index, valid, &s[..s.len().min(30)]);
            }
        }
    }
    println!();

    // Step 8: Create binary parser and try to parse
    println!("✓ BinaryParser::parse_records attempt:");
    let parser = BinaryParser::new(&data, &strings, &header.format_code);

    println!("  Calling parse_records(0x{:x})...", binary_offset);
    let records = parser.parse_records(binary_offset);

    println!("  Records parsed: {}", records.len());

    if records.is_empty() {
        println!("  ✗ FAILURE: 0 records parsed!");
        println!("\n  Debugging steps:");
        println!("  1. Check if offset 0x{:x} is correct", binary_offset);
        println!("  2. Verify format code '{}' interpretation", header.format_code);
        println!("  3. Confirm bit alignment (currently at byte boundary)");
        println!("  4. Test with different offsets");
    } else {
        println!("  ✓ SUCCESS: Parsed {} records!", records.len());
        println!("\n  First record:");
        println!("    Name: {}", records[0].name);
        println!("    Fields: {}", records[0].fields.len());
        println!("    Dep entries: {}", records[0].dep_entries.len());
    }
    println!();

    // Step 9: Test TagValueParser (tag-value encoding)
    println!("✓ TagValueParser::parse_records attempt (tag-value encoding):");
    let tag_parser = TagValueParser::new(&data, &strings);

    println!("  Calling parse_records(0x{:x})...", binary_offset);
    let tag_records = tag_parser.parse_records(binary_offset);

    println!("  Records parsed: {}", tag_records.len());

    if tag_records.is_empty() {
        println!("  ✗ FAILURE: 0 records parsed with tag-value parser!");
    } else {
        println!("  ✓ SUCCESS: Parsed {} records with tag-value parser!", tag_records.len());
        println!("\n  First record:");
        println!("    Name: {}", tag_records[0].name);
        println!("    Properties: {}", tag_records[0].properties.len());

        // Show first few properties
        for (i, (key, value)) in tag_records[0].properties.iter().enumerate().take(5) {
            println!("    - {}: {:?}", key, value);
        }
    }
    println!();

    // Step 10: Try different offset (from CLI code)
    println!("✓ Testing alternative offset calculation:");
    let alt_offset = find_strings_end(&data, &strings, header.string_table_offset);
    println!("  Alternative offset: 0x{:x} ({} bytes from start)", alt_offset, alt_offset);
    println!("  Difference from working: {} bytes", (alt_offset as i64 - binary_offset as i64).abs());

    if alt_offset != binary_offset {
        println!("\n  Trying parse with alternative offset...");
        let alt_records = parser.parse_records(alt_offset);
        println!("  Records parsed: {}", alt_records.len());

        if alt_records.len() > 0 {
            println!("  ✓ Alternative offset works! Using 0x{:x}", alt_offset);
        }
    }
}

/// Alternative offset calculation (from CLI code)
fn find_strings_end(data: &[u8], strings: &bl4_ncs::StringTable, string_table_offset: usize) -> usize {
    // Find the last string's position
    let mut pos = string_table_offset;

    // Skip to roughly where strings should end
    // Each string is null-terminated, so count null bytes
    let mut null_count = 0;
    let target_count = strings.len();

    while pos < data.len() && null_count < target_count {
        if data[pos] == 0 {
            null_count += 1;
        }
        pos += 1;
    }

    // Align to 4-byte boundary
    while pos % 4 != 0 && pos < data.len() {
        pos += 1;
    }

    pos
}
