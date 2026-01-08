use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count};
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

    println!("Binary section starts at: 0x{:x} ({} bytes)", binary_offset, binary_offset);
    println!("String table: {} strings", strings.len());

    // Examine last 256 bytes before binary section
    let pre_binary_start = (binary_offset - 256).max(0);
    let pre_binary_data = &data[pre_binary_start..binary_offset];

    println!("\nLast 256 bytes before binary section:");
    println!("Offset: 0x{:x} - 0x{:x}", pre_binary_start, binary_offset);
    println!();

    // Hex dump
    for (i, chunk) in pre_binary_data.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        println!();
    }

    println!("\nLooking for patterns:");

    // Check for 39 (0x27) - number of deps
    if let Some(pos) = pre_binary_data.iter().position(|&b| b == 39) {
        println!("  Found 0x27 (39 decimal) at offset -{}", pre_binary_data.len() - pos);
    }

    // Check for format code characters
    let format_chars = b"abcefhijl";
    for (i, window) in pre_binary_data.windows(format_chars.len()).enumerate() {
        if window == format_chars {
            println!("  Found format code 'abcefhijl' at offset -{}", pre_binary_data.len() - i);
        }
    }

    // Check for 6 (number of records)
    if let Some(pos) = pre_binary_data.iter().position(|&b| b == 6) {
        println!("  Found 0x06 (6 records) at offset -{}", pre_binary_data.len() - pos);
    }

    // Look for patterns that might be counts or sizes
    println!("\nPotential 16-bit counts:");
    for i in (0..pre_binary_data.len() - 1).step_by(2) {
        let val = u16::from_le_bytes([pre_binary_data[i], pre_binary_data[i + 1]]);
        if val == 6 || val == 39 || val == 539 || val == 204 {
            println!("  {} at offset -{} (bytes: {:02x} {:02x})",
                val, pre_binary_data.len() - i, pre_binary_data[i], pre_binary_data[i + 1]);
        }
    }
}
