//! Drop rate and location lookup for legendary items
//!
//! Provides a database of boss → legendary item mappings with drop probabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Drop source type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DropSource {
    /// Dedicated boss drop
    Boss,
    /// World drop (general legendary pool)
    WorldDrop,
    /// Black Market exclusive
    BlackMarket,
    /// Side mission reward
    Mission,
    /// Special source (Fish Collector, challenges, etc.)
    Special,
}

impl std::fmt::Display for DropSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DropSource::Boss => write!(f, "Boss"),
            DropSource::WorldDrop => write!(f, "World Drop"),
            DropSource::BlackMarket => write!(f, "Black Market"),
            DropSource::Mission => write!(f, "Mission"),
            DropSource::Special => write!(f, "Special"),
        }
    }
}

/// A single drop entry mapping a source to an item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEntry {
    /// Source name (internal boss name, "Black Market", mission name, etc.)
    pub source: String,
    /// Display name for the source (human-readable, from NameData)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_display: Option<String>,
    /// Source type
    pub source_type: DropSource,
    /// Manufacturer code (e.g., "JAK", "VLA")
    pub manufacturer: String,
    /// Gear type code (e.g., "PS", "SR", "SG", "SM", "AR", "SHIELD")
    pub gear_type: String,
    /// Item display name (e.g., "Hellwalker", "PlasmaCoil")
    pub item_name: String,
    /// Full item ID (e.g., "JAK_SG.comp_05_legendary_Hellwalker")
    pub item_id: String,
    /// Internal pool name
    pub pool: String,
    /// Drop tier (Primary, Secondary, Tertiary, or empty for non-boss)
    pub drop_tier: String,
    /// Drop probability (0.0 to 1.0, or 0 if unknown)
    pub drop_chance: f64,
}

/// Drop probability tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropProbabilities {
    #[serde(rename = "Primary")]
    pub primary: f64,
    #[serde(rename = "Secondary")]
    pub secondary: f64,
    #[serde(rename = "Tertiary")]
    pub tertiary: f64,
    #[serde(rename = "Shiny")]
    pub shiny: f64,
    #[serde(rename = "TrueBoss")]
    pub true_boss: f64,
    #[serde(rename = "TrueBossShiny")]
    pub true_boss_shiny: f64,
}

impl Default for DropProbabilities {
    fn default() -> Self {
        Self {
            primary: 0.20,
            secondary: 0.08,
            tertiary: 0.03,
            shiny: 0.01,
            true_boss: 0.08,
            true_boss_shiny: 0.03,
        }
    }
}

/// The drops database manifest format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropsManifest {
    pub version: u32,
    pub drops: Vec<DropEntry>,
    pub probabilities: DropProbabilities,
}

/// Result of a drop location query
#[derive(Debug, Clone)]
pub struct DropLocation {
    /// Item name
    pub item_name: String,
    /// Source name (internal boss name, "Black Market", etc.)
    pub source: String,
    /// Display name for the source (human-readable)
    pub source_display: Option<String>,
    /// Source type
    pub source_type: DropSource,
    /// Drop tier (for bosses)
    pub tier: String,
    /// Drop chance (0.0 to 1.0)
    pub chance: f64,
    /// Chance as percentage string (e.g., "20%")
    pub chance_display: String,
}

/// Drops database for querying item drop locations
pub struct DropsDb {
    manifest: DropsManifest,
    /// Index: lowercase item name → entries
    by_name: HashMap<String, Vec<usize>>,
    /// Index: item_id → entries
    by_id: HashMap<String, Vec<usize>>,
    /// Index: source name (internal) → entries
    by_source: HashMap<String, Vec<usize>>,
    /// Index: source display name → entries
    by_source_display: HashMap<String, Vec<usize>>,
}

