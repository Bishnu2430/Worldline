//! The app's look (step 8.3): a deep dark theme with one accent color, so
//! the sky stays the brightest thing on screen. Inter for text, cards for
//! grouping, soft shadows under floating windows, and quick animations.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin,
    Response, RichText, Shadow, Stroke, TextStyle, Ui, vec2,
};

/// The accent: selection, links, the primary button and the selected
/// body's ring.
pub const ACCENT: Color32 = Color32::from_rgb(110, 160, 255);

/// The accent as a fill behind white text: the primary button.
pub const ACCENT_FILL: Color32 = Color32::from_rgb(44, 96, 210);

/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(226, 231, 240);

/// Secondary text: hints, units, counts.
pub const MUTED: Color32 = Color32::from_rgb(124, 134, 152);

/// The panels' background, the darkest surface.
pub const BACKGROUND: Color32 = Color32::from_rgb(9, 11, 16);

/// Cards and floating windows, a step up.
pub const SURFACE: Color32 = Color32::from_rgb(15, 18, 26);

/// Buttons and fields, a step further.
pub const RAISED: Color32 = Color32::from_rgb(23, 28, 39);

/// Hairlines between things.
pub const BORDER: Color32 = Color32::from_rgb(33, 40, 55);

/// A model's verdict on a body's situation: within range, approximate,
/// beyond it.
pub const WITHIN: Color32 = Color32::from_rgb(110, 205, 140);
pub const APPROXIMATE: Color32 = Color32::from_rgb(240, 180, 80);
pub const BEYOND: Color32 = Color32::from_rgb(240, 110, 100);

/// The font family for headings, buttons that matter and the brand: Inter
/// SemiBold.
pub fn strong_family() -> FontFamily {
    FontFamily::Name("strong".into())
}

/// Inter SemiBold at `size` points.
pub fn strong(size: f32) -> FontId {
    FontId::new(size, strong_family())
}

/// How long the interface's own animations take (opening sections,
/// highlights fading in), in seconds: quick enough to feel immediate.
pub const ANIMATION: f32 = 0.12;

/// Inter for text, egui's own fonts after it for anything Inter lacks
/// (arrows, symbols, the icons on buttons).
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "Inter",
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            "Inter SemiBold",
            &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("egui's proportional family")
        .insert(0, "Inter".to_owned());
    let mut strong = vec!["Inter SemiBold".to_owned()];
    strong.extend(fallbacks);
    fonts.families.insert(strong_family(), strong);
    // The monospace clock falls back to Inter for anything Hack lacks.
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.push("Inter".to_owned());
    }
    fonts
}

/// Applies the theme to every part of the interface.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(6, 8, 12);
    visuals.faint_bg_color = Color32::from_rgb(14, 17, 24);
    visuals.code_bg_color = RAISED;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_shadow = Shadow {
        offset: [0, 10],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(150),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(130),
    };
    visuals.selection.bg_fill = Color32::from_rgb(36, 64, 120);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(214, 228, 255));
    visuals.hyperlink_color = ACCENT;
    visuals.weak_text_color = Some(MUTED);
    visuals.collapsing_header_frame = false;
    visuals.slider_trailing_fill = true;
    visuals.striped = false;

    let radius = CornerRadius::same(7);
    let widgets = &mut visuals.widgets;
    // Separators and plain text.
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 207, 218));
    widgets.noninteractive.bg_fill = SURFACE;
    widgets.noninteractive.weak_bg_fill = SURFACE;
    widgets.noninteractive.corner_radius = radius;
    for (state, fill, text) in [
        (
            &mut widgets.inactive,
            RAISED,
            Color32::from_rgb(206, 212, 222),
        ),
        (
            &mut widgets.hovered,
            Color32::from_rgb(32, 39, 54),
            Color32::from_rgb(242, 245, 250),
        ),
        (
            &mut widgets.active,
            Color32::from_rgb(42, 52, 74),
            Color32::WHITE,
        ),
        (
            &mut widgets.open,
            Color32::from_rgb(30, 37, 51),
            Color32::from_rgb(232, 236, 244),
        ),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0, text);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(58, 72, 98));
    widgets.active.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.6));

    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.animation_time = ANIMATION;
        style.spacing.item_spacing = vec2(8.0, 5.0);
        style.spacing.button_padding = vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.indent = 16.0;
        style.spacing.window_margin = Margin::same(12);
        style.spacing.menu_margin = Margin::same(8);
        style.spacing.combo_width = 160.0;
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        for (text_style, font) in [
            (TextStyle::Heading, strong(15.0)),
            (TextStyle::Body, FontId::proportional(13.0)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ] {
            style.text_styles.insert(text_style, font);
        }
    });
}

/// A card: a raised, rounded block grouping related things in a panel.
pub fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui)
        })
        .inner
}

/// A card's title: small capitals-like, muted, with something beside it.
pub fn card_title(ui: &mut Ui, title: &str, beside: Option<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(strong(12.5)).color(TEXT));
        if let Some(beside) = beside {
            ui.label(RichText::new(beside).small().color(MUTED));
        }
    });
}

/// A small rounded tag in `color`, its text on a faint fill of it:
/// statuses, warnings, "live".
pub fn chip(ui: &mut Ui, text: &str, color: Color32) -> Response {
    Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(text).small().color(color)).truncate())
        })
        .inner
}

