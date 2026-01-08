use bl4_ncs::{decompress_ncs, parse_header, parse_ncs_string_table};
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

    println!("String table: {} strings\n", strings.len());

    let deps_to_find = vec![
        "inv_comp", "primary_augment", "secondary_augment", "core_augment", "barrel",
        "barrel_acc", "body", "body_acc", "foregrip", "grip",
    ];

    for dep in deps_to_find {
        if let Some(&idx) = strings.index_map.get(dep) {
            println!("{:25} -> index {}", dep, idx);
        } else {
            println!("{:25} -> NOT FOUND", dep);
        }
    }
}
