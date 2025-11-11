use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("📝").size(60.0).weak());
            ui.add_space(20.0);
            ui.heading("No Save File Selected");
            ui.add_space(15.0);
            ui.label(
                egui::RichText::new("Select a save file from the dropdown above to begin editing")
                    .size(16.0)
                    .weak(),
            );
        });
        return;
    }

    // Check if save data exists (without holding a borrow)
    let has_save_data = app
        .state
        .selected_save
        .as_ref()
        .and_then(|name| app.state.save_files.get(name))
        .and_then(|entry| entry.save_data.as_ref())
        .is_some();

    if !has_save_data {
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
    }

    // Macro to get save data without keeping a borrow
    macro_rules! get_save {
        () => {
            app.state
                .selected_save
                .as_ref()
                .and_then(|name| app.state.save_files.get(name))
                .and_then(|entry| entry.save_data.as_ref())
        };
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        let available_width = ui.available_width();
        let use_columns = available_width > 800.0;

        if use_columns {
            // Two-column layout for wider screens
            egui::Grid::new("character_grid")
                .num_columns(2)
                .spacing([40.0, 20.0])
                .striped(false)
                .show(ui, |ui| {
                    // Left column
                    ui.vertical(|ui| {
                        // Character Name
                        let char_name = get_save!()
                            .and_then(|s| s.get_character_name())
                            .unwrap_or("Unknown")
                            .to_string();
                        let mut name_buffer = char_name.clone();

                        ui.label("Character Name:");
                        ui.horizontal(|ui| {
                            let name_changed = ui
                                .add_sized(
                                    [200.0, 24.0],
                                    egui::TextEdit::singleline(&mut name_buffer),
                                )
                                .changed();
                            if name_changed && name_buffer != char_name {
                                app.with_changeset_mut(|cs| cs.set_character_name(&name_buffer));
                            }
                            if app.has_change("state.char_name") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });

                        ui.add_space(10.0);

                        // Character Class
                        if let Some(class) = get_save!().and_then(|s| s.get_character_class()) {
                            ui.label("Class:");
                            ui.label(egui::RichText::new(class).strong());
                            ui.add_space(10.0);
                        }

                        // Difficulty
                        if let Some(difficulty) = get_save!().and_then(|s| s.get_difficulty()) {
                            ui.label("Difficulty:");
                            ui.label(egui::RichText::new(difficulty).strong());
                        }
                    });

                    // Right column
                    ui.vertical(|ui| {
                        // Character Level
                        if let Some((level, xp)) = get_save!().and_then(|s| s.get_character_level())
                        {
                            ui.label("Character Level:");
                            ui.label(egui::RichText::new(format!("{}", level)).strong());
                            ui.add_space(5.0);

                            ui.label("Experience Points:");
                            ui.horizontal(|ui| {
                                let mut xp_buffer = xp.to_string();
                                let xp_changed = ui
                                    .add_sized(
                                        [150.0, 24.0],
                                        egui::TextEdit::singleline(&mut xp_buffer),
                                    )
                                    .changed();
                                if xp_changed {
                                    if let Ok(new_xp) = xp_buffer.parse::<u64>() {
                                        app.with_changeset_mut(|cs| cs.set_character_xp(new_xp));
                                    }
                                }
                                if app.has_change("state.experience[0].points") {
                                    ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                                }
                            });
                        }

                        ui.add_space(15.0);

                        // Specialization Level
                        if let Some((spec_level, spec_xp)) =
                            get_save!().and_then(|s| s.get_specialization_level())
                        {
                            ui.label("Specialization Level:");
                            ui.label(egui::RichText::new(format!("{}", spec_level)).strong());
                            ui.add_space(5.0);

                            ui.label("Specialization XP:");
                            ui.horizontal(|ui| {
                                let mut spec_xp_buffer = spec_xp.to_string();
                                let spec_xp_changed = ui
                                    .add_sized(
                                        [150.0, 24.0],
                                        egui::TextEdit::singleline(&mut spec_xp_buffer),
                                    )
                                    .changed();
                                if spec_xp_changed {
                                    if let Ok(new_xp) = spec_xp_buffer.parse::<u64>() {
                                        app.with_changeset_mut(|cs| {
                                            cs.set_specialization_xp(new_xp)
                                        });
                                    }
                                }
                                if app.has_change("state.experience[1].points") {
                                    ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                                }
                            });
                        }
                    });

                    ui.end_row();
                });

            ui.add_space(20.0);

            // Full-width sections
            egui::Grid::new("playtime_actions_grid")
                .num_columns(2)
                .spacing([40.0, 20.0])
                .striped(false)
                .show(ui, |ui| {
                    // Playtime Section
                    ui.vertical(|ui| {
                        if let Ok(playtime_value) = get_save!()
                            .and_then(|s| s.get("state.total_playtime").ok())
                            .ok_or(())
                        {
                            if let Some(seconds) = playtime_value.as_f64() {
                                let hours = seconds / 3600.0;
                                ui.label("Total Playtime:");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.1} hours ({:.0} seconds)",
                                        hours, seconds
                                    ))
                                    .strong(),
                                );
                                ui.add_space(5.0);

                                ui.label("Set Playtime (seconds):");
                                ui.horizontal(|ui| {
                                    let mut playtime_buffer = format!("{:.0}", seconds);
                                    let playtime_changed = ui
                                        .add_sized(
                                            [150.0, 24.0],
                                            egui::TextEdit::singleline(&mut playtime_buffer),
                                        )
                                        .changed();
                                    if playtime_changed {
                                        if let Ok(new_seconds) = playtime_buffer.parse::<f64>() {
                                            app.with_changeset_mut(|cs| {
                                                cs.add_parsed(
                                                    "state.total_playtime".to_string(),
                                                    &format!("{}", new_seconds),
                                                )
                                            });
                                        }
                                    }
                                    if app.has_change("state.total_playtime") {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(255, 200, 100),
                                            "●",
                                        );
                                    }
                                });
                            }
                        }
                    });

                    // Quick Actions
                    ui.vertical(|ui| {
                        if ui.button("Max Level (50)").clicked() {
                            app.with_changeset_mut(|cs| cs.set_character_xp(999999));
                        }

                        if ui.button("Max Specialization").clicked() {
                            app.with_changeset_mut(|cs| cs.set_specialization_xp(999999));
                        }
                    });

                    ui.end_row();
                });
        } else {
            // Single column layout for narrow screens (keep original vertical layout)
            let char_name = get_save!()
                .and_then(|s| s.get_character_name())
                .unwrap_or("Unknown")
                .to_string();
            let mut name_buffer = char_name.clone();
            ui.horizontal(|ui| {
                ui.label("Character Name:");
                let name_changed = ui.text_edit_singleline(&mut name_buffer).changed();
                if name_changed && name_buffer != char_name {
                    app.with_changeset_mut(|cs| cs.set_character_name(&name_buffer));
                }
                if app.has_change("state.char_name") {
                    ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                }
            });

            ui.add_space(10.0);

            if let Some(class) = get_save!().and_then(|s| s.get_character_class()) {
                ui.horizontal(|ui| {
                    ui.label("Class:");
                    ui.label(egui::RichText::new(class).strong());
                });
            }

            ui.add_space(10.0);

            if let Some(difficulty) = get_save!().and_then(|s| s.get_difficulty()) {
                ui.horizontal(|ui| {
                    ui.label("Difficulty:");
                    ui.label(egui::RichText::new(difficulty).strong());
                });
            }

            ui.add_space(30.0);

            if let Some((level, xp)) = get_save!().and_then(|s| s.get_character_level()) {
                ui.horizontal(|ui| {
                    ui.label("Character Level:");
                    ui.label(egui::RichText::new(format!("{}", level)).strong());
                });

                ui.horizontal(|ui| {
                    ui.label("Experience Points:");
                    let mut xp_buffer = xp.to_string();
                    let xp_changed = ui.text_edit_singleline(&mut xp_buffer).changed();
                    if xp_changed {
                        if let Ok(new_xp) = xp_buffer.parse::<u64>() {
                            app.with_changeset_mut(|cs| cs.set_character_xp(new_xp));
                        }
                    }
                    if app.has_change("state.experience[0].points") {
                        ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                    }
                });
            }

            ui.add_space(10.0);

            if let Some((spec_level, spec_xp)) =
                get_save!().and_then(|s| s.get_specialization_level())
            {
                ui.horizontal(|ui| {
                    ui.label("Specialization Level:");
                    ui.label(egui::RichText::new(format!("{}", spec_level)).strong());
                });

                ui.horizontal(|ui| {
                    ui.label("Specialization XP:");
                    let mut spec_xp_buffer = spec_xp.to_string();
                    let spec_xp_changed = ui.text_edit_singleline(&mut spec_xp_buffer).changed();
                    if spec_xp_changed {
                        if let Ok(new_xp) = spec_xp_buffer.parse::<u64>() {
                            app.with_changeset_mut(|cs| cs.set_specialization_xp(new_xp));
                        }
                    }
                    if app.has_change("state.experience[1].points") {
                        ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                    }
                });
            }

            ui.add_space(30.0);

            ui.heading("Playtime");
            ui.add_space(10.0);

            if let Ok(playtime_value) = get_save!()
                .and_then(|s| s.get("state.total_playtime").ok())
                .ok_or(())
            {
                if let Some(seconds) = playtime_value.as_f64() {
                    let hours = seconds / 3600.0;
                    ui.horizontal(|ui| {
                        ui.label("Total Playtime:");
                        ui.label(egui::RichText::new(format!("{:.1} hours", hours)).strong());
                        ui.label(format!("({:.0} seconds)", seconds));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Set Playtime (seconds):");
                        let mut playtime_buffer = format!("{:.0}", seconds);
                        let playtime_changed =
                            ui.text_edit_singleline(&mut playtime_buffer).changed();
                        if playtime_changed {
                            if let Ok(new_seconds) = playtime_buffer.parse::<f64>() {
                                app.with_changeset_mut(|cs| {
                                    cs.add_parsed(
                                        "state.total_playtime".to_string(),
                                        &format!("{}", new_seconds),
                                    )
                                });
                            }
                        }
                        if app.has_change("state.total_playtime") {
                            ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                        }
                    });
                }
            }

            ui.add_space(30.0);

            ui.heading("Quick Actions");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Max Level (50)").clicked() {
                    app.with_changeset_mut(|cs| cs.set_character_xp(999999));
                }

                if ui.button("Max Specialization").clicked() {
                    app.with_changeset_mut(|cs| cs.set_specialization_xp(999999));
                }
            });
        }

        ui.add_space(20.0);
    });
}
