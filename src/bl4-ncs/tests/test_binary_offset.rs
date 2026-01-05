use bl4_ncs::{parse_header, parse_ncs_string_table, find_binary_section_with_count, BitReader};

#[test]
#[ignore]
fn test_binary_offset_and_remap() {
    let data = std::fs::read("/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin")
        .expect("File not found");

    // Parse header and strings
    let header = parse_header(&data).expect("Failed to parse header");
    let strings = parse_ncs_string_table(&data, &header);

    eprintln!("Strings: {}", strings.len());
    eprintln!("String table offset: 0x{:x}", header.string_table_offset);

    // Calculate binary offset
    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(strings.len() as u32))
        .expect("Failed to find binary section");

    eprintln!("Binary offset: 0x{:x}", binary_offset);

    // Show bytes at that offset
    eprintln!("Bytes at binary offset:");
    for i in 0..16 {
        eprint!("{:02x} ", data[binary_offset + i]);
    }
    eprintln!();

    // Try reading as FixedWidthArray24
    let binary_data = &data[binary_offset..];
    let mut reader = BitReader::new(binary_data);

    let count = reader.read_bits(24).expect("Failed to read count");
    let width = reader.read_bits(8).expect("Failed to read width") as u8;

    eprintln!("FixedWidthArray24: count={}, width={}", count, width);

    // This should give reasonable values
    assert!(count < 50000, "Count too large: {}", count);
    assert!(width > 0 && width <= 32, "Width invalid: {}", width);
}
