//! Item serial command handlers

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Display groups and the slot types that map to each, in display order.
const DISPLAY_GROUPS: &[(&str, &[&str])] = &[
    ("Barrel", &["barrel"]),
    ("Underbarrel", &["underbarrel"]),
    (
        "Body",
        &[
            "body",
            "body_armor",
            "body_bolt",
            "body_element",
            "body_mag",
        ],
    ),
    ("Grip", &["grip"]),
    ("Foregrip", &["foregrip"]),
    ("Scope", &["scope"]),
    ("Magazine", &["mag"]),
    ("Element", &["element", "secondary_elem"]),
    ("Ammo", &["secondary_ammo", "secondary"]),
    ("Shield", &["shield"]),
    ("Rarity", &["rarity"]),
    ("Stats", &["stat", "stat2", "stat3"]),
    ("Firmware", &["firmware"]),
    ("Passive", &["passive"]),
    ("Class Mod", &["class_mod"]),
    ("Multi", &["multi"]),
    ("Unique", &["unique"]),
    ("Unknown", &["unknown"]),
];

/// A CLI-specific resolved part with display formatting.
struct ResolvedPart {
    slot: &'static str,
    display: String,
    index: u64,
}

impl From<bl4::ResolvedPart> for ResolvedPart {
    fn from(p: bl4::ResolvedPart) -> Self {
        Self {
            slot: p.slot,
            display: p.short_name,
            index: p.index,
        }
    }
}

/// Resolve parts list into display-ready structs using core library.
fn resolve_parts(item: &bl4::ItemSerial) -> Vec<ResolvedPart> {
    item.resolved_parts()
        .into_iter()
        .map(ResolvedPart::from)
        .collect()
}

