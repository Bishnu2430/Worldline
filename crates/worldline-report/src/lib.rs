//! Worldline's validation report (step 8.2): the figures the validation
//! tests record, for the plots in `docs/validation/`.
//!
//! A test describes what it checked as a figure and saves it. With
//! `WORLDLINE_REPORT` set, saving writes the figure's data as JSON to
//! `docs/validation/data/<id>.json`, with the test's name and file, which
//! [`here!`] records. Otherwise it does nothing, so ordinary test runs
//! change no files. `tools/validation_report.py` draws the plots from the
//! data.
//!
//! Two kinds of figure:
//! - [`Margins`]: for each check, how far the result is from what's
//!   expected, as a share of the limit it must stay within. Below 1 passes.
//! - [`Lines`]: curves, simulated and expected, against a common axis.

use std::fmt::Write;
use std::path::PathBuf;

/// Where a figure was saved from: the test function's path and its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Here {
    /// The test function's path, as `validation_mercury::mercury_perihelion…`.
    pub function: &'static str,
    /// The file, from the workspace's root.
    pub file: &'static str,
}

/// The test function and file it's written in, for [`Margins::save`] and
/// [`Lines::save`].
#[macro_export]
macro_rules! here {
    () => {{
        fn f() {}
        fn name_of<T>(_: T) -> &'static str {
            ::std::any::type_name::<T>()
        }
        let name = name_of(f);
        $crate::Here {
            function: &name[..name.len() - 3],
            file: file!(),
        }
    }};
}

/// Whether figures are written: when `WORLDLINE_REPORT` is set.
pub fn recording() -> bool {
    std::env::var_os("WORLDLINE_REPORT").is_some()
}

/// The folder the figures' data goes in.
pub fn data_folder() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/validation/data"
    ))
}

/// One check of a [`Margins`] figure.
#[derive(Debug, Clone, PartialEq)]
struct Check {
    label: String,
    off: f64,
    limit: f64,
    shown: String,
}

/// For each check, how far its result is from what's expected, against the
/// limit it must stay within.
#[derive(Debug, Clone, PartialEq)]
pub struct Margins {
    id: String,
    title: String,
    checks: Vec<Check>,
}

/// A figure of checks, with an id (the plot's file name) and a title.
pub fn margins(id: &str, title: &str) -> Margins {
    Margins {
        id: id.to_string(),
        title: title.to_string(),
        checks: Vec::new(),
    }
}

impl Margins {
    /// A check: how far off the result is, `off`, against the `limit` it
    /// must stay within (the same units, both positive), and the result as
    /// text, with what it's compared with.
    pub fn check(mut self, label: &str, off: f64, limit: f64, shown: impl Into<String>) -> Self {
        self.checks.push(Check {
            label: label.to_string(),
            off: off.abs(),
            limit: limit.abs(),
            shown: shown.into(),
        });
        self
    }

    /// Saves the figure, if figures are being recorded.
    pub fn save(self, at: Here) {
        let mut checks = String::new();
        for (k, c) in self.checks.iter().enumerate() {
            let _ = write!(
                checks,
                "{}\n    {{\"label\": {}, \"off\": {}, \"limit\": {}, \"shown\": {}}}",
                if k > 0 { "," } else { "" },
                quoted(&c.label),
                number(c.off),
                number(c.limit),
                quoted(&c.shown)
            );
        }
        write_figure(
            &self.id,
            at,
            &format!(
                "\"kind\": \"margins\",\n  \"title\": {},\n  \"checks\": [{checks}\n  ]",
                quoted(&self.title)
            ),
        );
    }
}

/// One curve of a [`Lines`] figure.
#[derive(Debug, Clone, PartialEq)]
struct Series {
    name: String,
    points: Vec<(f64, f64)>,
    expected: bool,
    dots: bool,
}

/// Curves against a common axis: what was simulated or measured, and what's
/// expected.
#[derive(Debug, Clone, PartialEq)]
pub struct Lines {
    id: String,
    title: String,
    x: String,
    y: String,
    log_x: bool,
    log_y: bool,
    series: Vec<Series>,
    note: String,
}

/// A figure of curves, with an id, a title and its axes' labels.
pub fn lines(id: &str, title: &str, x: &str, y: &str) -> Lines {
    Lines {
        id: id.to_string(),
        title: title.to_string(),
        x: x.to_string(),
        y: y.to_string(),
        log_x: false,
        log_y: false,
        series: Vec::new(),
        note: String::new(),
    }
}

/// At most this many points a curve: enough to draw it smoothly.
const MOST_POINTS: usize = 2000;

