//! NCS debug command

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::util::print_hex;

pub fn debug_file(path: &Path, show_hex: bool, do_parse: bool, show_offsets: bool) -> Result<()> {
    use bl4_ncs::{
        parse_header, parse_ncs_string_table, parse_binary_section, bit_width, BitReader,
        extract_inline_strings, extract_field_abbreviation, create_combined_string_table,
        find_packed_strings, UnpackedValue,
    };

    let data = fs::read(path).context("Failed to read file")?;
    println!("File: {}", path.display());
    println!("Size: {} bytes", data.len());

    // Parse header
    let header = parse_header(&data).context("Failed to parse header")?;
    println!("\n=== Header ===");
    println!("Type: {}", header.type_name);
    println!("Format: {}", header.format_code);
    println!("Field count: {}", header.field_count);

    if show_offsets {
        println!("\n=== Offsets ===");
        println!("Type offset: 0x{:x}", header.type_offset);
        println!("Format offset: 0x{:x}", header.format_offset);
        println!("Entry section: 0x{:x}", header.entry_section_offset);
        println!("String table: 0x{:x}", header.string_table_offset);
        if let Some(ctrl) = header.control_section_offset {
            println!("Control section: 0x{:x}", ctrl);
        }
        if let Some(cat) = header.category_names_offset {
            println!("Category names: 0x{:x}", cat);
        }
        println!("Binary section: 0x{:x}", header.binary_offset);
        if let Some(sc) = header.string_count {
            println!("String count (from header): {}", sc);
        }
    }

    // Parse string table
    let strings = parse_ncs_string_table(&data, &header);
    println!("\n=== String Table ({} strings) ===", strings.len());
    for (i, s) in strings.strings.iter().enumerate().take(20) {
        println!("  {:3}: {}", i, s);
    }
    if strings.len() > 20 {
        println!("  ... and {} more", strings.len() - 20);
    }

    // Show packed strings
    let packed = find_packed_strings(&strings.strings);
    if !packed.is_empty() {
        println!("\n=== Packed Strings ({} found) ===", packed.len());
        for unpacked in packed.iter().take(10) {
            let values_str: Vec<String> = unpacked.values.iter().map(|v| match v {
                UnpackedValue::Integer(n) => format!("int({})", n),
                UnpackedValue::Float(f) => format!("float({})", f),
                UnpackedValue::String(s) => format!("str(\"{}\")", s),
                UnpackedValue::Boolean(b) => format!("bool({})", b),
            }).collect();
            println!("  \"{}\" -> [{}]", unpacked.original, values_str.join(", "));
        }
        if packed.len() > 10 {
            println!("  ... and {} more", packed.len() - 10);
        }
    }

    // Find section markers
    println!("\n=== Section Markers ===");
    for i in 0..data.len().saturating_sub(3) {
        if data[i] != 0 && data[i+1] != 0 && data[i+2] == 0 && data[i+3] == 0 {
            if i > header.string_table_offset {
                println!("  0x{:03x}: {:02x} {:02x} 00 00", i, data[i], data[i+1]);
            }
        }
    }

    // Find 0x7a marker
    for i in 0..data.len().saturating_sub(5) {
        if data[i..i+6] == [0x7a, 0x00, 0x00, 0x00, 0x00, 0x00] {
            println!("  0x{:03x}: 7a 00 00 00 00 00 (section divider)", i);
        }
    }

    // Try reading from first section marker
    // Find first XX XX 00 00 pattern after string table
    let string_bits = bit_width(strings.len() as u32);
    println!("\n=== Entry Data Test (from first marker) ===");
    for i in header.string_table_offset..data.len().saturating_sub(3) {
        if data[i] != 0 && data[i+1] != 0 && data[i+2] == 0 && data[i+3] == 0 {
            println!("Testing offset 0x{:x}:", i);
            let test_data = &data[i..];
            let mut reader = BitReader::new(test_data);
            print!("  As {}-bit indices: ", string_bits);
            for _ in 0..8 {
                if let Some(v) = reader.read_bits(string_bits) {
                    let valid = (v as usize) < strings.len();
                    if valid {
                        print!("{} ", v);
                    } else {
                        print!("({}) ", v);
                    }
                }
            }
            println!();
            break;
        }
    }

    // Extract inline strings (category names) and field abbreviation
    let inline_strings = extract_inline_strings(&data, &header, strings.len());
    let field_abbrev = extract_field_abbreviation(&data, &header);

    // Build combined string table: primary + inline + field abbreviation + type name
    let mut all_inline = inline_strings.clone();
    if let Some(ref abbrev) = field_abbrev {
        all_inline.push(abbrev.clone());
    }
    // Add type name as final string (may be referenced by table_id)
    all_inline.push(header.type_name.clone());
    let combined_strings = create_combined_string_table(&strings, &all_inline);

    if !inline_strings.is_empty() || field_abbrev.is_some() {
        println!("\n=== Inline Strings ===");
        let mut idx = strings.len();
        for s in inline_strings.iter() {
            println!("  {:3}: {} (category)", idx, s);
            idx += 1;
        }
        if let Some(ref abbrev) = field_abbrev {
            println!("  {:3}: {} (field abbrev)", idx, abbrev);
            idx += 1;
        }
        println!("  {:3}: {} (type name)", idx, header.type_name);
    }

    let total_strings = combined_strings.len();
    let total_string_bits = bit_width(total_strings as u32);

    // Binary section analysis
    if header.binary_offset < data.len() {
        let binary_data = &data[header.binary_offset..];
        println!("\n=== Binary Section ===");
        println!("Starts at: 0x{:x}", header.binary_offset);
        println!("Length: {} bytes", binary_data.len());
        println!("Primary strings: {} ({} bits)", strings.len(), string_bits);
        println!("Total strings (with inline): {} ({} bits)", total_strings, total_string_bits);

        if show_hex {
            println!("\nFirst 64 bytes:");
            print_hex(&binary_data[..binary_data.len().min(64)]);
        }

        // Try bit reading
        println!("\n=== Bit Reader Test ===");
        let mut reader1 = BitReader::new(binary_data);

        // Read first few values different ways
        println!("Reading as bytes:");
        for i in 0..8.min(binary_data.len()) {
            let v = reader1.read_bits(8);
            if let Some(v) = v {
                let c = if (32..127).contains(&v) { v as u8 as char } else { '.' };
                println!("  Byte {}: 0x{:02x} ({:3}) '{}'", i, v, v, c);
            }
        }

        // Read with total_string_bits (including inline strings)
        let mut reader3 = BitReader::new(binary_data);
        println!("\nReading {} bit values (combined strings):", total_string_bits);
        for i in 0..10 {
            let v = reader3.read_bits(total_string_bits);
            if let Some(v) = v {
                let s = combined_strings.strings.get(v as usize).map(|s| s.as_str()).unwrap_or("(oob)");
                println!("  Value {}: {} -> {:?}", i, v, s);
            }
        }

        if do_parse {
            println!("\n=== Binary Parse Attempt ===");
            // Use combined string table for binary parsing
            match parse_binary_section(&data, header.binary_offset, &combined_strings) {
                Some(result) => {
                    println!("table_id: {} -> {:?}", result.table_id,
                        combined_strings.strings.get(result.table_id as usize));
                    println!("bit_indices: {} values", result.bit_indices.len());

                    // Show first few bit indices with string lookups
                    println!("\nFirst 20 bit-packed indices:");
                    for (i, &idx) in result.bit_indices.iter().take(20).enumerate() {
                        let s = combined_strings.strings.get(idx as usize)
                            .map(|s| s.as_str())
                            .unwrap_or("(oob)");
                        let marker = if idx as usize >= combined_strings.len() { "*" } else { "" };
                        println!("  [{:2}] {:2} -> {}{}", i, idx, s, marker);
                    }
                    if result.bit_indices.len() > 20 {
                        println!("  ... and {} more", result.bit_indices.len() - 20);
                    }

                    // Show entry groups
                    println!("\nEntry groups: {} found (matching entries)", result.entry_groups.len());
                    for (i, group) in result.entry_groups.iter().enumerate().take(10) {
                        println!("  Entry {}: values={:?}", i, group.values);
                    }
                    if result.entry_groups.len() > 10 {
                        println!("  ... and {} more entries", result.entry_groups.len() - 10);
                    }

                    // Show tail data
                    if !result.tail_data.is_empty() {
                        println!("\nTail data: {} bytes", result.tail_data.len());
                        let preview: Vec<String> = result.tail_data.iter().take(32)
                            .map(|b| format!("{:02x}", b)).collect();
                        println!("  {}", preview.join(" "));
                    }
                }
                None => {
                    println!("Failed to parse binary section");
                }
            }
        }
    }

    Ok(())
}
