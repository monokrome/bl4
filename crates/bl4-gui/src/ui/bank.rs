use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, _app: &mut BL4App) {
    if _app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("🏦").size(60.0).weak());
            ui.add_space(20.0);
            ui.heading("No Save File Selected");
            ui.add_space(15.0);
            ui.label(
                egui::RichText::new("Select profile.sav from the dropdown above")
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
            ui.heading("Profile Not Loaded");
            ui.add_space(15.0);
            ui.label(
                egui::RichText::new("Click on profile.sav in the dropdown to decrypt and load it")
                    .size(16.0)
                    .weak(),
            );
        });
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        let mut total_items = 0;

        // Try common paths where bank items might be stored
        let possible_paths = [
            "state.inventory.items.bank",
            "state.inventory.items.shared",
            "inventory.items.bank",
            "inventory.items.shared",
            "bank.items",
            "shared_inventory.items",
        ];

        let mut found_items = false;

        for path in &possible_paths {
            if let Ok(bank_data) = save.get(path) {
                if let Some(bank_map) = bank_data.as_mapping() {
                    if !bank_map.is_empty() {
                        found_items = true;

                        for (key, value) in bank_map.iter() {
                            if let Some(key_str) = key.as_str() {
                                if key_str.starts_with("slot_") {
                                    total_items += 1;

                                    ui.group(|ui| {
                                        ui.set_min_width(ui.available_width());

                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("🏦").size(18.0));
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Bank Slot {}",
                                                    key_str.trim_start_matches("slot_")
                                                ))
                                                .strong(),
                                            );
                                        });

                                        // Show serial if available
                                        if let Some(serial) =
                                            value.get("serial").and_then(|s| s.as_str())
                                        {
                                            ui.horizontal(|ui| {
                                                ui.label("Serial:");
                                                ui.label(
                                                    egui::RichText::new(serial).monospace().weak(),
                                                );
                                            });
                                        } else if let Some(serial) = value.as_str() {
                                            // Sometimes the value itself is the serial
                                            ui.horizontal(|ui| {
                                                ui.label("Serial:");
                                                ui.label(
                                                    egui::RichText::new(serial).monospace().weak(),
                                                );
                                            });
                                        }
                                    });

                                    ui.add_space(5.0);
                                }
                            }
                        }
                        break; // Found items, stop searching
                    }
                }
            }
        }

        if !found_items {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(egui::RichText::new("No bank items found").weak());
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Bank storage is shared across all characters").weak(),
                );
                ui.label(
                    egui::RichText::new("Store items in the bank in-game to see them here").weak(),
                );
            });
        } else if total_items > 0 {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("Total items: {}", total_items)).weak());
        }

        ui.add_space(20.0);
    });
}
