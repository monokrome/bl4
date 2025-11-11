use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("👔").size(60.0).weak());
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

        // Tab buttons for categories
        ui.horizontal(|ui| {
            let _ = ui.selectable_label(true, "Heads");
            let _ = ui.selectable_label(false, "Skins");
            let _ = ui.selectable_label(false, "Emotes");
            let _ = ui.selectable_label(false, "Echo Themes");
            let _ = ui.selectable_label(false, "Room Decorations");
            let _ = ui.selectable_label(false, "Weapon Ornaments");
            let _ = ui.selectable_label(false, "Weapon Skins");
            let _ = ui.selectable_label(false, "Vehicle Skins");
        });

        ui.add_space(20.0);

        // Display unlocked heads as example
        if let Ok(heads_value) = save.get("domains.unlockables.unlockable_heads.entries") {
            if let Some(heads_array) = heads_value.as_sequence() {
                ui.label(format!("Showing {} unlocked heads", heads_array.len()));
                ui.add_space(15.0);

                let available_width = ui.available_width();
                let items_per_row = if available_width > 1200.0 {
                    6
                } else if available_width > 900.0 {
                    4
                } else if available_width > 600.0 {
                    3
                } else {
                    2
                };

                let cell_width =
                    (available_width - (items_per_row as f32 * 15.0)) / items_per_row as f32;

                egui::Grid::new("heads_grid")
                    .num_columns(items_per_row)
                    .spacing([15.0, 15.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (i, head) in heads_array.iter().enumerate() {
                            if let Some(head_name) = head.as_str() {
                                ui.vertical(|ui| {
                                    ui.set_width(cell_width);

                                    // Placeholder box for cosmetic preview
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(cell_width, cell_width * 0.75),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        2.5,
                                        egui::Color32::from_rgb(60, 63, 70),
                                    );

                                    // Item name (shortened)
                                    let short_name =
                                        head_name.split('.').next_back().unwrap_or(head_name);
                                    ui.label(egui::RichText::new(short_name).size(14.0));

                                    // Full path as tooltip
                                    ui.label(egui::RichText::new(head_name).size(12.0).weak())
                                        .on_hover_text(head_name);
                                });
                            }

                            if (i + 1) % items_per_row == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        } else {
            ui.label("No cosmetics data found");
        }

        ui.add_space(30.0);

        // Info
        ui.separator();
        ui.add_space(15.0);
        ui.label(
            egui::RichText::new("ℹ Cosmetics are account-wide and shared between all characters")
                .weak(),
        );
        ui.label(
            egui::RichText::new(
                "Note: This is a read-only view. Unlocking cosmetics requires modifying game data.",
            )
            .weak()
            .italics(),
        );

        ui.add_space(20.0);
    });
}
