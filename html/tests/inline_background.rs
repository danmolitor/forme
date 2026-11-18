//! Inline backgrounds (#250): `<mark>` and a `background-color` on a span
//! reach the engine as a run's background, and a space collapsed at the
//! edge of an inline element belongs to the text before it, as in CSS.

use forme::model::{Node, NodeKind};
use forme_pdf_html::{html_to_document, HtmlOptions};

/// (content, background red channel) for every run of the first Text.
fn runs(html: &str) -> Vec<(String, Option<f64>)> {
    fn find(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if let NodeKind::Text { runs, .. } = &n.kind {
                if !runs.is_empty() {
                    return Some(n);
                }
            }
            if let Some(f) = find(&n.children) {
                return Some(f);
            }
        }
        None
    }
    let (doc, _) = html_to_document(html, &HtmlOptions::default());
    let NodeKind::Text { runs, .. } = &find(&doc.children).expect("a text with runs").kind else {
        unreachable!()
    };
    runs.iter()
        .map(|r| (r.content.clone(), r.style.background_color.map(|c| c.r)))
        .collect()
}

#[test]
fn mark_and_span_backgrounds_reach_the_runs() {
    let r = runs("<p>Skills: <mark>highlighted</mark> and <span style=\"background-color: #e8eafb\">chip</span>.</p>");
    assert!(
        r.iter()
            .any(|(c, bg)| c == "highlighted" && *bg == Some(1.0)),
        "{r:?}"
    );
    assert!(r.iter().any(|(c, bg)| c == "chip" && bg.is_some()), "{r:?}");
}

#[test]
fn a_collapsed_space_belongs_to_the_text_before_it() {
    // The space before <mark> is not highlighted, and the one after it is
    // not either: both sit in the unhighlighted runs around it.
    let r = runs("<p>Skills: <mark>highlighted</mark> tail.</p>");
    let marked: Vec<&String> = r
        .iter()
        .filter(|(_, bg)| bg.is_some())
        .map(|(c, _)| c)
        .collect();
    assert_eq!(marked, ["highlighted"], "{r:?}");
    assert_eq!(r[0].0, "Skills: ", "{r:?}");
}

#[test]
fn a_nested_inline_keeps_the_highlight() {
    let r = runs("<p><mark>Nested <b>bold</b> inside</mark></p>");
    assert!(r.iter().all(|(_, bg)| *bg == Some(1.0)), "{r:?}");
}
