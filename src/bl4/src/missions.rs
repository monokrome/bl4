//! Mission graph data for campaign progression.
//!
//! Embedded from `share/manifest/missions/` TSVs. Provides the dependency
//! graph of mission sets and per-mission metadata (region, type, difficulty).

use once_cell::sync::Lazy;
use std::collections::HashMap;

static MISSION_SETS_TSV: &str = include_str!(concat!(env!("OUT_DIR"), "/mission_sets.tsv"));
static MISSIONS_TSV: &str = include_str!(concat!(env!("OUT_DIR"), "/missions.tsv"));
static MISSION_NAMES_TSV: &str = include_str!(concat!(env!("OUT_DIR"), "/mission_names.tsv"));

#[derive(Debug, Clone)]
pub struct MissionSet {
    pub name: String,
    pub prerequisite: Option<String>,
    pub category: String,
    pub chained: bool,
    pub region: String,
}

#[derive(Debug, Clone)]
pub struct Mission {
    pub name: String,
    pub mission_set: String,
    pub mission_type: String,
    pub world_region: String,
    pub zone: String,
    pub difficulty: String,
}

static MISSION_SETS: Lazy<HashMap<String, MissionSet>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for line in MISSION_SETS_TSV.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let prereq = if cols[1].is_empty() {
            None
        } else {
            Some(cols[1].to_string())
        };
        let ms = MissionSet {
            name: cols[0].to_string(),
            prerequisite: prereq,
            category: cols[2].to_string(),
            chained: cols[3] == "true",
            region: cols[4].to_string(),
        };
        map.insert(ms.name.clone(), ms);
    }
    map
});

static MISSIONS: Lazy<HashMap<String, Mission>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for line in MISSIONS_TSV.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 6 {
            continue;
        }
        let m = Mission {
            name: cols[0].to_string(),
            mission_set: cols[1].to_string(),
            mission_type: cols[2].to_string(),
            world_region: cols[3].to_string(),
            zone: cols[4].to_string(),
            difficulty: cols[5].to_string(),
        };
        map.insert(m.name.clone(), m);
    }
    map
});

/// Internal name → display name
static DISPLAY_NAMES: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for line in MISSION_NAMES_TSV.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 2 && !cols[1].is_empty() {
            map.insert(cols[0].to_string(), cols[1].to_string());
        }
    }
    map
});

/// Display name (lowercase) → internal name
static DISPLAY_NAME_REVERSE: Lazy<HashMap<String, String>> = Lazy::new(|| {
    DISPLAY_NAMES
        .iter()
        .map(|(k, v)| (v.to_lowercase(), k.clone()))
        .collect()
});

/// Get the display name for a mission, if known.
pub fn display_name(internal_name: &str) -> Option<&'static str> {
    DISPLAY_NAMES.get(internal_name).map(|s| s.as_str())
}

/// Get a mission set by name.
pub fn mission_set(name: &str) -> Option<&'static MissionSet> {
    MISSION_SETS.get(name)
}

/// Get a mission by name.
pub fn mission(name: &str) -> Option<&'static Mission> {
    MISSIONS.get(name)
}

/// All mission sets.
pub fn all_mission_sets() -> &'static HashMap<String, MissionSet> {
    &MISSION_SETS
}

/// All missions.
pub fn all_missions() -> &'static HashMap<String, Mission> {
    &MISSIONS
}

/// Returns the main story mission sets in topological order (prerequisite chain).
///
/// Filters to `category == "main"` sets, then sorts by walking the prerequisite
/// chain from the root (prisonprologue, which has no prerequisite).
pub fn main_story_order() -> Vec<&'static MissionSet> {
    // Build forward graph: prereq -> [dependents]
    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut roots = Vec::new();

    for ms in MISSION_SETS.values() {
        if ms.category != "main" {
            continue;
        }
        match &ms.prerequisite {
            Some(prereq) => {
                forward.entry(prereq.as_str()).or_default().push(&ms.name);
            }
            None => roots.push(ms.name.as_str()),
        }
    }

    // BFS from roots to get topological order
    let mut order = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    let mut visited = std::collections::HashSet::new();

    // Start from prisonprologue if it exists, otherwise use all roots
    let start = roots
        .iter()
        .find(|r| r.contains("prisonprologue"))
        .copied()
        .unwrap_or_else(|| roots.first().copied().unwrap_or(""));

    if !start.is_empty() {
        queue.push_back(start);
    }

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) {
            continue;
        }
        if let Some(ms) = MISSION_SETS.get(current) {
            order.push(ms);
        }
        if let Some(children) = forward.get(current) {
            let mut sorted = children.clone();
            sorted.sort();
            for child in sorted {
                queue.push_back(child);
            }
        }
    }

    order
}

