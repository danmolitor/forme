//! Vertical centering in fixed-height boxes — fails-first.
//!
//! The launch-demo logo mark: a 36x36 box with "ND". Chrome centers it
//! by flex align-items or by line-height half-leading; we top-aligned
//! under every mechanism. Root causes: (1) layout_flex_row sized the
//! flex line at max(item heights), never the container's definite cross
//! size (CSS 9.4.8) — fixed; (2) glyph baselines sit exactly font_size
//! below the line-box top with no half-leading — STRUCTURAL, deferred,
//! and it must warn (silent top-alignment when centering was asked for
//! is the render-defect class).

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

fn line_y(out: &HtmlLayoutOutput, needle: &str) -> (f64, f64) {
    let mut hit = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if hit.is_none() && el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        hit = Some((el.y, el.height));
                    }
                }
            }
        });
    }
    hit.expect("text line")
}

const MARK: &str = "width: 36pt; height: 36pt; background: #1d4ed8; color: #fff; font-weight: bold; font-size: 14pt;";

#[test]
fn flex_align_items_centers_bare_text_vertically() {
    // Variant 1. Box top y=54; item is one ~19.6pt line; centered top
    // = 54 + (36 - 19.6)/2 = 62.2.
    let out = render(&format!(
        "<html><head><style>body{{margin:0}} .mark {{ {MARK} display: flex; align-items: center; justify-content: center; }}</style></head>\
         <body><div class=\"mark\">ND</div></body></html>"
    ));
    let (y, h) = line_y(&out, "ND");
    let centered = 54.0 + (36.0 - h) / 2.0;
    assert!(
        (y - centered).abs() < 0.5,
        "align-items: center must center vertically: y={y:.1}, expected {centered:.1}"
    );
}

#[test]
fn flex_align_items_centers_a_span_vertically() {
    // Variant 3 — same expectation through an explicit inline wrapper.
    let out = render(&format!(
        "<html><head><style>body{{margin:0}} .mark {{ {MARK} display: flex; align-items: center; justify-content: center; }}</style></head>\
         <body><div class=\"mark\"><span>ND</span></div></body></html>"
    ));
    let (y, h) = line_y(&out, "ND");
    let centered = 54.0 + (36.0 - h) / 2.0;
    assert!(
        (y - centered).abs() < 0.5,
        "span in flex must center vertically: y={y:.1}, expected {centered:.1}"
    );
}

#[test]
fn flex_align_flex_end_bottoms_out() {
    // The same line-cross-size fix drives flex-end; pin it too.
    let out = render(&format!(
        "<html><head><style>body{{margin:0}} .mark {{ {MARK} display: flex; align-items: flex-end; }}</style></head>\
         <body><div class=\"mark\">ND</div></body></html>"
    ));
    let (y, h) = line_y(&out, "ND");
    let bottomed = 54.0 + 36.0 - h;
    assert!(
        (y - bottomed).abs() < 0.5,
        "align-items: flex-end must bottom-align: y={y:.1}, expected {bottomed:.1}"
    );
}

#[test]
fn line_height_centering_warns_as_a_render_defect() {
    // Variant 2. Half-leading is not applied (structural, deferred) —
    // but a 36pt line-height on 14pt text is the pre-flexbox centering
    // idiom, and silently top-aligning it is the exact class the
    // render-defect channel exists for. This warning must not go silent
    // until half-leading ships.
    let out = render(&format!(
        "<html><head><style>body{{margin:0}} .mark {{ {MARK} text-align: center; line-height: 36pt; }}</style></head>\
         <body><div class=\"mark\">ND</div></body></html>"
    ));
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("render defect:") && w.contains("line box")),
        "line-height centering must warn: {:?}",
        out.warnings
    );
}
