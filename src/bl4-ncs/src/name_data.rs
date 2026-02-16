//! NameData extraction for entity display name mappings
//!
//! NCS files contain `NameData_*` entries that map internal entity type names
//! to their in-game display names.
//!
//! Format: `NameData_<InternalType>, <UUID>, <DisplayName>`
//!
//! Example: `NameData_Meathead, D342D6EE47173677CE1C068BADA88F69, Saddleback`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A single NameData entry mapping an internal type to a display name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameDataEntry {
    /// Internal type (e.g., "Meathead", "Thresher", "Bat")
    pub internal_type: String,
    /// UUID identifier for this specific variant
    pub uuid: String,
    /// Human-readable display name shown in-game
    pub display_name: String,
}

/// Collection of NameData mappings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NameDataMap {
    /// All extracted entries
    pub entries: Vec<NameDataEntry>,
    /// Index: internal_type (lowercase) → list of entries
    #[serde(skip)]
    by_type: HashMap<String, Vec<usize>>,
    /// Index: display_name (lowercase) → entry index
    #[serde(skip)]
    by_display: HashMap<String, usize>,
}

impl NameDataMap {
    /// Create an empty NameDataMap
    pub fn new() -> Self {
        Self::default()
    }

    /// Build indices after loading entries
    pub fn build_indices(&mut self) {
        self.by_type.clear();
        self.by_display.clear();

        for (i, entry) in self.entries.iter().enumerate() {
            let type_key = entry.internal_type.to_lowercase();
            self.by_type.entry(type_key).or_default().push(i);

            let display_key = entry.display_name.to_lowercase();
            self.by_display.insert(display_key, i);
        }
    }

    /// Add an entry
    pub fn add(&mut self, entry: NameDataEntry) {
        let type_key = entry.internal_type.to_lowercase();
        let display_key = entry.display_name.to_lowercase();
        let idx = self.entries.len();

        self.by_type.entry(type_key).or_default().push(idx);
        self.by_display.insert(display_key, idx);
        self.entries.push(entry);
    }

    /// Find display name for an internal boss/entity name
    ///
    /// Tries multiple matching strategies:
    /// 1. Exact match on internal type
    /// 2. Prefix match (e.g., "MeatheadRider" starts with "Meathead")
    ///
    /// Returns the first non-prefixed display name found (prefers base variants)
    pub fn find_display_name(&self, internal_name: &str) -> Option<&str> {
        let name_lower = internal_name.to_lowercase();

        // Strategy 1: Exact type match
        if let Some(indices) = self.by_type.get(&name_lower) {
            if let Some(entry) = self.find_best_entry(indices) {
                return Some(&entry.display_name);
            }
        }

        // Strategy 2: Try extracting base type from compound names
        // e.g., "MeatheadRider_Jockey" -> try "MeatheadRider" then "Meathead"
        for part in extract_name_parts(&name_lower) {
            if let Some(indices) = self.by_type.get(&part) {
                if let Some(entry) = self.find_best_entry(indices) {
                    return Some(&entry.display_name);
                }
            }
        }

        // Strategy 3: Check if internal name starts with a known type
        // This is more strict - only match if the name starts with the type
        // followed by an underscore (word boundary)
        // e.g., "Meathead_Rider" matches "Meathead", but
        //       "BatMatriarch" should NOT match "Bat" (no word boundary)
        for (type_name, indices) in &self.by_type {
            if name_lower.starts_with(type_name) {
                let remaining = &name_lower[type_name.len()..];
                // Must be followed by nothing or underscore
                if remaining.is_empty() || remaining.starts_with('_') {
                    if let Some(entry) = self.find_best_entry(indices) {
                        return Some(&entry.display_name);
                    }
                }
            }
        }

        None
    }

    /// Find the best entry from a list of indices
    /// Prefers entries that look like boss names (simple, short, proper nouns)
    #[allow(clippy::too_many_lines)]
    fn find_best_entry(&self, indices: &[usize]) -> Option<&NameDataEntry> {
        // Prefixes that indicate variants (we prefer the base name)
        let variant_prefixes = [
            "big encore",
            "badass",
            "vile",
            "burning",
            "acidic",
            "atomic",
            "galvanic",
            "boreal",
            "frostbite",
            "scorched",
            "crackling",
            "noxious",
            "quasar",
            "the ",
            "not-so-",
            "cold ",
            "burnt ",
            "spicy ",
            "rancid ",
            "icy ",
            "queen's ",
            "launcher ",
            "loot ",
            "'rager ",
        ];

        // Common suffixes/patterns for regular enemies (not bosses)
        let enemy_patterns = [
            "meatball",
            "icehead",
            "hothead",
            "fissionhead",
            "watthead",
            "wastehead",
            "'head",
            "icebox",
            "bandit",
            "thresher",
            "kratch",
            "engine",
            "pangolin",
        ];

        // Score each entry - higher is better for boss names
        let mut best_idx: Option<usize> = None;
        let mut best_score: i32 = i32::MIN;

        for &idx in indices {
            let entry = &self.entries[idx];
            let name_lower = entry.display_name.to_lowercase();
            let mut score: i32 = 0;

            // Penalize variant prefixes
            if variant_prefixes.iter().any(|p| name_lower.starts_with(p)) {
                score -= 100;
            }

            // Penalize compound names with &
            if name_lower.contains('&') {
                score -= 50;
            }

            // Penalize common enemy patterns
            if enemy_patterns.iter().any(|p| name_lower.contains(p)) {
                score -= 30;
            }

            // Prefer shorter names (likely boss names)
            let word_count = entry.display_name.split_whitespace().count();
            if word_count == 1 {
                score += 20; // Single word names are great
            } else if word_count == 2 {
                score += 10; // Two words is ok
            } else {
                score -= word_count as i32 * 5; // Penalize long names
            }

            // Prefer names that start with capital letter (proper nouns)
            if entry.display_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                score += 5;
            }

            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        best_idx.map(|idx| &self.entries[idx])
    }

