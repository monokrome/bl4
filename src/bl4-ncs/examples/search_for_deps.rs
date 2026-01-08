use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, bit_width, BitReader};
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

    println!("String table offset: 0x{:x}", header.string_table_offset);
    println!("Binary offset: 0x7b72e");

    // Check the area BETWEEN string table end and binary section start
    // String table ends around 0x169e7c, binary starts at 0x7b72e
    // Wait, that doesn't make sense - binary should be AFTER strings

    // Actually, let me find where strings end
    let string_bits = bit_width(strings.len() as u32);
    println!("\nString table: {} strings (requires {} bits per index)", strings.len(), string_bits);

    // Check if there's data between control section and binary section
    if let Some(control_offset) = header.control_section_offset {
        println!("\nControl section at: 0x{:x}", control_offset);
        println!("Area between control and binary: 0x{:x} to 0x{:x}",
            control_offset, 0x7b72e);

        let gap_data = &data[control_offset..0x7b72e];
        println!("Gap size: {} bytes", gap_data.len());

        // Try to read as a list of 15-bit indices
        let mut reader = BitReader::new(gap_data);

        // Maybe there's a count first?
        if let Some(count) = reader.read_bits(16) {
            println!("\nFirst 16 bits (potential count): {}", count);

            if count == 39 {
                println!("  -> This matches the 39 deps!");
                println!("\nReading 39 indices as 15-bit values:");
                let mut indices = Vec::new();
                for i in 0..39 {
                    if let Some(idx) = reader.read_bits(15) {
                        indices.push(idx);
                        if let Some(name) = strings.get(idx as usize) {
                            println!("  Dep {}: index {} = {:?}", i, idx, name);
                        } else {
                            println!("  Dep {}: index {} = INVALID", i, idx);
                        }
                    }
                }
            }
        }
    }
}
