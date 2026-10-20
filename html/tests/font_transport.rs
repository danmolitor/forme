//! Fonts from `HtmlOptions` reach the engine as bytes.
//!
//! They used to be encoded back to base64 into `Document::fonts` for the
//! engine to decode again, keeping both copies for the whole render: about
//! 1.7 MiB of WASM memory per registered face, enough to push a Cloudflare
//! Worker over its limit with the twelve fonts-standard faces (#197).

use forme_pdf_html::{html_to_document, render_html, render_html_owned, FontSpec, HtmlOptions};

fn options_with_font() -> HtmlOptions {
    HtmlOptions {
        fonts: vec![FontSpec {
            family: "Lib".to_string(),
            data: std::fs::read("../packages/fonts-standard/fonts/LiberationSans-Regular.ttf")
                .expect("Liberation Sans in the repo"),
            weight: 400,
            italic: false,
        }],
        ..Default::default()
    }
}

const HTML: &str = r#"<p style="font-family: Lib">Hello</p>"#;

#[test]
fn option_fonts_are_not_encoded_into_the_document() {
    let (doc, _) = html_to_document(HTML, &options_with_font());
    assert!(
        doc.fonts.is_empty(),
        "option fonts are handed to the engine as bytes, not base64 in doc.fonts"
    );
}

#[test]
fn option_fonts_are_still_embedded_and_used() {
    let borrowed = render_html(HTML, &options_with_font()).unwrap().pdf;
    let owned = render_html_owned(HTML, options_with_font()).unwrap().pdf;
    assert_eq!(borrowed, owned, "both entry points render the same bytes");
    let raw = String::from_utf8_lossy(&owned);
    assert!(
        raw.contains("/FontFile2"),
        "the registered font is embedded"
    );
    assert!(raw.contains("/BaseFont /Lib"), "and used for the text");
}
