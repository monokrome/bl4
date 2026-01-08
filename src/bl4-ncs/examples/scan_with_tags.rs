#!/usr/bin/env rust-script
//! Use tag-based scanning to extract records

use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count, bit_width, BitReader};
use std::env;
use std::fs;
use std::collections::{HashMap, BTreeMap};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-inv4.ncs>", args[0]);
        std::process::exit(1);
    }

    let inv_path = &args[1];
    let compressed_data = fs::read(inv_path).expect("Failed to read NCS file");

    println!("=== Tag-Based Record Scanner ===\n");

    // Decompress
    let data = decompress_ncs(&compressed_data).expect("Failed to decompress NCS file");

    // Parse header and strings
    let header = parse_header(&data).expect("Failed to parse header");
    let strings = parse_ncs_string_table(&data, &header);

    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary section");

    let binary_data = &data[binary_offset..];
    let string_bits = bit_width(strings.len() as u32);

    println!("Format code: {}", header.format_code);
    println!("Strings: {}", strings.len());
    println!("String bits: {}", string_bits);
    println!("Binary section: {} bytes at 0x{:x}\n", binary_data.len(), binary_offset);

    // Try parsing records by:
    // 1. Reading bit-packed name (15 bits)
    // 2. Reading bit-packed fields until byte-aligned
    // 3. Scanning for tag bytes in remaining data

    let mut reader = BitReader::new(binary_data);
    let mut record_count = 0;

    while reader.has_bits(string_bits as usize) && record_count < 10 {
        let start_pos = reader.position();

        // Read record name
        let name_idx = match reader.read_bits(string_bits) {
            Some(idx) => idx,
            None => break,
        };

        let name = match strings.get(name_idx as usize) {
            Some(s) if !s.is_empty() && !s.eq_ignore_ascii_case("none") => s,
            _ => break,
        };

        println!("Record {}: {:?}", record_count, name);
        println!("  Start bit position: {}", start_pos);
        println!("  After name: bit {}", reader.position());

        // Try to read some bit-packed fields
        // Based on format "abcefhijl":
        // a=name (done), b=U32, c=U32F32

        if let Some(field_b) = reader.read_bits(32) {
            println!("  Field 'b' (U32): {}", field_b);
        }

        if let Some(field_c_bits) = reader.read_bits(32) {
            let as_float = f32::from_bits(field_c_bits);
            println!("  Field 'c' (U32F32): {} / {}", field_c_bits, as_float);
        }

        println!("  After fixed fields: bit {}", reader.position());

        // Align to byte boundary
        reader.align_byte();
        let byte_start = reader.position() / 8;
        println!("  Aligned to byte: {} (bit {})", byte_start, reader.position());

        // Now scan ahead for tag bytes
        let scan_range = 100.min(binary_data.len() - byte_start);
        let scan_data = &binary_data[byte_start..byte_start + scan_range];

        let mut found_tags = Vec::new();

        for (offset, &byte) in scan_data.iter().enumerate() {
            match byte {
                0x61 => found_tags.push(('a', byte_start + offset, byte_start + offset + 5)),
                0x66 => found_tags.push(('f', byte_start + offset, byte_start + offset + 27)),
                _ => {}
            }
        }

        if !found_tags.is_empty() {
            println!("  Found tags in next {} bytes:", scan_range);
            for (tag, tag_pos, val_pos) in found_tags.iter().take(5) {
                if *val_pos < binary_data.len() {
                    let val = binary_data[*val_pos];
                    println!("    '{}' at byte {} -> value at byte {}: {} (0x{:02x})",
                        tag, tag_pos, val_pos, val, val);

                    // Check if it's a valid serial index (1-541)
                    if val >= 1 {
                        println!("      -> Possible serial index: {}", val);
                    }
                }
            }
        }

        println!();

        // Skip ahead for next record (heuristic: 50 bytes)
        // In reality we don't know record boundaries
        let skip_bytes = 50;
        let skip_bits = skip_bytes * 8;
        for _ in 0..skip_bits {
            if reader.read_bit().is_none() {
                break;
            }
        }

        record_count += 1;
    }

    println!("\nScanned {} records", record_count);
}
