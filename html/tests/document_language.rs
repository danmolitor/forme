//! `<html lang>` reaches the PDF's `/Lang`.
//!
//! The attribute was parsed into the DOM and never read. Three places said
//! otherwise: the `lang` option's doc comment, the TypeScript type in
//! `@formepdf/html`, and — worst — the PDF/UA warning itself, which told
//! users to set an `<html lang>` attribute that the mapper never consulted.
//! So a PDF/UA file could carry `/Lang (en)` over French content, which is a
//! false conformance claim, and the advice for fixing it did not work.
//!
//! These pin the whole precedence chain rather than just the happy path,
//! because the fix moves where the option is applied: an explicit option
//! beats the document's declaration, which beats the "en" default.

use forme_pdf_html::{render_html, HtmlOptions};

/// Extract `/Lang (...)` from PDF bytes. The catalog entry is written
/// uncompressed, so a byte scan is enough and does not need a parser.
fn lang_of(pdf: &[u8]) -> Option<String> {
    let needle = b"/Lang (";
    let start = pdf.windows(needle.len()).position(|w| w == needle)? + needle.len();
    let end = start + pdf[start..].iter().position(|&b| b == b')')?;
    Some(String::from_utf8_lossy(&pdf[start..end]).to_string())
}

fn ua_options() -> HtmlOptions {
    // No embeddable font registered: PDF/UA warns about that rather than
    // erroring, and these tests assert only about the LANGUAGE warning.
    HtmlOptions {
        pdf_ua: true,
        ..Default::default()
    }
}

#[test]
fn the_html_lang_attribute_becomes_the_document_language() {
    let html = r#"<!DOCTYPE html><html lang="fr"><head><meta charset="utf-8"><title>T</title></head>
        <body><h1>Bonjour</h1><p>Le texte est en francais.</p></body></html>"#;
    let pdf = render_html(
        html,
        &HtmlOptions {
            tagged: true,
            ..Default::default()
        },
    )
    .expect("renders")
    .pdf;
    assert_eq!(
        lang_of(&pdf).as_deref(),
        Some("fr"),
        "the document's own <html lang> reaches /Lang"
    );
}

#[test]
fn a_lang_on_body_is_honoured_too() {
    let html = r#"<!DOCTYPE html><html><head><meta charset="utf-8"></head>
        <body lang="de"><p>Hallo</p></body></html>"#;
    let pdf = render_html(
        html,
        &HtmlOptions {
            tagged: true,
            ..Default::default()
        },
    )
    .expect("renders")
    .pdf;
    assert_eq!(lang_of(&pdf).as_deref(), Some("de"));
}

#[test]
fn an_explicit_option_beats_the_documents_own_declaration() {
    // The caller knows something the markup does not. The option used to be
    // read only inside the PDF/UA branch; it now applies to every render, so
    // that honouring the attribute everywhere cannot leave the option weaker
    // than the markup it is supposed to override.
    let html = r#"<!DOCTYPE html><html lang="fr"><head><meta charset="utf-8"></head>
        <body><p>Bonjour</p></body></html>"#;
    let pdf = render_html(
        html,
        &HtmlOptions {
            tagged: true,
            lang: Some("en-GB".to_string()),
            ..Default::default()
        },
    )
    .expect("renders")
    .pdf;
    assert_eq!(lang_of(&pdf).as_deref(), Some("en-GB"));
}

#[test]
fn pdf_ua_stops_warning_about_a_language_the_document_already_declares() {
    // The regression this fixes: the warning named the exact remedy the
    // document had already applied.
    let html = r#"<!DOCTYPE html><html lang="fr"><head><meta charset="utf-8"><title>T</title></head>
        <body><h1>Bonjour</h1><p>Le texte est en francais.</p></body></html>"#;
    let out = render_html(html, &ua_options()).expect("renders");
    assert_eq!(lang_of(&out.pdf).as_deref(), Some("fr"));
    assert!(
        !out.warnings
            .iter()
            .any(|w| w.contains("no document language set")),
        "no language warning when the document declares one: {:?}",
        out.warnings
    );
}

#[test]
fn pdf_ua_still_warns_and_defaults_when_nothing_declares_a_language() {
    let html = r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>T</title></head>
        <body><h1>Hello</h1><p>Text.</p></body></html>"#;
    let out = render_html(html, &ua_options()).expect("renders");
    assert_eq!(lang_of(&out.pdf).as_deref(), Some("en"));
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("no document language set")),
        "the default is still announced: {:?}",
        out.warnings
    );
}
