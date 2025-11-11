use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("💰").size(60.0).weak());
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
        let use_three_columns = available_width > 1100.0;
        let use_two_columns = available_width > 750.0;

        // Extract all data we need before entering the UI closure
        // to avoid holding borrows while trying to mutate
        let (cash, eridium, golden_key) = {
            let cash = get_save!().and_then(|s| s.get_cash()).unwrap_or(0);
            let eridium = get_save!().and_then(|s| s.get_eridium()).unwrap_or(0);
            let golden_key = get_save!()
                .and_then(|s| s.get("state.currencies.golden_key").ok())
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "shift".to_string());
            (cash, eridium, golden_key)
        };

        let ammo_types = [
            ("Pistol", "state.ammo.pistol"),
            ("Assault Rifle", "state.ammo.assaultrifle"),
            ("Shotgun", "state.ammo.shotgun"),
            ("SMG", "state.ammo.smg"),
            ("Sniper", "state.ammo.sniper"),
            ("Repair Kit", "state.ammo.repairkit"),
        ];

        if use_three_columns {
            // 3-column layout for very wide screens
            egui::Grid::new("currency_grid")
                .num_columns(3)
                .spacing([50.0, 25.0])
                .striped(false)
                .show(ui, |ui| {
                    // Currencies row
                    ui.vertical(|ui| {
                        ui.label("Cash:");
                        ui.label(egui::RichText::new(format!("{}", cash)).size(20.0).strong());
                        ui.add_space(8.0);
                        let mut cash_buffer = cash.to_string();
                        ui.horizontal(|ui| {
                            let changed = ui
                                .add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut cash_buffer),
                                )
                                .changed();
                            if changed {
                                if let Ok(new_cash) = cash_buffer.parse::<u64>() {
                                    app.with_changeset_mut(|cs| cs.set_cash(new_cash));
                                }
                            }
                            if app.has_change("state.currencies.cash") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });
                        ui.add_space(10.0);
                        if ui.button("Max (9,999,999)").clicked() {
                            app.with_changeset_mut(|cs| cs.set_cash(9999999));
                        }
                    });

                    ui.vertical(|ui| {
                        ui.label("Eridium:");
                        ui.label(
                            egui::RichText::new(format!("{}", eridium))
                                .size(20.0)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        let mut eridium_buffer = eridium.to_string();
                        ui.horizontal(|ui| {
                            let changed = ui
                                .add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut eridium_buffer),
                                )
                                .changed();
                            if changed {
                                if let Ok(new_eridium) = eridium_buffer.parse::<u64>() {
                                    app.with_changeset_mut(|cs| cs.set_eridium(new_eridium));
                                }
                            }
                            if app.has_change("state.currencies.eridium") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });
                        ui.add_space(10.0);
                        if ui.button("Max (99,999)").clicked() {
                            app.with_changeset_mut(|cs| cs.set_eridium(99999));
                        }
                    });

                    ui.vertical(|ui| {
                        ui.label("Golden Keys:");
                        ui.label(egui::RichText::new(&golden_key).size(20.0).strong());
                        ui.add_space(8.0);
                        let mut key_buffer = golden_key.clone();
                        ui.horizontal(|ui| {
                            let changed = ui
                                .add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut key_buffer),
                                )
                                .changed();
                            if changed {
                                app.with_changeset_mut(|cs| {
                                    cs.add_parsed(
                                        "state.currencies.golden_key".to_string(),
                                        &key_buffer,
                                    )
                                });
                            }
                            if app.has_change("state.currencies.golden_key") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });
                    });

                    ui.end_row();
                });

            ui.add_space(30.0);

            // Ammo grid - 3 columns
            ui.add_space(5.0);

            egui::Grid::new("ammo_grid")
                .num_columns(3)
                .spacing([50.0, 15.0])
                .striped(false)
                .show(ui, |ui| {
                    for (i, (label, path)) in ammo_types.iter().enumerate() {
                        if let Ok(ammo_value) = get_save!().and_then(|s| s.get(path).ok()).ok_or(())
                        {
                            if let Some(ammo) = ammo_value.as_u64() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", label));
                                    let mut ammo_buffer = ammo.to_string();
                                    let changed = ui
                                        .add_sized(
                                            [100.0, 24.0],
                                            egui::TextEdit::singleline(&mut ammo_buffer),
                                        )
                                        .changed();
                                    if changed {
                                        if let Ok(new_ammo) = ammo_buffer.parse::<u64>() {
                                            app.with_changeset_mut(|cs| {
                                                cs.add_parsed(
                                                    path.to_string(),
                                                    &new_ammo.to_string(),
                                                )
                                            });
                                        }
                                    }
                                    if app.has_change(path) {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(255, 200, 100),
                                            "●",
                                        );
                                    }
                                });
                            }
                        }
                        if (i + 1) % 3 == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(20.0);
            if ui.button("Max All Ammo (9999)").clicked() {
                for (_, path) in ammo_types.iter() {
                    app.with_changeset_mut(|cs| cs.add_parsed(path.to_string(), "9999"));
                }
            }
        } else if use_two_columns {
            // 2-column layout for medium screens
            egui::Grid::new("currency_grid")
                .num_columns(2)
                .spacing([40.0, 25.0])
                .striped(false)
                .show(ui, |ui| {
                    // Currency cards
                    ui.vertical(|ui| {
                        ui.label("Cash:");
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(format!("{}", cash)).size(20.0).strong());
                        ui.add_space(8.0);
                        let mut cash_buffer = cash.to_string();
                        ui.horizontal(|ui| {
                            let changed = ui
                                .add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut cash_buffer),
                                )
                                .changed();
                            if changed {
                                if let Ok(new_cash) = cash_buffer.parse::<u64>() {
                                    app.with_changeset_mut(|cs| cs.set_cash(new_cash));
                                }
                            }
                            if app.has_change("state.currencies.cash") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });
                        ui.add_space(10.0);
                        if ui.button("Max (9,999,999)").clicked() {
                            app.with_changeset_mut(|cs| cs.set_cash(9999999));
                        }
                    });

                    ui.vertical(|ui| {
                        ui.label("Eridium:");
                        ui.label(
                            egui::RichText::new(format!("{}", eridium))
                                .size(20.0)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        let mut eridium_buffer = eridium.to_string();
                        ui.horizontal(|ui| {
                            let changed = ui
                                .add_sized(
                                    [180.0, 28.0],
                                    egui::TextEdit::singleline(&mut eridium_buffer),
                                )
                                .changed();
                            if changed {
                                if let Ok(new_eridium) = eridium_buffer.parse::<u64>() {
                                    app.with_changeset_mut(|cs| cs.set_eridium(new_eridium));
                                }
                            }
                            if app.has_change("state.currencies.eridium") {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "●");
                            }
                        });
                        ui.add_space(10.0);
                        if ui.button("Max (99,999)").clicked() {
                            app.with_changeset_mut(|cs| cs.set_eridium(99999));
                        }
                    });

                    ui.end_row();
                });

            ui.add_space(30.0);

            // Ammo section - 2 columns
            ui.add_space(5.0);

            egui::Grid::new("ammo_grid")
                .num_columns(2)
                .spacing([40.0, 12.0])
                .striped(false)
                .show(ui, |ui| {
                    for (i, (label, path)) in ammo_types.iter().enumerate() {
                        if let Ok(ammo_value) = get_save!().and_then(|s| s.get(path).ok()).ok_or(())
                        {
                            if let Some(ammo) = ammo_value.as_u64() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", label));
                                    let mut ammo_buffer = ammo.to_string();
                                    let changed = ui
                                        .add_sized(
                                            [120.0, 24.0],
                                            egui::TextEdit::singleline(&mut ammo_buffer),
                                        )
                                        .changed();
                                    if changed {
                                        if let Ok(new_ammo) = ammo_buffer.parse::<u64>() {
                                            app.with_changeset_mut(|cs| {
                                                cs.add_parsed(
                                                    path.to_string(),
                                                    &new_ammo.to_string(),
                                                )
                                            });
                                        }
                                    }
                                    if app.has_change(path) {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(255, 200, 100),
                                            "●",
                                        );
                                    }
                                });
                            }
                        }
                        if (i + 1) % 2 == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(20.0);
            if ui.button("Max All Ammo (9999)").clicked() {
                for (_, path) in ammo_types.iter() {
                    app.with_changeset_mut(|cs| cs.add_parsed(path.to_string(), "9999"));
                }
            }
        } else {
            // Single column for narrow screens
            ui.label(format!("{} Cash", cash));
            let mut cash_buffer = cash.to_string();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut cash_buffer);
                if ui.button("Max").clicked() {
                    app.with_changeset_mut(|cs| cs.set_cash(9999999));
                }
            });

            ui.add_space(15.0);

            ui.label(format!("{} Eridium", eridium));
            let mut eridium_buffer = eridium.to_string();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut eridium_buffer);
                if ui.button("Max").clicked() {
                    app.with_changeset_mut(|cs| cs.set_eridium(99999));
                }
            });
        }

        ui.add_space(20.0);
    });
}
