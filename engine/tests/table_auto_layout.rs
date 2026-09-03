//! Automatic table layout — fails-first.
//!
//! Root cause pinned by the template-compat experiment: with no column
//! defs, the engine inferred column count from the FIRST row's cell count,
//! ignoring colspan (`children.first().map(|row| row.children.len())`).
//! The most common invoice shape on earth — a full-width banner row over
//! two-column body rows — became a one-column table, and every second
//! cell collapsed to per-character vertical text. Silently.
//!
//! These pins are deliberately minimal (a four-line table, not the corpus
//! file): the corpus at template-compat/ is the acceptance test; these
//! survive template churn.

fn table_doc(columns_json: &str, rows: &[String]) -> String {
    format!(
        r#"{{
        "children": [{{
            "kind": {{ "type": "Table", "columns": {columns} }},
            "style": {{}},
            "children": [{rows}]
        }}],
        "metadata": {{}},
        "defaultPage": {{ "size": "A4", "margin": {{ "top": 54, "right": 54, "bottom": 54, "left": 54 }}, "wrap": true }}
    }}"#,
        columns = columns_json,
        rows = rows.join(",")
    )
}

fn row(cells: &[(&str, u32)]) -> String {
    let cells_json = cells
        .iter()
        .map(|(text, span)| {
            format!(
                r#"{{ "kind": {{ "type": "TableCell", "colSpan": {span} }}, "style": {{}}, "children": [
                    {{ "kind": {{ "type": "Text", "content": "{text}" }}, "style": {{}}, "children": [] }}
                ] }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{ "kind": {{ "type": "TableRow", "isHeader": false }}, "style": {{}}, "children": [{cells_json}] }}"#
    )
}

/// Collect (text, count-of-TextLines, first-line width) for elements whose
/// text contains `needle`.
fn text_lines_for(layout: &forme::layout::LayoutInfo, needle: &str) -> Vec<(f64, f64)> {
    fn walk(els: &[forme::layout::ElementInfo], needle: &str, out: &mut Vec<(f64, f64)>) {
        for e in els {
            if e.node_type == "TextLine" {
                if let Some(t) = &e.text_content {
                    if t.contains(needle) {
                        out.push((e.x, e.width));
                    }
                }
            }
            walk(&e.children, needle, out);
        }
    }
    let mut out = Vec::new();
    for page in &layout.pages {
        walk(&page.elements, needle, &mut out);
    }
    out
}

// ── Tier 1: colspan-aware column counting ──────────────────────────

#[test]
fn banner_first_row_still_yields_two_columns() {
    // Row 1: one colspan=2 banner. Rows 2-3: two plain cells. The old
    // inference made this a ONE-column table and shredded every second
    // cell into per-character vertical text.
    let json = table_doc(
        "[]",
        &[
            row(&[("INVOICE", 2)]),
            row(&[("Payment Method", 1), ("Check #", 1)]),
            row(&[("Check", 1), ("1000", 1)]),
        ],
    );
    let (_pdf, layout, _w) = forme::render_json_with_layout(&json).expect("renders");

    // "Check #" must be ONE text line (not C/h/e/c/k shredded), wide
    // enough to actually hold the text.
    let lines = text_lines_for(&layout, "Check #");
    assert_eq!(
        lines.len(),
        1,
        "second-column text must be a single line, got {lines:?}"
    );
    assert!(
        lines[0].1 > 35.0,
        "second column must have real width, got {}pt",
        lines[0].1
    );
    // And the whole thing stays on one page.
    assert_eq!(layout.pages.len(), 1, "no shred-driven page explosion");
}

// ── Tier 2: content-based distribution ─────────────────────────────

#[test]
fn auto_columns_size_by_content() {
    // A long-description column next to a tiny quantity column: automatic
    // layout gives the long column more room (Chrome-style), never an
    // even split that wraps the description while "1" floats in space.
    let json = table_doc(
        "[]",
        &[
            row(&[
                ("A reasonably long product description that wants room", 1),
                ("1", 1),
            ]),
            row(&[("Another descriptive line item entry here", 1), ("2", 1)]),
        ],
    );
    let (_pdf, layout, _w) = forme::render_json_with_layout(&json).expect("renders");
    let long = text_lines_for(&layout, "wants room");
    let short = text_lines_for(&layout, "1");
    assert!(!long.is_empty() && !short.is_empty());
    // The description column is strictly, substantially wider.
    assert!(
        long[0].1 > short[0].1 + 50.0,
        "content-based sizing: long column {} vs short {}",
        long[0].1,
        short[0].1
    );
}

// ── The render-defect channel ──────────────────────────────────────

#[test]
fn overflowing_fixed_columns_clamp_and_warn() {
    // Fixed widths exceeding the available width used to hand the Auto
    // column a NEGATIVE share. Now: clamped, and reported through the
    // render-defect channel (our output is wrong, not the caller's CSS).
    let json = table_doc(
        r#"[{ "width": { "Fixed": 700.0 } }, { "width": "Auto" }]"#,
        &[row(&[("wide", 1), ("squeezed", 1)])],
    );
    let (_pdf, _layout, warnings) = forme::render_json_with_layout(&json).expect("renders");
    assert!(
        warnings.iter().any(|w| w.contains("render defect")),
        "the defect channel must report the clamp: {warnings:?}"
    );
}

#[test]
fn min_content_shortfall_warns_as_render_defect() {
    // A table that genuinely cannot fit its content reports the shortfall
    // by name instead of silently shredding.
    let json = r#"{
        "children": [{
            "kind": { "type": "Table", "columns": [] },
            "style": {},
            "children": [{ "kind": { "type": "TableRow", "isHeader": false }, "style": {}, "children": [
                { "kind": { "type": "TableCell", "colSpan": 1 }, "style": {}, "children": [
                    { "kind": { "type": "Text", "content": "Unbreakablecolumncontent" }, "style": {}, "children": [] }
                ] },
                { "kind": { "type": "TableCell", "colSpan": 1 }, "style": {}, "children": [
                    { "kind": { "type": "Text", "content": "Anotherunbreakableword" }, "style": {}, "children": [] }
                ] }
            ] }]
        }],
        "metadata": {},
        "defaultPage": { "size": { "Custom": { "width": 160, "height": 400 } }, "margin": { "top": 10, "right": 10, "bottom": 10, "left": 10 }, "wrap": true }
    }"#;
    let (_pdf, _layout, warnings) = forme::render_json_with_layout(json).expect("renders");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("render defect") && w.contains("min-content")),
        "min-content shortfall must be named: {warnings:?}"
    );
}
