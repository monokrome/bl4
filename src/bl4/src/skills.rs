//! Class mod skill manipulation
//!
//! Decode, resolve, and edit passive skills on class mod items.
//! Skills are encoded as Part tokens with indices mapping to
//! `passive_{color}_{position}_tier_{N}` part names.

use crate::manifest;
use crate::serial::Token;

/// Class mod category IDs
pub const CLASS_MOD_CATEGORIES: &[i64] = &[254, 255, 256, 259, 404];

/// A decoded skill from a class mod serial
#[derive(Debug, Clone)]
pub struct DecodedSkill {
    pub position: String,
    pub tier: u8,
    pub display_name: String,
    pub part_name: String,
    pub part_index: i64,
}

/// A skill to add via --add
#[derive(Debug, Clone)]
pub struct SkillAdd {
    pub position: String,
    pub tier: u8,
}

/// A skill to remove via --remove
#[derive(Debug, Clone)]
pub struct SkillRemove {
    pub position: String,
}

/// Before/after comparison for a skill slot
#[derive(Debug, Clone)]
pub struct SkillDiffEntry {
    pub slot: usize,
    pub before: Option<DecodedSkill>,
    pub after: Option<DecodedSkill>,
    pub changed: bool,
}

/// Check if a category is a class mod
pub fn is_class_mod(category: i64) -> bool {
    CLASS_MOD_CATEGORIES.contains(&category)
}

/// Extract all passive skills from a token list for a given category.
pub fn decode_skills(tokens: &[Token], category: i64) -> Vec<DecodedSkill> {
    let mut skills = Vec::new();

    for token in tokens {
        let (index, _values) = match token {
            Token::Part { index, values, .. } => (*index, values),
            _ => continue,
        };

        let part_name = match manifest::part_name(category, index as i64) {
            Some(n) => n,
            None => continue,
        };

        let (position, tier) = match manifest::parse_passive_part(part_name) {
            Some(p) => p,
            None => continue,
        };

        let display_name = manifest::skill_display_name(category, position)
            .map(|info| info.display_name.clone())
            .unwrap_or_default();

        skills.push(DecodedSkill {
            position: position.to_string(),
            tier,
            display_name,
            part_name: part_name.to_string(),
            part_index: index as i64,
        });
    }

    // Group by position, keep only the highest tier per position
    let mut by_position: std::collections::HashMap<String, DecodedSkill> =
        std::collections::HashMap::new();
    for skill in skills {
        let entry = by_position.entry(skill.position.clone()).or_insert_with(|| skill.clone());
        if skill.tier > entry.tier {
            *entry = skill;
        }
    }

    let mut result: Vec<DecodedSkill> = by_position.into_values().collect();
    result.sort_by(|a, b| a.position.cmp(&b.position));
    result
}

/// Parse a --add argument: `"Name@Tier"` or `"position@Tier"`
pub fn parse_add(spec: &str, category: i64) -> Result<SkillAdd, String> {
    let (name, tier_str) = spec.rsplit_once('@')
        .ok_or_else(|| format!("missing @tier in '{}' (expected 'Name@N')", spec))?;

    let tier: u8 = tier_str.parse()
        .map_err(|_| format!("invalid tier '{}' in '{}'", tier_str, spec))?;

    if tier < 1 || tier > 5 {
        return Err(format!("tier must be 1-5, got {}", tier));
    }

    let position = resolve_skill_name(name.trim(), category)?;
    Ok(SkillAdd { position, tier })
}

/// Parse a --remove argument: just the skill name or position
pub fn parse_remove(name: &str, category: i64) -> Result<SkillRemove, String> {
    let position = resolve_skill_name(name.trim(), category)?;
    Ok(SkillRemove { position })
}

