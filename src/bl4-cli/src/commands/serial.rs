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
    ("Body", &["body", "body_armor", "body_bolt", "body_element", "body_mag"]),
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

/// A resolved part ready for display.
struct ResolvedPart {
    slot: &'static str,
    display: String,
    index: u64,
}

/// Resolve parts list into display-ready structs.
fn resolve_parts(parts: &[(u64, Option<&'static str>, Vec<u64>)]) -> Vec<ResolvedPart> {
    let mut resolved = Vec::new();
    for (index, name, _values) in parts {
        if *index == 0 {
            continue;
        }
        if let Some(element) = bl4::serial::Element::from_index(*index) {
            resolved.push(ResolvedPart {
                slot: "element",
                display: element.name().to_string(),
                index: *index,
            });
        } else if let Some(n) = name {
            let short_name = n.split('.').next_back().unwrap_or(n);
            let slot = bl4::manifest::slot_from_part_name(n);
            resolved.push(ResolvedPart {
                slot,
                display: short_name.to_string(),
                index: *index,
            });
        } else {
            resolved.push(ResolvedPart {
                slot: "unknown",
                display: format!("[{}]", index),
                index: *index,
            });
        }
    }
    resolved
}

/// Try to resolve a legendary name from the parts list.
///
/// Three-pass resolution:
/// 1. Look for `comp_05_legendary_*` suffix in resolved comp parts
/// 2. Look for legendary barrel names (`part_barrel_*_<suffix>`)
/// 3. If legendary with generic barrel, check per-category NCS metadata
fn resolve_legendary_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
    is_legendary: bool,
) -> Option<String> {
    if let Some(name) = resolve_from_legendary_comp(parts) {
        return Some(name);
    }

    let (barrel_result, generic_barrel_base) = resolve_from_barrel_parts(parts);
    if let Some(name) = barrel_result {
        return Some(name);
    }

    if is_legendary {
        return resolve_from_category_metadata(category, generic_barrel_base);
    }

    None
}

/// Pass 1: scan parts for `comp_05_legendary_*` suffix.
fn resolve_from_legendary_comp(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
) -> Option<String> {
    for (_index, name, _values) in parts {
        if let Some(n) = name {
            let segment = n.split('.').next_back().unwrap_or(n);
            if let Some(suffix) = segment.strip_prefix("comp_05_legendary_") {
                if !suffix.is_empty() {
                    return match_legendary_suffix(suffix);
                }
            }
        }
    }
    None
}

/// Pass 2: scan barrel parts for legendary suffixes.
/// Returns (legendary_name, generic_barrel_base) where the barrel base
/// is needed for pass 3 if no legendary barrel was found.
fn resolve_from_barrel_parts(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
) -> (Option<String>, Option<&'static str>) {
    let mut generic_barrel_base: Option<&str> = None;
    let mut barrel_candidates: Vec<&str> = Vec::new();

    for (_index, name, _values) in parts {
        if let Some(n) = name {
            let segment = n.split('.').next_back().unwrap_or(n);
            if let Some(suffix) = legendary_barrel_suffix(segment) {
                barrel_candidates.push(suffix);
            }
            if generic_barrel_base.is_none() {
                generic_barrel_base = generic_barrel_base_name(segment);
            }
        }
    }

    if barrel_candidates.is_empty() {
        return (None, generic_barrel_base);
    }

    // If the weapon has a generic barrel (part_barrel_01/02), the legendary
    // identity likely comes from per-category NCS metadata (pass 3), not the
    // shared barrel namespace. Shared barrel indices collide across legendaries,
    // so defer to pass 3 when a generic barrel is also present.
    if generic_barrel_base.is_some() {
        return (None, generic_barrel_base);
    }

    // Prefer a candidate that matches KNOWN_LEGENDARIES
    if let Some(known) = best_known_legendary(&barrel_candidates) {
        return (Some(known), generic_barrel_base);
    }

    // Prefer last candidate (later serial positions are more weapon-specific)
    let name = match_legendary_suffix(barrel_candidates.last().unwrap());
    (name, generic_barrel_base)
}

/// Check barrel suffixes against KNOWN_LEGENDARIES for a match.
fn best_known_legendary(candidates: &[&str]) -> Option<String> {
    for suffix in candidates {
        let suffix_lower = suffix.to_lowercase();
        for leg in bl4::KNOWN_LEGENDARIES {
            let leg_segment = leg.internal.split('.').next_back().unwrap_or(leg.internal);
            if let Some(leg_suffix) = leg_segment.strip_prefix("comp_05_legendary_") {
                if leg_suffix.to_lowercase() == suffix_lower {
                    return Some(leg.name.to_string());
                }
            }
        }
    }
    None
}

/// Pass 3: check per-category NCS metadata for legendary barrel alias.
fn resolve_from_category_metadata(
    category: Option<i64>,
    generic_barrel_base: Option<&str>,
) -> Option<String> {
    let cat = category?;
    let barrel_base = generic_barrel_base?;
    let alias = bl4::manifest::legendary_barrel_alias(cat, barrel_base)?;
    let segment = alias.split('.').next_back().unwrap_or(alias);
    let prefix = format!("part_{}_", barrel_base);
    let suffix = segment.strip_prefix(&prefix)?;
    match_legendary_suffix(suffix)
}

/// Extract legendary suffix from a barrel part name.
///
/// Returns Some(suffix) for legendary barrels like:
/// - `part_barrel_01_seamstress` → "seamstress"
/// - `part_barrel_goldengod` → "goldengod"
///
/// Returns None for generic/sub-variant/licensed barrels.
fn legendary_barrel_suffix(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("part_barrel_")?;

    if rest.starts_with("licensed_") {
        return None;
    }

    for prefix in ["01_", "02_"] {
        if let Some(suffix) = rest.strip_prefix(prefix) {
            // Single-letter sub-variants (a, b, c, d) are barrel accessories, not legendaries
            if suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_lowercase()) {
                return None;
            }
            return Some(suffix);
        }
    }

    // Bare "01" or "02" = generic barrel
    if rest == "01" || rest == "02" {
        return None;
    }

    // part_barrel_<suffix> (no 01/02 prefix, e.g., part_barrel_goldengod)
    Some(rest)
}

