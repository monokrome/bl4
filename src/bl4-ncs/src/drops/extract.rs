//! Drop extraction from NCS data files

use super::types::{BossNameMapping, DropEntry, DropProbabilities, DropsManifest, DropSource};
use crate::document::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Recursively collect all Leaf string values from a Value tree
fn collect_leaf_strings(value: &Value) -> Vec<&str> {
    match value {
        Value::Leaf(s) => vec![s.as_str()],
        Value::Array(arr) => arr.iter().flat_map(collect_leaf_strings).collect(),
        Value::Map(map) => map.values().flat_map(collect_leaf_strings).collect(),
        Value::Ref { r#ref: s } => vec![s.as_str()],
        Value::Null => Vec::new(),
    }
}

/// Extract drops from itempoollist NCS data
///
/// Parses itempoollist.bin content and extracts boss → legendary mappings.
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub fn extract_drops_from_itempoollist(data: &[u8]) -> Vec<DropEntry> {
    let doc = match crate::parse::parse(data) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut drops = Vec::new();
    let mut current_boss: Option<String> = None;
    let mut is_true_boss = false;

    for table in doc.tables.values() {
        for record in &table.records {
            for entry in &record.entries {
                let name = &entry.key;

                // Boss pool pattern: ItemPoolList_<BossName>
                if name.starts_with("ItemPoolList_") && !name.contains("Enemy_BaseLoot") {
                    if name.ends_with("_TrueBoss") {
                        is_true_boss = true;
                    } else {
                        current_boss = Some(name.replace("ItemPoolList_", ""));
                        is_true_boss = false;
                    }

                    // Extract items from entry value tree
                    let boss = current_boss.as_ref().unwrap();
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_")
                            && !s.starts_with("itempool_")
                        {
                            if let Some(mut drop_entry) =
                                parse_legendary_item_id(boss, s, DropSource::Boss)
                            {
                                if is_true_boss {
                                    drop_entry.drop_tier = "TrueBoss".to_string();
                                }
                                drops.push(drop_entry);
                            }
                        }
                    }
                    continue;
                }

                // Skip if no current boss
                let boss = match &current_boss {
                    Some(b) => b.clone(),
                    None => continue,
                };

                // Check if this is a tier record with items in its value tree
                if let Some(tier) = extract_tier_name(name) {
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_")
                            && !s.starts_with("itempool_")
                        {
                            if let Some(mut drop_entry) =
                                parse_legendary_item_id(&boss, s, DropSource::Boss)
                            {
                                drop_entry.drop_tier =
                                    if is_true_boss && !tier.is_empty() {
                                        format!("TrueBoss{}", tier)
                                    } else if is_true_boss {
                                        "TrueBoss".to_string()
                                    } else {
                                        tier.clone()
                                    };
                                drops.push(drop_entry);
                            }
                        }
                    }
                }
                // Check if entry key itself is a legendary item
                else if name.to_lowercase().contains(".comp_05_legendary_") {
                    if let Some(mut drop_entry) =
                        parse_legendary_item_id(&boss, name, DropSource::Boss)
                    {
                        if is_true_boss {
                            drop_entry.drop_tier = "TrueBoss".to_string();
                        }
                        drops.push(drop_entry);
                    }

                    // Also extract nested items from this entry's value tree
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_")
                            && !s.starts_with("itempool_")
                        {
                            if let Some(mut drop_entry) =
                                parse_legendary_item_id(&boss, s, DropSource::Boss)
                            {
                                if is_true_boss {
                                    drop_entry.drop_tier = "TrueBoss".to_string();
                                }
                                drops.push(drop_entry);
                            }
                        }
                    }
                }
            }
        }
    }

    drops
}

/// Extract tier name from a tier reference string
fn extract_tier_name(s: &str) -> Option<String> {
    let tier_prefixes = [
        ("Primary_", "Primary"),
        ("Secondary_", "Secondary"),
        ("Tertiary_", "Tertiary"),
        ("Quaternary_", "Quaternary"),
        ("Shiny_", "Shiny"),
        ("TrueBoss_", ""),
        ("TrueBossShiny_", "Shiny"),
    ];

    for (prefix, tier) in tier_prefixes {
        if let Some(rest) = s.strip_prefix(prefix) {
            if rest.contains('_') && rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Some(tier.to_string());
            }
        }
    }
    None
}

