//! The CSS colour-name table exists three times: here (the HTML path), in
//! `@formepdf/shared` (every JSX adapter) and in the Python SDK. They must
//! name the same colours with the same values, or a document renders in a
//! different colour depending on how it was written.

use std::collections::BTreeMap;

type Table = BTreeMap<String, [u8; 4]>;

/// One entry per line, as the generated files lay them out: the colour's
/// name is the line's first lowercase word, and its value the line's last
/// four numbers.
fn parse(src: &str) -> Table {
    src.lines()
        .filter_map(|line| {
            let name: String = line
                .trim_start()
                .trim_start_matches(['"', '('])
                .chars()
                .take_while(|c| c.is_ascii_lowercase())
                .collect();
            let nums: Vec<u8> = line
                .split(|c: char| !c.is_ascii_digit())
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            let t = line.trim_start();
            let entry = t.starts_with("(\"")
                || t.starts_with(&format!("\"{name}\":"))
                || t.starts_with(&format!("{name}:"));
            (entry && nums.len() >= 4 && !name.is_empty()).then(|| {
                let v = &nums[nums.len() - 4..];
                (name, [v[0], v[1], v[2], v[3]])
            })
        })
        .collect()
}

fn rust_table() -> Table {
    parse(include_str!("../src/colors.rs"))
}

#[test]
fn colour_names_match_in_all_three_parsers() {
    let rust = rust_table();
    assert_eq!(rust.len(), 149, "148 CSS names plus transparent");
    let ts = parse(include_str!("../../packages/shared/src/colors.ts"));
    let py = parse(include_str!(
        "../../packages/python-sdk/formepdf/_colors.py"
    ));
    assert_eq!(
        ts, rust,
        "packages/shared/src/colors.ts differs from html/src/colors.rs"
    );
    assert_eq!(
        py, rust,
        "formepdf/_colors.py differs from html/src/colors.rs"
    );
}

#[test]
fn html_reads_any_css_colour_name() {
    let (doc, warnings) = forme_pdf_html::html_to_document(
        r#"<div style="background-color: rebeccapurple; color: AliceBlue">x</div>"#,
        &forme_pdf_html::HtmlOptions::default(),
    );
    assert!(
        !warnings.iter().any(|w| w.contains("color")),
        "{warnings:?}"
    );
    let json = serde_json::to_string(&doc).unwrap();
    assert!(
        json.contains("0.4"),
        "rebeccapurple's red channel (102/255): {json}"
    );
}
