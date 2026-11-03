//! `font-feature-settings` and `font-variant-numeric` reach the engine as
//! OpenType features (#224).

use forme::style::FontFeature;
use forme::{Node, NodeKind};
use forme_pdf_html::{html_to_document, HtmlOptions};

fn feature(tag: &str, value: u32) -> FontFeature {
    FontFeature {
        tag: tag.to_string(),
        value,
    }
}

/// The font features on the first Text node's style, and on its runs.
fn text_features(html: &str) -> (Option<Vec<FontFeature>>, Vec<Option<Vec<FontFeature>>>) {
    fn find(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if matches!(n.kind, NodeKind::Text { .. }) {
                return Some(n);
            }
            if let Some(t) = find(&n.children) {
                return Some(t);
            }
        }
        None
    }
    let (doc, _) = html_to_document(html, &HtmlOptions::default());
    let text = find(&doc.children).expect("a text node");
    let runs = match &text.kind {
        NodeKind::Text { runs, .. } => runs.iter().map(|r| r.style.font_features.clone()).collect(),
        _ => vec![],
    };
    (text.style.font_features.clone(), runs)
}

#[test]
fn font_feature_settings_and_variant_numeric() {
    let (f, _) =
        text_features(r#"<p style="font-feature-settings: 'tnum' 1, 'zero', 'liga' off">1</p>"#);
    assert_eq!(
        f,
        Some(vec![
            feature("tnum", 1),
            feature("zero", 1),
            feature("liga", 0)
        ])
    );

    let (f, _) =
        text_features(r#"<p style="font-variant-numeric: tabular-nums slashed-zero">1</p>"#);
    assert_eq!(f, Some(vec![feature("tnum", 1), feature("zero", 1)]));

    // As in CSS, font-feature-settings wins for the same feature.
    let (f, _) = text_features(
        r#"<p style="font-variant-numeric: tabular-nums; font-feature-settings: 'tnum' 0">1</p>"#,
    );
    assert_eq!(f, Some(vec![feature("tnum", 0)]));

    let (f, _) = text_features(r#"<p style="font-feature-settings: normal">1</p>"#);
    assert_eq!(f, Some(vec![]));
}

#[test]
fn an_inline_span_carries_its_own_features() {
    let (_, runs) = text_features(
        r#"<p>Total <span style="font-variant-numeric: tabular-nums">1,011.00</span></p>"#,
    );
    assert!(
        runs.iter()
            .any(|r| r.as_deref() == Some(&[feature("tnum", 1)][..])),
        "{runs:?}"
    );
}

#[test]
fn features_change_the_drawn_glyphs() {
    let glyph = |css: &str| {
        let html = format!(r#"<p style="font-family: 'Noto Sans'; {css}">0</p>"#);
        let (doc, _) = html_to_document(&html, &HtmlOptions::default());
        let pages =
            forme::layout::LayoutEngine::new().layout(&doc, &forme::font::FontContext::new());
        fn first(els: &[forme::layout::LayoutElement]) -> Option<u16> {
            for el in els {
                if let forme::layout::DrawCommand::Text { lines, .. } = &el.draw {
                    if let Some(g) = lines.first().and_then(|l| l.glyphs.first()) {
                        return Some(g.glyph_id);
                    }
                }
                if let Some(g) = first(&el.children) {
                    return Some(g);
                }
            }
            None
        }
        first(&pages[0].elements).expect("a glyph")
    };
    assert_ne!(glyph(""), glyph("font-variant-numeric: slashed-zero"));
}
