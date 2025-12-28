//! Item serial command handlers

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Handle `serial decode` command
#[allow(clippy::too_many_lines)] // Serial decoding with debug output
pub fn decode(
    serial: &str,
    verbose: bool,
    debug: bool,
    analyze: bool,
    parts_db: &Path,
) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;

    println!("Serial: {}", item.original);
    println!(
        "Item type: {} ({})",
        item.item_type,
        item.item_type_description()
    );

    // Show weapon info based on format type
    if let Some((mfr, weapon_type)) = item.weapon_info() {
        println!("Weapon: {} {}", mfr, weapon_type);
    } else if let Some(group_id) = item.part_group_id() {
        let category_name =
            bl4::category_name_for_type(item.item_type, group_id).unwrap_or("Unknown");
        println!("Category: {} ({})", category_name, group_id);
    }

    // Show elements if detected
    if let Some(elements) = item.element_names() {
        println!("Element: {}", elements);
    }

    // Show rarity if detected
    if let Some(rarity) = item.rarity_name() {
        println!("Rarity: {}", rarity);
    }

    // Show raw manufacturer ID if we couldn't resolve it
    if item.weapon_info().is_none() {
        if let Some(mfr) = item.manufacturer_name() {
            println!("Manufacturer: {}", mfr);
        } else if let Some(mfr_id) = item.manufacturer {
            println!("Manufacturer ID: {} (unknown)", mfr_id);
        }
    }

    // Show level and seed for VarInt-first format
    if let Some(level) = item.level {
        if let Some(raw) = item.raw_level {
            if raw > level {
                println!(
                    "Level: {} (WARNING: decoded as {}, capped - decoding may be wrong)",
                    level, raw
                );
            } else {
                println!("Level: {}", level);
            }
        } else {
            println!("Level: {}", level);
        }
    }
    if let Some(seed) = item.seed {
        println!("Seed: {}", seed);
    }

    println!("Decoded bytes: {}", item.raw_bytes.len());
    println!("Hex: {}", item.hex_dump());
    println!("Tokens: {}", item.format_tokens());

    // Try to resolve part names from database
    let category: Option<i64> = item.parts_category();

    // Resolve part names if we have a category and parts database
    let parts = item.parts();
    if let (Some(category), false) = (category, parts.is_empty()) {
        #[derive(Debug, Deserialize)]
        struct PartsDb {
            parts: Vec<PartDbEntry>,
        }
        #[derive(Debug, Deserialize)]
        struct PartDbEntry {
            name: String,
            category: i64,
            index: i64,
        }

        if let Ok(db_content) = fs::read_to_string(parts_db) {
            if let Ok(db) = serde_json::from_str::<PartsDb>(&db_content) {
                let lookup: HashMap<(i64, i64), &str> = db
                    .parts
                    .iter()
                    .map(|p| ((p.category, p.index), p.name.as_str()))
                    .collect();

                println!("\nResolved parts:");
                for (part_index, values) in &parts {
                    let has_flag = *part_index >= 128;
                    let actual_index = if has_flag {
                        (*part_index & 0x7F) as i64
                    } else {
                        *part_index as i64
                    };
                    let flag_str = if has_flag { " [+]" } else { "" };
                    let extra = if values.is_empty() {
                        String::new()
                    } else {
                        format!(" (values: {:?})", values)
                    };
                    if let Some(name) = lookup.get(&(category, actual_index)) {
                        println!("  {}{}{}", name, flag_str, extra);
                    } else {
                        let idx_display = if has_flag {
                            format!(
                                "{} (0x{:02x} = flag + {})",
                                part_index, part_index, actual_index
                            )
                        } else {
                            format!("{}", part_index)
                        };
                        println!("  [unknown part index {}]{}", idx_display, extra);
                    }
                }
            }
        }
    }

    if verbose {
        println!("\n{}", item.detailed_dump());
    }

    if debug {
        println!("\nDebug parsing:");
        bl4::serial::parse_tokens_debug(&item.raw_bytes);
    }

    if analyze {
        analyze_first_token(&item)?;
    }

    Ok(())
}

