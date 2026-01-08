use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count, bit_width, BitReader};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-inv4.ncs>", args[0]);
        std::process::exit(1);
    }

    let compressed = fs::read(&args[1]).expect("Failed to read");
    let data = decompress_ncs(&compressed).expect("Failed to decompress");
    let header = parse_header(&data).expect("Failed to parse header");
    let strings = parse_ncs_string_table(&data, &header);

    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary");

    println!("Binary section starts at: 0x{:x}", binary_offset);
    println!("String table has: {} strings", strings.len());

    let binary_data = &data[binary_offset..];
    let string_bits = bit_width(strings.len() as u32);

    println!("\nTrying to parse deps from start of binary section:");
    println!("(Expecting 39 deps as 15-bit string indices)\n");

    let mut reader = BitReader::new(binary_data);

    // Try different structures:
    // 1. Direct list of 39 indices (no count)
    println!("=== Attempt 1: Direct list of 39 indices ===");
    for i in 0..39 {
        if let Some(idx) = reader.read_bits(string_bits) {
            if let Some(name) = strings.get(idx as usize) {
                println!("  Dep {:2}: index {:5} = {}", i, idx, name);
            } else {
                println!("  Dep {:2}: index {:5} = INVALID", i, idx);
                break;
            }
        }
    }

    // Reset and try with count prefix
    reader = BitReader::new(binary_data);

    println!("\n=== Attempt 2: 8-bit count + indices ===");
    if let Some(count) = reader.read_bits(8) {
        println!("Count: {}", count);
        if count == 39 {
            for i in 0..count {
                if let Some(idx) = reader.read_bits(string_bits) {
                    if let Some(name) = strings.get(idx as usize) {
                        println!("  Dep {:2}: index {:5} = {}", i, idx, name);
                    }
                }
            }
        }
    }

    // Reset and try 16-bit count
    reader = BitReader::new(binary_data);

    println!("\n=== Attempt 3: 16-bit count + indices ===");
    if let Some(count) = reader.read_bits(16) {
        println!("Count: {}", count);
        if count == 39 {
            for i in 0..count {
                if let Some(idx) = reader.read_bits(string_bits) {
                    if let Some(name) = strings.get(idx as usize) {
                        println!("  Dep {:2}: index {:5} = {}", i, idx, name);
                    }
                }
            }
        }
    }

    // Reset and try 32-bit count
    reader = BitReader::new(binary_data);

    println!("\n=== Attempt 4: 32-bit count + indices ===");
    if let Some(count) = reader.read_bits(32) {
        println!("Count: {}", count);
        if count == 39 {
            for i in 0..count {
                if let Some(idx) = reader.read_bits(string_bits) {
                    if let Some(name) = strings.get(idx as usize) {
                        println!("  Dep {:2}: index {:5} = {}", i, idx, name);
                    }
                }
            }
        }
    }
}