/// The primary button: the one action that matters most where it is,
/// filled with the accent.
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).font(strong(13.0)).color(Color32::WHITE))
        .fill(ACCENT_FILL)
        .min_size(vec2(0.0, 28.0))
}

/// A toolbar button: borderless until hovered, highlighted when its
/// window is open.
pub fn tool_button(text: impl Into<String>, open: bool) -> egui::Button<'static> {
    let text = RichText::new(text).color(if open { Color32::WHITE } else { TEXT });
    let button = egui::Button::new(text).min_size(vec2(0.0, 28.0));
    if open {
        button
            .fill(Color32::from_rgb(34, 50, 84))
            .stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.55)))
    } else {
        button.frame_when_inactive(false)
    }
}

/// The window's icon: an orbit around a star on a dark disk, drawn here
/// rather than shipped as an image.
pub fn icon() -> egui::IconData {
    const SIZE: usize = 64;
    // Four by four samples a pixel, for smooth edges.
    const SAMPLES: usize = 4;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut color = [0.0f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32 - center;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32 - center;
                    let sample = icon_sample(px, py);
                    for (c, s) in color.iter_mut().zip(sample) {
                        *c += s / (SAMPLES * SAMPLES) as f32;
                    }
                }
            }
            let at = (y * SIZE + x) * 4;
            // Unpremultiply: the icon's alpha is separate.
            let alpha = color[3];
            for k in 0..3 {
                let value = if alpha > 0.0 { color[k] / alpha } else { 0.0 };
                rgba[at + k] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            rgba[at + 3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

/// One sample of the icon at (x, y) from its center, in pixels of a 64-pixel
/// icon: premultiplied red, green, blue and alpha.
fn icon_sample(x: f32, y: f32) -> [f32; 4] {
    let r = x.hypot(y);
    if r > 31.0 {
        return [0.0; 4];
    }
    let mut color = [0.06, 0.08, 0.12];
    // An orbit, tilted: an ellipse 24 by 10 pixels, turned 25°.
    let (sin, cos) = (-25f32).to_radians().sin_cos();
    let (u, v) = (x * cos - y * sin, x * sin + y * cos);
    let ellipse = ((u / 24.0).powi(2) + (v / 10.0).powi(2)).sqrt();
    let ring = (1.0 - ((ellipse - 1.0) * 24.0 / 1.6).abs()).clamp(0.0, 1.0);
    let accent = [0.43, 0.63, 1.0];
    for k in 0..3 {
        color[k] += (accent[k] - color[k]) * ring;
    }
    // A planet on the orbit, and the star at its center.
    let planet = (1.0 - ((u - 24.0 * 0.6).hypot(v + 10.0 * 0.8) - 3.2)).clamp(0.0, 1.0);
    for k in 0..3 {
        color[k] += ([0.85, 0.92, 1.0][k] - color[k]) * planet;
    }
    let star = (1.0 - (r - 6.0)).clamp(0.0, 1.0);
    let glow = (1.0 - r / 14.0).clamp(0.0, 1.0).powi(2) * 0.5;
    for k in 0..3 {
        color[k] += ([1.0, 0.86, 0.5][k] - color[k]) * star.max(glow);
    }
    [color[0], color[1], color[2], 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context with the theme applied and its fonts loaded.
    fn themed() -> egui::Context {
        let ctx = egui::Context::default();
        apply(&ctx);
        // Fonts load at the start of the first frame. Its textures go to
        // no GPU here, so they're let go of.
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        ctx
    }

    #[test]
    fn every_character_the_interface_writes_has_a_glyph() {
        // Every character in the app's string literals, in Inter or a
        // fallback: units, Greek, superscripts, arrows, the toolbar's
        // icons. One without would show as an empty box.
        let ctx = themed();
        let written = written_characters();
        assert!(written.len() > 30, "found {}", written.len());
        ctx.fonts_mut(|fonts| {
            for (c, at) in &written {
                for font in [FontId::proportional(13.0), strong(13.0)] {
                    assert!(fonts.has_glyph(&font, *c), "no glyph for {c:?} ({at})");
                }
            }
        });
    }

    /// Every character outside ASCII in the app's string literals, with a
    /// file and line it's written at. Comments are left out.
    fn written_characters() -> std::collections::BTreeMap<char, String> {
        let mut found = std::collections::BTreeMap::new();
        let mut folders = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).expect("the source folder") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    folders.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("a source file");
                for (number, line) in source.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    let mut inside = false;
                    let mut escaped = false;
                    for c in line.chars() {
                        match c {
                            _ if escaped => escaped = false,
                            '\\' if inside => escaped = true,
                            '"' => inside = !inside,
                            c if inside && !c.is_ascii() => {
                                found.entry(c).or_insert_with(|| {
                                    format!("{}:{}", path.display(), number + 1)
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        found
    }

    #[test]
    fn the_icon_is_a_disk_on_a_clear_background() {
        let icon = icon();
        assert_eq!((icon.width, icon.height), (64, 64));
        let alpha = |x: usize, y: usize| icon.rgba[(y * 64 + x) * 4 + 3];
        // Clear in the corners, opaque inside the disk.
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(32, 50), 255);
        // The star at the center is bright and warm.
        let at = (32 * 64 + 32) * 4;
        assert!(icon.rgba[at] > 240 && icon.rgba[at + 2] < icon.rgba[at]);
    }
}
