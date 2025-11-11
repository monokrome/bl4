use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(30.0);

        ui.horizontal(|ui| {
            ui.heading("💾 Backup Manager");
            ui.add_space(20.0);

            // Create Manual Backup button
            if ui.button(egui::RichText::new("➕ Create Backup").size(16.0)).clicked() {
                app.show_manual_backup_dialog = true;
                app.manual_backup_save_selection = None;
                app.manual_backup_tag.clear();
                app.manual_backup_description.clear();
            }
        });

        ui.add_space(30.0);

        // Check if we have any save files
        if app.state.save_files.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(egui::RichText::new("📂").size(60.0).weak());
                ui.add_space(20.0);
                ui.heading("No Save Directory Loaded");
                ui.add_space(15.0);
                ui.label(egui::RichText::new("Load a save directory to view backups").size(16.0).weak());
            });
            return;
        }

        // Collect save file info first to avoid borrow conflicts
        let save_file_infos: Vec<_> = app.state.save_files.iter()
            .map(|(name, entry)| (name.clone(), entry.path.clone()))
            .collect();

        let mut save_files: Vec<_> = save_file_infos.iter().map(|(name, _)| name.clone()).collect();
        save_files.sort();

        // Track actions to perform after rendering
        let mut delete_action: Option<(std::path::PathBuf, String)> = None;
        let mut restore_action: Option<(std::path::PathBuf, String, String, String)> = None;
        let mut edit_action: Option<(String, std::path::PathBuf, Option<String>, Option<String>)> = None;

        for save_name in save_files {
            if let Some((_, path)) = save_file_infos.iter().find(|(n, _)| n == &save_name) {
                // Save file header
                ui.label(egui::RichText::new(&save_name).size(20.0).strong());
                ui.add_space(5.0);
                ui.label(egui::RichText::new(format!("{}", path.display())).weak());

                ui.add_space(15.0);

                // Load backup versions for this save
                match bl4::list_backup_versions(path) {
                    Ok(versions) if !versions.is_empty() => {
                        ui.label(egui::RichText::new(format!("{} backup version(s)", versions.len())).weak());
                        ui.add_space(15.0);

                        // Show each version
                        for version in versions {
                            ui.horizontal(|ui| {
                                // Left column: Version info
                                ui.vertical(|ui| {
                                    // Timestamp
                                    let timestamp = version.timestamp.split('T')
                                        .next()
                                        .unwrap_or(&version.timestamp);
                                    let time = version.timestamp.split('T')
                                        .nth(1)
                                        .and_then(|t| t.split('.').next())
                                        .unwrap_or("");

                                    ui.label(egui::RichText::new(format!("📅 {} {}", timestamp, time)).size(17.0));

                                    ui.add_space(5.0);

                                    // Tag (if present)
                                    if let Some(tag) = &version.tag {
                                        ui.label(egui::RichText::new(format!("🏷 {}", tag)).color(egui::Color32::from_rgb(120, 170, 255)));
                                    }

                                    // Description (if present)
                                    if let Some(desc) = &version.description {
                                        ui.label(egui::RichText::new(desc).weak());
                                    }

                                    ui.add_space(5.0);

                                    // Size and type
                                    let size_kb = version.file_size / 1024;
                                    let type_label = if version.auto_created {
                                        "Auto"
                                    } else {
                                        "Manual"
                                    };
                                    ui.label(egui::RichText::new(format!("{} KB  •  {}", size_kb, type_label)).weak());
                                });

                                // Right column: Actions
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Delete button
                                    if ui.button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(255, 120, 120))).clicked() {
                                        delete_action = Some((path.clone(), version.id.clone()));
                                    }

                                    // Restore button
                                    if ui.button(egui::RichText::new("Restore").color(egui::Color32::from_rgb(120, 200, 120))).clicked() {
                                        restore_action = Some((path.clone(), version.id.clone(), version.timestamp.clone(), save_name.clone()));
                                    }

                                    // Edit tag/description button
                                    if ui.button("Edit").clicked() {
                                        edit_action = Some((version.id.clone(), path.clone(), version.tag.clone(), version.description.clone()));
                                    }
                                });
                            });

                            ui.add_space(20.0);
                        }
                    }
                    Ok(_) => {
                        // No backups for this save
                        ui.label(egui::RichText::new("No backups yet").weak());
                        ui.add_space(5.0);
                        ui.label(egui::RichText::new("Backups will be created automatically when you load or save this file").weak().italics());
                        ui.add_space(20.0);
                    }
                    Err(e) => {
                        ui.colored_label(egui::Color32::RED, format!("Error loading backups: {}", e));
                        ui.add_space(20.0);
                    }
                }

                ui.add_space(25.0);
            }
        }

        // Process actions after rendering
        if let Some((path, version_id)) = delete_action {
            match bl4::delete_backup_version(&path, &version_id) {
                Ok(_) => {
                    app.set_status("Deleted backup version".to_string());
                }
                Err(e) => {
                    app.set_error(format!("Failed to delete backup: {}", e));
                }
            }
        }

        if let Some((path, version_id, timestamp, save_name)) = restore_action {
            match bl4::restore_backup_version(&path, &version_id) {
                Ok(_) => {
                    app.set_status(format!("Restored backup from {}", timestamp));

                    // Reload the save file if it's currently selected
                    if app.state.selected_save.as_deref() == Some(&save_name) {
                        let _ = app.load_current_save();
                    }
                }
                Err(e) => {
                    app.set_error(format!("Failed to restore backup: {}", e));
                }
            }
        }

        if let Some((version_id, save_path, tag, description)) = edit_action {
            app.show_edit_version_dialog = true;
            app.edit_version_id = version_id;
            app.edit_version_save_path = save_path;
            app.edit_version_tag = tag.unwrap_or_default();
            app.edit_version_description = description.unwrap_or_default();
        }

        ui.add_space(20.0);
    });

    // Manual backup creation dialog
    if app.show_manual_backup_dialog {
        egui::Window::new("Create Manual Backup")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.set_min_width(500.0);

                ui.heading("Create Manual Backup");
                ui.add_space(15.0);

                // Save file selection
                ui.label("Select save file to backup:");
                ui.add_space(5.0);

                let mut save_files: Vec<_> = app.state.save_files.keys().cloned().collect();
                save_files.sort();
                save_files.insert(0, "All save files".to_string());

                let selected_text = app
                    .manual_backup_save_selection
                    .as_deref()
                    .unwrap_or("Select a save file...");

                egui::ComboBox::from_id_salt("manual_backup_save_select")
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        for save_file in &save_files {
                            if ui
                                .selectable_label(
                                    app.manual_backup_save_selection.as_deref() == Some(save_file),
                                    save_file,
                                )
                                .clicked()
                            {
                                app.manual_backup_save_selection = Some(save_file.clone());
                            }
                        }
                    });

                ui.add_space(15.0);

                // Tag field
                ui.label("Tag (optional):");
                ui.add_space(5.0);
                ui.text_edit_singleline(&mut app.manual_backup_tag);

                ui.add_space(10.0);

                // Description field
                ui.label("Description (optional):");
                ui.add_space(5.0);
                ui.text_edit_multiline(&mut app.manual_backup_description);

                ui.add_space(20.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        app.show_manual_backup_dialog = false;
                    }

                    ui.add_space(10.0);

                    let can_create = app.manual_backup_save_selection.is_some();
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create Backup"))
                        .clicked()
                    {
                        // Create the backup(s)
                        let selection = app.manual_backup_save_selection.clone().unwrap();
                        let tag = if app.manual_backup_tag.is_empty() {
                            None
                        } else {
                            Some(app.manual_backup_tag.clone())
                        };
                        let description = if app.manual_backup_description.is_empty() {
                            None
                        } else {
                            Some(app.manual_backup_description.clone())
                        };

                        if selection == "All save files" {
                            // Backup all save files
                            let mut success_count = 0;
                            let mut error_count = 0;

                            for entry in app.state.save_files.values() {
                                match bl4::create_versioned_backup(
                                    &entry.path,
                                    Some(&app.state.steam_id),
                                    tag.clone(),
                                    description.clone(),
                                    false, // Manual backup
                                ) {
                                    Ok(_) => success_count += 1,
                                    Err(e) => {
                                        eprintln!("Failed to backup {}: {}", entry.name, e);
                                        error_count += 1;
                                    }
                                }
                            }

                            if error_count == 0 {
                                app.set_status(format!("Created {} backup(s)", success_count));
                            } else {
                                app.set_error(format!(
                                    "Created {} backup(s), {} failed",
                                    success_count, error_count
                                ));
                            }
                        } else {
                            // Backup single file
                            if let Some(entry) = app.state.save_files.get(&selection) {
                                match bl4::create_versioned_backup(
                                    &entry.path,
                                    Some(&app.state.steam_id),
                                    tag,
                                    description,
                                    false, // Manual backup
                                ) {
                                    Ok(_) => {
                                        app.set_status(format!("Created backup of {}", selection));
                                    }
                                    Err(e) => {
                                        app.set_error(format!("Failed to create backup: {}", e));
                                    }
                                }
                            }
                        }

                        app.show_manual_backup_dialog = false;
                    }
                });
            });
    }

    // Edit version dialog
    if app.show_edit_version_dialog {
        egui::Window::new("Edit Backup Version")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.set_min_width(500.0);

                ui.heading("Edit Backup Version");
                ui.add_space(15.0);

                // Tag field
                ui.label("Tag:");
                ui.add_space(5.0);
                ui.text_edit_singleline(&mut app.edit_version_tag);

                ui.add_space(10.0);

                // Description field
                ui.label("Description:");
                ui.add_space(5.0);
                ui.text_edit_multiline(&mut app.edit_version_description);

                ui.add_space(20.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        app.show_edit_version_dialog = false;
                    }

                    ui.add_space(10.0);

                    if ui.button("Save").clicked() {
                        // Update the backup metadata
                        let tag = if app.edit_version_tag.is_empty() {
                            None
                        } else {
                            Some(app.edit_version_tag.clone())
                        };
                        let description = if app.edit_version_description.is_empty() {
                            None
                        } else {
                            Some(app.edit_version_description.clone())
                        };

                        match bl4::update_backup_version_metadata(
                            &app.edit_version_save_path,
                            &app.edit_version_id,
                            tag,
                            description,
                        ) {
                            Ok(_) => {
                                app.set_status("Updated backup metadata".to_string());
                            }
                            Err(e) => {
                                app.set_error(format!("Failed to update metadata: {}", e));
                            }
                        }

                        app.show_edit_version_dialog = false;
                    }
                });
            });
    }
}
