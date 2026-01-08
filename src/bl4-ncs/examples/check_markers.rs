use bl4_ncs::{decompress_ncs};
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

    let binary_offset: usize = 0x7b72e;

    // Check offset -252 (39 decimal)
    let pos_39: usize = binary_offset - 252;
    println!("At offset 0x{:x} (binary - 252): Context around 39 (0x27)", pos_39);
    let context = &data[pos_39.saturating_sub(10)..pos_39+10];
    print!("  Bytes: ");
    for b in context {
        print!("{:02x} ", b);
    }
    println!();
    println!("  Value at position: {} (0x{:02x})", data[pos_39], data[pos_39]);

    // Check offset -129 (6 records)
    let pos_6: usize = binary_offset - 129;
    println!("\nAt offset 0x{:x} (binary - 129): Context around 6 (0x06)", pos_6);
    let context = &data[pos_6.saturating_sub(10)..pos_6+10];
    print!("  Bytes: ");
    for b in context {
        print!("{:02x} ", b);
    }
    println!();
    println!("  Value at position: {} (0x{:02x})", data[pos_6], data[pos_6]);

    // Also check if these are part of null-terminated strings
    println!("\n Checking if part of strings:");

    // Look backward from pos_39 for null terminator
    let mut start = pos_39;
    while start > 0 && data[start - 1] != 0 {
        start -= 1;
    }
    if start < pos_39 {
        if let Ok(s) = std::str::from_utf8(&data[start..pos_39 + 20]) {
            if let Some(null_pos) = s.find('\0') {
                println!("  Near 39: \"{}\"", &s[..null_pos]);
            }
        }
    }

    // Look backward from pos_6 for null terminator
    let mut start = pos_6;
    while start > 0 && data[start - 1] != 0 {
        start -= 1;
    }
    if start < pos_6 {
        if let Ok(s) = std::str::from_utf8(&data[start..pos_6 + 20]) {
            if let Some(null_pos) = s.find('\0') {
                println!("  Near 6: \"{}\"", &s[..null_pos]);
            }
        }
    }
}