impl DropsDb {
    /// Load drops database from a manifest file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let manifest: DropsManifest = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Self::from_manifest(manifest))
    }

    /// Create from an already-loaded manifest
    pub fn from_manifest(manifest: DropsManifest) -> Self {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_id: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_source: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_source_display: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, entry) in manifest.drops.iter().enumerate() {
            // Index by lowercase item name
            let name_key = entry.item_name.to_lowercase();
            by_name.entry(name_key).or_default().push(i);

            // Index by item_id
            by_id.entry(entry.item_id.clone()).or_default().push(i);

            // Index by source (internal name)
            let source_key = entry.source.to_lowercase();
            by_source.entry(source_key).or_default().push(i);

            // Index by source display name (if available)
            if let Some(ref display) = entry.source_display {
                let display_key = display.to_lowercase();
                by_source_display.entry(display_key).or_default().push(i);
            }
        }

        Self {
            manifest,
            by_name,
            by_id,
            by_source,
            by_source_display,
        }
    }

    /// Find drop locations for an item by name (fuzzy match)
    ///
    /// Returns locations sorted by drop chance (highest first)
    pub fn find_by_name(&self, query: &str) -> Vec<DropLocation> {
        let query_lower = query.to_lowercase();
        // Also try with spaces removed and underscores removed
        let query_no_space = query_lower.replace(' ', "");
        let query_underscore = query_lower.replace(' ', "_");

        // Try exact match first (with variations)
        for q in [&query_lower, &query_no_space, &query_underscore] {
            if let Some(indices) = self.by_name.get(q) {
                return self.indices_to_locations(indices);
            }
        }

        // Try partial match
        let mut matches: Vec<usize> = Vec::new();
        for (name, indices) in &self.by_name {
            if name.contains(&query_lower)
                || name.contains(&query_no_space)
                || query_lower.contains(name)
                || query_no_space.contains(name)
            {
                matches.extend(indices);
            }
        }

        // Also check if query matches manufacturer_type pattern (e.g., "JAK_SG")
        let query_parts: Vec<&str> = query.split(|c| c == '_' || c == ' ').collect();
        if query_parts.len() >= 2 {
            let manu = query_parts[0].to_uppercase();
            let wtype = query_parts[1].to_uppercase();
            for (i, entry) in self.manifest.drops.iter().enumerate() {
                if entry.manufacturer == manu && entry.gear_type == wtype {
                    if query_parts.len() > 2 {
                        // Also check item name
                        let item_query = query_parts[2..].join("_").to_lowercase();
                        if entry.item_name.to_lowercase().contains(&item_query) {
                            if !matches.contains(&i) {
                                matches.push(i);
                            }
                        }
                    } else if !matches.contains(&i) {
                        matches.push(i);
                    }
                }
            }
        }

        self.indices_to_locations(&matches)
    }

    /// Find all items dropped by a specific source (boss, black market, etc.)
    ///
    /// Searches both internal names and display names.
    /// Returns items sorted by drop chance (highest first)
    pub fn find_by_source(&self, source: &str) -> Vec<&DropEntry> {
        let source_lower = source.to_lowercase();
        // Try variations: with underscores, without spaces
        let source_underscore = source_lower.replace(' ', "_");
        let source_no_space = source_lower.replace(' ', "");

        let mut results: Vec<&DropEntry> = Vec::new();
        let mut seen_indices = std::collections::HashSet::new();

        // Try exact match on internal name (with variations)
        let exact_match = self
            .by_source
            .get(&source_lower)
            .or_else(|| self.by_source.get(&source_underscore))
            .or_else(|| self.by_source.get(&source_no_space));

        if let Some(indices) = exact_match {
            for &i in indices {
                if seen_indices.insert(i) {
                    results.push(&self.manifest.drops[i]);
                }
            }
        }

        // Try exact match on display name
        let display_match = self
            .by_source_display
            .get(&source_lower)
            .or_else(|| self.by_source_display.get(&source_underscore))
            .or_else(|| self.by_source_display.get(&source_no_space));

        if let Some(indices) = display_match {
            for &i in indices {
                if seen_indices.insert(i) {
                    results.push(&self.manifest.drops[i]);
                }
            }
        }

        // If no exact matches, try partial match
        if results.is_empty() {
            // Partial match on internal names
            for (name, indices) in &self.by_source {
                if name.contains(&source_lower)
                    || name.contains(&source_underscore)
                    || name.contains(&source_no_space)
                    || source_lower.contains(name)
                    || source_no_space.contains(name)
                {
                    for &i in indices {
                        if seen_indices.insert(i) {
                            results.push(&self.manifest.drops[i]);
                        }
                    }
                }
            }

            // Partial match on display names
            for (name, indices) in &self.by_source_display {
                if name.contains(&source_lower)
                    || name.contains(&source_underscore)
                    || name.contains(&source_no_space)
                    || source_lower.contains(name)
                    || source_no_space.contains(name)
                {
                    for &i in indices {
                        if seen_indices.insert(i) {
                            results.push(&self.manifest.drops[i]);
                        }
                    }
                }
            }
        }

        // Sort by drop chance (highest first)
        results.sort_by(|a, b| b.drop_chance.partial_cmp(&a.drop_chance).unwrap());
        results
    }

    /// Find all items dropped by a specific boss (alias for find_by_source)
    pub fn find_by_boss(&self, boss: &str) -> Vec<&DropEntry> {
        self.find_by_source(boss)
    }

    /// Get all unique item names in the database
    pub fn all_items(&self) -> Vec<&str> {
        let mut items: Vec<&str> = self.by_name.keys().map(|s| s.as_str()).collect();
        items.sort();
        items
    }

    /// Get all unique source names in the database
    pub fn all_sources(&self) -> Vec<&str> {
        let mut sources: Vec<&str> = self.by_source.keys().map(|s| s.as_str()).collect();
        sources.sort();
        sources
    }

    /// Get all unique boss names (sources with type Boss)
    pub fn all_bosses(&self) -> Vec<&str> {
        let mut bosses: Vec<&str> = self
            .manifest
            .drops
            .iter()
            .filter(|e| e.source_type == DropSource::Boss)
            .map(|e| e.source.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        bosses.sort();
        bosses
    }

    /// Get the drop probabilities
    pub fn probabilities(&self) -> &DropProbabilities {
        &self.manifest.probabilities
    }

    fn indices_to_locations(&self, indices: &[usize]) -> Vec<DropLocation> {
        let mut locations: Vec<DropLocation> = indices
            .iter()
            .map(|&i| {
                let entry = &self.manifest.drops[i];
                DropLocation {
                    item_name: entry.item_name.clone(),
                    source: entry.source.clone(),
                    source_display: entry.source_display.clone(),
                    source_type: entry.source_type.clone(),
                    tier: entry.drop_tier.clone(),
                    chance: entry.drop_chance,
                    chance_display: format!("{:.2}%", entry.drop_chance * 100.0),
                }
            })
            .collect();

        // Sort by drop chance (highest first)
        locations.sort_by(|a, b| b.chance.partial_cmp(&a.chance).unwrap());

        // Deduplicate by source (keep highest chance)
        let mut seen = std::collections::HashSet::new();
        locations.retain(|loc| seen.insert(loc.source.clone()));

        locations
    }
}

