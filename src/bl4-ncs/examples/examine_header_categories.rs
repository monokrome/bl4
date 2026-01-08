use bl4_ncs::{decompress_ncs, parse_header};
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

    println!("Header info:");
    println!("  Type: {}", header.type_name);
    println!("  Format: {}", header.format_code);
    println!("  String table offset: 0x{:x}", header.string_table_offset);
    println!("  String count: {:?}", header.string_count);
   println!("  Control section: {:?}", header.control_section_offset.map(|o| format!("0x{:x}", o)));
    println!("  Category names offset: {:?}", header.category_names_offset.map(|o| format!("0x{:x}", o)));

    // Check if there are category names
    if let Some(cat_offset) = header.category_names_offset {
        println!("\nCategory names section:");
        let cat_data = &data[cat_offset..cat_offset.min(data.len()).min(cat_offset + 1000)];

        // Try to read null-terminated strings
        let mut categories = Vec::new();
        let mut start = 0;
        for (i, &byte) in cat_data.iter().enumerate() {
            if byte == 0 {
                if i > start {
                    if let Ok(s) = std::str::from_utf8(&cat_data[start..i]) {
                        categories.push(s.to_string());
                        if categories.len() >= 50 {
                            break;
                        }
                    }
                }
                start = i + 1;
            }
        }

        println!("Found {} category strings:", categories.len());
        for (i, cat) in categories.iter().enumerate() {
            println!("  {}: {}", i, cat);
        }
    }
}