/// Extract drops from itempool NCS data
///
/// Parses itempool.bin content and extracts black market, fish collector, and mission drops
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub fn extract_drops_from_itempool(data: &[u8]) -> Vec<DropEntry> {
    let doc = match crate::parse::parse(data) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut drops = Vec::new();

    for table in doc.tables.values() {
        for record in &table.records {
            for entry in &record.entries {
                let name = &entry.key;

                // Black Market items
                if name.starts_with("ItemPool_BlackMarket_") {
                    let item_part = name.replace("ItemPool_BlackMarket_Comp_", "");
                    if let Some(drop_entry) = parse_black_market_item(&item_part) {
                        drops.push(drop_entry);
                    }
                }

                // Fish Collector rewards
                if name.starts_with("ItemPool_FishCollector_Reward_") {
                    let tier = name.replace("ItemPool_FishCollector_Reward_", "");
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_") {
                            if let Some(mut drop_entry) =
                                parse_legendary_item_id("Fish Collector", s, DropSource::Special)
                            {
                                drop_entry.drop_tier = tier.clone();
                                drops.push(drop_entry);
                            }
                        }
                    }
                }

                // Side mission rewards
                if name.starts_with("ItemPool_SideMission_") && !name.ends_with("_TurretDrop") {
                    let mission_name = name
                        .replace("ItemPool_SideMission_", "")
                        .replace('_', " ");
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_") {
                            if let Some(drop_entry) =
                                parse_legendary_item_id(&mission_name, s, DropSource::Mission)
                            {
                                drops.push(drop_entry);
                            }
                        }
                    }
                }

                // Main mission rewards
                if name.starts_with("ItemPool_MainMission_") {
                    let mission_name = name
                        .replace("ItemPool_MainMission_", "")
                        .replace('_', " ");
                    for s in collect_leaf_strings(&entry.value) {
                        if s.to_lowercase().contains(".comp_05_legendary_") {
                            if let Some(drop_entry) =
                                parse_legendary_item_id(&mission_name, s, DropSource::Mission)
                            {
                                drops.push(drop_entry);
                            }
                        }
                    }
                }
            }
        }
    }

    drops
}

/// Parse black market item from pool name
fn parse_black_market_item(item_part: &str) -> Option<DropEntry> {
    let parts: Vec<&str> = item_part.split('_').collect();
    if parts.len() < 3 {
        return None;
    }

    let manu = parts[0].to_uppercase();
    let gear_type = parts[1].to_uppercase();
    let item_name = parts[2..].join("_");

    Some(DropEntry {
        source: "Black Market".to_string(),
        source_display: Some("Black Market".to_string()),
        source_type: DropSource::BlackMarket,
        manufacturer: manu.clone(),
        gear_type: gear_type.clone(),
        item_name,
        item_id: format!(
            "{}_{}.comp_blackmarket",
            manu.to_lowercase(),
            gear_type.to_lowercase()
        ),
        pool: format!(
            "itempool_blackmarket_comp_{}_{}",
            manu.to_lowercase(),
            gear_type.to_lowercase()
        ),
        drop_tier: String::new(),
        drop_chance: 0.0,
    })
}

fn parse_legendary_item_id(source: &str, item_id: &str, source_type: DropSource) -> Option<DropEntry> {
    let parts: Vec<&str> = item_id.split('.').collect();
    if parts.len() != 2 {
        return None;
    }

    let prefix = parts[0];
    let comp_part = parts[1];

    let prefix_parts: Vec<&str> = prefix.split('_').collect();
    if prefix_parts.len() < 2 {
        return None;
    }

    let manu = prefix_parts[0].to_uppercase();
    let gear_type = prefix_parts[1..].join("_").to_uppercase();

    let comp_lower = comp_part.to_lowercase();
    let item_name = if let Some(pos) = comp_lower.find("comp_05_legendary_") {
        &comp_part[pos + "comp_05_legendary_".len()..]
    } else {
        return None;
    };
    if item_name.is_empty() {
        return None;
    }

    Some(DropEntry {
        source: source.to_string(),
        source_display: None,
        source_type,
        manufacturer: manu.clone(),
        gear_type: gear_type.clone(),
        item_name: item_name.to_string(),
        item_id: item_id.to_string(),
        pool: format!(
            "itempool_{}_{}_05_legendary_{}_shiny",
            manu.to_lowercase(),
            gear_type.to_lowercase(),
            item_name
        ),
        drop_tier: String::new(),
        drop_chance: 0.0,
    })
}

