use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, _app: &mut BL4App) {
    if _app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("🎒").size(60.0).weak());
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
    let save_data = _app
        .state
        .selected_save
        .as_ref()
        .and_then(|name| _app.state.save_files.get(name))
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

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        let mut total_items = 0;

        // First: Show Equipped Items
        if let Ok(equipped) = save.get("state.inventory.equipped_inventory.equipped") {
            if let Some(equipped_map) = equipped.as_mapping() {
                for (key, value) in equipped_map.iter() {
                    if let Some(key_str) = key.as_str() {
                        if key_str.starts_with("slot_") {
                            total_items += 1;

                            ui.group(|ui| {
                                ui.set_min_width(ui.available_width());

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⚔").size(18.0));
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Weapon Slot {}",
                                            key_str.trim_start_matches("slot_")
                                        ))
                                        .strong(),
                                    );
                                    ui.colored_label(
                                        egui::Color32::from_rgb(100, 200, 255),
                                        "(equipped)",
                                    );
                                });

                                if let Some(serial) = value.as_str() {
                                    ui.horizontal(|ui| {
                                        ui.label("Serial:");
                                        ui.label(egui::RichText::new(serial).monospace().weak());
                                    });
                                }
                            });

                            ui.add_space(5.0);
                        }
                    }
                }
            }
        }

        // Second: Show Backpack Items
        if let Ok(inventory) = save.get("state.inventory.items.backpack") {
            if let Some(backpack) = inventory.as_mapping() {
                for (key, value) in backpack.iter() {
                    if let Some(key_str) = key.as_str() {
                        if key_str.starts_with("slot_") {
                            total_items += 1;

                            // Check if item is equipped (flags == 1)
                            let is_equipped = value
                                .get("flags")
                                .and_then(|f| f.as_u64())
                                .map(|f| f == 1)
                                .unwrap_or(false);

                            ui.group(|ui| {
                                ui.set_min_width(ui.available_width());

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("🎒").size(18.0));
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Backpack Slot {}",
                                            key_str.trim_start_matches("slot_")
                                        ))
                                        .strong(),
                                    );

                                    if is_equipped {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(100, 200, 255),
                                            "(equipped)",
                                        );
                                    }
                                });

                                // Show serial if available
                                if let Some(serial) = value.get("serial").and_then(|s| s.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.label("Serial:");
                                        ui.label(egui::RichText::new(serial).monospace().weak());
                                    });
                                }
                            });

                            ui.add_space(5.0);
                        }
                    }
                }
            }
        }

        if total_items == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(egui::RichText::new("No items found").weak());
                ui.label(egui::RichText::new("Start playing to collect items").weak());
            });
        } else {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("Total items: {}", total_items)).weak());
        }

        ui.add_space(20.0);
    });
}
