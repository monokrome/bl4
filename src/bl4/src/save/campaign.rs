//! Campaign progression reading and mutation.
//!
//! Reads mission status from decrypted save YAML, and provides mutation
//! functions to set campaign progress to a specific point.

use super::SaveError;
use crate::missions;

/// Status of a mission set in the save file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignStatus {
    NotStarted,
    Active,
    Completed,
}

impl std::fmt::Display for CampaignStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "not started"),
            Self::Active => write!(f, "active"),
            Self::Completed => write!(f, "completed"),
        }
    }
}

/// A mission set's status as read from the save file.
#[derive(Debug, Clone)]
pub struct CampaignEntry {
    pub mission_set: String,
    pub status: CampaignStatus,
    pub region: String,
}

/// Description of changes that will be applied by `set_campaign_progress`.
#[derive(Debug, Clone)]
pub struct CampaignChanges {
    pub completed_sets: Vec<String>,
    pub active_set: String,
    pub active_mission: String,
}

/// Read current campaign status from save data.
///
/// Returns main story mission sets in order with their completion status.
pub fn get_campaign_status(data: &serde_yaml::Value) -> Vec<CampaignEntry> {
    let order = missions::main_story_order();
    let missions_node = &data["missions"];

    order
        .iter()
        .map(|ms| {
            let status = read_set_status(missions_node, &ms.name);
            CampaignEntry {
                mission_set: ms.name.clone(),
                status,
                region: ms.region.clone(),
            }
        })
        .collect()
}

/// Compute what changes are needed to set progress to a target mission set.
///
/// Returns `None` if the target is not a valid main story mission set.
pub fn plan_campaign_progress(target: &str) -> Option<CampaignChanges> {
    let resolved = missions::resolve_mission_set_name(target)?;
    let prereqs = missions::prerequisites_for(resolved);

    if prereqs.is_empty() {
        return None;
    }

    let (completed, active) = prereqs.split_at(prereqs.len() - 1);
    let active_set = &active[0];

    let active_mission = missions::first_mission_in_set(&active_set.name)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| {
            active_set
                .name
                .replace("missionset_", "mission_")
                .to_string()
        });

    Some(CampaignChanges {
        completed_sets: completed.iter().map(|ms| ms.name.clone()).collect(),
        active_set: active_set.name.clone(),
        active_mission,
    })
}

/// Apply campaign progress changes to save data.
///
/// Marks all prerequisite sets as completed and the target as active.
pub fn apply_campaign_progress(
    data: &mut serde_yaml::Value,
    changes: &CampaignChanges,
) -> Result<(), SaveError> {
    ensure_missions_structure(data);

    // Mark all prerequisite sets as completed
    for set_name in &changes.completed_sets {
        mark_set_completed(data, set_name);
    }

    // Mark the target set as active with its first mission active
    mark_set_active(data, &changes.active_set, &changes.active_mission);

    // Update tracked missions
    let tracked_name = title_case_mission(&changes.active_mission);
    data["missions"]["tracked_missions"] =
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(tracked_name)]);

    Ok(())
}

// --- Internal helpers ---

fn read_set_status(missions: &serde_yaml::Value, set_name: &str) -> CampaignStatus {
    // Check local_sets first (completed sets live here)
    if let Some(set_data) = missions["local_sets"][set_name].as_mapping() {
        if set_data
            .get(serde_yaml::Value::String("status".to_string()))
            .and_then(|v| v.as_str())
            == Some("completed")
        {
            return CampaignStatus::Completed;
        }
        // Has missions but no completed status = active
        if set_data.contains_key(serde_yaml::Value::String("missions".to_string())) {
            return CampaignStatus::Active;
        }
    }

    // Check remote_sets (active sets sometimes appear here)
    if missions["remote_sets"][set_name].is_mapping() {
        return CampaignStatus::Active;
    }

    CampaignStatus::NotStarted
}

fn ensure_missions_structure(data: &mut serde_yaml::Value) {
    if data["missions"].is_null() {
        data["missions"] = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    }
    if data["missions"]["local_sets"].is_null() {
        data["missions"]["local_sets"] =
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    }
}

fn mark_set_completed(data: &mut serde_yaml::Value, set_name: &str) {
    let set_key = serde_yaml::Value::String(set_name.to_string());

    // Remove from remote_sets if present
    if let Some(remote) = data["missions"]["remote_sets"].as_mapping_mut() {
        remote.remove(&set_key);
    }

    let new_entry = build_completed_set_entry(set_name);

    // Write to local_sets, preserving existing data if present
    if let Some(local) = data["missions"]["local_sets"].as_mapping_mut() {
        let existing = local.get(&set_key).cloned();
        if let Some(serde_yaml::Value::Mapping(mut existing_map)) = existing {
            complete_existing_set(&mut existing_map);
            local.insert(set_key, serde_yaml::Value::Mapping(existing_map));
        } else {
            local.insert(set_key, serde_yaml::Value::Mapping(new_entry));
        }
    }
}

fn build_completed_set_entry(set_name: &str) -> serde_yaml::Mapping {
    let mut set_entry = serde_yaml::Mapping::new();
    set_entry.insert(
        serde_yaml::Value::String("status".to_string()),
        serde_yaml::Value::String("completed".to_string()),
    );

    if let Some(mission) = missions::first_mission_in_set(set_name) {
        let mut mission_entry = serde_yaml::Mapping::new();
        mission_entry.insert(
            serde_yaml::Value::String("ui_flags".to_string()),
            serde_yaml::Value::Number(1.into()),
        );
        mission_entry.insert(
            serde_yaml::Value::String("status".to_string()),
            serde_yaml::Value::String("completed".to_string()),
        );

        let mut missions_map = serde_yaml::Mapping::new();
        missions_map.insert(
            serde_yaml::Value::String(mission.name.clone()),
            serde_yaml::Value::Mapping(mission_entry),
        );
        set_entry.insert(
            serde_yaml::Value::String("missions".to_string()),
            serde_yaml::Value::Mapping(missions_map),
        );
    }

    set_entry
}

