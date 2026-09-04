//! Table auto-layout through the HTML mapper + warning dedup — fails-first.
//!
//! Minimal repros of the template-compat findings (the corpus at
//! template-compat/ is the acceptance test; these pins are small enough
//! to survive template churn).

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

fn text_lines_containing(out: &HtmlLayoutOutput, needle: &str) -> Vec<(f64, f64)> {
    let mut lines = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        lines.push((el.x, el.width));
                    }
                }
            }
        });
    }
    lines
}

// ── The sparksuite shape: banner row over two-column body ──────────

#[test]
fn banner_first_row_table_does_not_shred() {
    // THE production-invoice shape: full-width banner row, then 2-column
    // rows. Used to become a one-column table whose second cells rendered
    // one character per line.
    let html = "<html><head><style>table { width: 100% }</style></head><body>\
        <table>\
          <tr><td colspan=\"2\">INVOICE #123</td></tr>\
          <tr><td>Payment Method</td><td>Check #</td></tr>\
          <tr><td>Check</td><td>1000</td></tr>\
        </table></body></html>";
    let out = render(html);
    let lines = text_lines_containing(&out, "Check #");
    assert_eq!(
        lines.len(),
        1,
        "'Check #' must be one line, not shredded: {lines:?}"
    );
    assert!(lines[0].1 > 35.0, "real column width, got {}pt", lines[0].1);
    assert_eq!(out.layout.pages.len(), 1, "one page, no shred explosion");
}

#[test]
fn mapper_harvests_widths_from_first_plain_row() {
    // Widths on a LATER row used to be discarded entirely when the first
    // row had a colspan. Now the first colspan-free row supplies them.
    let html = "<html><head><style>table { width: 100% } td.a { width: 30% } td.b { width: 70% }</style></head><body>\
        <table>\
          <tr><td colspan=\"2\">Banner</td></tr>\
          <tr><td class=\"a\">left</td><td class=\"b\">right side content</td></tr>\
        </table></body></html>";
    let out = render(html);
    let left = text_lines_containing(&out, "left");
    let right = text_lines_containing(&out, "right side");
    assert!(!left.is_empty() && !right.is_empty());
    // A4 default: content width 487.28pt; 30% boundary at 54 + 146.2.
    // The right cell's text starts near that boundary, not at the middle.
    let boundary = 54.0 + 487.28 * 0.30;
    assert!(
        (right[0].0 - boundary).abs() < 12.0,
        "70% column starts at the 30% boundary (~{boundary:.0}pt), got {}",
        right[0].0
    );
}

// ── Warning dedup ──────────────────────────────────────────────────

#[test]
fn duplicate_warnings_collapse_with_a_count() {
    // Framework CSS produced thousands of identical lines (AdminLTE:
    // 6,905). Identical messages collapse to one entry carrying a count.
    let html = "<html><body>\
        <div style=\"box-shadow: 0 0 2px red\">a</div>\
        <div style=\"box-shadow: 0 0 2px red\">b</div>\
        <div style=\"box-shadow: 0 0 2px red\">c</div>\
        </body></html>";
    let out = render(html);
    let shadow_warnings: Vec<&String> = out
        .warnings
        .iter()
        .filter(|w| w.contains("box-shadow"))
        .collect();
    assert_eq!(
        shadow_warnings.len(),
        1,
        "identical warnings must dedup to one entry: {shadow_warnings:?}"
    );
    assert!(
        shadow_warnings[0].contains("×3"),
        "the entry carries the count: {}",
        shadow_warnings[0]
    );
}
