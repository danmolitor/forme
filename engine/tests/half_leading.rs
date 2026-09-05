//! Half-leading — fails-first.
//!
//! The engine placed every glyph baseline exactly `font_size` below the
//! line-box top: line-height grew the box and the spacing, but glyphs
//! hugged the top. Per CSS (and every browser), the leading splits in
//! half above and below the glyph block — which is also how every
//! pre-flexbox site vertically centers text in buttons, badges, and
//! avatars (`line-height` equal to the box height).
//!
//! Approved global behavior change (2026-09-05): every baseline moves
//! down by (line_height − font_size)/2 — +2.4pt at the 1.4 default on
//! 12pt text. Line BOXES do not move or resize, so layout geometry,
//! page breaks, and structural snapshots are unchanged; only the ink
//! inside each line box shifts.

use forme::font::FontContext;
use forme::layout::{DrawCommand, LayoutEngine, LayoutPage};
use forme::Document;

fn baselines(pages: &[LayoutPage]) -> Vec<f64> {
    fn walk(els: &[forme::layout::LayoutElement], out: &mut Vec<f64>) {
        for el in els {
            if let DrawCommand::Text { lines, .. } = &el.draw {
                for l in lines {
                    out.push(l.y);
                }
            }
            walk(&el.children, out);
        }
    }
    let mut out = Vec::new();
    for p in pages {
        walk(&p.elements, &mut out);
    }
    out
}

fn layout(doc_json: &str) -> Vec<LayoutPage> {
    let doc: Document = serde_json::from_str(doc_json).expect("doc json");
    let font_context = FontContext::new();
    LayoutEngine::new().layout(&doc, &font_context)
}

const PAGE: &str = r#""metadata": {}, "defaultPage": { "size": "A4", "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 }, "wrap": true }"#;

#[test]
fn default_line_height_splits_leading_around_the_glyphs() {
    // 12pt font, 1.4 line-height → 16.8pt line box, 4.8pt leading.
    // Baseline = content top + half-leading + font_size = 54 + 2.4 + 12.
    let pages = layout(&format!(
        r#"{{ "children": [ {{ "kind": {{ "type": "Text", "content": "hello" }}, "style": {{ "fontSize": 12 }} }} ], {PAGE} }}"#
    ));
    let b = baselines(&pages);
    assert_eq!(b.len(), 1);
    assert!(
        (b[0] - (54.0 + 2.4 + 12.0)).abs() < 0.05,
        "baseline {:.2}, expected 68.40 (top + half-leading + font size)",
        b[0]
    );
}

#[test]
fn line_height_equal_to_box_height_centers_the_glyphs() {
    // The pre-flexbox centering idiom: 14pt text, 36pt line box.
    // Baseline = 54 + (36 − 14)/2 + 14 = 79.
    let pages = layout(&format!(
        r#"{{ "children": [ {{ "kind": {{ "type": "Text", "content": "ND" }}, "style": {{ "fontSize": 14, "lineHeight": {lh} }} }} ], {PAGE} }}"#,
        lh = 36.0 / 14.0
    ));
    let b = baselines(&pages);
    assert!(
        (b[0] - 79.0).abs() < 0.05,
        "baseline {:.2}, expected 79.00 (centered in the 36pt line box)",
        b[0]
    );
}

#[test]
fn multi_line_spacing_is_unchanged_by_half_leading() {
    // Successive baselines stay line_height apart — half-leading shifts
    // the whole ladder, never the rungs' spacing.
    let pages = layout(&format!(
        r#"{{ "children": [ {{ "kind": {{ "type": "Text", "content": "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty" }}, "style": {{ "fontSize": 12, "width": {{ "Pt": 100 }} }} }} ], {PAGE} }}"#
    ));
    let b = baselines(&pages);
    assert!(b.len() >= 3, "need several lines, got {}", b.len());
    for w in b.windows(2) {
        assert!(
            ((w[1] - w[0]) - 16.8).abs() < 0.05,
            "inter-baseline spacing {:.2}, expected 16.80",
            w[1] - w[0]
        );
    }
}

#[test]
fn runs_text_gets_the_same_half_leading() {
    // The runs path (multi-style text) shares the convention.
    let pages = layout(&format!(
        r#"{{ "children": [ {{ "kind": {{ "type": "Text", "content": "", "runs": [ {{ "content": "styled", "style": {{}} }} ] }}, "style": {{ "fontSize": 12 }} }} ], {PAGE} }}"#
    ));
    let b = baselines(&pages);
    assert_eq!(b.len(), 1);
    assert!(
        (b[0] - 68.4).abs() < 0.05,
        "runs baseline {:.2}, expected 68.40",
        b[0]
    );
}