    /// Get all entries for a given internal type
    pub fn get_by_type(&self, internal_type: &str) -> Vec<&NameDataEntry> {
        let type_key = internal_type.to_lowercase();
        self.by_type
            .get(&type_key)
            .map(|indices| indices.iter().map(|&i| &self.entries[i]).collect())
            .unwrap_or_default()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all unique internal types
    pub fn types(&self) -> Vec<&str> {
        self.by_type.keys().map(|s| s.as_str()).collect()
    }
}

/// Extract name parts for matching
/// "MeatheadRider_Jockey" -> ["meatheadrider_jockey", "meatheadrider", "meathead"]
/// "Thresher_BioArmoredBig" -> ["thresher_bioarmoredbig", "thresher"]
fn extract_name_parts(name: &str) -> Vec<String> {
    let mut parts = vec![name.to_string()];

    // Split by underscore and try progressively shorter prefixes
    let underscore_parts: Vec<&str> = name.split('_').collect();
    if underscore_parts.len() > 1 {
        // Try without last part (e.g., "meatheadrider" from "meatheadrider_jockey")
        parts.push(underscore_parts[..underscore_parts.len() - 1].join("_"));
        // Try just first part (e.g., "thresher" from "thresher_bioarmoredbig")
        parts.push(underscore_parts[0].to_string());
    }

    // Common compound word suffixes used in boss/entity names
    // These indicate a role/type modifier and the base type comes before them
    let compound_suffixes = [
        "rider", // MeatheadRider -> Meathead (has NameData)
    ];

    // Apply suffix stripping to all current parts
    let parts_to_check: Vec<String> = parts.clone();
    for part in parts_to_check {
        for suffix in &compound_suffixes {
            if part.ends_with(suffix) {
                let base = &part[..part.len() - suffix.len()];
                if !base.is_empty() && !parts.contains(&base.to_string()) {
                    parts.push(base.to_string());
                }
            }
        }
    }

    parts
}

/// Extract NameData entries from a single NCS binary file
///
/// Scans the raw bytes for strings matching the NameData pattern
pub fn extract_from_binary(data: &[u8]) -> Vec<NameDataEntry> {
    let mut entries = Vec::new();

    // Extract printable strings from binary
    let strings = extract_strings(data);

    for s in strings {
        if let Some(entry) = parse_namedata_line(&s) {
            entries.push(entry);
        }
    }

    entries
}

/// Extract printable strings from binary data (similar to `strings` command)
fn extract_strings(data: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    const MIN_LENGTH: usize = 10; // NameData entries are at least 10 chars

    for &byte in data {
        if (0x20..0x7f).contains(&byte) {
            current.push(byte);
        } else if !current.is_empty() {
            if current.len() >= MIN_LENGTH {
                if let Ok(s) = String::from_utf8(current.clone()) {
                    strings.push(s);
                }
            }
            current.clear();
        }
    }

    // Don't forget the last string
    if current.len() >= MIN_LENGTH {
        if let Ok(s) = String::from_utf8(current) {
            strings.push(s);
        }
    }

    strings
}

/// Parse a single NameData line
/// Formats:
/// - "NameData_<Type>, <UUID>, <DisplayName>" - enemy/entity variants
/// - "discovery_ui_data, <UUID>, <DisplayName>" - boss discovery names
fn parse_namedata_line(line: &str) -> Option<NameDataEntry> {
    // Split by ", " to get parts
    let parts: Vec<&str> = line.splitn(3, ", ").collect();
    if parts.len() != 3 {
        return None;
    }

    // UUID should be 32 hex characters
    let uuid = parts[1].to_string();
    if uuid.len() != 32 || !uuid.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    // Display name (trim whitespace)
    let display_name = parts[2].trim().to_string();
    if display_name.is_empty() {
        return None;
    }

    // Determine internal type based on format
    let internal_type = if let Some(type_name) = parts[0].strip_prefix("NameData_") {
        // Standard NameData entry: NameData_<Type> -> use Type as internal type
        type_name.to_string()
    } else if parts[0] == "discovery_ui_data" {
        // Discovery entry: use display name as internal type (for boss lookup)
        // Convert display name to internal format for matching
        // "The Backhive" -> "Backhive", "Meathead Riders" -> "Meathead"
        extract_boss_internal_name(&display_name)
    } else {
        return None;
    };

    Some(NameDataEntry {
        internal_type,
        uuid,
        display_name,
    })
}

/// Extract internal boss name from display name for discovery_ui_data entries
/// "The Backhive" -> "Backhive"
/// "Meathead Riders" -> "MeatheadRider"
/// "Primordial Guardian Inceptus" -> "Grasslands_Commander" (can't infer, use base)
/// "Callis, the Ripper Queen" -> "Callis"
fn extract_boss_internal_name(display_name: &str) -> String {
    let name = display_name.trim();

    // Remove common prefixes
    let name = name.strip_prefix("The ").unwrap_or(name);

    // Handle comma-separated names (e.g., "Callis, the Ripper Queen" -> "Callis")
    let name = name.split(',').next().unwrap_or(name).trim();

    // Handle "Primordial Guardian X" -> "X"
    let name = name.strip_prefix("Primordial Guardian ").unwrap_or(name);

    // Remove spaces and convert to PascalCase-ish for matching
    // "Meathead Riders" should become something we can match to "MeatheadRider"
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() == 1 {
        words[0].to_string()
    } else {
        // Join words, handling plurals
        let mut result = String::new();
        for (i, word) in words.iter().enumerate() {
            if i == words.len() - 1 && word.ends_with('s') && word.len() > 3 {
                // Remove trailing 's' from last word (Riders -> Rider)
                result.push_str(&word[..word.len() - 1]);
            } else {
                result.push_str(word);
            }
        }
        result
    }
}

/// Extract NameData from all NCS files in a directory
pub fn extract_from_directory<P: AsRef<Path>>(ncs_dir: P) -> NameDataMap {
    let mut map = NameDataMap::new();

    for entry in walkdir::WalkDir::new(ncs_dir.as_ref())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "bin") {
            if let Ok(data) = std::fs::read(path) {
                for name_entry in extract_from_binary(&data) {
                    map.add(name_entry);
                }
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_namedata_line() {
        let line = "NameData_Meathead, D342D6EE47173677CE1C068BADA88F69, Saddleback";
        let entry = parse_namedata_line(line).unwrap();
        assert_eq!(entry.internal_type, "Meathead");
        assert_eq!(entry.uuid, "D342D6EE47173677CE1C068BADA88F69");
        assert_eq!(entry.display_name, "Saddleback");
    }

    #[test]
    fn test_parse_namedata_with_spaces() {
        let line = "NameData_Meathead, B8EAFB724DAB6362B39A5592718B54B0, The Immortal Boneface";
        let entry = parse_namedata_line(line).unwrap();
        assert_eq!(entry.display_name, "The Immortal Boneface");
    }

    #[test]
    fn test_parse_invalid_lines() {
        assert!(parse_namedata_line("Not a NameData line").is_none());
        assert!(parse_namedata_line("NameData_Meathead, INVALID").is_none());
        assert!(parse_namedata_line("NameData_Meathead, , ").is_none());
    }

    #[test]
    fn test_extract_name_parts() {
        let parts = extract_name_parts("meatheadrider_jockey");
        assert!(parts.contains(&"meatheadrider_jockey".to_string()));
        assert!(parts.contains(&"meatheadrider".to_string()));
        assert!(parts.contains(&"meathead".to_string()));
    }

    #[test]
    fn test_find_display_name() {
        let mut map = NameDataMap::new();
        map.add(NameDataEntry {
            internal_type: "Meathead".to_string(),
            uuid: "D342D6EE47173677CE1C068BADA88F69".to_string(),
            display_name: "Saddleback".to_string(),
        });
        map.add(NameDataEntry {
            internal_type: "Meathead".to_string(),
            uuid: "B8EAFB724DAB6362B39A5592718B54B0".to_string(),
            display_name: "The Immortal Boneface".to_string(),
        });

        // Should find Saddleback (base variant) for MeatheadRider
        assert_eq!(
            map.find_display_name("MeatheadRider_Jockey"),
            Some("Saddleback")
        );
    }

    #[test]
    fn test_best_entry_prefers_base() {
        let mut map = NameDataMap::new();
        map.add(NameDataEntry {
            internal_type: "Thresher".to_string(),
            uuid: "A".repeat(32),
            display_name: "Badass Thresher".to_string(),
        });
        map.add(NameDataEntry {
            internal_type: "Thresher".to_string(),
            uuid: "B".repeat(32),
            display_name: "Vile Thresher".to_string(),
        });
        map.add(NameDataEntry {
            internal_type: "Thresher".to_string(),
            uuid: "C".repeat(32),
            display_name: "Ravenous Thresher".to_string(),
        });

        // Should prefer "Ravenous Thresher" as it has no variant prefix
        assert_eq!(
            map.find_display_name("Thresher"),
            Some("Ravenous Thresher")
        );
    }
}
