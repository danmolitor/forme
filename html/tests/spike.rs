//! The Phase 0 spike's empirical gate: the invoice fixture must render
//! correctly, and the quiet failure modes (doubled margins, mangled
//! whitespace) must be loud red tests, not eyeball judgments.
//!
//! Also writes `target/spike-invoice.pdf` for side-by-side comparison with
//! the frozen Chrome reference at `tests/fixtures/invoice.chrome-reference.pdf`.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

const FIXTURE: &str = include_str!("fixtures/invoice.html");

fn render_fixture() -> HtmlLayoutOutput {
    render_html_with_layout(FIXTURE, &HtmlOptions::default()).expect("fixture must render")
}

/// Depth-first walk over the element tree of every page.
fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

fn collect_by_node_type<'a>(out: &'a HtmlLayoutOutput, node_type: &str) -> Vec<&'a ElementInfo> {
    let mut found = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == node_type {
                found.push(el);
            }
        });
    }
    found
}

/// Find the element whose text content contains `needle`.
fn find_text<'a>(out: &'a HtmlLayoutOutput, needle: &str) -> Option<&'a ElementInfo> {
    let mut found = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if found.is_none() {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        found = Some(el);
                    }
                }
            }
        });
    }
    found
}

#[test]
fn fixture_renders_to_a_single_page_pdf() {
    let out = render_fixture();
    assert!(out.pdf.starts_with(b"%PDF-"), "output must be a PDF");
    assert_eq!(out.layout.pages.len(), 1, "invoice must fit one page");

    // Write the artifact for the side-by-side against the Chrome reference.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/spike-invoice.pdf");
    std::fs::write(path, &out.pdf).expect("write spike artifact");
}

#[test]
fn margin_collapse_h1_to_p_gap_is_max_not_sum() {
    // THE quiet-failure tripwire. h1's UA margin-bottom is 0.67em against
    // its own 24pt font (16.08pt); p's margin-top is 1em against 12pt
    // (12pt). CSS collapses them to max = 16.08. The engine adds margins,
    // so if the mapper failed to pre-collapse, the gap would be 28.08 —
    // renders fine, looks almost right, and is exactly the doubled-spacing
    // failure that erodes migration confidence.
    let out = render_fixture();
    let h1 = find_text(&out, "Invoice #2024-001").expect("h1 text");
    let p = find_text(&out, "Billed to").expect("first paragraph");

    let gap = p.y - (h1.y + h1.height);
    let expected = 0.67 * 24.0; // 16.08
    assert!(
        (gap - expected).abs() < 0.5,
        "h1→p gap must be the collapsed max ({expected}pt), got {gap:.2}pt"
    );
}

#[test]
fn margin_collapse_p_to_p_gap() {
    // Default-margin sequence #2: two adjacent <p>s at 12pt font. Both
    // margins are 1em = 12pt; collapsed gap = 12, additive would be 24.
    let out = render_fixture();
    let p1 = find_text(&out, "Billed to").expect("first paragraph");
    let p2 = find_text(&out, "Due net 30 days.").expect("second paragraph");

    let gap = p2.y - (p1.y + p1.height);
    assert!(
        (gap - 12.0).abs() < 0.5,
        "p→p gap must be the collapsed 12pt, got {gap:.2}pt"
    );
}

#[test]
fn whitespace_collapses_through_the_whole_pipeline() {
    // Asserted from the LAYOUT, not the mapper output: this proves the
    // sloppy source (newlines inside <strong>, spans split across lines)
    // survived collapsing all the way through the engine.
    let out = render_fixture();
    assert!(
        find_text(&out, "Due net 30 days.").is_some(),
        "'Due net 30 days.' must render with single spaces"
    );
    assert!(
        find_text(&out, "Billed to Wayne Enterprises on August 28, 2026.").is_some(),
        "paragraph with inline elements split across source lines must collapse to single spaces"
    );
    // No doubled spaces anywhere in rendered text.
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if let Some(t) = &el.text_content {
                assert!(
                    !t.contains("  "),
                    "rendered text contains a doubled space: {t:?}"
                );
            }
        });
    }
}

#[test]
fn table_maps_with_header_and_all_rows() {
    let out = render_fixture();
    // The engine emits no Table wrapper element — rows are pushed directly
    // (observed in the layout dump; recorded in the gate verdict).
    let rows = collect_by_node_type(&out, "TableRow");
    // 1 header + 6 line items + 1 totals row.
    assert_eq!(rows.len(), 8, "8 table rows");

    // Cell content renders.
    assert!(find_text(&out, "Widget Enterprise").is_some());
    assert!(find_text(&out, "$705.00").is_some());
}

#[test]
fn heading_list_and_image_map_to_engine_node_types() {
    let out = render_fixture();
    // Headings surface with their tagged-PDF node types (H1..H6).
    assert_eq!(collect_by_node_type(&out, "H1").len(), 1, "one h1");
    assert_eq!(collect_by_node_type(&out, "H2").len(), 1, "one h2");
    assert!(
        !collect_by_node_type(&out, "Image").is_empty(),
        "logo data-URI image renders"
    );
    assert!(find_text(&out, "Make checks payable to").is_some());
}

#[test]
fn br_produces_a_multi_line_address_block() {
    // The <br> probe (recorded in the gate verdict either way): the address
    // block has two <br>s, so it must render as three lines. If the engine
    // ignored the mapper's '\n', this would be one long line.
    let out = render_fixture();
    // Only TextLine leaf elements carry text_content, so a hard break
    // shows up as the address rendering as three separate lines at
    // distinct, stacked y positions.
    let line1 = find_text(&out, "Acme Widget Co.").expect("line 1");
    let line2 = find_text(&out, "123 Main St").expect("line 2");
    let line3 = find_text(&out, "Springfield, IL 62704").expect("line 3");
    assert!(
        line1.y < line2.y && line2.y < line3.y,
        "address lines must stack vertically: {} / {} / {}",
        line1.y,
        line2.y,
        line3.y
    );
    // If the engine had ignored the '\n', all three would share one line.
    assert!(line2.y - line1.y > 10.0 && line3.y - line2.y > 10.0);
}

#[test]
fn unknown_css_lands_in_warnings_not_errors() {
    // The fixture's closing <p> carries transform: rotate(0.5deg) — the
    // negative-discipline case: unsupported property, graceful, loud.
    let out = render_fixture();
    assert!(
        out.warnings.iter().any(|w| w.contains("transform")),
        "transform must be reported as unsupported, warnings: {:?}",
        out.warnings
    );
    // And the paragraph it was on still rendered.
    assert!(find_text(&out, "Thank you for your business!").is_some());
}

#[test]
fn stylesheet_block_is_ignored_not_rendered() {
    // <style> content must not leak into the document as text (Phase 1
    // will parse it; the spike must at least not render it).
    let out = render_fixture();
    assert!(
        find_text(&out, "color: #111").is_none(),
        "stylesheet text must not render"
    );
}