/// Resolve a skill name (display name or part position) to a position string.
fn resolve_skill_name(name: &str, category: i64) -> Result<String, String> {
    // Try as part position directly (with or without passive_ prefix)
    let bare = name.strip_prefix("passive_").unwrap_or(name);
    if manifest::skill_display_name(category, bare).is_some() {
        return Ok(bare.to_string());
    }

    // Try as part position by checking if tier_1 exists in the parts database
    let test_part = format!("passive_{}_tier_1", bare);
    if manifest::part_index(category, &test_part).is_some() {
        return Ok(bare.to_string());
    }

    // Try as display name (case-insensitive)
    if let Some(pos) = manifest::skill_position_from_name(category, name) {
        return Ok(pos.to_string());
    }

    // Try fuzzy: prefix match on display names
    let available = manifest::skills_for_category(category);
    let lower = name.to_lowercase();
    let matches: Vec<_> = available.iter()
        .filter(|(_, info)| info.display_name.to_lowercase().starts_with(&lower))
        .collect();

    if matches.len() == 1 {
        return Ok(matches[0].0.to_string());
    }

    if matches.len() > 1 {
        let names: Vec<_> = matches.iter().map(|(_, info)| info.display_name.as_str()).collect();
        return Err(format!("ambiguous skill name '{}', matches: {}", name, names.join(", ")));
    }

    Err(format!("skill '{}' not found in category {}", name, category))
}

/// Compute the token edits for add/remove operations.
///
/// Returns (indices_to_remove, parts_to_add) where parts_to_add are
/// (part_index, part_name) tuples.
pub fn compute_edits(
    current: &[DecodedSkill],
    adds: &[SkillAdd],
    removes: &[SkillRemove],
    category: i64,
) -> Result<(Vec<i64>, Vec<(i64, String)>), String> {
    let mut remove_indices: Vec<i64> = Vec::new();
    let mut add_parts: Vec<(i64, String)> = Vec::new();

    // Process explicit removals first
    for rm in removes {
        collect_tier_removals(&rm.position, category, &mut remove_indices);
    }

    // For adds: if the skill already exists, remove its old tiers first
    // If at max capacity and no explicit removal freed a slot, auto-remove lowest tier
    let mut replaced_positions: Vec<String> = removes.iter().map(|r| r.position.clone()).collect();
    let add_positions: Vec<&str> = adds.iter().map(|a| a.position.as_str()).collect();

    for add in adds {
        // If this skill already exists, remove its old tiers
        if current.iter().any(|s| s.position == add.position) {
            collect_tier_removals(&add.position, category, &mut remove_indices);
            if !replaced_positions.contains(&add.position) {
                replaced_positions.push(add.position.clone());
            }
        } else {
            // New skill — need a free slot
            let current_count = current.len();
            let removed_count = replaced_positions.len();
            let net = current_count.saturating_sub(removed_count);

            // If no slot available, auto-remove the lowest-tier skill
            if net >= current_count && !current.is_empty() {
                let replaced_set: Vec<bool> = current.iter()
                    .map(|s| replaced_positions.contains(&s.position))
                    .collect();

                if let Some(victim) = find_replacement_slot(current, &replaced_set, &add_positions) {
                    let victim_pos = current[victim].position.clone();
                    collect_tier_removals(&victim_pos, category, &mut remove_indices);
                    replaced_positions.push(victim_pos);
                }
            }
        }

        // Add tier parts for the new skill (tier_1 through tier_N)
        for t in 1..=add.tier {
            let part_name = format!("passive_{}_tier_{}", add.position, t);
            let part_idx = manifest::part_index(category, &part_name)
                .ok_or_else(|| format!("part '{}' not found in category {}", part_name, category))?;
            add_parts.push((part_idx, part_name));
        }
    }

    Ok((remove_indices, add_parts))
}

