use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);

        if app.state.save_directory.is_none() {
            ui.add_space(50.0);
            ui.heading("Welcome to OnTheBorder");
            ui.add_space(30.0);

            ui.label(egui::RichText::new("BL4 Editor").size(18.0).weak());
            ui.add_space(40.0);

            // Drop zone visual
            let (rect, _) = ui.allocate_exact_size(egui::vec2(500.0, 150.0), egui::Sense::hover());
            ui.painter().rect_stroke(
                rect,
                8.0,
                egui::Stroke::new(2.0, ui.visuals().weak_text_color()),
            );

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("📁").size(40.0));
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("Drop save directory or .sav file here").size(16.0),
                        );
                    });
                });
            });

            ui.add_space(30.0);

            let browse_response = ui.button("Browse for Save Directory");

            // Check for Ctrl+Click (Linux/Windows) or Cmd+Click (Mac)
            let modifier_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);

            if browse_response.clicked() {
                if modifier_held {
                    // Ctrl/Cmd+Click: Pick a single file
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Save Files", &["sav"])
                        .pick_file()
                    {
                        if let Some(parent) = path.parent() {
                            match app.load_save_directory(parent.to_path_buf()) {
                                Ok(_) => {
                                    if let Some(filename) =
                                        path.file_name().and_then(|f| f.to_str())
                                    {
                                        app.state.selected_save = Some(filename.to_string());
                                    }
                                    app.set_status(format!("Loaded: {}", path.display()));
                                }
                                Err(e) => {
                                    let error_msg = if app.state.steam_id.is_empty() {
                                        format!(
                                            "Cannot load save file: Steam ID not configured. {}",
                                            e
                                        )
                                    } else {
                                        format!(
                                            "Cannot read save file at {}: {}",
                                            path.display(),
                                            e
                                        )
                                    };
                                    app.set_error(error_msg);
                                }
                            }
                        }
                    }
                } else {
                    // Normal click: Pick a directory
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        match app.load_save_directory(path.clone()) {
                            Ok(_) => {
                                app.set_status(format!("Loaded directory: {}", path.display()))
                            }
                            Err(e) => {
                                let error_msg = if !path.exists() {
                                    format!("Directory does not exist: {}", path.display())
                                } else if !path.is_dir() {
                                    format!("Path is not a directory: {}", path.display())
                                } else {
                                    format!("Cannot read directory {}: {}", path.display(), e)
                                };
                                app.set_error(error_msg);
                            }
                        }
                    }
                }
            }

            ui.add_space(40.0);
            ui.add_space(10.0);
            ui.add_space(30.0);

            // Steam ID configuration section
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Steam ID:").size(16.0));
                let mut steam_id = app.state.steam_id.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut steam_id)
                        .desired_width(250.0)
                        .hint_text("Enter your Steam ID"),
                );

                if response.changed() {
                    app.state.steam_id = steam_id;
                }
            });

            ui.add_space(10.0);

            if app.state.steam_id.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚠").color(egui::Color32::from_rgb(255, 180, 0)));
                    ui.label(egui::RichText::new("Steam ID required for decryption").weak());
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("✓").color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                    ui.label(egui::RichText::new("Steam ID configured").weak());
                });
            }
        } else {
            // Directory is loaded - show nothing (empty content area)
        }
    });
}
