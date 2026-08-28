//! The web-font failure mode: skipped @font-face / @import must be LOUD
//! and specific — naming the family that falls back and the fix — and
//! provided fonts (options.fonts, the offline recipe) must register.

use forme_pdf_html::{html_to_document, render_html, FontSpec, HtmlOptions};

const FIXTURE: &str = include_str!("fixtures/webfonts.html");

#[test]
fn skipped_webfonts_warn_loud_and_specific() {
    let out = render_html(FIXTURE, &HtmlOptions::default()).expect("must render on fallback");
    assert!(out.pdf.starts_with(b"%PDF-"), "fallback render sane");
    let has = |needle: &str| out.warnings.iter().any(|w| w.contains(needle));

    // The import: actionable, names the alternative.
    assert!(
        has("external stylesheet imports are not fetched"),
        "{:?}",
        out.warnings
    );
    assert!(has("--css / options.css"), "{:?}", out.warnings);

    // The remote @font-face: names the family AND the fix.
    assert!(
        has("@font-face 'Brand Sans': remote fonts are not fetched"),
        "{:?}",
        out.warnings
    );
    assert!(has("options.fonts / --font"), "{:?}", out.warnings);

    // The local @font-face: pending, names family + path + the flag.
    assert!(
        has("@font-face 'Local Serif': local src './fonts/local-serif.ttf' is not loaded yet"),
        "{:?}",
        out.warnings
    );

    // Usage attribution: the rules referencing skipped families name them.
    assert!(
        has("font-family 'Brand Sans' references a skipped @font-face"),
        "{:?}",
        out.warnings
    );
    assert!(
        has("font-family 'Local Serif' references a skipped @font-face"),
        "{:?}",
        out.warnings
    );
}

#[test]
fn generic_families_map_to_standard_fonts() {
    let (doc, _) = html_to_document(FIXTURE, &HtmlOptions::default());
    fn find_family(nodes: &[forme::Node], text: &str) -> Option<String> {
        for n in nodes {
            if let forme::NodeKind::Text { content, runs, .. } = &n.kind {
                let t: String = if runs.is_empty() {
                    content.clone()
                } else {
                    runs.iter().map(|r| r.content.as_str()).collect()
                };
                if t.contains(text) {
                    return n.style.font_family.clone();
                }
            }
            if let Some(f) = find_family(&n.children, text) {
                return Some(f);
            }
        }
        None
    }
    // body's chain lives on the body View (paragraphs inherit through
    // the engine's cascade).
    let body_fam = doc.children[0]
        .style
        .font_family
        .clone()
        .unwrap_or_default();
    assert!(
        body_fam.contains("Helvetica"),
        "sans-serif → Helvetica in '{body_fam}'"
    );
    let fine = find_family(&doc.children, "Fine print").unwrap_or_default();
    assert!(fine.contains("Times"), "serif → Times in '{fine}'");
    // The declared-but-skipped family stays first in the chain, so a
    // future font provision changes nothing else.
    assert!(fine.starts_with("Local Serif"), "'{fine}'");
}

#[test]
fn provided_font_registers_and_silences_nothing_it_shouldnt() {
    // The offline recipe end-state: provide the TTF under the family name.
    let noto = include_bytes!("../../engine/fonts/NotoSans-Regular.ttf").to_vec();
    let options = HtmlOptions {
        fonts: vec![FontSpec::new("Brand Sans", noto)],
        ..Default::default()
    };
    let out = render_html(FIXTURE, &options).expect("render with provided font");
    assert!(out.pdf.starts_with(b"%PDF-"));
    // The font reached the engine document...
    let (doc, warnings) = html_to_document(FIXTURE, &options);
    assert_eq!(doc.fonts.len(), 1);
    assert_eq!(doc.fonts[0].family, "Brand Sans");
    assert_eq!(doc.fonts[0].weight, 400);
    // ...and the skipped-@font-face warnings still fire (the @font-face
    // WAS skipped; the provided font is what fixed it — the warning is
    // how the user found the recipe).
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("@font-face 'Brand Sans'")),
        "{warnings:?}"
    );
}
