use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("🗺").size(60.0).weak());
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

    let mut reveal_clicked: Option<String> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        // Current Location
        if let Ok(current_region) = save.get("state.world_region_name") {
            if let Some(region_str) = current_region.as_str() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📍").size(20.0));
                    ui.label("Current Location:");
                    ui.label(
                        egui::RichText::new(region_str)
                            .strong()
                            .color(egui::Color32::from_rgb(100, 200, 255)),
                    );
                });
                ui.add_space(15.0);
            }
        }

        // Discovered Regions
        ui.label(egui::RichText::new("Discovered Regions").size(18.0));
        ui.add_space(10.0);

        if let Ok(regions_value) = save.get("gbx_discovery_pc.hasseenregionlist") {
            if let Some(regions) = regions_value.as_sequence() {
                if regions.is_empty() {
                    ui.label(egui::RichText::new("No regions discovered yet").weak());
                } else {
                    let available_width = ui.available_width();
                    let columns = if available_width > 1000.0 {
                        3
                    } else if available_width > 600.0 {
                        2
                    } else {
                        1
                    };

                    egui::Grid::new("regions_grid")
                        .num_columns(columns)
                        .spacing([20.0, 10.0])
                        .show(ui, |ui| {
                            for (i, region) in regions.iter().enumerate() {
                                if let Some(region_name) = region.as_str() {
                                    ui.horizontal(|ui| {
                                        ui.label("✓");
                                        ui.label(region_name);
                                    });
                                }
                                if (i + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                }
            } else {
                ui.label(egui::RichText::new("Invalid region data format").weak());
            }
        } else {
            ui.label(egui::RichText::new("No region discovery data found").weak());
        }

        ui.add_space(30.0);

        // Map Fog of War Data
        ui.label(egui::RichText::new("Map Exploration").size(18.0));
        ui.add_space(10.0);

        if let Ok(fod_datas) = save.get("gbx_discovery_pc.foddatas") {
            if let Some(fod_array) = fod_datas.as_sequence() {
                if fod_array.is_empty() {
                    ui.label(egui::RichText::new("No map exploration data").weak());
                } else {
                    for fod_entry in fod_array.iter() {
                        let level_name = fod_entry
                            .get("levelname")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");

                        let fod_x = fod_entry
                            .get("foddimensionx")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        let fod_y = fod_entry
                            .get("foddimensiony")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        let fod_data = fod_entry.get("foddata").and_then(|v| v.as_str());

                        ui.group(|ui| {
                            ui.set_min_width(ui.available_width());

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("🗺").size(20.0));
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(level_name).strong());
                                    ui.label(
                                        egui::RichText::new(format!("Grid: {}×{}", fod_x, fod_y))
                                            .weak(),
                                    );

                                    if let Some(data_str) = fod_data {
                                        match calculate_exploration_percentage(
                                            data_str, fod_x, fod_y,
                                        ) {
                                            Ok(percentage) => {
                                                let color = if percentage >= 90.0 {
                                                    egui::Color32::from_rgb(100, 200, 100)
                                                } else if percentage >= 50.0 {
                                                    egui::Color32::from_rgb(200, 200, 100)
                                                } else {
                                                    egui::Color32::from_rgb(200, 100, 100)
                                                };
                                                ui.colored_label(
                                                    color,
                                                    format!("Explored: {:.1}%", percentage),
                                                );
                                            }
                                            Err(_) => {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Unable to calculate exploration",
                                                    )
                                                    .weak(),
                                                );
                                            }
                                        }
                                    }
                                });

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Reveal All").clicked() {
                                            reveal_clicked = Some(level_name.to_string());
                                        }
                                    },
                                );
                            });
                        });
                        ui.add_space(10.0);
                    }
                }
            }
        } else {
            ui.label(egui::RichText::new("No fog of war data found").weak());
        }

        ui.add_space(30.0);

        // Discovered Worlds
        ui.label(egui::RichText::new("Discovered Worlds").size(18.0));
        ui.add_space(10.0);

        if let Ok(worlds_value) = save.get("gbx_discovery_pc.hasseenworldlist") {
            if let Some(worlds) = worlds_value.as_sequence() {
                if worlds.is_empty() {
                    ui.label(egui::RichText::new("No worlds discovered yet").weak());
                } else {
                    for world in worlds {
                        if let Some(world_name) = world.as_str() {
                            ui.horizontal(|ui| {
                                ui.label("🌍");
                                ui.label(world_name);
                            });
                        }
                    }
                }
            }
        }

        ui.add_space(20.0);
    });

    // Handle button click after borrow is released
    if let Some(map_name) = reveal_clicked {
        app.set_status(format!("Revealing map: {}", map_name));
        // TODO: Implement reveal all - set all bits in foddata to 1
    }
}

/// Calculate the exploration percentage from FOD data
fn calculate_exploration_percentage(
    base64_data: &str,
    grid_x: u64,
    grid_y: u64,
) -> anyhow::Result<f32> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    // Decode base64
    let compressed = BASE64.decode(base64_data)?;

    // Decompress zlib
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut bitmap = Vec::new();
    decoder.read_to_end(&mut bitmap)?;

    // Count set bits
    let total_cells = (grid_x * grid_y) as usize;
    let mut explored_cells = 0;

    for byte in &bitmap {
        explored_cells += byte.count_ones() as usize;
    }

    // Calculate percentage
    if total_cells > 0 {
        Ok((explored_cells as f32 / total_cells as f32) * 100.0)
    } else {
        Ok(0.0)
    }
}