/// The three branch endpoints that must all be completed before the
/// story continues past the open-world section. The NCS data only
/// records one prerequisite per mission set, but the game requires
/// all three branches to converge at searchforlilith.
const CONVERGENCE_BRANCHES: &[&str] = &[
    "missionset_main_grasslands3",
    "missionset_main_mountains3",
    "missionset_main_shatteredlands3",
];

/// The mission set where the three branches converge.
const CONVERGENCE_POINT: &str = "missionset_main_searchforlilith";

/// Compute all prerequisite mission sets for a target (inclusive).
///
/// Walks backward through the prerequisite chain, collecting every
/// mission set that must be completed before the target can be active.
/// Returns them in topological order (roots first, target last).
///
/// At the convergence point (searchforlilith), all three open-world
/// branches are required, not just the grasslands chain recorded in
/// the NCS dependency data.
pub fn prerequisites_for(target: &str) -> Vec<&'static MissionSet> {
    let mut visited = std::collections::HashSet::new();
    let mut ancestors = Vec::new();

    collect_prerequisites(target, &mut ancestors, &mut visited);

    // Deduplicate while preserving order (earlier entries first)
    let mut seen = std::collections::HashSet::new();
    ancestors.retain(|ms| seen.insert(ms.name.as_str()));

    ancestors
}

fn collect_prerequisites(
    target: &str,
    ancestors: &mut Vec<&MissionSet>,
    visited: &mut std::collections::HashSet<String>,
) {
    // Walk backward through single-prerequisite chain
    let mut chain = Vec::new();
    let mut current = Some(target.to_string());

    while let Some(name) = current {
        if !visited.insert(name.clone()) {
            break;
        }

        // At the convergence point, pull in all three branches
        if name == CONVERGENCE_POINT {
            for &branch_end in CONVERGENCE_BRANCHES {
                collect_prerequisites(branch_end, ancestors, visited);
            }
        }

        if let Some(ms) = MISSION_SETS.get(&name) {
            chain.push(ms);
            current = ms.prerequisite.clone();
        } else {
            break;
        }
    }

    chain.reverse();
    ancestors.extend(chain);
}

/// Find all missions belonging to a mission set.
pub fn missions_in_set(set_name: &str) -> Vec<&'static Mission> {
    let lower = set_name.to_lowercase();
    let mut result: Vec<&Mission> = MISSIONS
        .values()
        .filter(|m| m.mission_set.to_lowercase() == lower)
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// Get the mission name for a set, with fallback derivation from the set name.
pub fn mission_name_for_set(set_name: &str) -> String {
    first_mission_in_set(set_name)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| set_name.replace("missionset_", "mission_"))
}

/// Find the first mission belonging to a mission set.
pub fn first_mission_in_set(set_name: &str) -> Option<&'static Mission> {
    // Try the obvious name derivation first (missionset_main_X → mission_main_X)
    let mission_name = set_name
        .replace("missionset_", "mission_")
        // Handle suffixes like "grasslands2a" → "grasslands2"
        .trim_end_matches(|c: char| c.is_ascii_lowercase() && !c.is_ascii_digit())
        .to_string();

    if let Some(m) = MISSIONS.get(&mission_name) {
        return Some(m);
    }

    // Fallback: search all missions for matching mission_set
    MISSIONS
        .values()
        .find(|m| m.mission_set.to_lowercase() == set_name)
}

/// Resolve a short mission name to a full individual mission name.
///
/// Accepts: "huntedpart1", "mission_micro_huntedpart1", "Recruitment Drive", etc.
pub fn resolve_mission_name(input: &str) -> Option<&'static Mission> {
    let lower = input.to_lowercase();

    if let Some(m) = MISSIONS.get(&lower) {
        return Some(m);
    }

    for prefix in &[
        "mission_main_",
        "mission_dlc_",
        "mission_side_",
        "mission_micro_",
    ] {
        let with_prefix = format!("{}{}", prefix, lower);
        if let Some(m) = MISSIONS.get(&with_prefix) {
            return Some(m);
        }
    }

    // Try display name lookup
    if let Some(internal) = DISPLAY_NAME_REVERSE.get(&lower) {
        if let Some(m) = MISSIONS.get(internal) {
            return Some(m);
        }
    }

    None
}

