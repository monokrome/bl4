#!/usr/bin/env rust-script
//! Examine inv binary section structure

use bl4_ncs::{decompress_ncs, parse_header, bit_width, BitReader};
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

    let binary_offset = 0x7b72e; // We know this from previous tests
    let binary_data = &data[binary_offset..];

    println!("Binary section: {} bytes", binary_data.len());
    println!("\nFirst 128 bytes (hex):");
    for (i, chunk) in binary_data[..128.min(binary_data.len())].chunks(16).enumerate() {
        print!("  {:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        println!();
    }

    println!("\nAs 32-bit little-endian values (first 8):");
    for i in 0..8 {
        let offset = i * 4;
        if offset + 4 <= binary_data.len() {
            let val = u32::from_le_bytes([
                binary_data[offset],
                binary_data[offset + 1],
                binary_data[offset + 2],
                binary_data[offset + 3],
            ]);
            println!("  Offset {}: {} (0x{:08x})", offset, val, val);
        }
    }

    // Try reading as bit-packed with 15-bit indices
    println!("\nTrying to read as 15-bit string indices (first 16):");
    let mut reader = BitReader::new(binary_data);
    for i in 0..16 {
        if let Some(idx) = reader.read_bits(15) {
            println!("  Index {}: {} (bit position {})", i, idx, reader.position());
        } else {
            break;
        }
    }
}