/// Analyze the first token for group ID research
fn analyze_first_token(item: &bl4::ItemSerial) -> Result<()> {
    use bl4::serial::Token;

    if let Some(first_token) = item.tokens.first() {
        let value = match first_token {
            Token::VarInt(v) => Some((*v, "VarInt")),
            Token::VarBit(v) => Some((*v, "VarBit")),
            _ => None,
        };

        if let Some((value, token_type)) = value {
            println!("\n=== First Token Analysis ===");
            println!("Type:   {}", token_type);
            println!("Value:  {} (decimal)", value);
            println!("Hex:    0x{:x}", value);
            println!("Binary: {:024b}", value);
            println!();

            // Decode Part Group ID based on item type
            println!("Part Group ID decoding:");
            match item.item_type {
                'r' | 'a'..='d' | 'f' | 'g' | 'v'..='z' => {
                    let group_id = value / 8192;
                    let offset = value % 8192;
                    println!("  Formula: group_id = value / 8192 (weapons)");
                    println!("  Group ID: {} (offset {})", group_id, offset);

                    let group_name = bl4::category_name(group_id as i64).unwrap_or("Unknown");
                    println!("  Identified: {}", group_name);
                }
                'e' => {
                    let group_id = value / 384;
                    let offset = value % 384;
                    println!("  Formula: group_id = value / 384 (equipment)");
                    println!("  Group ID: {} (offset {})", group_id, offset);

                    let group_name =
                        bl4::category_name(group_id as i64).unwrap_or("Unknown Equipment");
                    println!("  Identified: {}", group_name);
                }
                'u' => {
                    println!("  Utility items - encoding formula not yet determined");
                    println!("  Raw value: {}", value);
                }
                '!' | '#' => {
                    println!("  Class mods - encoding formula not yet determined");
                    println!("  Raw value: {}", value);
                }
                _ => {
                    println!(
                        "  Unknown item type '{}' - encoding formula not determined",
                        item.item_type
                    );
                }
            }

            println!();
            println!("Bit split analysis (for research):");
            for split in [8, 10, 12, 13, 14] {
                let high = value >> split;
                let low = value & ((1 << split) - 1);
                println!("  Split at bit {:2}: high={:6}  low={:6}", split, high, low);
            }
        } else {
            println!("\n=== First Token Analysis ===");
            println!("First token is not numeric: {:?}", first_token);
        }
    }

    Ok(())
}

/// Handle `serial encode` command
pub fn encode(serial: &str) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;
    let re_encoded = item.encode();

    println!("Original:   {}", serial);
    println!("Re-encoded: {}", re_encoded);

    if serial == re_encoded {
        println!("\n✓ Round-trip encoding successful!");
    } else {
        println!("\n✗ Round-trip encoding differs");
        println!("  Original length:   {}", serial.len());
        println!("  Re-encoded length: {}", re_encoded.len());

        // Decode both to compare tokens
        let re_item = bl4::ItemSerial::decode(&re_encoded)?;
        println!("\nOriginal tokens:   {}", item.format_tokens());
        println!("Re-encoded tokens: {}", re_item.format_tokens());
    }

    Ok(())
}

/// Handle `serial compare` command
#[allow(clippy::too_many_lines)] // Serial comparison output
pub fn compare(serial1: &str, serial2: &str) -> Result<()> {
    let item1 = bl4::ItemSerial::decode(serial1).context("Failed to decode serial 1")?;
    let item2 = bl4::ItemSerial::decode(serial2).context("Failed to decode serial 2")?;

    // Header comparison
    println!("=== SERIAL 1 ===");
    println!("Serial: {}", item1.original);
    println!(
        "Type: {} ({})",
        item1.item_type,
        item1.item_type_description()
    );
    if let Some((mfr, wtype)) = item1.weapon_info() {
        println!("Weapon: {} {}", mfr, wtype);
    }
    if let Some(level) = item1.level {
        println!("Level: {}", level);
    }
    if let Some(seed) = item1.seed {
        println!("Seed: {}", seed);
    }
    println!("Tokens: {}", item1.format_tokens());

    println!();
    println!("=== SERIAL 2 ===");
    println!("Serial: {}", item2.original);
    println!(
        "Type: {} ({})",
        item2.item_type,
        item2.item_type_description()
    );
    if let Some((mfr, wtype)) = item2.weapon_info() {
        println!("Weapon: {} {}", mfr, wtype);
    }
    if let Some(level) = item2.level {
        println!("Level: {}", level);
    }
    if let Some(seed) = item2.seed {
        println!("Seed: {}", seed);
    }
    println!("Tokens: {}", item2.format_tokens());

    // Byte comparison
    println!();
    println!("=== BYTE COMPARISON ===");
    println!(
        "Lengths: {} vs {} bytes",
        item1.raw_bytes.len(),
        item2.raw_bytes.len()
    );

    let max_len = std::cmp::max(item1.raw_bytes.len(), item2.raw_bytes.len());
    let mut first_diff = None;
    let mut diff_count = 0;

    for i in 0..max_len {
        let b1 = item1.raw_bytes.get(i);
        let b2 = item2.raw_bytes.get(i);
        if b1 != b2 {
            diff_count += 1;
            if first_diff.is_none() {
                first_diff = Some(i);
            }
        }
    }

    if diff_count == 0 {
        println!("Bytes: IDENTICAL");
    } else {
        println!("Bytes: {} differences", diff_count);
        if let Some(first) = first_diff {
            println!("First diff at byte {}", first);
            println!();
            println!("Byte-by-byte (first 20 bytes or until divergence + 5):");
            println!("{:>4}  {:>12}  {:>12}", "Idx", "Serial 1", "Serial 2");
            let show_until = std::cmp::min(max_len, first + 10);
            for i in 0..show_until {
                let b1 = item1.raw_bytes.get(i).copied();
                let b2 = item2.raw_bytes.get(i).copied();
                let marker = if b1 != b2 { " <--" } else { "" };
                let s1 = b1
                    .map(|b| format!("{:3} {:08b}", b, b))
                    .unwrap_or_else(|| "-".to_string());
                let s2 = b2
                    .map(|b| format!("{:3} {:08b}", b, b))
                    .unwrap_or_else(|| "-".to_string());
                println!("{:4}  {}  {}{}", i, s1, s2, marker);
            }
        }
    }

    Ok(())
}