/// Extract drops from itempoollist NCS data
///
/// Parses itempoollist.bin content and extracts boss → legendary mappings
pub fn extract_drops_from_itempoollist(data: &[u8]) -> Vec<DropEntry> {
    let doc = match crate::parser::parse_document(data) {
        Some(d) => d,
        None => return Vec::new(),
    };

    // Find boss→legendary mappings in the record names
    let mut drops = Vec::new();
    let mut current_boss: Option<String> = None;

    for record in &doc.records {
        let name = &record.name;

        // Boss pool pattern: ItemPoolList_<BossName>
        if name.starts_with("ItemPoolList_")
            && !name.contains("Enemy_BaseLoot")
            && !name.ends_with("_TrueBoss")
        {
            current_boss = Some(name.replace("ItemPoolList_", ""));
        }
        // Legendary item ID pattern: MANU_TYPE.comp_05_legendary_<name> (case-insensitive)
        else if name.to_lowercase().contains(".comp_05_legendary_") {
            if let Some(ref boss) = current_boss {
                if let Some(entry) = parse_legendary_item_id(boss, name, DropSource::Boss) {
                    drops.push(entry);
                }
            }
        }

        // Also check field values for legendary items (shields, gear, etc.)
        if let Some(ref boss) = current_boss {
            for value in record.fields.values() {
                if let crate::parser::Value::String(s) = value {
                    if s.to_lowercase().contains(".comp_05_legendary_") {
                        if let Some(entry) = parse_legendary_item_id(boss, s, DropSource::Boss) {
                            drops.push(entry);
                        }
                    }
                }
            }
        }
    }

    // Assign drop tiers based on order per boss
    let mut by_source: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, entry) in drops.iter().enumerate() {
        by_source.entry(entry.source.clone()).or_default().push(i);
    }

    let tiers = ["Primary", "Secondary", "Tertiary"];
    let probs = [0.20, 0.08, 0.03];

    for indices in by_source.values() {
        for (i, &idx) in indices.iter().enumerate() {
            let tier_idx = i.min(2);
            drops[idx].drop_tier = tiers[tier_idx].to_string();
            drops[idx].drop_chance = probs[tier_idx];
        }
    }

    drops
}