fn complete_existing_set(existing: &mut serde_yaml::Mapping) {
    existing.insert(
        serde_yaml::Value::String("status".to_string()),
        serde_yaml::Value::String("completed".to_string()),
    );
    if let Some(serde_yaml::Value::Mapping(missions)) =
        existing.get_mut(serde_yaml::Value::String("missions".to_string()))
    {
        for (_name, mdata) in missions.iter_mut() {
            if let serde_yaml::Value::Mapping(m) = mdata {
                m.insert(
                    serde_yaml::Value::String("status".to_string()),
                    serde_yaml::Value::String("completed".to_string()),
                );
                m.insert(
                    serde_yaml::Value::String("ui_flags".to_string()),
                    serde_yaml::Value::Number(1.into()),
                );
                m.remove(serde_yaml::Value::String("objectives".to_string()));
            }
        }
    }
}

fn mark_set_active(data: &mut serde_yaml::Value, set_name: &str, mission_name: &str) {
    let set_key = serde_yaml::Value::String(set_name.to_string());

    // Build active mission entry
    let mut mission_entry = serde_yaml::Mapping::new();
    mission_entry.insert(
        serde_yaml::Value::String("status".to_string()),
        serde_yaml::Value::String("Active".to_string()),
    );

    let mut missions_map = serde_yaml::Mapping::new();
    missions_map.insert(
        serde_yaml::Value::String(mission_name.to_string()),
        serde_yaml::Value::Mapping(mission_entry),
    );

    let mut set_entry = serde_yaml::Mapping::new();
    set_entry.insert(
        serde_yaml::Value::String("missions".to_string()),
        serde_yaml::Value::Mapping(missions_map),
    );

    // Place in local_sets (matching observed save behavior for active main missions)
    if let Some(local) = data["missions"]["local_sets"].as_mapping_mut() {
        local.insert(set_key, serde_yaml::Value::Mapping(set_entry));
    }
}

/// Convert "mission_main_grasslands1" to "Mission_Main_Grasslands1" for tracked_missions.
fn title_case_mission(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_save_data() -> serde_yaml::Value {
        serde_yaml::from_str(
            r#"
missions:
  tracked_missions:
  - Mission_Main_Beach
  local_sets:
    missionset_main_prisonprologue:
      status: completed
      missions:
        mission_main_prisonprologue:
          ui_flags: 1
          status: completed
    missionset_main_beach:
      missions:
        mission_main_beach:
          status: Active
          objectives:
            reach_village:
              status: Active
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_get_campaign_status() {
        let data = sample_save_data();
        let status = get_campaign_status(&data);
        assert!(!status.is_empty());

        let prologue = status
            .iter()
            .find(|e| e.mission_set.contains("prisonprologue"))
            .unwrap();
        assert_eq!(prologue.status, CampaignStatus::Completed);

        let beach = status
            .iter()
            .find(|e| e.mission_set.contains("beach"))
            .unwrap();
        assert_eq!(beach.status, CampaignStatus::Active);

        let grasslands1 = status
            .iter()
            .find(|e| e.mission_set == "missionset_main_grasslands1")
            .unwrap();
        assert_eq!(grasslands1.status, CampaignStatus::NotStarted);
    }

    #[test]
    fn test_plan_campaign_progress() {
        let changes = plan_campaign_progress("grasslands1").unwrap();
        assert_eq!(changes.active_set, "missionset_main_grasslands1");
        assert!(changes.completed_sets.contains(&"missionset_main_prisonprologue".to_string()));
        assert!(changes.completed_sets.contains(&"missionset_main_beach".to_string()));
        assert!(!changes.completed_sets.contains(&"missionset_main_grasslands1".to_string()));
    }

    #[test]
    fn test_apply_campaign_progress() {
        let mut data = sample_save_data();
        let changes = plan_campaign_progress("grasslands1").unwrap();
        apply_campaign_progress(&mut data, &changes).unwrap();

        // Prologue should be completed
        let prologue_status = data["missions"]["local_sets"]["missionset_main_prisonprologue"]
            ["status"]
            .as_str();
        assert_eq!(prologue_status, Some("completed"));

        // Beach should be completed
        let beach_status =
            data["missions"]["local_sets"]["missionset_main_beach"]["status"].as_str();
        assert_eq!(beach_status, Some("completed"));

        // Grasslands1 should be active
        let g1_mission = &data["missions"]["local_sets"]["missionset_main_grasslands1"]
            ["missions"]["mission_main_grasslands1"]["status"];
        assert_eq!(g1_mission.as_str(), Some("Active"));
    }

    #[test]
    fn test_title_case_mission() {
        assert_eq!(
            title_case_mission("mission_main_grasslands1"),
            "Mission_Main_Grasslands1"
        );
        assert_eq!(
            title_case_mission("mission_main_prisonprologue"),
            "Mission_Main_Prisonprologue"
        );
    }

    #[test]
    fn test_plan_returns_none_for_invalid() {
        assert!(plan_campaign_progress("nonexistent_mission").is_none());
    }
}
