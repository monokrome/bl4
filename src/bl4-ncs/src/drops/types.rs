//! Drop-related type definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Boss name mappings loaded from embedded data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BossNameMapping {
    /// Main mapping from ItemPoolList suffix to display name
    pub boss_names: HashMap<String, String>,
    /// Alias mappings for fuzzy matching
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

impl BossNameMapping {
    /// Load boss name mappings from embedded data
    pub fn load() -> Self {
        Self::default_mapping()
    }

    /// Build boss name mapping from table_bossreplay_costs DataTable.
    /// Data table names are authoritative game display names.
    pub fn from_data_table(table: &crate::data_table::DataTable) -> Self {
        let mut boss_names = HashMap::new();
        let mut aliases = HashMap::new();

        for row in &table.rows {
            let comment = match row.fields.get("comment") {
                Some(c) => c.as_str(),
                None => continue,
            };

            let display_name = match crate::data_table::parse_boss_replay_comment(comment) {
                Some((_, name)) => name.to_string(),
                None => continue,
            };

            boss_names.insert(row.row_name.clone(), display_name.clone());

            // Build singular-form aliases for multi-boss entries
            let row_lower = row.row_name.to_lowercase();
            if row_lower == "foundryfreaks" {
                aliases.insert("FoundryFreak".into(), display_name);
            } else if row_lower == "meatheadriders" {
                aliases.insert("MeatheadRider".into(), display_name);
            } else if row_lower == "hovercarts" {
                aliases.insert("Hovercart".into(), display_name);
            } else if row_lower == "pangobango" {
                aliases.insert("Pango".into(), display_name.clone());
                aliases.insert("Bango".into(), display_name);
            }
        }

        Self { boss_names, aliases }
    }

    /// Merge entries from `other` that aren't already present in `self`.
    /// Used to add hardcoded fallback entries for bosses not in the data table.
    pub fn merge_missing(&mut self, other: &BossNameMapping) {
        for (key, value) in &other.boss_names {
            if !self.boss_names.contains_key(key) {
                self.boss_names.insert(key.clone(), value.clone());
            }
        }
        for (key, value) in &other.aliases {
            if !self.aliases.contains_key(key) {
                self.aliases.insert(key.clone(), value.clone());
            }
        }
    }

    /// Default mapping built from compiled-in data table + hardcoded fallbacks.
    ///
    /// Primary source: `table_bossreplay_costs.tsv` (authoritative game data).
    /// Fallbacks cover DLC bosses, variant entries, and ItemPoolList key
    /// differences (e.g., `GlidePackPsycho` vs data table's `GlidePack`).
    pub fn default_mapping() -> Self {
        let mut mapping = Self::from_compiled_tsv();
        mapping.merge_missing(&Self::hardcoded_fallbacks());
        mapping
    }

    /// Parse boss names from the compiled-in boss replay costs TSV.
    fn from_compiled_tsv() -> Self {
        const TSV: &str = include_str!(
            "../../../../share/manifest/data_tables/table_bossreplay_costs.tsv"
        );

        let mut boss_names = HashMap::new();
        let mut aliases = HashMap::new();

        for line in TSV.lines().skip(1) {
            let cols: Vec<&str> = line.splitn(5, '\t').collect();
            if cols.len() < 2 {
                continue;
            }
            let row_name = cols[0];
            let comment = cols[1];

            let display_name =
                match crate::data_table::parse_boss_replay_comment(comment) {
                    Some((_, name)) => name.to_string(),
                    None => continue,
                };

            boss_names.insert(row_name.to_string(), display_name.clone());

            // Singular-form aliases for multi-boss entries
            let row_lower = row_name.to_lowercase();
            if row_lower == "foundryfreaks" {
                aliases.insert("FoundryFreak".into(), display_name);
            } else if row_lower == "meatheadriders" {
                aliases.insert("MeatheadRider".into(), display_name);
            } else if row_lower == "hovercarts" {
                aliases.insert("Hovercart".into(), display_name);
            } else if row_lower == "pangobango" {
                aliases.insert("Pango".into(), display_name.clone());
                aliases.insert("Bango".into(), display_name);
            }
        }

        Self { boss_names, aliases }
    }

