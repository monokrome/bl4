mod app;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 700.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "OnTheBorder - BL4 Editor",
        options,
        Box::new(|cc| {
            // Create a cozy, comfortable style
            let mut style = (*cc.egui_ctx.style()).clone();

            // Text sizes for different areas
            style.text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::proportional(16.8), // Content area: 25% smaller than original 22.4
            );
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(16.8));
            style.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::proportional(27.3), // 25% smaller than 36.4
            );
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::monospace(14.7), // 25% smaller than 19.6
            );
            // Smaller text for status/secondary UI
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(15.7));

            // More generous spacing for comfort (40% larger)
            style.spacing.item_spacing = egui::vec2(16.8, 19.6);
            style.spacing.button_padding = egui::vec2(16.8, 8.4);
            style.spacing.indent = 28.0;
            style.spacing.window_margin = egui::Margin::same(22.4);

            // Larger interact sizes for forms/controls
            style.spacing.interact_size.y = 28.0; // Height of buttons, text edits, etc

            // Modern clean look - 2025 not 1998
            let visuals = &mut style.visuals;

            // Base background colors - using subtle variation for hierarchy
            visuals.window_fill = egui::Color32::from_rgb(32, 34, 37);
            visuals.panel_fill = egui::Color32::from_rgb(32, 34, 37);

            // Extreme background (for popups, etc) - slightly lighter
            visuals.extreme_bg_color = egui::Color32::from_rgb(40, 42, 46);

            // Faint background for cards/sections
            visuals.faint_bg_color = egui::Color32::from_rgb(40, 42, 46);

            // Widget colors - ZERO borders/outlines on hover
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(45, 48, 52);
            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(40, 42, 46);
            visuals.widgets.noninteractive.bg_stroke.width = 0.0; // NO BORDER
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(220, 220, 225);

            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(50, 53, 58);
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(45, 48, 52);
            visuals.widgets.inactive.bg_stroke.width = 0.0; // NO BORDER
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(220, 220, 225);

            // Hover state - subtle color shift, NO OUTLINE
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(58, 62, 68);
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(52, 56, 62);
            visuals.widgets.hovered.bg_stroke.width = 0.0; // NO OUTLINE/BORDER
            visuals.widgets.hovered.fg_stroke.color = egui::Color32::from_rgb(240, 240, 245);

            // Active/clicked state - more muted blue
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 75, 95);
            visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(55, 68, 88);
            visuals.widgets.active.bg_stroke.width = 0.0; // NO BORDER
            visuals.widgets.active.fg_stroke.color = egui::Color32::from_rgb(240, 240, 245);

            // Open menu/combobox
            visuals.widgets.open.bg_fill = egui::Color32::from_rgb(58, 62, 68);
            visuals.widgets.open.weak_bg_fill = egui::Color32::from_rgb(52, 56, 62);
            visuals.widgets.open.bg_stroke.width = 0.0; // NO BORDER
            visuals.widgets.open.fg_stroke.color = egui::Color32::from_rgb(240, 240, 245);

            // Selection color (for text selection, etc) - more subtle
            visuals.selection.bg_fill = egui::Color32::from_rgb(60, 75, 95);
            visuals.selection.stroke.width = 0.0; // NO BORDER

            // Hyperlink colors
            visuals.hyperlink_color = egui::Color32::from_rgb(120, 170, 255);

            // Remove window stroke/border
            visuals.window_stroke.width = 0.0;
            visuals.window_shadow = egui::epaint::Shadow::NONE; // No shadow

            // Rounded corners - subtle but modern
            visuals.widgets.noninteractive.rounding = egui::Rounding::same(2.5);
            visuals.widgets.inactive.rounding = egui::Rounding::same(2.5);
            visuals.widgets.hovered.rounding = egui::Rounding::same(2.5);
            visuals.widgets.active.rounding = egui::Rounding::same(2.5);
            visuals.widgets.open.rounding = egui::Rounding::same(2.5);
            visuals.window_rounding = egui::Rounding::same(2.5);
            visuals.menu_rounding = egui::Rounding::same(2.5);

            cc.egui_ctx.set_style(style);

            Ok(Box::new(app::BL4App::new(cc)))
        }),
    )
}
