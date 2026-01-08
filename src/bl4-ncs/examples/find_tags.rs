#!/usr/bin/env rust-script
//! Find tag bytes in binary section

use bl4_ncs::{decompress_ncs, parse_header, find_binary_section_with_count};
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

    // Decompress
    let data = decompress_ncs(&compressed_data).expect("Failed to decompress NCS file");

    // Parse header
    let header = parse_header(&data).expect("Failed to parse header");

    // Find binary offset
    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary section");

    let binary_data = &data[binary_offset..];

    println!("Binary section: {} bytes at offset 0x{:x}\n", binary_data.len(), binary_offset);

    // Search for first 'f' tag (0x66)
    if let Some(pos_f) = binary_data.iter().position(|&b| b == 0x66) {
        println!("First 0x66 ('f') found at position: {} (0x{:x})", pos_f, pos_f);
        println!("Context:");
        let start = pos_f.saturating_sub(10);
        let end = (pos_f + 40).min(binary_data.len());

        for i in (start..end).step_by(16) {
            print!("  {:04x}: ", i);
            for j in 0..16 {
                if i + j < end {
                    print!("{:02x} ", binary_data[i + j]);
                } else {
                    break;
                }
            }
            println!();

            if i <= pos_f && pos_f < i + 16 {
                let offset = pos_f - i;
                println!("        {}^^ tag 'f' here", "   ".repeat(offset));
                if pos_f + 27 < binary_data.len() {
                    let val = binary_data[pos_f + 27];
                    println!("        Value at +27 (pos {}): {} (0x{:02x})", pos_f + 27, val, val);
                }
            }
        }
        println!();
    } else {
        println!("No 0x66 ('f') found in binary section\n");
    }

    // Search for first 'a' tag (0x61)
    if let Some(pos_a) = binary_data.iter().position(|&b| b == 0x61) {
        println!("First 0x61 ('a') found at position: {} (0x{:x})", pos_a, pos_a);
        println!("Context:");
        let start = pos_a.saturating_sub(10);
        let end = (pos_a + 20).min(binary_data.len());

        for i in (start..end).step_by(16) {
            print!("  {:04x}: ", i);
            for j in 0..16 {
                if i + j < end {
                    print!("{:02x} ", binary_data[i + j]);
                } else {
                    break;
                }
            }
            println!();

            if i <= pos_a && pos_a < i + 16 {
                let offset = pos_a - i;
                println!("        {}^^ tag 'a' here", "   ".repeat(offset));
                if pos_a + 5 < binary_data.len() {
                    let val = binary_data[pos_a + 5];
                    println!("        Value at +5 (pos {}): {} (0x{:02x})", pos_a + 5, val, val);
                }
            }
        }
        println!();
    } else {
        println!("No 0x61 ('a') found in binary section\n");
    }

    // Count occurrences
    let count_f = binary_data.iter().filter(|&&b| b == 0x66).count();
    let count_a = binary_data.iter().filter(|&&b| b == 0x61).count();

    println!("Total occurrences:");
    println!("  0x66 ('f'): {} times", count_f);
    println!("  0x61 ('a'): {} times", count_a);
}