/// Handle `serial decode` command
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools
)]
pub fn decode(
    serial: &str,
    verbose: bool,
    debug: bool,
    analyze: bool,
    rarity: bool,
    short: bool,
    parts_db: &Path,
    remove: &[String],
    add: &[String],
    level: Option<u8>,
) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;

    // Level editing: modify the level in the header, re-encode, and decode
    if let Some(new_level) = level {
        if new_level == 0 {
            bail!("Level must be at least 1, got {}", new_level);
        }
        let modified = item
            .with_level(new_level)
            .context("Could not find level token in serial header")?;
        let new_serial = modified.original.clone();
        println!("Modified serial: {}\n", new_serial);
        return decode(
            &new_serial,
            verbose,
            debug,
            analyze,
            rarity,
            short,
            parts_db,
            remove,
            add,
            None,
        );
    }

    // Part editing: modify tokens, re-encode, and decode the result
    if !remove.is_empty() || !add.is_empty() {
        let category = item
            .parts_category()
            .context("Cannot edit parts: unable to determine item category")?;

        let mut new_tokens = item.tokens.clone();

        for name in remove {
            let target_index = bl4::manifest::part_index(category, name)
                .with_context(|| format!("part '{}' not found in category {}", name, category))?;
            let before = new_tokens.len();
            new_tokens.retain(|t| {
                !matches!(t, bl4::serial::Token::Part { index, .. } if *index == target_index as u64)
            });
            if new_tokens.len() == before {
                bail!(
                    "part '{}' (index {}) not present in serial tokens",
                    name,
                    target_index
                );
            }
        }

        for name in add {
            let index = if let Ok(raw) = name.parse::<i64>() {
                raw
            } else {
                bl4::manifest::part_index(category, name).with_context(|| {
                    format!("part '{}' not found in category {}", name, category)
                })?
            };
            new_tokens.push(bl4::serial::Token::Part {
                index: index as u64,
                values: vec![],
                encoding: bl4::serial::PartEncoding::None,
            });
        }

        let modified = item.with_tokens(new_tokens);
        let new_serial = modified.encode_from_tokens();
        println!("Modified serial: {}\n", new_serial);
        return decode(
            &new_serial,
            verbose,
            debug,
            analyze,
            rarity,
            short,
            parts_db,
            &[],
            &[],
            None,
        );
    }

    let parts = item.parts_with_names();

    // Derive legendary status from rarity comp parts, not the (unimplemented)
    // rarity field. An item with both comp_04_epic and comp_05_legendary_* is
    // epic, not legendary.
    let core_parts = item.resolved_parts();
    let is_legendary = {
        let mut has_leg = false;
        let mut has_base = false;
        for p in &core_parts {
            if p.slot != "rarity" {
                continue;
            }
            let name = match p.name {
                Some(n) => n.split('.').next_back().unwrap_or(n),
                None => continue,
            };
            if name.starts_with("comp_05_legendary") {
                has_leg = true;
            } else if name.starts_with("comp_0") {
                has_base = true;
            }
        }
        has_leg && !has_base
    };
    let legendary_name =
        bl4::resolve::resolve_item_name(&parts, item.parts_category(), is_legendary);
    let base_name = if let Some((mfr, weapon_type)) = item.weapon_info() {
        format!("{} {}", mfr, weapon_type)
    } else if let Some(group_id) = item.part_group_id() {
        let category_name = bl4::category_name(group_id).unwrap_or("Unknown");
        format!("{} ({})", category_name, group_id)
    } else {
        item.item_type_description().to_string()
    };

    let validation = item.validate();
    let legality_icon = match validation.legality {
        bl4::serial::Legality::Legal => "✓",
        bl4::serial::Legality::Illegal => "✗",
        bl4::serial::Legality::Unknown => "?",
    };

    if let Some(ref leg) = legendary_name {
        println!("{} ({}) {}", base_name, leg, legality_icon);
    } else {
        println!("{} {}", base_name, legality_icon);
    }

    // Show elements if detected
    if let Some(elements) = item.element_names() {
        println!("Element: {}", elements);
    }

    // Show rarity if detected
    if let Some(rarity_name) = item.rarity_name() {
        println!("Rarity: {}", rarity_name);
    }

    // Show level
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

    // Show raw manufacturer ID if we couldn't resolve it
    if item.weapon_info().is_none() {
        if let Some(mfr) = item.manufacturer_name() {
            println!("Manufacturer: {}", mfr);
        } else if let Some(mfr_id) = item.manufacturer {
            println!("Manufacturer ID: {} (unknown)", mfr_id);
        }
    }

    // Verbose: serial internals
    if verbose {
        println!("\nSerial: {}", item.original);
        println!("Format: {} ({})", item.format, item.item_type_description());
        if let Some(seed) = item.seed {
            println!("Seed: {}", seed);
        }
        println!("Decoded bytes: {}", item.raw_bytes.len());
        println!("Hex: {}", item.hex_dump());
        println!("Tokens: {}", item.format_tokens());
    }

    // Parts display
    if !parts.is_empty() {
        if short {
            print_parts_short(&item);
        } else {
            print_parts_grouped(&item, verbose);
        }
    }

    let _ = parts_db;

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

    if rarity {
        print_rarity_estimate(&item);
    }

    Ok(())
}

/// Print parts in short (compact) format: comma-separated on one line.
fn print_parts_short(item: &bl4::ItemSerial) {
    let summary = item.parts_summary();
    if !summary.is_empty() {
        println!("\nParts: {}", summary);
    }
}

/// Print parts grouped by display group.
fn print_parts_grouped(item: &bl4::ItemSerial, verbose: bool) {
    let resolved = resolve_parts(item);
    if resolved.is_empty() {
        return;
    }

    println!();

    // Build groups: map group_name → Vec<ResolvedPart>
    let mut groups: Vec<(&str, Vec<&ResolvedPart>)> = Vec::new();
    for (group_name, slots) in DISPLAY_GROUPS {
        let members: Vec<&ResolvedPart> = resolved
            .iter()
            .filter(|p| slots.contains(&p.slot))
            .collect();
        if !members.is_empty() {
            groups.push((group_name, members));
        }
    }

    // Catch any slots not covered by DISPLAY_GROUPS → Unknown
    let known_slots: Vec<&str> = DISPLAY_GROUPS
        .iter()
        .flat_map(|(_, slots)| slots.iter().copied())
        .collect();
    let uncovered: Vec<&ResolvedPart> = resolved
        .iter()
        .filter(|p| !known_slots.contains(&p.slot))
        .collect();
    if !uncovered.is_empty() {
        // Merge into existing Unknown group or add new one
        if let Some(existing) = groups.iter_mut().find(|(name, _)| *name == "Unknown") {
            existing.1.extend(uncovered);
        } else {
            groups.push(("Unknown", uncovered));
        }
    }

    for (group_name, mut members) in groups {
        members.sort_by(|a, b| a.display.cmp(&b.display));

        if verbose {
            // Verbose: show index within group
            if members.len() == 1 {
                let p = members[0];
                println!("{}: [{:3}] {} ({})", group_name, p.index, p.display, p.slot);
            } else {
                println!("{}:", group_name);
                for p in &members {
                    println!("  - [{:3}] {} ({})", p.index, p.display, p.slot);
                }
            }
        } else if members.len() == 1 {
            println!("{:<12} {}", format!("{}:", group_name), members[0].display);
        } else {
            println!("{}:", group_name);
            for p in &members {
                println!("  - {}", p.display);
            }
        }
    }
}

