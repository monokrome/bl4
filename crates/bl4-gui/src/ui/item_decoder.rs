use crate::app::BL4App;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, app: &mut BL4App) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        ui.label(egui::RichText::new("Item Serial Decoder").size(20.0));
        ui.add_space(10.0);
        ui.label("Paste an item serial code (starts with @Ug) to decode its properties:");
        ui.add_space(15.0);

        // Serial input
        ui.horizontal(|ui| {
            ui.label("Serial Code:");
            // Create a persistent string for the input if it doesn't exist
            let serial_input = ui.data_mut(|d| {
                d.get_temp_mut_or_default::<String>(egui::Id::new("item_serial_input"))
                    .clone()
            });

            let mut input_buffer = serial_input;
            let response = ui.add_sized(
                [600.0, 24.0],
                egui::TextEdit::singleline(&mut input_buffer)
                    .hint_text("@Uga`wSaA`L54ppc~ZK@8c7Ahy/90C"),
            );

            // Save the input back
            ui.data_mut(|d| {
                d.insert_temp(egui::Id::new("item_serial_input"), input_buffer.clone());
            });

            if ui.button("Decode").clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                // Try to decode
                match bl4::ItemSerial::decode(&input_buffer) {
                    Ok(item) => {
                        // Store decoded item
                        ui.data_mut(|d| {
                            d.insert_temp(egui::Id::new("decoded_item"), item);
                            d.insert_temp(egui::Id::new("decode_error"), None::<String>);
                        });
                        app.set_status("Item decoded successfully".to_string());
                    }
                    Err(e) => {
                        ui.data_mut(|d| {
                            d.insert_temp(
                                egui::Id::new("decode_error"),
                                Some(format!("Decode error: {}", e)),
                            );
                            d.remove::<bl4::ItemSerial>(egui::Id::new("decoded_item"));
                        });
                        app.set_error(format!("Failed to decode: {}", e));
                    }
                }
            }

            if ui.button("Clear").clicked() {
                ui.data_mut(|d| {
                    d.insert_temp(egui::Id::new("item_serial_input"), String::new());
                    d.remove::<bl4::ItemSerial>(egui::Id::new("decoded_item"));
                    d.remove::<Option<String>>(egui::Id::new("decode_error"));
                });
            }
        });

        ui.add_space(20.0);

        // Show decode error if any
        let decode_error = ui.data(|d| {
            d.get_temp::<Option<String>>(egui::Id::new("decode_error"))
                .flatten()
        });

        if let Some(error) = decode_error {
            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), error);
            ui.add_space(15.0);
        }

        // Show decoded item if available
        let decoded_item =
            ui.data(|d| d.get_temp::<bl4::ItemSerial>(egui::Id::new("decoded_item")));

        if let Some(item) = decoded_item {
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());

                ui.label(
                    egui::RichText::new("Decoded Item Information")
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(10.0);

                // Display hex dump
                ui.label(egui::RichText::new("Hex Dump:").strong());
                ui.add_space(5.0);

                let hex_dump = item.hex_dump();
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                        ui.label(&hex_dump);
                    });

                ui.add_space(15.0);

                // Display detailed dump
                ui.label(egui::RichText::new("Detailed Information:").strong());
                ui.add_space(5.0);

                let detailed_dump = item.detailed_dump();
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                        ui.label(&detailed_dump);
                    });
            });

            ui.add_space(20.0);
        } else {
            // Show example
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());

                ui.label(egui::RichText::new("ℹ️ How to use").weak());
                ui.add_space(5.0);

                ui.label("1. Find an item serial in your save file or copy from in-game");
                ui.label("2. Paste it into the Serial Code field above");
                ui.label("3. Click Decode or press Enter");
                ui.add_space(10.0);

                ui.label("Example serial codes:");
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                ui.label("  @Uga`wSaA`L54ppc~ZK@8c7Ahy/90C");
                ui.label("  @UgaDbwSaA`54ppc~ZK@8c7Ahy/90C");
            });
        }

        ui.add_space(20.0);
    });
}
