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
    // The Table wrapper element exists since the engine's Table-wrapper
    // change (gate-verdict gap #2) — table-level border/background now
    // have a paint target and structural consumers see a real Table node.
    let tables = collect_by_node_type(&out, "Table");
    assert_eq!(tables.len(), 1, "exactly one Table wrapper element");
    assert_eq!(
        tables[0].children.len(),
        8,
        "all rows nest inside the Table wrapper"
    );
    assert_eq!(
        tables[0].kind, "Rect",
        "the fixture's 1px table border must have a paint target"
    );

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

#[test]
fn flex_container_inline_children_become_separate_items() {
    // CSS Flexbox: each in-flow child ELEMENT of a flex container is its
    // own flex item — inline or not — and each contiguous run of bare
    // text wraps in an anonymous item. The mapper merged consecutive
    // inline children into ONE Text node (correct for block containers,
    // wrong here), so <div style="display:flex; justify-content:
    // space-between"><span>Label</span><span>$1,234</span></div> — the
    // label/figure row in any document — silently fused: no spread, no
    // independent alignment, the line box sized by whichever font came
    // first (the Northmoor amount-due row: a 25.5pt figure inside an
    // 11.25pt line).
    let html = r#"<html><body>
      <div style="display: flex; justify-content: space-between; width: 255pt">
        <span style="font-size: 6pt">AMOUNT DUE</span>
        <span style="font-size: 25.5pt">$4,647.07</span>
      </div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
    let mut lines: Vec<(String, f64, f64, f64)> = Vec::new();
    for p in &out.layout.pages {
        walk(&p.elements, &mut |e| {
            if let Some(t) = &e.text_content {
                if !t.trim().is_empty() {
                    lines.push((t.clone(), e.x, e.width, e.height));
                }
            }
        });
    }
    assert_eq!(
        lines.len(),
        2,
        "two separate flex items, not one merged line: {lines:?}"
    );
    let label = lines
        .iter()
        .find(|l| l.0.contains("AMOUNT"))
        .expect("label");
    let figure = lines
        .iter()
        .find(|l| l.0.contains("4,647"))
        .expect("figure");
    // space-between: label at the container's left edge, figure's right
    // edge at the container's right edge (x starts at the page margin).
    let left = label.1;
    let right = figure.1 + figure.2;
    assert!(
        (right - left - 255.0).abs() < 2.0,
        "space-between must spread items to the box edges: left {left}, right {right}"
    );
    // The figure's line box is sized by its OWN font, not the label's.
    assert!(
        figure.3 > 20.0,
        "a 25.5pt run needs its own line box, got height {}",
        figure.3
    );
}

#[test]
fn flex_container_bare_text_runs_are_their_own_items() {
    // Contiguous bare text wraps in one anonymous item; an element
    // sibling is a separate item. Whitespace-only text between items
    // produces no item at all.
    let html = r#"<html><body>
      <div style="display: flex; gap: 12pt">plain run <b>bold item</b></div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
    let mut lines: Vec<String> = Vec::new();
    for p in &out.layout.pages {
        walk(&p.elements, &mut |e| {
            if let Some(t) = &e.text_content {
                if !t.trim().is_empty() {
                    lines.push(t.clone());
                }
            }
        });
    }
    assert_eq!(lines.len(), 2, "bare-text item + element item: {lines:?}");
}

