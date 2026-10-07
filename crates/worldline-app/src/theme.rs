//! The app's look: a quiet dark theme with one accent color, so the sky
//! stays the brightest thing on screen.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, vec2};

/// The accent: selection, links and the selected body's ring.
pub const ACCENT: Color32 = Color32::from_rgb(120, 168, 255);

/// Secondary text: hints, units, counts.
pub const MUTED: Color32 = Color32::from_rgb(128, 136, 150);

/// Applies the theme to every part of the interface.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(12, 14, 20);
    visuals.window_fill = Color32::from_rgb(16, 19, 26);
    visuals.extreme_bg_color = Color32::from_rgb(8, 10, 14);
    visuals.faint_bg_color = Color32::from_rgb(17, 20, 27);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(32, 37, 48));
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.selection.bg_fill = Color32::from_rgb(34, 58, 104);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(214, 228, 255));
    visuals.hyperlink_color = ACCENT;
    visuals.weak_text_color = Some(MUTED);

    let radius = CornerRadius::same(6);
    let widgets = &mut visuals.widgets;
    // Separators and plain text.
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(30, 35, 46));
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(198, 204, 214));
    widgets.noninteractive.corner_radius = radius;
    for (state, fill, text) in [
        (&mut widgets.inactive, (24, 28, 38), (196, 202, 212)),
        (&mut widgets.hovered, (34, 40, 54), (238, 242, 248)),
        (&mut widgets.active, (44, 54, 74), (255, 255, 255)),
        (&mut widgets.open, (30, 36, 49), (230, 234, 242)),
    ] {
        let fill = Color32::from_rgb(fill.0, fill.1, fill.2);
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0, Color32::from_rgb(text.0, text.1, text.2));
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(58, 70, 94));

    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.item_spacing = vec2(8.0, 3.0);
        style.spacing.button_padding = vec2(8.0, 2.0);
        style.spacing.interact_size.y = 20.0;
        style.spacing.indent = 16.0;
        for (text_style, size) in [
            (TextStyle::Heading, 16.0),
            (TextStyle::Body, 13.5),
            (TextStyle::Button, 13.5),
            (TextStyle::Small, 11.0),
            (TextStyle::Monospace, 13.0),
        ] {
            let family = if text_style == TextStyle::Monospace {
                FontId::monospace(size)
            } else {
                FontId::proportional(size)
            };
            style.text_styles.insert(text_style, family);
        }
    });
}