    /// Hardcoded fallback entries for bosses not in the data table.
    ///
    /// Covers: ItemPoolList key variants (different from data table row_name),
    /// DLC bosses, Primordial Guardians, and fuzzy-match aliases.
    fn hardcoded_fallbacks() -> Self {
        let mut boss_names = HashMap::new();
        let mut aliases = HashMap::new();

        // ItemPoolList keys that differ from data table row_names
        boss_names.insert("Grasslands_Commander".into(), "Primordial Guardian Inceptus".into());
        boss_names.insert("MountainCommander".into(), "Primordial Guardian Radix".into());
        boss_names.insert("ShatterlandsCommanderElpis".into(), "Primordial Guardian Origo".into());
        boss_names.insert("ShatterlandsCommanderFortress".into(), "Primordial Guardian Origo".into());
        boss_names.insert("Timekeeper_TKBoss".into(), "The Timekeeper".into());
        boss_names.insert("Grasslands_Guardian".into(), "Grasslands Guardian".into());
        boss_names.insert("MountainGuardian".into(), "Mountain Guardian".into());
        boss_names.insert("ShatterlandsGuardian".into(), "Shatterlands Guardian".into());
        boss_names.insert("Timekeeper_Guardian".into(), "Timekeeper Guardian".into());
        boss_names.insert("GlidePackPsycho".into(), "Splashzone".into());
        boss_names.insert("KOTOMotherbaseBrute".into(), "Bio-Bulkhead".into());
        boss_names.insert("KotoLieutenant".into(), "Horace".into());
        boss_names.insert("FoundryFreak_MeatheadFrackingBoss".into(), "Foundry Freaks".into());
        boss_names.insert("Thresher_BioArmoredBig".into(), "Bio-Thresher Omega".into());
        boss_names.insert("MeatheadRider_Jockey".into(), "Jockey".into());
        boss_names.insert("Redguard".into(), "Directive-0".into());

        // DLC bosses (not in base game data table)
        boss_names.insert("Donk".into(), "Donk".into());
        boss_names.insert("MinisterScrew".into(), "Minister Screw".into());
        boss_names.insert("Bloomreaper".into(), "Bloomreaper".into());

        // Variant entries
        boss_names.insert("SideCity_Psycho".into(), "Side City Psycho".into());
        boss_names.insert("FoundryFreak_Psycho".into(), "Foundry Freak Psycho".into());
        boss_names.insert("FoundryFreak_Splice".into(), "Foundry Freak Splice".into());

        // Aliases for fuzzy matching
        aliases.insert("Grasslands".into(), "Primordial Guardian Inceptus".into());
        aliases.insert("Mountains".into(), "Primordial Guardian Radix".into());
        aliases.insert("Shatterlands".into(), "Primordial Guardian Origo".into());
        aliases.insert("Castilleia".into(), "Castilleia".into());
        aliases.insert("Mimicron".into(), "Mimicron".into());
        aliases.insert("Axemaul".into(), "Axemaul".into());
        aliases.insert("Shadowpelt".into(), "Shadowpelt".into());
        aliases.insert("Tabnak".into(), "Tabnak, the Ripper Prince".into());
        aliases.insert("Harbinger".into(), "Callous Harbinger of Annihilating Death".into());

        Self { boss_names, aliases }
    }

    /// Get display name for a boss internal name
    pub fn get_display_name(&self, internal_name: &str) -> Option<&str> {
        // Try exact match first
        if let Some(name) = self.boss_names.get(internal_name) {
            return Some(name);
        }

        // Try case-insensitive match
        let name_lower = internal_name.to_lowercase();
        for (key, value) in &self.boss_names {
            if key.to_lowercase() == name_lower {
                return Some(value);
            }
        }

        // Try without underscores (case-insensitive)
        let normalized = name_lower.replace('_', "");
        for (key, value) in &self.boss_names {
            if key.to_lowercase().replace('_', "") == normalized {
                return Some(value);
            }
        }

        // Try aliases (case-insensitive)
        for (alias, name) in &self.aliases {
            if name_lower.contains(&alias.to_lowercase()) {
                return Some(name);
            }
        }

        None
    }
}

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
            primary: 0.06,
            secondary: 0.045,
            tertiary: 0.03,
            shiny: 0.01,
            true_boss: 0.25,
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