/// Resolve a short mission name to a full mission set name.
///
/// Accepts: "grasslands1", "missionset_main_grasslands1", "mountains2a", etc.
pub fn resolve_mission_set_name(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();

    // Try exact match first
    if MISSION_SETS.contains_key(&lower) {
        return MISSION_SETS.get(&lower).map(|ms| ms.name.as_str());
    }

    // Try with common prefixes
    for prefix in &[
        "missionset_main_",
        "missionset_dlc_",
        "missionset_side_",
        "missionset_micro_",
        "missionset_vault_",
        "missionset_zoneactivity_",
    ] {
        let with_prefix = format!("{}{}", prefix, lower);
        if let Some(ms) = MISSION_SETS.get(&with_prefix) {
            return Some(ms.name.as_str());
        }
    }

    // Try display name → mission → mission set
    if let Some(internal) = DISPLAY_NAME_REVERSE.get(&lower) {
        if let Some(m) = MISSIONS.get(internal) {
            let set_lower = m.mission_set.to_lowercase();
            if let Some(ms) = MISSION_SETS.get(&set_lower) {
                return Some(ms.name.as_str());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mission_sets_loaded() {
        let sets = all_mission_sets();
        assert!(sets.len() > 20, "Expected 20+ mission sets, got {}", sets.len());
        assert!(sets.contains_key("missionset_main_prisonprologue"));
    }

    #[test]
    fn test_missions_loaded() {
        let missions = all_missions();
        assert!(missions.len() > 100, "Expected 100+ missions, got {}", missions.len());
        assert!(missions.contains_key("mission_main_prisonprologue"));
    }

    #[test]
    fn test_main_story_order() {
        let order = main_story_order();
        assert!(!order.is_empty());
        assert_eq!(order[0].name, "missionset_main_prisonprologue");
        // Beach should follow prologue
        let beach_pos = order.iter().position(|ms| ms.name.contains("beach"));
        assert!(beach_pos.is_some());
        assert!(beach_pos.unwrap() > 0);
    }

    #[test]
    fn test_prerequisites_for() {
        let prereqs = prerequisites_for("missionset_main_grasslands1");
        assert!(prereqs.len() >= 3); // prologue, beach, grasslands1
        assert_eq!(prereqs[0].name, "missionset_main_prisonprologue");
        assert_eq!(prereqs.last().unwrap().name, "missionset_main_grasslands1");
    }

    #[test]
    fn test_resolve_mission_set_name() {
        assert_eq!(
            resolve_mission_set_name("grasslands1"),
            Some("missionset_main_grasslands1")
        );
        assert_eq!(
            resolve_mission_set_name("missionset_main_beach"),
            Some("missionset_main_beach")
        );
        assert_eq!(resolve_mission_set_name("nonexistent"), None);
    }

    #[test]
    fn test_convergence_includes_all_branches() {
        let prereqs = prerequisites_for("missionset_main_searchforlilith");
        let names: Vec<&str> = prereqs.iter().map(|ms| ms.name.as_str()).collect();

        // Must include all three branch endpoints
        assert!(names.contains(&"missionset_main_grasslands3"), "missing grasslands3");
        assert!(names.contains(&"missionset_main_mountains3"), "missing mountains3");
        assert!(names.contains(&"missionset_main_shatteredlands3"), "missing shatteredlands3");

        // Must include branch interiors too
        assert!(names.contains(&"missionset_main_mountains1"), "missing mountains1");
        assert!(names.contains(&"missionset_main_shatteredlands1"), "missing shatteredlands1");

        // Target should be last
        assert_eq!(prereqs.last().unwrap().name, "missionset_main_searchforlilith");
    }

    #[test]
    fn test_post_convergence_includes_all_branches() {
        // Anything after searchforlilith should also include all branches
        let prereqs = prerequisites_for("missionset_main_elpis");
        let names: Vec<&str> = prereqs.iter().map(|ms| ms.name.as_str()).collect();
        assert!(names.contains(&"missionset_main_mountains3"));
        assert!(names.contains(&"missionset_main_shatteredlands3"));
        assert!(names.contains(&"missionset_main_searchforlilith"));
    }

    #[test]
    fn test_branch_only_does_not_include_other_branches() {
        // Setting to mountains2a should NOT include shatteredlands or grasslands2b+
        let prereqs = prerequisites_for("missionset_main_mountains2a");
        let names: Vec<&str> = prereqs.iter().map(|ms| ms.name.as_str()).collect();
        assert!(names.contains(&"missionset_main_mountains1"));
        assert!(names.contains(&"missionset_main_grasslands2a")); // shared root
        assert!(!names.contains(&"missionset_main_shatteredlands1"));
        assert!(!names.contains(&"missionset_main_grasslands2b"));
    }

    #[test]
    fn test_branch_point() {
        // grasslands2a should have multiple successors
        let order = main_story_order();
        let g2a_pos = order
            .iter()
            .position(|ms| ms.name == "missionset_main_grasslands2a")
            .unwrap();
        // After grasslands2a, multiple sets should appear
        let after_g2a: Vec<_> = order[g2a_pos + 1..]
            .iter()
            .take(3)
            .map(|ms| ms.name.as_str())
            .collect();
        // Should contain the three branches
        assert!(
            after_g2a.iter().any(|n| n.contains("grasslands2b")
                || n.contains("mountains1")
                || n.contains("shatteredlands1")),
            "Expected branch after grasslands2a, got {:?}",
            after_g2a
        );
    }
}
