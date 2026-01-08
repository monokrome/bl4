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

    println!("=== Examining Gap Between Header and First Tag ===\n");

    // Read known header fields
    let name_idx = reader.read_bits(15).expect("name");
    let field_b = reader.read_bits(32).expect("field_b");
    let field_c = reader.read_bits(32).expect("field_c");

    println!("Header (bits 0-78):");
    println!("  Bit 0-14: name index = {} ({})", name_idx,
        strings.get(name_idx as usize).unwrap_or(&String::from("INVALID")));
    println!("  Bit 15-46: field_b = {}", field_b);
    println!("  Bit 47-78: field_c = {} / {}", field_c, f32::from_bits(field_c));
    println!("\nCurrent position: bit {}", reader.position());

    // The gap: bits 79-119 (41 bits before first tag at bit 120)
    println!("\n=== Gap Data (bits 79-119, 41 bits total) ===\n");

    // Try reading as 15-bit string index (common pattern)
    println!("Interpretation 1: 15-bit string index at bit 79");
    let potential_idx = reader.read_bits(15).expect("potential_idx");
    println!("  Value: {} ({})", potential_idx,
        strings.get(potential_idx as usize).unwrap_or(&String::from("INVALID INDEX")));
    println!("  Position now: bit {}", reader.position());

    // Try reading next 26 bits in various ways
    println!("\nInterpretation 2: Next 26 bits (bit 94-119)");
    let next_26 = reader.read_bits(26).expect("next_26");
    println!("  As u32: {}", next_26);
    println!("  As f32: {}", f32::from_bits(next_26));
    println!("  Position now: bit {}", reader.position());

    // Reset and try as 32-bit value
    let mut reader = BitReader::new(binary_data);
    reader.read_bits(79).unwrap(); // Skip to bit 79

    println!("\nInterpretation 3: 32-bit value at bit 79");
    let val_32 = reader.read_bits(32).expect("val_32");
    println!("  As u32: {}", val_32);
    println!("  As f32: {}", f32::from_bits(val_32));
    println!("  Position now: bit {}", reader.position());

    // Try reading remaining 9 bits
    let remaining_9 = reader.read_bits(9).expect("remaining_9");
    println!("  Next 9 bits: {}", remaining_9);
    println!("  Position now: bit {} (should be 120)", reader.position());

    // Reset and show raw bytes
    let mut reader = BitReader::new(binary_data);
    reader.read_bits(79).unwrap();

    println!("\n=== Raw byte view (bytes 9-15) ===");
    println!("Byte 9 (bit 72-79, bit 7 of byte used): 0x{:02x}", binary_data[9]);
    for i in 10..16 {
        println!("Byte {}: 0x{:02x} ('{}')", i, binary_data[i],
            if binary_data[i].is_ascii_graphic() { binary_data[i] as char } else { '.' });
    }

    // Check if byte 15 is really 'a' (0x61)
    println!("\nVerification: Byte 15 = 0x{:02x} (expected 0x61 for 'a')", binary_data[15]);

    // Try reading 8 bits at position 120 (byte 15)
    let mut reader = BitReader::new(binary_data);
    reader.read_bits(120).unwrap();
    let tag_at_120 = reader.read_bits(8).expect("tag_at_120");
    println!("8 bits at position 120: 0x{:02x} ('{}'), is valid tag: {}",
        tag_at_120,
        (tag_at_120 as u8) as char,
        matches!(tag_at_120 as u8, 0x61 | 0x62 | 0x63 | 0x65 | 0x66 | 0x68 | 0x69 | 0x6a | 0x6c));

    // Alternative: maybe tags are byte-aligned but we need to skip some bytes?
    println!("\n=== Alternative: What if we read full bytes after header? ===");
    let mut reader = BitReader::new(binary_data);
    reader.read_bits(15).unwrap();
    reader.read_bits(32).unwrap();
    reader.read_bits(32).unwrap();
    // Position 79, byte 9 bit offset 7

    // Align to byte 10
    reader.align_byte();
    println!("After align_byte(), position: bit {}", reader.position());

    // Read bytes 10-15
    for i in 0..6 {
        if let Some(byte) = reader.read_bits(8) {
            let pos = reader.position() - 8;
            let byte_pos = pos / 8;
            println!("  Byte {}: 0x{:02x} ('{}')", byte_pos, byte,
                if (byte as u8).is_ascii_graphic() { byte as u8 as char } else { '.' });
        }
    }
}
