//! CSS `opacity` on block-level elements reaches the engine; on an inline
//! element, which text runs can't express, it is reported.

use forme_pdf_html::{html_to_document, render_html, HtmlOptions};

#[test]
fn opacity_maps_to_the_engine() {
    let (doc, _) = html_to_document(
        r#"<div style="opacity: 0.5"><p style="opacity: 40%">x</p></div>"#,
        &HtmlOptions::default(),
    );
    fn opacities(n: &[forme::Node], out: &mut Vec<f64>) {
        for x in n {
            if let Some(o) = x.style.opacity {
                out.push(o);
            }
            opacities(&x.children, out);
        }
    }
    let mut found = Vec::new();
    opacities(&doc.children, &mut found);
    assert!(found.contains(&0.5), "{found:?}");
    assert!(found.iter().any(|o| (o - 0.4).abs() < 1e-9), "{found:?}");
}

#[test]
fn opacity_is_supported_on_blocks_and_reported_on_inline_elements() {
    let out = render_html(
        r#"<p style="opacity: 0.5">a <span style="opacity: 0.5">b</span></p>"#,
        &HtmlOptions::default(),
    )
    .unwrap();
    assert!(
        !out.warnings
            .iter()
            .any(|w| w.contains("unsupported property: opacity")),
        "{:?}",
        out.warnings
    );
    assert!(
        out.warnings
            .iter()
            .any(|w| w.starts_with("opacity on an inline element (<span>)")),
        "{:?}",
        out.warnings
    );
}