/// Handle `serial modify` command
#[allow(clippy::too_many_lines)] // Serial modification command
pub fn modify(base: &str, source: &str, parts: &str) -> Result<()> {
    use bl4::serial::Token;

    // Parse part indices
    let part_indices: Vec<u64> = parts
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if part_indices.is_empty() {
        bail!("No valid part indices provided");
    }

    let base_item = bl4::ItemSerial::decode(base).context("Failed to decode base serial")?;
    let source_item = bl4::ItemSerial::decode(source).context("Failed to decode source serial")?;

    println!("Base serial:   {}", base);
    println!("Source serial: {}", source);
    println!("Copying part indices: {:?}", part_indices);
    println!();

    // Build a map of source parts by index
    let source_parts: HashMap<u64, Vec<u64>> = source_item
        .tokens
        .iter()
        .filter_map(|t| {
            if let Token::Part { index, values } = t {
                Some((*index, values.clone()))
            } else {
                None
            }
        })
        .collect();

    // Modify base tokens - replace parts at specified indices
    let new_tokens: Vec<Token> = base_item
        .tokens
        .iter()
        .map(|t| {
            if let Token::Part { index, values } = t {
                if part_indices.contains(index) {
                    if let Some(source_values) = source_parts.get(index) {
                        println!(
                            "  Swapping part {}: {:?} -> {:?}",
                            index, values, source_values
                        );
                        return Token::Part {
                            index: *index,
                            values: source_values.clone(),
                        };
                    }
                }
            }
            t.clone()
        })
        .collect();

    // Encode the new serial
    let modified = base_item.with_tokens(new_tokens);
    let new_serial = modified.encode();

    println!();
    println!("New serial: {}", new_serial);

    Ok(())
}

/// Handle `serial batch-decode` command
pub fn batch_decode(input: &Path, output: &Path) -> Result<()> {
    use std::io::{BufRead, BufReader, BufWriter};

    let file =
        fs::File::open(input).with_context(|| format!("Failed to open input file: {:?}", input))?;
    let reader = BufReader::new(file);

    let out_file = fs::File::create(output)
        .with_context(|| format!("Failed to create output file: {:?}", output))?;
    let mut writer = BufWriter::new(out_file);

    let mut count = 0;
    let mut errors = 0;

    for line in reader.lines() {
        let serial = line.context("Failed to read line")?;
        let serial = serial.trim();
        if serial.is_empty() {
            continue;
        }

        match bl4::ItemSerial::decode(serial) {
            Ok(item) => {
                // Write length as u16 followed by raw bytes
                let bytes = &item.raw_bytes;
                let len = bytes.len() as u16;
                writer.write_all(&len.to_le_bytes())?;
                writer.write_all(bytes)?;
                count += 1;
            }
            Err(_) => {
                // Write 0 length to indicate decode failure (keeps alignment)
                writer.write_all(&0u16.to_le_bytes())?;
                errors += 1;
            }
        }
    }

    writer.flush()?;
    println!(
        "Decoded {} serials to {:?} ({} errors)",
        count, output, errors
    );

    Ok(())
}