/// Analyze the first token for group ID research
fn analyze_first_token(item: &bl4::ItemSerial) -> Result<()> {
    use bl4::serial::Token;

    if let Some(first_token) = item.tokens.first() {
        let value = match first_token {
            Token::Var {
                val: v,
                encoding: bl4::serial::VarEncoding::Int,
            } => Some((*v, "VarInt")),
            Token::Var {
                val: v,
                encoding: bl4::serial::VarEncoding::Bit,
            } => Some((*v, "VarBit")),
            _ => None,
        };

        if let Some((value, token_type)) = value {
            println!("\n=== First Token Analysis ===");
            println!("Type:   {}", token_type);
            println!("Value:  {} (decimal)", value);
            println!("Hex:    0x{:x}", value);
            println!("Binary: {:024b}", value);
            println!();

            // Decode Part Group ID based on format and token value
            println!("Part Group ID decoding:");
            match first_token {
                Token::Var {
                    val: v,
                    encoding: bl4::serial::VarEncoding::Bit,
                } => {
                    if let Some(cat) = item.part_group_id() {
                        let divisor = if *v >= 131_072 { 8192 } else { 384 };
                        let offset = value % divisor;
                        println!(
                            "  Formula: category = varbit / {} ({})",
                            divisor,
                            if divisor == 8192 {
                                "weapons"
                            } else {
                                "equipment"
                            }
                        );
                        println!("  Category: {} (offset {})", cat, offset);
                        let name = bl4::category_name(cat).unwrap_or("Unknown");
                        println!("  Identified: {}", name);
                    }
                }
                Token::Var {
                    encoding: bl4::serial::VarEncoding::Int,
                    ..
                } => {
                    if let Some((mfr, wtype)) = item.weapon_info() {
                        println!("  VarInt-first: serial ID {} → {} {}", value, mfr, wtype);
                    } else {
                        println!("  VarInt-first: serial ID {} (not in WEAPON_INFO)", value);
                    }
                    if let Some(cat) = item.parts_category() {
                        let name = bl4::category_name(cat).unwrap_or("Unknown");
                        println!("  Parts category: {} ({})", cat, name);
                    }
                }
                _ => {}
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

fn print_rarity_estimate(item: &bl4::ItemSerial) {
    match item.rarity_estimate() {
        Some(est) => {
            println!("\nRarity estimate:");
            println!(
                "  Tier: {} ({:.6}%, {})",
                est.rarity.name(),
                est.tier_probability * 100.0,
                est.odds_display()
            );
            if let Some(category) = &est.category {
                if let Some(pool_size) = est.pool_size {
                    let world_total = est.world_pool_size.unwrap_or(pool_size);
                    println!(
                        "  Pool: {} ({} legendaries, {} in world pool)",
                        category, pool_size, world_total
                    );
                } else {
                    println!("  Category: {}", category);
                }
            }
            if let Some(per_item) = est.per_item_probability {
                println!(
                    "  Per-item: {:.8}% (~1 in {})",
                    per_item * 100.0,
                    if per_item > 0.0 {
                        format!("{}", (1.0 / per_item).round() as u64)
                    } else {
                        "?".to_string()
                    }
                );
            }
            if let Some(bosses) = est.boss_sources {
                println!("  Boss sources: {}", bosses);
            }
        }
        None => {
            println!("\nRarity estimate: unavailable (insufficient serial data)");
        }
    }
}

/// Handle `serial validate` command
pub fn validate(serials: &[String], verbose: bool) -> Result<()> {
    use bl4::serial::Legality;

    for serial in serials {
        let item = match bl4::ItemSerial::decode(serial) {
            Ok(item) => item,
            Err(e) => {
                println!("✗ {}: decode failed: {}", serial, e);
                continue;
            }
        };

        let result = item.validate();

        let icon = match result.legality {
            Legality::Legal => "✓",
            Legality::Illegal => "✗",
            Legality::Unknown => "?",
        };

        // Build item description
        let desc = if let Some((mfr, wtype)) = item.weapon_info() {
            format!("{} {}", mfr, wtype)
        } else if let Some(group_id) = item.part_group_id() {
            bl4::category_name(group_id)
                .unwrap_or("Unknown")
                .to_string()
        } else {
            item.item_type_description().to_string()
        };

        let truncated = if serial.len() > 30 {
            format!("{}...", &serial[..27])
        } else {
            serial.to_string()
        };

        println!("{} {:<25} {}", icon, desc, truncated);

        if verbose {
            for check in &result.checks {
                let check_icon = match check.passed {
                    Some(true) => "  ✓",
                    Some(false) => "  ✗",
                    None => "  ?",
                };
                println!("{}  {}: {}", check_icon, check.name, check.detail);
            }
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
        "Format: {} ({})",
        item1.format,
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
        "Format: {} ({})",
        item2.format,
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
            if let Token::Part { index, values, .. } = t {
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
            if let Token::Part { index, values, .. } = t {
                if part_indices.contains(index) {
                    if let Some(source_values) = source_parts.get(index) {
                        println!(
                            "  Swapping part {}: {:?} -> {:?}",
                            index, values, source_values
                        );
                        return Token::Part {
                            index: *index,
                            values: source_values.clone(),
                            encoding: bl4::serial::PartEncoding::None,
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

/// View or modify class mod skills
pub fn skills(serial: &str, list: bool, color_filter: Option<&str>, adds: &[String], removes: &[String], force: bool) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;
    let category = item
        .parts_category()
        .context("Cannot determine item category")?;

    if !bl4::skills::is_class_mod(category) {
        let name = bl4::manifest::category_name(category).unwrap_or("Unknown");
        bail!("Not a class mod (category {} = {})", category, name);
    }

    // List available skills
    if list {
        let cat_name = bl4::manifest::category_name(category).unwrap_or("Unknown");
        println!("Available skills for {} (category {}):\n", cat_name, category);

        let mut skills = bl4::manifest::skills_for_category(category);
        skills.sort_by(|a, b| a.1.tree_name.cmp(&b.1.tree_name).then(a.0.cmp(b.0)));

        if let Some(color) = color_filter {
            skills.retain(|(_, info)| info.tree_color == color);
        }

        let mut current_tree = "";
        for (pos, info) in &skills {
            if info.tree_name != current_tree {
                current_tree = &info.tree_name;
                let color = &info.tree_color;
                println!("  {} ({}):", current_tree, color);
            }
            println!("    {} ({})", info.display_name, pos);
        }

        return Ok(());
    }

    let current = bl4::skills::decode_skills(&item.tokens, category);

    // List mode: no modifications
    if adds.is_empty() && removes.is_empty() {
        let cat_name = bl4::manifest::category_name(category).unwrap_or("Unknown");
        println!("Class Mod: {} (category {})", cat_name, category);
        if current.is_empty() {
            println!("  No passive skills found");
        } else {
            println!("Skills:");
            for (i, skill) in current.iter().enumerate() {
                let name = if skill.display_name.is_empty() {
                    &skill.part_name
                } else {
                    &skill.display_name
                };
                println!("  {}. {} ({}) @ {}", i + 1, name, skill.position, skill.tier);
            }
        }
        return Ok(());
    }

    // Parse add/remove specs
    let parsed_adds: Vec<bl4::skills::SkillAdd> = adds
        .iter()
        .map(|s| bl4::skills::parse_add(s, category).map_err(|e| anyhow::anyhow!(e)))
        .collect::<Result<Vec<_>>>()?;

    let parsed_removes: Vec<bl4::skills::SkillRemove> = removes
        .iter()
        .map(|s| bl4::skills::parse_remove(s, category).map_err(|e| anyhow::anyhow!(e)))
        .collect::<Result<Vec<_>>>()?;

    // Validate drops unless --force
    if !force {
        for add in &parsed_adds {
            bl4::skills::validate_skill_drop(&add.position, add.tier, category)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
    }

    // Build and show diff
    let diff = bl4::skills::build_diff(&current, &parsed_adds, &parsed_removes, category)
        .map_err(|e| anyhow::anyhow!(e))?;

    println!("Skills:");
    for entry in &diff {
        match (&entry.before, &entry.after) {
            (Some(before), Some(after)) if entry.changed => {
                let before_name = if before.display_name.is_empty() { &before.position } else { &before.display_name };
                let after_name = if after.display_name.is_empty() { &after.position } else { &after.display_name };
                println!("  {}. {} @ {}  ->  {} @ {}", entry.slot, before_name, before.tier, after_name, after.tier);
            }
            (Some(before), None) => {
                let name = if before.display_name.is_empty() { &before.position } else { &before.display_name };
                println!("  {}. {} @ {}  ->  (removed)", entry.slot, name, before.tier);
            }
            (None, Some(after)) => {
                let name = if after.display_name.is_empty() { &after.position } else { &after.display_name };
                println!("  {}. (empty)  ->  {} @ {}", entry.slot, name, after.tier);
            }
            (Some(skill), _) => {
                let name = if skill.display_name.is_empty() { &skill.position } else { &skill.display_name };
                println!("  {}. {} @ {}  (unchanged)", entry.slot, name, skill.tier);
            }
            _ => {}
        }
    }

    // Confirm unless --force
    if !force {
        print!("\nApply changes? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Apply edits in-place (preserves firmware and other non-skill tokens)
    let (remove_indices, add_parts) =
        bl4::skills::compute_edits(&current, &parsed_adds, &parsed_removes, category)
            .map_err(|e| anyhow::anyhow!(e))?;

    let new_tokens = bl4::skills::apply_edits(&item.tokens, &remove_indices, &add_parts, category);
    let modified = item.with_tokens(new_tokens);
    let new_serial = modified.encode_from_tokens();
    println!("\n{}", new_serial);

    Ok(())
}

/// View or modify equipment firmware
pub fn firmware(serial: &str, list: bool, set: Option<&str>, clear: bool, force: bool) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;
    let category = item
        .parts_category()
        .context("Cannot determine item category")?;

    let cat_name = bl4::manifest::category_name(category).unwrap_or("Unknown");

    // List available firmware
    if list {
        println!("Available firmware for {} (category {}):\n", cat_name, category);
        for (idx, name) in bl4::firmware::available_firmware(category) {
            let display = name.strip_prefix("part_firmware_").unwrap_or(&name);
            println!("  [{}] {}", idx, display);
        }
        return Ok(());
    }

    // Show current firmware
    let current = bl4::firmware::detect(&item.tokens, category);
    if set.is_none() && !clear {
        println!("{} (category {})", cat_name, category);
        match &current {
            Some(fw) => {
                let display = fw.name.strip_prefix("part_firmware_").unwrap_or(&fw.name);
                println!("Firmware: {} (index {} in category {})", display, fw.index, fw.category);
            }
            None => println!("Firmware: none"),
        }
        return Ok(());
    }

    // Clear firmware
    if clear {
        if current.is_none() {
            println!("No firmware to remove.");
            return Ok(());
        }
        let fw = current.as_ref().unwrap();
        let display = fw.name.strip_prefix("part_firmware_").unwrap_or(&fw.name);

        if !force {
            println!("Remove firmware: {}", display);
            print!("Apply? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let new_tokens = bl4::firmware::remove(&item.tokens, category);
        let modified = item.with_tokens(new_tokens);
        println!("{}", modified.encode_from_tokens());
        return Ok(());
    }

    // Set firmware
    if let Some(name) = set {
        let (fw_cat, fw_idx) = bl4::firmware::resolve_firmware(name, category)
            .map_err(|e| anyhow::anyhow!(e))?;

        let new_name = bl4::manifest::part_name(fw_cat, fw_idx).unwrap_or("unknown");
        let new_display = new_name.strip_prefix("part_firmware_").unwrap_or(new_name);

        if !force {
            match &current {
                Some(fw) => {
                    let old_display = fw.name.strip_prefix("part_firmware_").unwrap_or(&fw.name);
                    println!("Firmware: {} -> {}", old_display, new_display);
                }
                None => println!("Firmware: (none) -> {}", new_display),
            }
            print!("Apply? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let new_tokens = bl4::firmware::apply(&item.tokens, fw_idx, category);
        let modified = item.with_tokens(new_tokens);
        println!("{}", modified.encode_from_tokens());
    }

    Ok(())
}