/// Generate world drop entries from existing drops
#[allow(clippy::too_many_lines)]
fn generate_world_drops(existing_drops: &[DropEntry]) -> Vec<DropEntry> {
    let world_drop_gear_types = [
        "AR", "PS", "SM", "SG", "SR", "SHIELD", "GRENADE_GADGET", "HW", "REPAIR_KIT",
    ];

    let mut items_by_type: HashMap<String, Vec<String>> = HashMap::new();
    let mut item_details: HashMap<String, (String, String, String)> = HashMap::new();

    for drop in existing_drops {
        if world_drop_gear_types.contains(&drop.gear_type.as_str()) {
            items_by_type
                .entry(drop.gear_type.clone())
                .or_default()
                .push(drop.item_id.clone());
            item_details.insert(
                drop.item_id.clone(),
                (
                    drop.manufacturer.clone(),
                    drop.gear_type.clone(),
                    drop.item_name.clone(),
                ),
            );
        }
    }

    for items in items_by_type.values_mut() {
        items.sort();
        items.dedup();
    }

    let mut world_drops = Vec::new();
    let mut seen_items: HashSet<String> = HashSet::new();

    for (gear_type, items) in &items_by_type {
        let pool_size = items.len();
        if pool_size == 0 {
            continue;
        }

        let per_item_chance = 1.0 / pool_size as f64;

        let pool_name = match gear_type.as_str() {
            "AR" => "Assault Rifles",
            "PS" => "Pistols",
            "SM" => "SMGs",
            "SG" => "Shotguns",
            "SR" => "Sniper Rifles",
            "SHIELD" => "Shields",
            "GRENADE_GADGET" => "Grenades",
            "HW" => "Heavy Weapons",
            "REPAIR_KIT" => "Repair Kits",
            _ => gear_type,
        };

        for item_id in items {
            if seen_items.contains(item_id) {
                continue;
            }
            seen_items.insert(item_id.clone());

            if let Some((manu, gtype, name)) = item_details.get(item_id) {
                let display = format!("World Drop ({})", pool_name);
                world_drops.push(DropEntry {
                    source: display.clone(),
                    source_display: Some(display),
                    source_type: DropSource::WorldDrop,
                    manufacturer: manu.clone(),
                    gear_type: gtype.clone(),
                    item_name: name.clone(),
                    item_id: item_id.clone(),
                    pool: format!("itempool_{}_05_legendary", gtype.to_lowercase()),
                    drop_tier: String::new(),
                    drop_chance: per_item_chance,
                });
            }
        }
    }

    world_drops
}