/// Extract drops from itempool NCS data
///
/// Parses itempool.bin content and extracts black market, fish collector, and mission drops
pub fn extract_drops_from_itempool(data: &[u8]) -> Vec<DropEntry> {
    let doc = match crate::parser::parse_document(data) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut drops = Vec::new();

    for record in &doc.records {
        let name = &record.name;

        // Black Market items: ItemPool_BlackMarket_Comp_*
        if name.starts_with("ItemPool_BlackMarket_") {
            // Extract item info from the pool name
            // Pattern: ItemPool_BlackMarket_Comp_MANU_TYPE_Name
            let item_part = name.replace("ItemPool_BlackMarket_Comp_", "");
            if let Some(entry) = parse_black_market_item(&item_part) {
                drops.push(entry);
            }
        }

        // Fish Collector rewards: ItemPool_FishCollector_Reward_*
        if name.starts_with("ItemPool_FishCollector_Reward_") {
            let tier = name.replace("ItemPool_FishCollector_Reward_", "");
            // Check field values for legendary items
            for value in record.fields.values() {
                if let crate::parser::Value::String(s) = value {
                    if s.to_lowercase().contains(".comp_05_legendary_") {
                        if let Some(mut entry) =
                            parse_legendary_item_id("Fish Collector", s, DropSource::Special)
                        {
                            entry.drop_tier = tier.clone();
                            drops.push(entry);
                        }
                    }
                }
            }
        }

        // Side mission rewards: ItemPool_SideMission_*
        if name.starts_with("ItemPool_SideMission_") && !name.ends_with("_TurretDrop") {
            let mission_name = name
                .replace("ItemPool_SideMission_", "")
                .replace('_', " ");
            // Check field values for legendary items
            for value in record.fields.values() {
                if let crate::parser::Value::String(s) = value {
                    if s.to_lowercase().contains(".comp_05_legendary_") {
                        if let Some(entry) =
                            parse_legendary_item_id(&mission_name, s, DropSource::Mission)
                        {
                            drops.push(entry);
                        }
                    }
                }
            }
        }

        // Main mission rewards: ItemPool_MainMission_*
        if name.starts_with("ItemPool_MainMission_") {
            let mission_name = name
                .replace("ItemPool_MainMission_", "")
                .replace('_', " ");
            // Check field values for legendary items
            for value in record.fields.values() {
                if let crate::parser::Value::String(s) = value {
                    if s.to_lowercase().contains(".comp_05_legendary_") {
                        if let Some(entry) =
                            parse_legendary_item_id(&mission_name, s, DropSource::Mission)
                        {
                            drops.push(entry);
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
    // Pattern: MANU_TYPE_Name (e.g., BOR_HW_DiscJockey)
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
        pool: format!("itempool_blackmarket_comp_{}_{}", manu.to_lowercase(), gear_type.to_lowercase()),
        drop_tier: String::new(),
        drop_chance: 0.0, // Unknown for black market
    })
}

fn parse_legendary_item_id(source: &str, item_id: &str, source_type: DropSource) -> Option<DropEntry> {
    // Pattern: PREFIX.comp_05_legendary_<name>
    // Weapons: ORD_SR.comp_05_legendary_Fisheye (MANU_TYPE)
    // Shields: dad_shield.comp_05_legendary_angel (manu_shield)
    // Other gear: ord_repair_kit.comp_05_legendary_TripleBypass
    let parts: Vec<&str> = item_id.split('.').collect();
    if parts.len() != 2 {
        return None;
    }

    let prefix = parts[0]; // MANU_TYPE or manu_geartype
    let comp_part = parts[1]; // comp_05_legendary_<name>

    // Extract manufacturer and type from prefix
    let prefix_parts: Vec<&str> = prefix.split('_').collect();
    if prefix_parts.len() < 2 {
        return None;
    }

    let manu = prefix_parts[0].to_uppercase();
    // Join remaining parts for gear type (e.g., "repair_kit" -> "REPAIR_KIT")
    let gear_type = prefix_parts[1..].join("_").to_uppercase();

    // Extract item name from comp part (case-insensitive)
    // comp_05_legendary_<name> or comp_05_Legendary_<name> -> <name>
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
        source_display: None, // Set later from NameData
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
        drop_tier: String::new(), // Set later
        drop_chance: 0.0,         // Set later
    })
}

/// World drop pool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDropPool {
    /// Pool name (e.g., "Shields", "Pistols")
    pub name: String,
    /// Number of items in this pool
    pub item_count: u32,
    /// Per-item chance within the pool (1 / item_count)
    pub per_item_chance: f64,
}

/// Generate world drop entries from existing drops
///
/// Creates WorldDrop entries for all unique items, with drop chance based on pool size
fn generate_world_drops(existing_drops: &[DropEntry]) -> Vec<DropEntry> {
    use std::collections::HashMap;

    // Gear types that are in world drop pools
    let world_drop_gear_types = [
        "AR", "PS", "SM", "SG", "SR",           // Weapons
        "SHIELD",                                // Shields
        "GRENADE_GADGET", "HW",                  // Gadgets
        "REPAIR_KIT",                            // Repair kits
    ];

    // Count unique items per gear type
    let mut items_by_type: HashMap<String, Vec<String>> = HashMap::new();
    let mut item_details: HashMap<String, (String, String, String)> = HashMap::new(); // item_id -> (manu, gear_type, name)

    for drop in existing_drops {
        if world_drop_gear_types.contains(&drop.gear_type.as_str()) {
            items_by_type
                .entry(drop.gear_type.clone())
                .or_default()
                .push(drop.item_id.clone());
            item_details.insert(
                drop.item_id.clone(),
                (drop.manufacturer.clone(), drop.gear_type.clone(), drop.item_name.clone()),
            );
        }
    }

    // Deduplicate items per type
    for items in items_by_type.values_mut() {
        items.sort();
        items.dedup();
    }

    // Generate world drop entries
    let mut world_drops = Vec::new();
    let mut seen_items: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (gear_type, items) in &items_by_type {
        let pool_size = items.len();
        if pool_size == 0 {
            continue;
        }

        // Per-item chance within the legendary pool (assuming all items have equal weight)
        let per_item_chance = 1.0 / pool_size as f64;

        // Pool display name
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
/// Also extracts NameData to populate display names for bosses.
pub fn generate_drops_manifest<P: AsRef<Path>>(ncs_dir: P) -> Result<DropsManifest, std::io::Error> {
    use std::collections::HashSet;

    // First, extract NameData for display name mappings
    let name_data = crate::name_data::extract_from_directory(ncs_dir.as_ref());

    let mut all_drops = Vec::new();
    let mut seen = HashSet::new();

    // Find all itempoollist.bin and itempool.bin files
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

                    // Populate source_display from NameData for boss drops
                    if drop.source_type == DropSource::Boss && drop.source_display.is_none() {
                        if let Some(display) = name_data.find_display_name(&drop.source) {
                            drop.source_display = Some(display.to_string());
                        }
                    }

                    all_drops.push(drop);
                }
            }
        }
    }

    // Generate world drops from the items we found
    let world_drops = generate_world_drops(&all_drops);
    for drop in world_drops {
        let key = (drop.source.clone(), drop.item_id.clone());
        if !seen.contains(&key) {
            seen.insert(key);
            all_drops.push(drop);
        }
    }

    // Sort by source type, then source name, then by drop tier
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
        (type_order(&a.source_type), &a.source, tier_order(&a.drop_tier))
            .cmp(&(type_order(&b.source_type), &b.source, tier_order(&b.drop_tier)))
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

    fn test_manifest() -> DropsManifest {
        DropsManifest {
            version: 1,
            probabilities: DropProbabilities::default(),
            drops: vec![
                DropEntry {
                    source: "MeatheadRider_Jockey".to_string(),
                    source_display: Some("Saddleback".to_string()),
                    source_type: DropSource::Boss,
                    manufacturer: "JAK".to_string(),
                    gear_type: "SG".to_string(),
                    item_name: "Hellwalker".to_string(),
                    item_id: "JAK_SG.comp_05_legendary_Hellwalker".to_string(),
                    pool: "itempool_jak_sg_05_legendary_Hellwalker_shiny".to_string(),
                    drop_tier: "Primary".to_string(),
                    drop_chance: 0.20,
                },
                DropEntry {
                    source: "Timekeeper_Guardian".to_string(),
                    source_display: Some("Guardian Timekeeper".to_string()),
                    source_type: DropSource::Boss,
                    manufacturer: "MAL".to_string(),
                    gear_type: "SM".to_string(),
                    item_name: "PlasmaCoil".to_string(),
                    item_id: "MAL_SM.comp_05_legendary_PlasmaCoil".to_string(),
                    pool: "itempool_mal_sm_05_legendary_PlasmaCoil_shiny".to_string(),
                    drop_tier: "Primary".to_string(),
                    drop_chance: 0.20,
                },
            ],
        }
    }

    #[test]
    fn test_find_by_name_exact() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("Hellwalker");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
        assert_eq!(results[0].chance, 0.20);
    }

    #[test]
    fn test_find_by_name_case_insensitive() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("hellwalker");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
    }

    #[test]
    fn test_find_by_name_partial() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("plasma");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "Timekeeper_Guardian");
    }

    #[test]
    fn test_find_by_boss() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_boss("Timekeeper_Guardian");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_name, "PlasmaCoil");
    }

    #[test]
    fn test_sorted_by_chance() {
        let mut manifest = test_manifest();
        manifest.drops.push(DropEntry {
            source: "AnotherBoss".to_string(),
            source_display: Some("Another Boss".to_string()),
            source_type: DropSource::Boss,
            manufacturer: "JAK".to_string(),
            gear_type: "SG".to_string(),
            item_name: "Hellwalker".to_string(),
            item_id: "JAK_SG.comp_05_legendary_Hellwalker".to_string(),
            pool: "itempool_jak_sg_05_legendary_Hellwalker_shiny".to_string(),
            drop_tier: "Secondary".to_string(),
            drop_chance: 0.08,
        });

        let db = DropsDb::from_manifest(manifest);
        let results = db.find_by_name("Hellwalker");

        // Should be sorted by drop chance, highest first
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
        assert_eq!(results[0].chance, 0.20);
        assert_eq!(results[1].source, "AnotherBoss");
        assert_eq!(results[1].chance, 0.08);
    }
}
