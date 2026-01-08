#!/usr/bin/env rust-script
//! Validate serial index extraction quality
//!
//! This program analyzes the serial index extraction from inv.bin to determine
//! if we're over-extracting (false positives) or extracting correctly.

use bl4_ncs::{parse_header, parse_ncs_string_table, find_binary_section_with_count};
use std::collections::{HashMap, HashSet, BTreeMap};
use std::env;
use std::fs;

#[derive(Debug)]
struct ValidationReport {
    total_raw_extractions: usize,
    unique_positions: usize,
    unique_values: usize,
    value_distribution: BTreeMap<u32, usize>,
    position_distribution: BTreeMap<u32, usize>,
    tag_f_count: usize,
    tag_a_count: usize,
    overlap_count: usize,
    sample_contexts: Vec<SampleContext>,
}

#[derive(Debug)]
struct SampleContext {
    index: u32,
    position: usize,
    found_by: String,
    nearby_strings: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-inv4.bin>", args[0]);
        std::process::exit(1);
    }

    let inv_path = &args[1];
    let data = fs::read(inv_path).expect("Failed to read inv file");

    println!("=== Serial Index Extraction Validation ===\n");

    // Parse header and strings
    let header = parse_header(&data).expect("Failed to parse header");
    let strings = parse_ncs_string_table(&data, &header);

    println!("File: {}", inv_path);
    println!("Type: {}", header.type_name);
    println!("Format: {}", header.format_code);
    println!("Strings: {}\n", strings.len());

    // Find binary section
    let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
        .expect("Failed to find binary section");
    let binary_data = &data[binary_offset..];

    println!("Binary section at offset: 0x{:x} ({} bytes)\n", binary_offset, binary_data.len());

    // Extract using both tags
    let mut tag_f_positions = HashSet::new();
    let mut tag_a_positions = HashSet::new();
    let mut all_extractions = Vec::new();

    // Tag 'f' at offset +27
    for i in 0..binary_data.len() {
        if binary_data[i] == 0x66 && i + 27 < binary_data.len() {
            let pos = i + 27;

            let val_u8 = binary_data[pos] as u32;
            if val_u8 >= 1 && val_u8 < 256 {
                tag_f_positions.insert(pos);
                all_extractions.push((pos, val_u8, "tag_f"));
            }

            if pos + 1 < binary_data.len() {
                let val_u16 = u16::from_le_bytes([binary_data[pos], binary_data[pos + 1]]) as u32;
                if val_u16 >= 256 && val_u16 <= 541 {
                    tag_f_positions.insert(pos);
                    all_extractions.push((pos, val_u16, "tag_f"));
                }
            }
        }
    }

    // Tag 'a' at offset +5
    for i in 0..binary_data.len() {
        if binary_data[i] == 0x61 && i + 5 < binary_data.len() {
            let pos = i + 5;

            let val_u8 = binary_data[pos] as u32;
            if val_u8 >= 1 && val_u8 < 256 {
                tag_a_positions.insert(pos);
                all_extractions.push((pos, val_u8, "tag_a"));
            }

            if pos + 1 < binary_data.len() {
                let val_u16 = u16::from_le_bytes([binary_data[pos], binary_data[pos + 1]]) as u32;
                if val_u16 >= 256 && val_u16 <= 541 {
                    tag_a_positions.insert(pos);
                    all_extractions.push((pos, val_u16, "tag_a"));
                }
            }
        }
    }

    // Deduplicate by position+value
    let mut position_value_pairs = HashSet::new();
    for &(pos, val, _) in &all_extractions {
        position_value_pairs.insert((pos, val));
    }

    // Calculate statistics
    let mut value_counts: HashMap<u32, usize> = HashMap::new();
    let mut position_counts: HashMap<u32, HashSet<usize>> = HashMap::new();

    for &(pos, val) in &position_value_pairs {
        *value_counts.entry(val).or_insert(0) += 1;
        position_counts.entry(val).or_insert_with(HashSet::new).insert(pos);
    }

    let overlap_count = tag_f_positions.intersection(&tag_a_positions).count();

    // Print summary
    println!("## Extraction Summary");
    println!("Total raw extractions: {}", all_extractions.len());
    println!("Unique positions: {}", position_value_pairs.len());
    println!("Unique values: {}", value_counts.len());
    println!();

    println!("## Tag Analysis");
    println!("Tag 'f' positions: {}", tag_f_positions.len());
    println!("Tag 'a' positions: {}", tag_a_positions.len());
    println!("Overlapping positions: {}", overlap_count);
    println!("Overlap %: {:.1}%", (overlap_count as f64 / position_value_pairs.len() as f64) * 100.0);
    println!();

    // Value distribution analysis
    println!("## Value Distribution (top 20 by frequency)");
    let mut sorted_values: Vec<_> = value_counts.iter().collect();
    sorted_values.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

    println!("{:<10} {:<15} {:<15}", "Index", "Occurrences", "Positions");
    for (val, count) in sorted_values.iter().take(20) {
        let pos_count = position_counts.get(val).map(|s| s.len()).unwrap_or(0);
        println!("{:<10} {:<15} {:<15}", val, count, pos_count);
    }
    println!();

    // Check for suspicious patterns
    println!("## Validation Checks");

    // Check 1: Do indices appear at multiple different positions?
    let multi_position_count = position_counts.values().filter(|positions| positions.len() > 1).count();
    println!("✓ Indices at multiple positions: {} ({:.1}%)",
        multi_position_count,
        (multi_position_count as f64 / position_counts.len() as f64) * 100.0
    );

    // Check 2: Distribution should be roughly uniform if quality is good
    let mean_count = all_extractions.len() as f64 / value_counts.len() as f64;
    let variance: f64 = value_counts.values()
        .map(|&count| {
            let diff = count as f64 - mean_count;
            diff * diff
        })
        .sum::<f64>() / value_counts.len() as f64;
    let std_dev = variance.sqrt();

    println!("✓ Mean occurrences per index: {:.1}", mean_count);
    println!("✓ Standard deviation: {:.1}", std_dev);

    // Check 3: Sample contexts
    println!("\n## Sample Contexts (first 10)");
    for (i, &(pos, val, tag)) in all_extractions.iter().enumerate().take(10) {
        let abs_pos = binary_offset + pos;

        // Find nearby printable strings
        let start = abs_pos.saturating_sub(50);
        let nearby = find_nearby_strings(&data[start..abs_pos.min(data.len())]);

        println!("\nSample {}:", i + 1);
        println!("  Index: {}", val);
        println!("  Position: 0x{:x}", abs_pos);
        println!("  Found by: {}", tag);
        if !nearby.is_empty() {
            println!("  Nearby strings: {:?}", nearby);
        }
    }

    // Final assessment
    println!("\n## Assessment");
    println!("Target: 5,513 serial indices (from reference data)");
    println!("Extracted: {} unique positions", position_value_pairs.len());

    let diff = position_value_pairs.len() as i64 - 5513i64;
    let diff_pct = (diff as f64 / 5513.0) * 100.0;

    if diff > 0 {
        println!("Difference: +{} ({:+.1}% over target)", diff, diff_pct);
    } else {
        println!("Difference: {} ({:.1}% under target)", diff, diff_pct);
    }

    // Quality indicators
    println!("\nQuality Indicators:");
    if std_dev / mean_count < 2.0 {
        println!("✓ GOOD: Distribution variance is low (consistent extraction)");
    } else {
        println!("⚠ SUSPICIOUS: High variance suggests some false positives");
    }

    if (overlap_count as f64 / position_value_pairs.len() as f64) < 0.2 {
        println!("✓ GOOD: Low tag overlap (<20%) suggests independent extraction");
    } else {
        println!("⚠ WARNING: High tag overlap suggests possible duplicate counting");
    }

    if (multi_position_count as f64 / position_counts.len() as f64) > 0.5 {
        println!("✓ GOOD: Most indices appear at multiple positions (realistic)");
    } else {
        println!("⚠ SUSPICIOUS: Many indices only appear once");
    }
}

fn find_nearby_strings(region: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = Vec::new();

    for &byte in region {
        if byte == 0 {
            if !current.is_empty() {
                if let Ok(s) = String::from_utf8(current.clone()) {
                    if s.chars().all(|c| c.is_ascii() && !c.is_control()) && s.len() > 3 {
                        found.push(s);
                    }
                }
                current.clear();
            }
        } else if byte >= 32 && byte < 127 {
            current.push(byte);
        } else {
            current.clear();
        }
    }

    found.into_iter().rev().take(3).collect()
}