/// Generate drops manifest from NCS data directory
///
/// Scans for itempoollist.bin and itempool.bin files and extracts all drops.
#[allow(clippy::too_many_lines)]
pub fn generate_drops_manifest<P: AsRef<Path>>(ncs_dir: P) -> Result<DropsManifest, std::io::Error> {
    let boss_names = BossNameMapping::load();
    let name_data = crate::name_data::extract_from_directory(ncs_dir.as_ref());

    let mut all_drops = Vec::new();
    let mut seen = HashSet::new();

    for entry in walkdir::WalkDir::new(ncs_dir.as_ref())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let filename = path.file_name().map(|n| n.to_string_lossy());

        if let Some(name) = filename {
            let drops = if name == "itempoollist.bin" {
                let data = std::fs::read(path)?;
                extract_drops_from_itempoollist(&data)
            } else if name == "itempool.bin" {
                let data = std::fs::read(path)?;
                extract_drops_from_itempool(&data)
            } else {
                continue;
            };

            for mut drop in drops {
                let key = (drop.source.clone(), drop.item_id.clone());
                if !seen.contains(&key) {
                    seen.insert(key);

                    if drop.source_type == DropSource::Boss && drop.source_display.is_none() {
                        if let Some(display) = boss_names.get_display_name(&drop.source) {
                            drop.source_display = Some(display.to_string());
                        } else if let Some(display) = name_data.find_display_name(&drop.source) {
                            drop.source_display = Some(display.to_string());
                        }
                    }

                    all_drops.push(drop);
                }
            }
        }
    }

    let world_drops = generate_world_drops(&all_drops);
    for drop in world_drops {
        let key = (drop.source.clone(), drop.item_id.clone());
        if !seen.contains(&key) {
            seen.insert(key);
            all_drops.push(drop);
        }
    }

    all_drops.sort_by(|a, b| {
        let type_order = |t: &DropSource| match t {
            DropSource::Boss => 0,
            DropSource::Mission => 1,
            DropSource::BlackMarket => 2,
            DropSource::Special => 3,
            DropSource::WorldDrop => 4,
        };
        let tier_order = |t: &str| match t {
            "Primary" => 0,
            "Secondary" => 1,
            "Tertiary" => 2,
            _ => 3,
        };
        (
            type_order(&a.source_type),
            &a.source,
            tier_order(&a.drop_tier),
        )
            .cmp(&(
                type_order(&b.source_type),
                &b.source,
                tier_order(&b.drop_tier),
            ))
    });

    Ok(DropsManifest {
        version: 1,
        drops: all_drops,
        probabilities: DropProbabilities::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_leaf_strings_leaf() {
        let value = Value::Leaf("hello".to_string());
        assert_eq!(collect_leaf_strings(&value), vec!["hello"]);
    }

    #[test]
    fn test_collect_leaf_strings_null() {
        let value = Value::Null;
        assert!(collect_leaf_strings(&value).is_empty());
    }

    #[test]
    fn test_collect_leaf_strings_array() {
        let value = Value::Array(vec![
            Value::Leaf("a".to_string()),
            Value::Null,
            Value::Leaf("b".to_string()),
        ]);
        assert_eq!(collect_leaf_strings(&value), vec!["a", "b"]);
    }

    #[test]
    fn test_collect_leaf_strings_map() {
        let mut map = HashMap::new();
        map.insert("k1".to_string(), Value::Leaf("v1".to_string()));
        map.insert("k2".to_string(), Value::Leaf("v2".to_string()));
        let value = Value::Map(map);
        let result = collect_leaf_strings(&value);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"v1"));
        assert!(result.contains(&"v2"));
    }

    #[test]
    fn test_collect_leaf_strings_ref() {
        let value = Value::Ref {
            r#ref: "some_ref".to_string(),
        };
        assert_eq!(collect_leaf_strings(&value), vec!["some_ref"]);
    }

    #[test]
    fn test_collect_leaf_strings_nested() {
        let mut inner_map = HashMap::new();
        inner_map.insert("deep".to_string(), Value::Leaf("found_it".to_string()));

        let value = Value::Array(vec![
            Value::Leaf("top".to_string()),
            Value::Map(inner_map),
            Value::Array(vec![Value::Leaf("nested".to_string())]),
        ]);
        let result = collect_leaf_strings(&value);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"top"));
        assert!(result.contains(&"found_it"));
        assert!(result.contains(&"nested"));
    }

    #[test]
    fn test_extract_tier_name() {
        // Function checks for tier prefixes like "Primary_", "Shiny_" etc.
        // followed by a digit and underscore
        assert_eq!(
            extract_tier_name("Primary_01_SomePool"),
            Some("Primary".to_string())
        );
        assert_eq!(
            extract_tier_name("Shiny_42_Something"),
            Some("Shiny".to_string())
        );
        assert_eq!(
            extract_tier_name("TrueBoss_1_Boss"),
            Some("".to_string())
        );
        assert_eq!(extract_tier_name("SomethingElse"), None);
        assert_eq!(extract_tier_name("Primary_NoDig"), None);
    }
}
