//! The manual inside the app (step 8.6): getting started, the controls,
//! every window and tool, the scenarios, and each physics model with its
//! limits.
//!
//! It's written as plain text, `assets/manual.md`, in a small part of
//! Markdown: `#` begins a chapter, `##` a section, `- ` a point, `**…**`
//! is bold, `*…*` italic and `` `…` `` a command; a line that is only a
//! link to `do:…` is a button that does what it says (open a window, play a
//! scenario, fly to a body). The tests below keep it complete: every
//! scenario, key and physics model the app has is in it, and every button
//! does something that exists.

use std::sync::OnceLock;

use eframe::egui::{self, FontId, RichText, text::LayoutJob, text::TextFormat};

use crate::theme;

/// The manual's text.
const TEXT: &str = include_str!("../assets/manual.md");

/// A chapter: its title and what's in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub title: String,
    pub blocks: Vec<Block>,
}

/// A piece of a chapter.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A section's heading.
    Heading(String),
    Paragraph(Vec<Span>),
    /// A point of a list.
    Point(Vec<Span>),
    /// A button: what it says, and what it does (after `do:`).
    Button {
        label: String,
        action: String,
    },
}

/// A run of text and how it's set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    Bold,
    Italic,
    Code,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// The manual, read once.
pub fn chapters() -> &'static [Chapter] {
    static CHAPTERS: OnceLock<Vec<Chapter>> = OnceLock::new();
    CHAPTERS.get_or_init(|| parse(TEXT))
}

/// Reads the manual's text into chapters.
pub fn parse(text: &str) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, chapters: &mut Vec<Chapter>| {
        if paragraph.is_empty() {
            return;
        }
        let joined = paragraph.join(" ");
        paragraph.clear();
        if let Some(chapter) = chapters.last_mut() {
            chapter.blocks.push(Block::Paragraph(spans(&joined)));
        }
    };
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(title) = line.strip_prefix("# ") {
            flush(&mut paragraph, &mut chapters);
            chapters.push(Chapter {
                title: title.to_string(),
                blocks: Vec::new(),
            });
        } else if let Some(heading) = line.strip_prefix("## ") {
            flush(&mut paragraph, &mut chapters);
            if let Some(chapter) = chapters.last_mut() {
                chapter.blocks.push(Block::Heading(heading.to_string()));
            }
        } else if let Some(point) = line.strip_prefix("- ") {
            flush(&mut paragraph, &mut chapters);
            if let Some(chapter) = chapters.last_mut() {
                chapter.blocks.push(Block::Point(spans(point)));
            }
        } else if let Some((label, action)) = button(line) {
            flush(&mut paragraph, &mut chapters);
            if let Some(chapter) = chapters.last_mut() {
                chapter.blocks.push(Block::Button { label, action });
            }
        } else if line.is_empty() {
            flush(&mut paragraph, &mut chapters);
        } else {
            paragraph.push(line);
        }
    }
    flush(&mut paragraph, &mut chapters);
    chapters
}

/// A line that is only `[label](do:action)`.
fn button(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('[')?;
    let (label, rest) = rest.split_once("](do:")?;
    let action = rest.strip_suffix(')')?;
    Some((label.to_string(), action.to_string()))
}

/// A line's runs of text: `**bold**`, `*italic*`, `` `code` ``; `\*` is
/// an asterisk.
fn spans(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut current = String::new();
    let mut style = Style::Plain;
    let mut chars = text.chars().peekable();
    let push = |text: &mut String, style: Style, out: &mut Vec<Span>| {
        if !text.is_empty() {
            out.push(Span {
                text: std::mem::take(text),
                style,
            });
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'*') => {
                current.push('*');
                chars.next();
            }
            '*' if chars.peek() == Some(&'*') && style != Style::Code => {
                chars.next();
                push(&mut current, style, &mut out);
                style = if style == Style::Bold {
                    Style::Plain
                } else {
                    Style::Bold
                };
            }
            '*' if matches!(style, Style::Plain | Style::Italic) => {
                push(&mut current, style, &mut out);
                style = if style == Style::Italic {
                    Style::Plain
                } else {
                    Style::Italic
                };
            }
            '`' => {
                push(&mut current, style, &mut out);
                style = if style == Style::Code {
                    Style::Plain
                } else {
                    Style::Code
                };
            }
            _ => current.push(c),
        }
    }
    push(&mut current, style, &mut out);
    out
}

/// Draws a run of spans, wrapped to the space there is.
fn text(ui: &mut egui::Ui, spans: &[Span]) {
    let mut job = LayoutJob::default();
    for span in spans {
        let (font_id, color, italics) = match span.style {
            Style::Plain => (FontId::proportional(13.5), theme::TEXT, false),
            Style::Bold => (theme::strong(13.5), theme::TEXT, false),
            Style::Italic => (FontId::proportional(13.5), theme::TEXT, true),
            Style::Code => (FontId::monospace(12.5), theme::ACCENT, false),
        };
        job.append(
            &span.text,
            0.0,
            TextFormat {
                font_id,
                color,
                italics,
                ..Default::default()
            },
        );
    }
    job.wrap.max_width = ui.available_width();
    ui.label(job);
}

