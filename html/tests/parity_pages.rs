//! `@page :left` / `:right` — parity page selection (fails-first).
//!
//! Design (approved): flow layout always uses the BASE horizontal geometry;
//! mirrored margins preserve content width by construction, so page-crossing
//! fragments need only a constant x translation at finalize, never re-layout.
//! Parity per CSS Paged Media §page-selectors: with left-to-right page
//! progression the FIRST page is a RIGHT page (odd 1-based = right). `:first`
//! outranks `:left`/`:right` (specificity g > h). Unequal horizontal margin
//! sums cannot be expressed by translation and warn by name.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

/// 400x400pt page, base margins 50/50 horizontal (sum 100), enough repeated
/// paragraphs to flow across several pages.
fn doc(extra_css: &str, body: &str) -> String {
    format!(
        "<html><head><style>\
         @page {{ size: 400pt 400pt; margin: 40pt 50pt }} \
         body {{ margin: 0 }} p {{ margin: 0 0 10pt 0 }} \
         {extra_css}</style></head><body>{body}</body></html>"
    )
}

fn many_paragraphs(n: usize) -> String {
    (0..n)
        .map(|i| format!("<p>para{i} filler text that occupies a line</p>"))
        .collect()
}

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

/// First element on page `page` whose text contains `needle`.
fn find_on<'a>(out: &'a HtmlLayoutOutput, page: usize, needle: &str) -> &'a ElementInfo {
    let mut found: Option<&ElementInfo> = None;
    walk(&out.layout.pages[page].elements, &mut |el| {
        if found.is_none() {
            if let Some(t) = &el.text_content {
                if t.contains(needle) {
                    found = Some(el);
                }
            }
        }
    });
    found.unwrap_or_else(|| panic!("element containing {needle:?} not found on page {page}"))
}

/// First paragraph-level text needle present on page `page`.
fn first_text_x(out: &HtmlLayoutOutput, page: usize) -> f64 {
    let mut x: Option<f64> = None;
    walk(&out.layout.pages[page].elements, &mut |el| {
        if x.is_none() && el.node_type == "TextLine" {
            x = Some(el.x);
        }
    });
    x.unwrap_or_else(|| panic!("no text line on page {page}"))
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 0.5,
        "{what}: expected ~{expected}, got {actual}"
    );
}

fn warned(out: &HtmlLayoutOutput, needle: &str) {
    assert!(
        out.warnings.iter().any(|w| w.contains(needle)),
        "expected a warning containing {needle:?}; got {:?}",
        out.warnings
    );
}

// ── Parity selection + translation ─────────────────────────────────

#[test]
fn mirrored_margins_shift_content_per_page_parity() {
    // Base 50/50; right pages 70/30 (dx +20); left pages 30/70 (dx -20).
    // Page 1 is RIGHT (LTR page progression, 1-based odd), page 2 LEFT.
    let html = doc(
        "@page :right { margin-left: 70pt; margin-right: 30pt } \
         @page :left { margin-left: 30pt; margin-right: 70pt }",
        &many_paragraphs(80),
    );
    let out = render(&html);
    assert!(
        out.layout.pages.len() >= 3,
        "need 3+ pages, got {}",
        out.layout.pages.len()
    );
    assert!(
        out.warnings.is_empty(),
        "mirrored margins are in-subset: {:?}",
        out.warnings
    );

    assert_close(first_text_x(&out, 0), 70.0, "page 1 (right) x");
    assert_close(first_text_x(&out, 1), 30.0, "page 2 (left) x");
    assert_close(first_text_x(&out, 2), 70.0, "page 3 (right) x");

    // The page CONFIG reflects each side too (margin boxes / PDF margins).
    assert_close(out.layout.pages[0].content_x, 70.0, "page 1 content_x");
    assert_close(out.layout.pages[1].content_x, 30.0, "page 2 content_x");
}

