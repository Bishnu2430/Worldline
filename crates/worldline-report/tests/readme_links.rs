//! Step 8.2's check: every claim in the README's "Validated against
//! reality" table links to the test that checks it and to a plot of what
//! that test measured.
//!
//! For each row: at least one test link and one plot link. Each plot must
//! exist in `docs/validation/`, with the data it was drawn from, and that
//! data must name a test function that exists in a file the row links.
//! Every figure's data must have its plot drawn.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A string field's value in a figure's JSON, as the report writes it.
fn field(json: &str, name: &str) -> String {
    let key = format!("\"{name}\": \"");
    let start = json.find(&key).unwrap_or_else(|| panic!("no {name}")) + key.len();
    let end = start + json[start..].find('"').expect("a closing quote");
    json[start..end].to_string()
}

/// Every link target in a line of markdown.
fn links(line: &str) -> Vec<&str> {
    line.split("](")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .collect()
}

/// The rows of the table under "## Validated against reality", without
/// its header and separator.
fn claims(readme: &str) -> Vec<&str> {
    readme
        .lines()
        .skip_while(|l| *l != "## Validated against reality")
        .skip(1)
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .skip(2)
        .collect()
}

#[test]
fn every_claim_links_a_passing_test_and_its_plot() {
    let root = root();
    let readme = fs::read_to_string(root.join("README.md")).expect("the README");
    let rows = claims(&readme);
    assert!(rows.len() > 50, "the table has {} rows", rows.len());
    let mut plots = 0;
    for row in &rows {
        let claim = row.split('|').nth(1).unwrap_or("").trim();
        let status = row.split('|').nth(3).unwrap_or("");
        assert!(
            status.contains('✅') || status.contains('🟨'),
            "{claim}: not checked by a passing test"
        );
        let targets = links(row);
        let tests: Vec<&str> = targets
            .iter()
            .copied()
            .filter(|t| t.ends_with(".rs"))
            .collect();
        let figures: Vec<&str> = targets
            .iter()
            .copied()
            .filter(|t| t.starts_with("docs/validation/") && t.ends_with(".svg"))
            .collect();
        assert!(!tests.is_empty(), "{claim}: no test linked");
        assert!(!figures.is_empty(), "{claim}: no plot linked");
        for test in &tests {
            assert!(root.join(test).is_file(), "{claim}: {test} doesn't exist");
        }
        for figure in figures {
            assert!(root.join(figure).is_file(), "{claim}: {figure} isn't drawn");
            let id = figure
                .trim_start_matches("docs/validation/")
                .trim_end_matches(".svg");
            let data = root.join(format!("docs/validation/data/{id}.json"));
            let json = fs::read_to_string(&data)
                .unwrap_or_else(|_| panic!("{claim}: {id} has no recorded data"));
            let (file, function) = (field(&json, "file"), field(&json, "test"));
            assert!(
                tests.contains(&file.as_str()),
                "{claim}: {id} comes from {file}, which the row doesn't link"
            );
            let name = function.rsplit("::").next().expect("a name");
            let source = fs::read_to_string(root.join(&file)).expect("the test's file");
            assert!(
                source.contains(&format!("fn {name}(")),
                "{claim}: {file} has no test {name}"
            );
            plots += 1;
        }
    }
    println!(
        "{} claims, {plots} plots, each from a test that exists",
        rows.len()
    );
}

#[test]
fn every_recorded_figure_is_drawn() {
    let folder = root().join("docs/validation");
    let mut count = 0;
    for entry in fs::read_dir(folder.join("data")).expect("the data folder") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|e| e == "json") {
            let id = path.file_stem().unwrap().to_string_lossy().to_string();
            assert!(
                folder.join(format!("{id}.svg")).is_file(),
                "{id} isn't drawn: run python tools/validation_report.py"
            );
            count += 1;
        }
    }
    println!("{count} figures, each drawn");
}
