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

    println!("String table: {} to 0x{:x}", header.string_table_offset, binary_offset);
    println!("Binary section: 0x{:x} onwards", binary_offset);
    println!("Gap size: {} bytes", binary_offset - header.string_table_offset);

    // The gap contains string data, but maybe there's a deps list at the end?
    // Let's look at the last 1000 bytes before binary section
    let pre_binary_start = (binary_offset - 1000).max(header.string_table_offset);
    let pre_binary_data = &data[pre_binary_start..binary_offset];

    println!("\nLast {} bytes before binary section:", pre_binary_data.len());
    println!("Hex dump:");
    for (i, chunk) in pre_binary_data.chunks(16).enumerate() {
        print!("  {:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        println!();
    }

    // Try reading from various offsets as a count + list structure
    for test_offset in (0..pre_binary_data.len()).step_by(4).rev().take(20) {
        let test_data = &pre_binary_data[test_offset..];
        let mut reader = BitReader::new(test_data);

        // Try 16-bit count
        if let Some(count) = reader.read_bits(16) {
            if count == 39 {
                println!("\nFound count=39 at offset -0x{:x} from binary start!", pre_binary_data.len() - test_offset);
                println!("Absolute offset: 0x{:x}", pre_binary_start + test_offset);

                println!("\nReading 39 string indices:");
                for i in 0..39 {
                    if let Some(idx) = reader.read_bits(15) {
                        if let Some(name) = strings.get(idx as usize) {
                            println!("  Dep {:2}: index {:5} = {}", i, idx, name);
                        } else {
                            println!("  Dep {:2}: index {:5} = INVALID (max {})", i, idx, strings.len() - 1);
                            break;
                        }
                    }
                }
                break;
            }
        }
    }
}
