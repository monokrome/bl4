use crate::analysis::PositionStats;
use crate::records::{filter_by_position, group_by_position};

pub fn group_analysis(records: &[Vec<u8>], group_position: usize, max_positions: usize) {
    let groups = group_by_position(records, group_position);

    let mut keys: Vec<_> = groups.keys().copied().collect();
    keys.sort();

    println!(
        "Grouping by position {} ({} groups)\n",
        group_position,
        keys.len()
    );

    for key in keys {
        let group = &groups[&key];
        println!("=== Group 0x{:02x} ({} records) ===\n", key, group.len());

        let max_len = group.iter().map(|r| r.len()).max().unwrap_or(0);
        let positions = max_len.min(max_positions);

        println!(
            "{:>4}  {:>6}  {:>8}  {:>6}  {:>8}  Distribution",
            "Pos", "Count", "Unique", "Entropy", "Common"
        );
        println!("{}", "-".repeat(70));

        for pos in 0..positions {
            if let Some(stats) = PositionStats::from_records(group, pos) {
                println!(
                    "{:>4}  {:>6}  {:>8}  {:>6.2}  0x{:02x}:{:<4}  {}",
                    stats.position,
                    stats.count,
                    stats.unique,
                    stats.entropy,
                    stats.most_common.0,
                    stats.most_common.1,
                    stats.distribution_summary()
                );
            }
        }
        println!();
    }
}

pub fn filter_analysis(records: &[Vec<u8>], position: usize, value: u8, max_positions: usize) {
    let filtered = filter_by_position(records, position, value);

    println!(
        "Filtered: position {} = 0x{:02x} ({} records)\n",
        position,
        value,
        filtered.len()
    );

    if filtered.is_empty() {
        println!("No matching records");
        return;
    }

    let max_len = filtered.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);

    println!(
        "{:>4}  {:>6}  {:>8}  {:>6}  {:>8}  Distribution",
        "Pos", "Count", "Unique", "Entropy", "Common"
    );
    println!("{}", "-".repeat(70));

    for pos in 0..positions {
        if let Some(stats) = PositionStats::from_records(&filtered, pos) {
            println!(
                "{:>4}  {:>6}  {:>8}  {:>6.2}  0x{:02x}:{:<4}  {}",
                stats.position,
                stats.count,
                stats.unique,
                stats.entropy,
                stats.most_common.0,
                stats.most_common.1,
                stats.distribution_summary()
            );
        }
    }
}

pub fn compare_groups(records: &[Vec<u8>], group_position: usize, max_positions: usize) {
    let groups = group_by_position(records, group_position);

    let mut keys: Vec<_> = groups.keys().copied().collect();
    keys.sort();

    println!(
        "Comparing {} groups by position {}\n",
        keys.len(),
        group_position
    );

    // Find positions where groups differ
    let max_len = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);

    println!("Positions with significant variance between groups:\n");
    println!(
        "{:>4}  {}",
        "Pos",
        keys.iter()
            .map(|k| format!("0x{:02x}", k))
            .collect::<Vec<_>>()
            .join("     ")
    );
    println!("{}", "-".repeat(4 + keys.len() * 10));

    for pos in 0..positions {
        let stats: Vec<Option<PositionStats>> = keys
            .iter()
            .map(|&k| PositionStats::from_records(&groups[&k], pos))
            .collect();

        // Check if most common values differ across groups
        let common_values: Vec<u8> = stats
            .iter()
            .filter_map(|s| s.as_ref().map(|s| s.most_common.0))
            .collect();

        if common_values.is_empty() {
            continue;
        }

        let all_same = common_values.windows(2).all(|w| w[0] == w[1]);
        if all_same && pos > 0 {
            continue; // Skip positions where all groups agree (except pos 0 for reference)
        }

        let row: String = stats
            .iter()
            .map(|s| match s {
                Some(st) => format!("0x{:02x}:{:<3}", st.most_common.0, st.most_common.1),
                None => "   -   ".to_string(),
            })
            .collect::<Vec<_>>()
            .join("  ");

        let marker = if all_same { "" } else { " <-- DIFFERS" };
        println!("{:>4}  {}{}", pos, row, marker);
    }
}