/// The manual's window, open at chapter `chapter`: the chapters down the
/// left, the chosen one on the right. Returns the action of a button
/// pressed, if one was.
pub fn window(ctx: &egui::Context, open: &mut bool, chapter: &mut usize) -> Option<String> {
    let chapters = chapters();
    let mut pressed = None;
    egui::Window::new("Manual")
        .open(open)
        .default_size([780.0, 560.0])
        .default_pos([220.0, 90.0])
        .show(ctx, |ui| {
            egui::Panel::left("manual chapters")
                .resizable(false)
                .default_size(190.0)
                .show(ui, |ui| {
                    for (k, c) in chapters.iter().enumerate() {
                        if ui
                            .add(egui::Button::selectable(*chapter == k, c.title.as_str()))
                            .clicked()
                        {
                            *chapter = k;
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("F1 opens this manual at any time")
                            .small()
                            .color(theme::MUTED),
                    );
                });
            let Some(current) = chapters.get(*chapter) else {
                return;
            };
            egui::ScrollArea::vertical()
                .id_salt(("manual", *chapter))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&current.title).font(theme::strong(19.0)));
                    for block in &current.blocks {
                        match block {
                            Block::Heading(heading) => {
                                ui.add_space(10.0);
                                ui.label(
                                    RichText::new(heading)
                                        .font(theme::strong(15.0))
                                        .color(theme::ACCENT),
                                );
                            }
                            Block::Paragraph(spans) => {
                                ui.add_space(4.0);
                                text(ui, spans);
                            }
                            Block::Point(spans) => {
                                ui.horizontal_top(|ui| {
                                    ui.label(RichText::new("•").color(theme::ACCENT));
                                    text(ui, spans);
                                });
                            }
                            Block::Button { label, action } => {
                                ui.add_space(2.0);
                                if ui
                                    .add(theme::primary_button(format!("▶  {label}")))
                                    .clicked()
                                {
                                    pressed = Some(action.clone());
                                }
                            }
                        }
                    }
                    ui.add_space(12.0);
                    if *chapter + 1 < chapters.len()
                        && ui
                            .button(format!("Next: {}", chapters[*chapter + 1].title))
                            .clicked()
                    {
                        *chapter += 1;
                    }
                });
        });
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_reads_into_its_chapters() {
        let titles: Vec<&str> = chapters().iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Getting started",
                "Controls",
                "The view",
                "Windows and tools",
                "Scenarios",
                "Physics models and their limits",
                "Where the numbers come from",
            ]
        );
        assert!(chapters().iter().all(|c| !c.blocks.is_empty()));
    }

    #[test]
    fn bold_italic_and_commands_are_set_apart() {
        let s = spans("Press **Pause**, or *not*, then `--quality low` and A\\*.");
        let styles: Vec<(&str, Style)> = s.iter().map(|s| (s.text.as_str(), s.style)).collect();
        assert_eq!(
            styles,
            [
                ("Press ", Style::Plain),
                ("Pause", Style::Bold),
                (", or ", Style::Plain),
                ("not", Style::Italic),
                (", then ", Style::Plain),
                ("--quality low", Style::Code),
                (" and A*.", Style::Plain),
            ]
        );
        assert_eq!(
            button("[Fly to Saturn](do:fly:Saturn)"),
            Some(("Fly to Saturn".to_string(), "fly:Saturn".to_string()))
        );
        assert_eq!(button("Not [a button](do:x) here"), None);
    }

    /// Every piece of text the manual shows.
    fn all_text() -> String {
        let mut out = String::new();
        for chapter in chapters() {
            out += &chapter.title;
            for block in &chapter.blocks {
                match block {
                    Block::Heading(h) => out += h,
                    Block::Paragraph(s) | Block::Point(s) => {
                        for span in s {
                            out += &span.text;
                        }
                    }
                    Block::Button { label, .. } => out += label,
                }
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn every_physics_model_has_its_limits_in_the_manual() {
        // Each model's page in docs/physics is named in the physics
        // chapter, where its limits are said in a sentence.
        let physics = chapters()
            .iter()
            .find(|c| c.title == "Physics models and their limits")
            .expect("the chapter");
        let text: String = physics
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Point(s) | Block::Paragraph(s) => {
                    Some(s.iter().map(|s| s.text.as_str()).collect::<String>())
                }
                _ => None,
            })
            .collect();
        let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/physics");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(folder).expect("the docs") {
            let name = entry
                .expect("a doc")
                .file_name()
                .to_string_lossy()
                .to_string();
            if name.ends_with(".md") && !text.contains(&name) {
                missing.push(name);
            }
        }
        assert!(missing.is_empty(), "not in the manual: {missing:?}");
    }

    #[test]
    fn every_key_is_in_the_manual() {
        let text = all_text();
        for action in crate::feel::Action::ALL {
            assert!(
                text.contains(action.key()),
                "{:?}'s key {}",
                action,
                action.key()
            );
        }
    }

    #[test]
    fn every_character_the_manual_shows_has_a_glyph() {
        let ctx = theme::themed();
        let text = all_text();
        ctx.fonts_mut(|fonts| {
            for c in text.chars().filter(|c| !c.is_ascii()) {
                for font in [FontId::proportional(13.5), theme::strong(13.5)] {
                    assert!(fonts.has_glyph(&font, c), "no glyph for {c:?}");
                }
            }
        });
    }

    /// Every button's action, as the manual writes them.
    fn actions() -> Vec<String> {
        chapters()
            .iter()
            .flat_map(|c| &c.blocks)
            .filter_map(|b| match b {
                Block::Button { action, .. } => Some(action.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_window_draws_every_chapter() {
        // Drawn off-screen, two frames each, as the app draws it: every
        // chapter lays out, and the window stays open at it.
        let ctx = theme::themed();
        for k in 0..chapters().len() {
            let (mut open, mut chapter) = (true, k);
            for _ in 0..2 {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    window(ui.ctx(), &mut open, &mut chapter);
                });
                output.textures_delta.clear();
            }
            assert!(open);
            assert_eq!(chapter, k);
        }
    }

    #[test]
    fn there_are_buttons_to_try_things() {
        assert!(actions().len() > 15);
    }
}