/// Extract the generic barrel base from a barrel part name.
///
/// Returns Some("barrel_01") for `part_barrel_01`, Some("barrel_02") for `part_barrel_02`.
fn generic_barrel_base_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("part_")?;
    if rest == "barrel_01" || rest == "barrel_02" {
        Some(rest)
    } else {
        None
    }
}

/// Match a legendary suffix against KNOWN_LEGENDARIES, falling back to title-case.
fn match_legendary_suffix(suffix: &str) -> Option<String> {
    let suffix_lower = suffix.to_lowercase();

    for leg in bl4::KNOWN_LEGENDARIES {
        let leg_segment = leg.internal.split('.').next_back().unwrap_or(leg.internal);
        if let Some(leg_suffix) = leg_segment.strip_prefix("comp_05_legendary_") {
            if leg_suffix.to_lowercase() == suffix_lower {
                return Some(leg.name.to_string());
            }
        }
    }

    // No known match — title-case the suffix
    let title = suffix
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(title)
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
) -> Result<()> {
    let item = bl4::ItemSerial::decode(serial).context("Failed to decode serial")?;

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
                bl4::manifest::part_index(category, name)
                    .with_context(|| format!("part '{}' not found in category {}", name, category))?
            };
            new_tokens.push(bl4::serial::Token::Part {
                index: index as u64,
                values: vec![],
            });
        }

        let modified = item.with_tokens(new_tokens);
        let new_serial = modified.encode_from_tokens();
        println!("Modified serial: {}\n", new_serial);
        return decode(
            &new_serial, verbose, debug, analyze, rarity, short, parts_db, &[], &[],
        );
    }

    let parts = item.parts_with_names();

    // Build header with optional legendary name
    let is_legendary = item
        .rarity
        .map(|r| r == bl4::serial::Rarity::Legendary)
        .unwrap_or(false);
    let legendary_name = resolve_legendary_name(&parts, item.parts_category(), is_legendary);
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
        println!(
            "Format: {} ({})",
            item.format,
            item.item_type_description()
        );
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
            print_parts_grouped(&parts, verbose);
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
fn print_parts_grouped(parts: &[(u64, Option<&'static str>, Vec<u64>)], verbose: bool) {
    let resolved = resolve_parts(parts);
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

            // Decode Part Group ID based on format and token value
            println!("Part Group ID decoding:");
            match first_token {
                Token::VarBit(v) => {
                    if let Some(cat) = item.part_group_id() {
                        let divisor = if *v >= 131_072 { 8192 } else { 384 };
                        let offset = value % divisor;
                        println!("  Formula: category = varbit / {} ({})", divisor,
                            if divisor == 8192 { "weapons" } else { "equipment" });
                        println!("  Category: {} (offset {})", cat, offset);
                        let name = bl4::category_name(cat).unwrap_or("Unknown");
                        println!("  Identified: {}", name);
                    }
                }
                Token::VarInt(_) => {
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