#[test]
fn page_crossing_fragment_lands_correctly_on_both_sides() {
    // One long unbreakable-ish paragraph engineered to straddle the 1->2
    // boundary: its lines were broken ONCE at the base width; the design
    // pins that continuation lines translate to the left page's x.
    let long_para = format!(
        "<p>{}</p>",
        "straddle words flowing across the page boundary one after another ".repeat(60)
    );
    let html = doc(
        "@page :right { margin-left: 70pt; margin-right: 30pt } \
         @page :left { margin-left: 30pt; margin-right: 70pt }",
        &format!("{}{long_para}", many_paragraphs(10)),
    );
    let out = render(&html);
    let p1 = find_on(&out, 0, "straddle");
    let p2 = find_on(&out, 1, "straddle");
    assert_close(p1.x, 70.0, "fragment lines on page 1 (right)");
    assert_close(p2.x, 30.0, "fragment lines on page 2 (left)");
}

#[test]
fn first_outranks_parity_on_page_one() {
    // Page 1 is both :first and :right — :first wins (spec: specificity g > h).
    let html = doc(
        "@page :first { margin-left: 90pt; margin-right: 10pt } \
         @page :right { margin-left: 70pt; margin-right: 30pt } \
         @page :left { margin-left: 30pt; margin-right: 70pt }",
        &many_paragraphs(80),
    );
    let out = render(&html);
    assert_close(first_text_x(&out, 0), 90.0, "page 1 uses :first");
    assert_close(first_text_x(&out, 1), 30.0, "page 2 uses :left");
    assert_close(first_text_x(&out, 2), 70.0, "page 3 uses :right");
}

// ── Margin boxes resolve per side ──────────────────────────────────

#[test]
fn margin_boxes_resolve_per_side() {
    let html = doc(
        "@page { @top-center { content: \"BASE\" } } \
         @page :left { margin-left: 30pt; margin-right: 70pt; @top-center { content: \"VERSO\" } } \
         @page :right { margin-left: 70pt; margin-right: 30pt }",
        &many_paragraphs(80),
    );
    let out = render(&html);
    // Page 1 (right, no override): BASE. Page 2 (left): VERSO.
    find_on(&out, 0, "BASE");
    find_on(&out, 1, "VERSO");
    let mut base_on_p2 = false;
    walk(&out.layout.pages[1].elements, &mut |el| {
        if let Some(t) = &el.text_content {
            if t.contains("BASE") {
                base_on_p2 = true;
            }
        }
    });
    assert!(
        !base_on_p2,
        "left page's @top-center override replaces the base box"
    );
}

// ── The honest boundary: warnings by name ──────────────────────────

#[test]
fn unequal_horizontal_sum_warns_and_normalizes_to_base() {
    // 30 + 80 = 110 != base 100: not expressible by translation.
    let html = doc(
        "@page :left { margin-left: 30pt; margin-right: 80pt }",
        &many_paragraphs(80),
    );
    let out = render(&html);
    warned(&out, "must sum equally");
    warned(&out, "normalized to the base");
    // Page 2 falls back to base horizontal geometry.
    assert_close(first_text_x(&out, 1), 50.0, "page 2 normalized to base x");
}

#[test]
fn vertical_margin_overrides_on_side_pages_warn_and_normalize() {
    let html = doc(
        "@page :left { margin-left: 30pt; margin-right: 70pt; margin-top: 80pt }",
        &many_paragraphs(80),
    );
    let out = render(&html);
    warned(&out, "top/bottom margins");
    // Horizontal mirroring still applies.
    assert_close(first_text_x(&out, 1), 30.0, "page 2 keeps mirrored x");
}

#[test]
fn rtl_page_progression_warns_rather_than_silently_assuming_ltr() {
    let html = format!(
        "<html dir=\"rtl\"><head><style>\
         @page {{ size: 400pt 400pt; margin: 40pt 50pt }} \
         @page :right {{ margin-left: 70pt; margin-right: 30pt }} \
         @page :left {{ margin-left: 30pt; margin-right: 70pt }}\
         </style></head><body>{}</body></html>",
        many_paragraphs(10)
    );
    let out = render(&html);
    warned(&out, "page progression");
}
