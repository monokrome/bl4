use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count, BitReader};
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

    let binary_data = &data[binary_offset..];
    let mut reader = BitReader::new(binary_data);

    println!("=== Testing Tag Alignment Strategies ===\n");

    // Read known good fields
    let name_idx = reader.read_bits(15).expect("name");
    let field_b = reader.read_bits(32).expect("field_b");
    let field_c = reader.read_bits(32).expect("field_c");

    println!("Bit position 0-14: name index = {} ({})", name_idx,
        strings.get(name_idx as usize).unwrap_or(&String::from("INVALID")));
    println!("Bit position 15-46: field_b (U32) = {}", field_b);
    println!("Bit position 47-78: field_c (U32F32) = {} / {}", field_c, f32::from_bits(field_c));
    println!("\nCurrent position: bit {}", reader.position());
    println!("Current byte position: byte {}, bit offset {}", reader.position() / 8, reader.position() % 8);

    // Strategy 1: Read without alignment (bit 79)
    println!("\n--- Strategy 1: Read 8 bits at position 79 (no alignment) ---");
    let tag1 = reader.read_bits(8).expect("tag1");
    println!("  Tag byte: 0x{:02x} ({})", tag1, (tag1 as u8) as char);
    println!("  Position after: bit {}", reader.position());

    // Reset
    let mut reader = BitReader::new(binary_data);
    reader.read_bits(15).unwrap();
    reader.read_bits(32).unwrap();
    reader.read_bits(32).unwrap();

    // Strategy 2: Align to byte, then read
    println!("\n--- Strategy 2: Align to byte boundary, then read ---");
    reader.align_byte();
    println!("  Aligned to bit {}", reader.position());
    let tag2 = reader.read_bits(8).expect("tag2");
    println!("  Tag byte: 0x{:02x} ({})", tag2, (tag2 as u8) as char);

    // Strategy 3: Try various bit offsets
    println!("\n--- Strategy 3: Try reading at various bit offsets ---");
    for test_bits in [79, 80, 88, 96, 104, 112] {
        let mut reader = BitReader::new(binary_data);
        if let Some(_) = reader.read_bits(test_bits as u8) {
            if let Some(tag) = reader.read_bits(8) {
                let is_valid_tag = matches!(tag as u8, 0x61 | 0x62 | 0x63 | 0x64 | 0x65 | 0x66 | 0x68 | 0x69 | 0x6a | 0x6c);
                println!("  Bit {}: 0x{:02x} ('{}')", test_bits, tag, if is_valid_tag { tag as u8 as char } else { ' ' });
            }
        }
    }

    // Strategy 4: Scan for known tags in byte array
    println!("\n--- Strategy 4: Scan for known tags in first 200 bytes ---");
    for i in 0..200 {
        let byte = binary_data[i];
        if matches!(byte, 0x61 | 0x62 | 0x63 | 0x65 | 0x66 | 0x68 | 0x69 | 0x6a | 0x6c) {
            println!("  Byte {}: 0x{:02x} ('{}') - bit position {}", i, byte, byte as char, i * 8);
        }
    }
}
