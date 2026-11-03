//! Inline `<svg>` maps to the engine's Svg node (#196): the shapes are
//! drawn, the size follows the browser order, and what the engine cannot
//! draw is named in the warnings instead of printing as paragraphs.

use forme_pdf_html::{html_to_document, render_html, HtmlOptions};

fn svgs(nodes: &[forme::Node], out: &mut Vec<(f64, f64, Option<String>, String)>) {
    for n in nodes {
        if let forme::model::NodeKind::Svg {
            width,
            height,
            view_box,
            content,
        } = &n.kind
        {
            out.push((*width, *height, view_box.clone(), content.clone()));
        }
        svgs(&n.children, out);
    }
}

fn first_svg(html: &str) -> (f64, f64, Option<String>, String) {
    let (doc, _) = html_to_document(html, &HtmlOptions::default());
    let mut found = Vec::new();
    svgs(&doc.children, &mut found);
    assert_eq!(found.len(), 1, "expected one Svg node");
    found.remove(0)
}

#[test]
fn issue_196_repro_draws_the_rect_and_names_the_text() {
    let html = r##"<!doctype html><html><body><p>Before</p>
<svg width="200" height="60" viewBox="0 0 200 60">
  <rect x="1" y="1" width="120" height="40" fill="#e8eafb" stroke="#3b4cca"/>
  <text x="10" y="26">label</text>
</svg>
<p>After</p></body></html>"##;
    let (w, h, vb, content) = first_svg(html);
    assert_eq!((w, h), (150.0, 45.0));
    assert_eq!(vb.as_deref(), Some("0 0 200 60"));
    assert!(content.starts_with("<svg "), "{content}");
    assert!(content.contains("viewBox=\"0 0 200 60\""), "{content}");

    let out = render_html(html, &HtmlOptions::default()).unwrap();
    assert!(
        out.warnings.iter().any(|w| w.contains("SVG <text>")),
        "{:?}",
        out.warnings
    );
    assert!(out.pdf.starts_with(b"%PDF"));
    // "label" must not print as a paragraph between Before and After.
    let (doc, _) = html_to_document(html, &HtmlOptions::default());
    fn texts(n: &[forme::Node], out: &mut String) {
        for x in n {
            if let forme::model::NodeKind::Text { content, .. } = &x.kind {
                out.push_str(content);
            }
            texts(&x.children, out);
        }
    }
    let mut all = String::new();
    texts(&doc.children, &mut all);
    assert!(!all.contains("label"), "label printed as text: {all}");
}

#[test]
fn svg_case_survives_html_parsing() {
    let (_, _, _, content) = first_svg(
        r##"<svg width="10" height="10" viewBox="0 0 10 10"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="red"/></linearGradient><clipPath id="c"><rect width="5" height="5"/></clipPath></defs><use xlink:href="#c"/></svg>"##,
    );
    for needle in [
        "<linearGradient",
        "gradientUnits=",
        "<clipPath",
        "xlink:href=",
        "viewBox=",
    ] {
        assert!(content.contains(needle), "{needle} lost: {content}");
    }
}

#[test]
fn sizing_follows_the_browser_order() {
    // CSS first.
    let (w, h, ..) =
        first_svg(r#"<svg style="width: 100px; height: 40px" width="10" height="10"></svg>"#);
    assert_eq!((w, h), (75.0, 30.0));
    // Attributes next.
    let (w, h, ..) = first_svg(r#"<svg width="80" height="20"></svg>"#);
    assert_eq!((w, h), (60.0, 15.0));
    // viewBox proportions fill the missing dimension.
    let (w, h, ..) = first_svg(r#"<svg width="200" viewBox="0 0 100 50"></svg>"#);
    assert_eq!((w, h), (150.0, 75.0));
    let (w, h, ..) = first_svg(r#"<svg height="100" viewBox="0 0 100 50"></svg>"#);
    assert_eq!((w, h), (150.0, 75.0));
    // Neither set: 300 x 150 px.
    let (w, h, ..) = first_svg(r#"<svg></svg>"#);
    assert_eq!((w, h), (225.0, 112.5));
}

#[test]
fn percentage_size_is_named() {
    let out = render_html(
        r#"<svg width="100%" viewBox="0 0 100 50"><rect width="10" height="10"/></svg>"#,
        &HtmlOptions::default(),
    )
    .unwrap();
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("100%") && w.contains("<svg>")),
        "{:?}",
        out.warnings
    );
}

#[test]
fn title_becomes_the_alt_text() {
    let (doc, _) = html_to_document(
        r#"<svg width="10" height="10"><title>Revenue by month</title></svg>"#,
        &HtmlOptions::default(),
    );
    fn alt(n: &[forme::Node]) -> Option<String> {
        n.iter()
            .find_map(|x| x.alt.clone().or_else(|| alt(&x.children)))
    }
    assert_eq!(alt(&doc.children).as_deref(), Some("Revenue by month"));
}
