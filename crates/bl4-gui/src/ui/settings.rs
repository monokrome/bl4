use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    if app.state.selected_save.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(150.0);
            ui.label(egui::RichText::new("⚙").size(60.0).weak());
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

        let available_width = ui.available_width();
        let use_columns = available_width > 900.0;

        if use_columns {
            // Two-column grid layout for wide screens
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([50.0, 30.0])
                .striped(false)
                .show(ui, |ui| {
                    // Left column: Audio Settings
                    ui.vertical(|ui| {
                        // Volume controls
                        if let Ok(overall) = save.get("audioprefs.volume_overall") {
                            if let Some(vol) = overall.as_f64() {
                                let mut volume = vol as f32;
                                ui.label("Overall Volume");
                                ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false));
                                ui.label(format!("{}%", (volume * 100.0) as i32));
                                ui.add_space(8.0);
                            }
                        }

                        if let Ok(sfx) = save.get("audioprefs.volume_sfx") {
                            if let Some(vol) = sfx.as_f64() {
                                let mut volume = vol as f32;
                                ui.label("SFX Volume");
                                ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false));
                                ui.label(format!("{}%", (volume * 100.0) as i32));
                                ui.add_space(8.0);
                            }
                        }

                        if let Ok(music) = save.get("audioprefs.volume_music") {
                            if let Some(vol) = music.as_f64() {
                                let mut volume = vol as f32;
                                ui.label("Music Volume");
                                ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false));
                                ui.label(format!("{}%", (volume * 100.0) as i32));
                                ui.add_space(8.0);
                            }
                        }

                        if let Ok(dialog) = save.get("audioprefs.volume_dialog") {
                            if let Some(vol) = dialog.as_f64() {
                                let mut volume = vol as f32;
                                ui.label("Dialog Volume");
                                ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false));
                                ui.label(format!("{}%", (volume * 100.0) as i32));
                            }
                        }
                    });

                    // Right column: Input Settings
                    ui.vertical(|ui| {
                        // Sensitivity controls
                        if let Ok(look_h) = save.get("inputprefs.look_sensitivity_horizontal") {
                            if let Some(sens) = look_h.as_f64() {
                                let mut sensitivity = sens as f32;
                                ui.label("Look Sensitivity (Horizontal)");
                                ui.add(
                                    egui::Slider::new(&mut sensitivity, 0.0..=1.0)
                                        .show_value(false),
                                );
                                ui.label(format!("{:.2}", sensitivity));
                                ui.add_space(8.0);
                            }
                        }

                        if let Ok(look_v) = save.get("inputprefs.look_sensitivity_vertical") {
                            if let Some(sens) = look_v.as_f64() {
                                let mut sensitivity = sens as f32;
                                ui.label("Look Sensitivity (Vertical)");
                                ui.add(
                                    egui::Slider::new(&mut sensitivity, 0.0..=1.0)
                                        .show_value(false),
                                );
                                ui.label(format!("{:.2}", sensitivity));
                                ui.add_space(8.0);
                            }
                        }

                        // Toggles
                        ui.add_space(5.0);
                        if let Ok(aim_assist) = save.get("inputprefs.controller_aim_assist") {
                            if let Some(enabled) = aim_assist.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "Controller Aim Assist");
                            }
                        }

                        if let Ok(recentering) = save.get("inputprefs.controller_aim_recentering") {
                            if let Some(enabled) = recentering.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "Controller Aim Recentering");
                            }
                        }
                    });

                    ui.end_row();

                    // Second row
                    // Left column: UI Settings
                    ui.vertical(|ui| {
                        ui.add_space(15.0);

                        if let Ok(dmg_numbers) = save.get("inputprefs.display_damage_numbers") {
                            if let Some(enabled) = dmg_numbers.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "Display Damage Numbers");
                            }
                        }

                        if let Ok(rumble) = save.get("inputprefs.rumble_enabled") {
                            if let Some(enabled) = rumble.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "Controller Rumble");
                            }
                        }

                        ui.add_space(10.0);

                        if let Ok(camera_shake) = save.get("inputprefs.camera_shake_intensity") {
                            if let Some(intensity) = camera_shake.as_f64() {
                                let mut shake = intensity as f32;
                                ui.label("Camera Shake Intensity");
                                ui.add(egui::Slider::new(&mut shake, 0.0..=1.0).show_value(false));
                                ui.label(format!("{}%", (shake * 100.0) as i32));
                            }
                        }
                    });

                    // Right column: Accessibility
                    ui.vertical(|ui| {
                        ui.add_space(15.0);

                        if let Ok(colorblind) = save.get("ui.user_preferences.color_blind_mode") {
                            if let Some(mode) = colorblind.as_i64() {
                                ui.label("Colorblind Mode");
                                let mode_text = match mode {
                                    0 => "Off",
                                    1 => "Protanopia",
                                    2 => "Deuteranopia",
                                    3 => "Tritanopia",
                                    _ => "Unknown",
                                };
                                ui.label(egui::RichText::new(mode_text).strong());
                                ui.add_space(10.0);
                            }
                        }

                        if let Ok(high_contrast_hud) =
                            save.get("ui.user_preferences.high_contrast_mode.hud")
                        {
                            if let Some(enabled) = high_contrast_hud.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "High Contrast HUD");
                            }
                        }

                        if let Ok(high_contrast_xhair) =
                            save.get("ui.user_preferences.high_contrast_mode.crosshair")
                        {
                            if let Some(enabled) = high_contrast_xhair.as_bool() {
                                let mut checked = enabled;
                                ui.checkbox(&mut checked, "High Contrast Crosshair");
                            }
                        }

                        ui.add_space(10.0);

                        if let Ok(sub_size) = save.get("ui.subtitles.size") {
                            if let Some(size) = sub_size.as_f64() {
                                let mut subtitle_size = size as f32;
                                ui.label("Subtitle Size");
                                ui.add(
                                    egui::Slider::new(&mut subtitle_size, 0.5..=2.0)
                                        .show_value(false),
                                );
                                ui.label(format!("{:.1}x", subtitle_size));
                            }
                        }
                    });

                    ui.end_row();
                });
        } else {
            // Single column for narrow screens
            ui.heading("🔊 Audio");
            ui.add_space(10.0);

            if let Ok(overall) = save.get("audioprefs.volume_overall") {
                if let Some(vol) = overall.as_f64() {
                    let mut volume = vol as f32;
                    ui.label("Overall Volume");
                    ui.add(egui::Slider::new(&mut volume, 0.0..=1.0));
                    ui.add_space(8.0);
                }
            }

            // More fields in vertical layout...
            ui.label(egui::RichText::new("Additional settings available in wider window").weak());
        }

        ui.add_space(30.0);

        // Info footer
        ui.separator();
        ui.add_space(15.0);
        ui.label(egui::RichText::new("ℹ Profile settings apply to all characters").weak());
        ui.label(
            egui::RichText::new("Changes require saving and restarting the game")
                .weak()
                .italics(),
        );

        ui.add_space(20.0);
    });
}
