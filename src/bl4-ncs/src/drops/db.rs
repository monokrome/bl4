//! Drops database for querying item drop locations

use super::types::{DropEntry, DropLocation, DropProbabilities, DropSource, DropsManifest};
use std::collections::HashMap;
use std::path::Path;

/// Drops database for querying item drop locations
pub struct DropsDb {
    manifest: DropsManifest,
    /// Index: lowercase item name → entries
    by_name: HashMap<String, Vec<usize>>,
    /// Index: item_id → entries (reserved for future use)
    #[allow(dead_code)]
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
        let query_parts: Vec<&str> = query.split(['_', ' ']).collect();
        if query_parts.len() >= 2 {
            let manu = query_parts[0].to_uppercase();
            let wtype = query_parts[1].to_uppercase();
            for (i, entry) in self.manifest.drops.iter().enumerate() {
                if entry.manufacturer == manu && entry.gear_type == wtype {
                    if query_parts.len() > 2 {
                        let item_query = query_parts[2..].join("_").to_lowercase();
                        if entry.item_name.to_lowercase().contains(&item_query)
                            && !matches.contains(&i)
                        {
                            matches.push(i);
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
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub fn find_by_source(&self, source: &str) -> Vec<&DropEntry> {
        let source_lower = source.to_lowercase();
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

        locations.sort_by(|a, b| b.chance.partial_cmp(&a.chance).unwrap());

        // Deduplicate by source (keep highest chance)
        let mut seen = std::collections::HashSet::new();
        locations.retain(|loc| seen.insert(loc.source.clone()));

        locations
    }
}