#[test]
fn nested_flex_row_intrinsic_width_includes_its_gaps() {
    // measure_intrinsic_width read the raw `gap` field, but the HTML
    // path folds `gap:` into column_gap (the field layout reads) — so
    // every CSS-gapped flex row measured gapless. A nested row then
    // under-reported its intrinsic width by (n-1)*gap, was handed
    // exactly that width by its parent, went over-full, and SHRANK its
    // own fixed-width children: the masthead square that rendered
    // 25.9pt wide with `width: 33pt` declared.
    let html = r#"<html><body>
      <div style="display: flex">
        <div style="display: flex; gap: 12pt">
          <div style="width: 33pt; height: 33pt; background-color: #7B2233"></div>
          <div style="width: 60pt"><p>beside</p></div>
        </div>
        <div><p>filler that takes the rest of the outer row</p></div>
      </div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
    let mut squares: Vec<(f64, f64)> = Vec::new();
    for p in &out.layout.pages {
        walk(&p.elements, &mut |e| {
            if e.height > 30.0 && e.height < 36.0 && e.width < 40.0 && e.node_type == "View" {
                squares.push((e.width, e.height));
            }
        });
    }
    let sq = squares
        .iter()
        .find(|(w, h)| (*h - 33.0).abs() < 0.01 && *w > 20.0)
        .unwrap_or_else(|| panic!("no candidate square found: {squares:?}"));
    assert!(
        (sq.0 - 33.0).abs() < 0.01,
        "a declared 33pt square must render 33pt wide, got {}x{}",
        sq.0,
        sq.1
    );
}

#[test]
fn tracked_uppercase_text_measures_at_its_styled_width() {
    // split_box_and_text_style dropped letter_spacing and text_transform
    // from the node's text style, so tracked/uppercased text measured at
    // its untracked lowercase width — under-sizing every shrink-wrapped
    // container around styled text. A shrink-to-fit flex item around a
    // tracked uppercase paragraph must be at least as wide as the
    // untracked text PLUS the tracking.
    let html = r#"<html><body>
      <div style="display: flex">
        <div style="background-color: #eee">
          <p style="letter-spacing: 2pt; text-transform: uppercase; font-size: 10pt; margin: 0">northmoor</p>
        </div>
        <div><p>filler taking the remaining width of the row</p></div>
      </div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
    let mut line_w = 0.0f64;
    let mut box_w = 0.0f64;
    for p in &out.layout.pages {
        walk(&p.elements, &mut |e| {
            if let Some(t) = &e.text_content {
                if t.contains("NORTHMOOR") {
                    line_w = e.width;
                }
            }
            if e.node_type == "View"
                && e.width > 0.0
                && e.width < 200.0
                && e.x < 100.0
                && box_w == 0.0
            {
                box_w = e.width;
            }
        });
    }
    assert!(line_w > 0.0, "the tracked line must render (uppercased)");
    // "NORTHMOOR" at 10pt Helvetica untracked is ~61pt; with 2pt tracking
    // across 9 glyphs the line is ~79pt. The container must hold the
    // tracked width — not clip ~18pt of tracking away.
    assert!(
        box_w + 0.5 >= line_w,
        "shrink-wrapped box ({box_w}) must be at least the tracked line width ({line_w})"
    );
}

#[test]
fn letter_and_word_spacing_inherit_to_descendant_text() {
    // CSS: letter-spacing and word-spacing are inherited properties. The
    // engine resolved both with unwrap_or(0.0) — no parent fallback —
    // while text-transform beside them inherits correctly. The mapper
    // relies on engine inheritance for anything it doesn't set per node
    // ("the engine's own inheritance does the rest"), so tracking on a
    // container silently vanished from descendant text: the Northmoor
    // wordmark rendered untracked, and measured that way too.
    // (word-spacing inherits engine-side too now, but the CSS subset
    // does not parse the property yet — letter-spacing carries the pin.)
    let spaced = r#"<html><body>
      <div style="letter-spacing: 2pt"><p style="margin: 0">north moor group</p></div>
    </body></html>"#;
    let plain = r#"<html><body>
      <div><p style="margin: 0">north moor group</p></div>
    </body></html>"#;
    let width_of = |html: &str| {
        let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
        let mut w = 0.0f64;
        for p in &out.layout.pages {
            walk(&p.elements, &mut |e| {
                if e.text_content.as_deref() == Some("north moor group") {
                    w = e.width;
                }
            });
        }
        assert!(w > 0.0, "line must render");
        w
    };
    let ws = width_of(spaced);
    let wp = width_of(plain);
    // 16 chars => 15 inter-glyph gaps * 2pt tracking = 30pt.
    assert!(
        ws - wp > 29.5,
        "inherited tracking must widen the line: spaced {ws} vs plain {wp}"
    );
}
