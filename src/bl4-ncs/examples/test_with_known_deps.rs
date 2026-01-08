use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table, find_binary_section_with_count};
use bl4_ncs::ncs_parser::{parse_record, FixedWidthArray};
use bl4_ncs::BitReader;
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

    // Use the known deps from parser output
    let deps: Vec<String> = vec![
        "inv_comp",
        "primary_augment",
        "secondary_augment",
        "core_augment",
        "barrel",
        "barrel_acc",
        "body",
        "body_acc",
        "foregrip",
        "grip",
        "magazine",
        "magazine_ted_thrown",
        "magazine_acc",
        "scope",
        "scope_acc",
        "secondary_ammo",
        "hyperion_secondary_acc",
        "payload_augment",
        "payload",
        "class_mod_body",
        "passive_points",
        "action_skill_mod",
        "body_bolt",
        "body_mag",
        "element",
        "firmware",
        "stat_augment",
        "body_ele",
        "unique",
        "turret_weapon",
        "tediore_acc",
        "tediore_secondary_acc",
        "endgame",
        "enemy_augment",
        "active_augment",
        "underbarrel",
        "underbarrel_acc_vis",
        "underbarrel_acc",
        "barrel_licensed",
    ].iter().map(|s| s.to_string()).collect();

    println!("Using {} known deps", deps.len());
    println!("Binary offset: 0x{:x}\n", binary_offset);

    let binary_data = &data[binary_offset..];
    let mut reader = BitReader::new(binary_data);

    // Create empty remap_a (inv files don't have this)
    let remap_a = FixedWidthArray {
        count: 0,
        width: 0,
        values: Vec::new(),
    };

    // Try parsing records
    let mut records = Vec::new();
    let max_records = 10;

    println!("Attempting to parse records without remap_a/remap_b...\n");

    for i in 0..max_records {
        println!("=== Record {} ===", i);
        match parse_record(&mut reader, &strings, &deps, &remap_a) {
            Some(record) => {
                println!("  Tags: {}", record.tags.len());
                println!("  Entries: {}", record.entries.len());
                println!("  Dep entries: {}", record.dep_entries.len());

                // Count serial indices
                let mut serial_count = 0;
                for dep_entry in &record.dep_entries {
                    if let Some(si_obj) = dep_entry.fields.get("serialindex") {
                        serial_count += 1;
                    }
                }

                println!("  Serial indices found: {}", serial_count);
                records.push(record);
            }
            None => {
                println!("  Failed to parse");
                break;
            }
        }
        println!();
    }

    println!("\nTotal records parsed: {}", records.len());
    println!("Total serial indices: {}",
        records.iter().map(|r| r.dep_entries.iter().filter(|d| d.fields.contains_key("serialindex")).count()).sum::<usize>()
    );
}
