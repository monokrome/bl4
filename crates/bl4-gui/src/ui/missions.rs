use crate::app::{BL4App, MissionTab};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("📋").size(60.0).weak());
            ui.add_space(20.0);
            ui.heading("No Save File Selected");
            ui.add_space(15.0);
            ui.label(
                egui::RichText::new("Select a save file from the dropdown above")
                    .size(16.0)
                    .weak(),
            );
        });
        return;
    }

    // Get save data
    let save_data = app
        .state
        .selected_save
        .as_ref()
        .and_then(|name| app.state.save_files.get(name))
        .and_then(|entry| entry.save_data.as_ref());

    let Some(save) = save_data else {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("🔒").size(60.0).weak());
            ui.add_space(20.0);
            ui.heading("Save Not Loaded");
            ui.add_space(15.0);
            ui.label(
                egui::RichText::new(
                    "Click on the save file in the dropdown to decrypt and load it",
                )
                .size(16.0)
                .weak(),
            );
        });
        return;
    };

    // Sub-tab navigation
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.current_mission_tab,
            MissionTab::Missions,
            "Missions",
        );
        ui.selectable_value(
            &mut app.current_mission_tab,
            MissionTab::Challenges,
            "Challenges",
        );
    });
    ui.add_space(10.0);

    match app.current_mission_tab {
        MissionTab::Missions => show_missions(ui, save),
        MissionTab::Challenges => show_challenges(ui, save),
    }
}

fn show_missions(ui: &mut egui::Ui, save: &bl4::SaveFile) {
    // Get mission data
    let missions_result = save.get("missions");

    if missions_result.is_err() {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.label(egui::RichText::new("No mission data found").weak());
        });
        return;
    }

    // Two-panel layout: mission list on left, details on right
    let available_width = ui.available_width();
    let list_width = (available_width * 0.35).max(250.0);

    ui.horizontal(|ui| {
        // Left panel: Mission list
        egui::ScrollArea::vertical()
            .id_salt("mission_list")
            .show(ui, |ui| {
                ui.set_width(list_width);

                // Try to get missions from local_sets
                if let Ok(missions_value) = save.get("missions.local_sets") {
                    if let Some(local_sets) = missions_value.as_mapping() {
                        for (set_name, set_data) in local_sets {
                            let set_name_str = set_name.as_str().unwrap_or("Unknown");

                            ui.label(
                                egui::RichText::new(set_name_str)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(150, 150, 150)),
                            );
                            ui.add_space(5.0);

                            if let Some(set_mapping) = set_data.as_mapping() {
                                if let Some(missions) = set_mapping
                                    .get(serde_yaml::Value::String("missions".to_string()))
                                {
                                    if let Some(missions_map) = missions.as_mapping() {
                                        for (mission_name, mission_data) in missions_map {
                                            let mission_name_str =
                                                mission_name.as_str().unwrap_or("Unknown");

                                            // Get status
                                            let status = if let Some(mission_map) =
                                                mission_data.as_mapping()
                                            {
                                                mission_map
                                                    .get(serde_yaml::Value::String(
                                                        "status".to_string(),
                                                    ))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Unknown")
                                            } else {
                                                "Unknown"
                                            };

                                            // Color based on status
                                            let color = match status {
                                                "Completed" => {
                                                    egui::Color32::from_rgb(100, 200, 100)
                                                }
                                                "Active" => egui::Color32::from_rgb(255, 200, 100),
                                                "Kickoffing" => {
                                                    egui::Color32::from_rgb(150, 150, 200)
                                                }
                                                _ => egui::Color32::from_rgb(180, 180, 180),
                                            };

                                            let icon = match status {
                                                "Completed" => "✓",
                                                "Active" => "→",
                                                "Kickoffing" => "…",
                                                _ => "?",
                                            };

                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(icon).color(color));
                                                ui.label(
                                                    egui::RichText::new(mission_name_str)
                                                        .color(color),
                                                );
                                            });
                                            ui.add_space(3.0);
                                        }
                                    }
                                }
                            }

                            ui.add_space(10.0);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("No missions found").weak());
                }
            });

        ui.separator();

        // Right panel: Mission details (would show tasks when a mission is selected)
        egui::ScrollArea::vertical()
            .id_salt("mission_details")
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Select a mission to view details")
                        .weak()
                        .italics(),
                );
                ui.add_space(10.0);
                ui.label("Mission tasks and objectives will appear here");
            });
    });
}

fn show_challenges(ui: &mut egui::Ui, save: &bl4::SaveFile) {
    // Two-panel layout: challenge categories on left, challenge grid on right
    let available_width = ui.available_width();
    let list_width = (available_width * 0.25).max(200.0);

    ui.horizontal(|ui| {
        // Left panel: Challenge categories
        egui::ScrollArea::vertical()
            .id_salt("challenge_categories")
            .show(ui, |ui| {
                ui.set_width(list_width);

                // Try to get activities data
                if let Ok(activities_value) = save.get("activities.allactivities") {
                    if let Some(activities_map) = activities_value.as_mapping() {
                        for (activity_name, _activity_data) in activities_map {
                            let activity_str = activity_name.as_str().unwrap_or("Unknown");
                            let _ = ui.selectable_label(false, activity_str);
                        }

                        if activities_map.is_empty() {
                            ui.label(egui::RichText::new("No challenges found").weak());
                        }
                    } else {
                        ui.label(egui::RichText::new("No challenges found").weak());
                    }
                } else {
                    ui.label(egui::RichText::new("No challenge data").weak());
                }
            });

        ui.separator();

        // Right panel: Challenge grid
        egui::ScrollArea::vertical()
            .id_salt("challenge_grid")
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Select a category to view challenges")
                        .weak()
                        .italics(),
                );
                ui.add_space(10.0);

                // Placeholder for challenge grid
                ui.label("Challenge details will appear here in a grid layout");
                ui.label("Each challenge will show:");
                ui.label("  • Challenge name");
                ui.label("  • Progress (e.g., 45/100)");
                ui.label("  • Completion status");
            });
    });
}
