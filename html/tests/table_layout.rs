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

// ── tfoot renders at the bottom regardless of DOM position ─────────

#[test]
fn tfoot_before_tbody_renders_last() {
    // The niraj-invoice shape (template-compat 08): markup puts <tfoot>
    // (Subtotal/Total) BEFORE <tbody> — browsers render tfoot at the
    // bottom of the table regardless of DOM position; we rendered it in
    // DOM order, printing the totals above the line items.
    let out = render(
        "<html><body><table>\
           <thead><tr><th>Item</th><th>Price</th></tr></thead>\
           <tfoot><tr><td>Total</td><td>$30</td></tr></tfoot>\
           <tbody>\
             <tr><td>Widget</td><td>$10</td></tr>\
             <tr><td>Gadget</td><td>$20</td></tr>\
           </tbody>\
         </table></body></html>",
    );
    let y_of = |needle: &str| -> f64 {
        let mut hit = None;
        for page in &out.layout.pages {
            walk(&page.elements, &mut |el| {
                if hit.is_none() && el.node_type == "TextLine" {
                    if let Some(t) = &el.text_content {
                        if t.contains(needle) {
                            hit = Some(el.y);
                        }
                    }
                }
            });
        }
        hit.unwrap_or_else(|| panic!("no line containing {needle:?}"))
    };
    let header = y_of("Item");
    let widget = y_of("Widget");
    let gadget = y_of("Gadget");
    let total = y_of("Total");
    assert!(header < widget, "thead first");
    assert!(widget < gadget, "body rows in order");
    assert!(
        gadget < total,
        "tfoot renders below the body rows (Total at {total:.0}, Gadget at {gadget:.0})"
    );
}

#[test]
fn display_none_rows_and_sections_do_not_render() {
    // The niraj template hides its totals rows with display:none and
    // un-hides them with JS. We don't run JS (constitution), so hidden
    // must mean hidden — collect_rows previously bypassed the
    // display:none check that every other element gets.
    let out = render(
        "<html><body><table>\
           <tr><td>visible row</td></tr>\
           <tr style=\"display: none\"><td>hidden row</td></tr>\
           <thead style=\"display: none\"><tr><td>hidden section</td></tr></thead>\
         </table></body></html>",
    );
    assert!(!text_lines_containing(&out, "visible row").is_empty());
    assert!(
        text_lines_containing(&out, "hidden row").is_empty(),
        "display:none row must not render"
    );
    assert!(
        text_lines_containing(&out, "hidden section").is_empty(),
        "display:none section must not render"
    );
}

#[test]
fn second_thead_stays_in_dom_order() {
    // Per HTML, only the FIRST thead is the table header; a later thead
    // is an ordinary row group in DOM order. The engine hoists header
    // rows to the top (and repeats them per page), so marking a late
    // thead as header printed a totals block ABOVE the line items
    // (template-compat 08's actual shape — its "totals" live in a
    // second thead after tbody, not a tfoot).
    let out = render(
        "<html><body><table>\
           <thead><tr><th>Item</th></tr></thead>\
           <tbody><tr><td>Widget</td></tr></tbody>\
           <thead class=\"totals\"><tr><td>Total</td></tr></thead>\
         </table></body></html>",
    );
    let y_of = |needle: &str| -> f64 {
        let mut hit = None;
        for page in &out.layout.pages {
            walk(&page.elements, &mut |el| {
                if hit.is_none() && el.node_type == "TextLine" {
                    if let Some(t) = &el.text_content {
                        if t.contains(needle) {
                            hit = Some(el.y);
                        }
                    }
                }
            });
        }
        hit.unwrap_or_else(|| panic!("no line containing {needle:?}"))
    };
    assert!(y_of("Item") < y_of("Widget"), "first thead is the header");
    assert!(
        y_of("Widget") < y_of("Total"),
        "a second thead renders in DOM order, not hoisted"
    );
}

#[test]
fn shrink_to_fit_container_gives_a_table_its_intrinsic_width() {
    // Instance 2 of the measure/layout family (the crater templates):
    // measure_intrinsic_width had no Table arm, so a shrink-to-fit box
    // holding a table measured as its widest single CELL and crushed
    // the table to one column's width, wrapping header text per-word
    // ("Invoi ce Num ber"). Tables measure as the sum of per-column
    // content; the label must survive on one line.
    let out = render(
        "<html><body>\
         <div style=\"position:absolute; top:0; right:0\">\
           <table><tr><td>Invoice Number</td><td>INV-000012</td></tr>\
                  <tr><td>Date</td><td>2026-03-01</td></tr></table>\
         </div></body></html>",
    );
    let lines = text_lines_containing(&out, "Invoice Number");
    assert!(
        !lines.is_empty(),
        "the full label renders as ONE line, not per-word fragments"
    );
    let (_, w) = lines[0];
    assert!(
        w > 60.0,
        "label line spans the words, not a crushed column ({w:.1}pt wide)"
    );
}