/// Build the before/after diff for display.
pub fn build_diff(
    current: &[DecodedSkill],
    adds: &[SkillAdd],
    removes: &[SkillRemove],
    category: i64,
) -> Result<Vec<SkillDiffEntry>, String> {
    let mut diff = Vec::new();
    let mut handled: Vec<bool> = vec![false; current.len()];

    // Mark removals
    for rm in removes {
        if let Some(idx) = current.iter().position(|s| s.position == rm.position) {
            handled[idx] = true;
            diff.push(SkillDiffEntry {
                slot: idx + 1,
                before: Some(current[idx].clone()),
                after: None,
                changed: true,
            });
        }
    }

    // Process adds
    let add_positions: Vec<&str> = adds.iter().map(|a| a.position.as_str()).collect();
    let mut replaced_positions: Vec<String> = removes.iter().map(|r| r.position.clone()).collect();

    for add in adds {
        let display_name = manifest::skill_display_name(category, &add.position)
            .map(|info| info.display_name.clone())
            .unwrap_or_default();

        let new_skill = DecodedSkill {
            position: add.position.clone(),
            tier: add.tier,
            display_name,
            part_name: format!("passive_{}_tier_{}", add.position, add.tier),
            part_index: 0,
        };

        // Check if replacing an existing skill at this position
        if let Some(idx) = current.iter().position(|s| s.position == add.position) {
            handled[idx] = true;
            diff.push(SkillDiffEntry {
                slot: idx + 1,
                before: Some(current[idx].clone()),
                after: Some(new_skill),
                changed: current[idx].tier != add.tier,
            });
        } else {
            // Find a slot to replace
            let replaced_set: Vec<bool> = current.iter()
                .map(|s| handled[current.iter().position(|c| c.position == s.position).unwrap()] || replaced_positions.contains(&s.position))
                .collect();

            let slot = find_replacement_slot(current, &replaced_set, &add_positions);
            if let Some(victim) = slot {
                handled[victim] = true;
                replaced_positions.push(current[victim].position.clone());
                diff.push(SkillDiffEntry {
                    slot: victim + 1,
                    before: Some(current[victim].clone()),
                    after: Some(new_skill),
                    changed: true,
                });
            } else {
                // Adding to a new slot (was under capacity)
                diff.push(SkillDiffEntry {
                    slot: current.len() + 1,
                    before: None,
                    after: Some(new_skill),
                    changed: true,
                });
            }
        }
    }

    // Add unchanged skills
    for (i, skill) in current.iter().enumerate() {
        if !handled[i] {
            diff.push(SkillDiffEntry {
                slot: i + 1,
                before: Some(skill.clone()),
                after: Some(skill.clone()),
                changed: false,
            });
        }
    }

    diff.sort_by_key(|d| d.slot);
    Ok(diff)
}

/// Find the slot with the lowest tier to replace, excluding protected positions.
fn find_replacement_slot(
    current: &[DecodedSkill],
    already_replaced: &[bool],
    protected_positions: &[&str],
) -> Option<usize> {
    current.iter().enumerate()
        .filter(|(i, skill)| {
            !already_replaced[*i] && !protected_positions.contains(&skill.position.as_str())
        })
        .min_by_key(|(_, skill)| skill.tier)
        .map(|(i, _)| i)
}

/// Collect all part indices for a given skill position (all tiers) for removal.
fn collect_tier_removals(position: &str, category: i64, remove_indices: &mut Vec<i64>) {
    for t in 1..=5 {
        let part_name = format!("passive_{}_tier_{}", position, t);
        if let Some(idx) = manifest::part_index(category, &part_name) {
            remove_indices.push(idx);
        }
    }
}

/// Validate that a skill position can drop on the given category.
pub fn validate_skill_drop(position: &str, tier: u8, category: i64) -> Result<(), String> {
    let part_name = format!("passive_{}_tier_{}", position, tier);
    match manifest::part_index(category, &part_name) {
        Some(_) => Ok(()),
        None => Err(format!(
            "skill '{}' at tier {} is not a valid drop for category {} (part '{}' not found)",
            position, tier, category, part_name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_class_mod() {
        assert!(is_class_mod(254));
        assert!(is_class_mod(404));
        assert!(!is_class_mod(3));
        assert!(!is_class_mod(10024));
    }

    #[test]
    fn test_parse_add() {
        let result = parse_add("red_1_1@3", 254);
        assert!(result.is_ok(), "Should parse position-based add");
        let add = result.unwrap();
        assert_eq!(add.position, "red_1_1");
        assert_eq!(add.tier, 3);
    }

    #[test]
    fn test_parse_add_invalid_tier() {
        assert!(parse_add("red_1_1@0", 254).is_err());
        assert!(parse_add("red_1_1@6", 254).is_err());
    }

    #[test]
    fn test_parse_add_missing_at() {
        assert!(parse_add("red_1_1", 254).is_err());
    }

    #[test]
    fn test_parse_remove() {
        let result = parse_remove("red_1_1", 254);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().position, "red_1_1");
    }
}