impl Lines {
    fn add(
        mut self,
        name: &str,
        points: impl IntoIterator<Item = (f64, f64)>,
        expected: bool,
        dots: bool,
    ) -> Self {
        let all: Vec<(f64, f64)> = points
            .into_iter()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        let every = all.len().div_ceil(MOST_POINTS).max(1);
        let mut points: Vec<(f64, f64)> = all.iter().copied().step_by(every).collect();
        if let (Some(&last), Some(&kept)) = (all.last(), points.last())
            && last != kept
        {
            points.push(last);
        }
        self.series.push(Series {
            name: name.to_string(),
            points,
            expected,
            dots,
        });
        self
    }

    /// A curve of what was simulated or measured.
    pub fn series(self, name: &str, points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        self.add(name, points, false, false)
    }

    /// Points, not joined: samples, measurements.
    pub fn dots(self, name: &str, points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        self.add(name, points, false, true)
    }

    /// A dashed curve of what's expected.
    pub fn expected(self, name: &str, points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        self.add(name, points, true, false)
    }

    /// The x axis logarithmic.
    pub fn log_x(mut self) -> Self {
        self.log_x = true;
        self
    }

    /// The y axis logarithmic.
    pub fn log_y(mut self) -> Self {
        self.log_y = true;
        self
    }

    /// A line under the plot, saying what it shows.
    pub fn note(mut self, note: &str) -> Self {
        self.note = note.to_string();
        self
    }

    /// Saves the figure, if figures are being recorded.
    pub fn save(self, at: Here) {
        let mut series = String::new();
        for (k, s) in self.series.iter().enumerate() {
            let points: Vec<String> = s
                .points
                .iter()
                .map(|(x, y)| format!("[{}, {}]", number(*x), number(*y)))
                .collect();
            let _ = write!(
                series,
                "{}\n    {{\"name\": {}, \"expected\": {}, \"dots\": {}, \"points\": [{}]}}",
                if k > 0 { "," } else { "" },
                quoted(&s.name),
                s.expected,
                s.dots,
                points.join(", ")
            );
        }
        write_figure(
            &self.id,
            at,
            &format!(
                "\"kind\": \"lines\",\n  \"title\": {},\n  \"x\": {},\n  \"y\": {},\n  \
                 \"log_x\": {},\n  \"log_y\": {},\n  \"note\": {},\n  \"series\": [{series}\n  ]",
                quoted(&self.title),
                quoted(&self.x),
                quoted(&self.y),
                self.log_x,
                self.log_y,
                quoted(&self.note)
            ),
        );
    }
}

/// A number for JSON: shortest exact form; not-a-number and infinities as
/// null.
fn number(x: f64) -> String {
    if x.is_finite() {
        format!("{x:?}")
    } else {
        "null".to_string()
    }
}

/// A string for JSON.
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes figure `id`'s data, with where it came from.
fn write_figure(id: &str, at: Here, body: &str) {
    if !recording() {
        return;
    }
    let folder = data_folder();
    let _ = std::fs::create_dir_all(&folder);
    let file = at.file.replace('\\', "/");
    let text = format!(
        "{{\n  \"id\": {},\n  \"test\": {},\n  \"file\": {},\n  {body}\n}}\n",
        quoted(id),
        quoted(at.function),
        quoted(&file)
    );
    std::fs::write(folder.join(format!("{id}.json")), text)
        .unwrap_or_else(|e| panic!("writing figure {id}: {e}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn here_names_the_function_and_file() {
        let at = here!();
        assert!(
            at.function
                .ends_with("tests::here_names_the_function_and_file")
        );
        assert!(at.file.ends_with("lib.rs"));
    }

    #[test]
    fn text_and_numbers_are_valid_json() {
        assert_eq!(quoted("a \"b\"\\c\n"), "\"a \\\"b\\\"\\\\c\\n\"");
        assert_eq!(number(0.1), "0.1");
        assert_eq!(number(1e-300), "1e-300");
        assert_eq!(number(f64::NAN), "null");
    }

    #[test]
    fn a_long_curve_keeps_its_ends_and_at_most_its_limit() {
        let figure = lines("x", "x", "x", "y").series("s", (0..10_001).map(|k| (k as f64, 0.0)));
        let points = &figure.series[0].points;
        assert!(points.len() <= MOST_POINTS + 1);
        assert_eq!(points.first(), Some(&(0.0, 0.0)));
        assert_eq!(points.last(), Some(&(10_000.0, 0.0)));
    }
}
