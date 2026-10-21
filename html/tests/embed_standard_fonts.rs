//! `embed_standard_fonts`: the base-14 fonts are embedded as their
//! metric-compatible substitutes with no conformance claim (#197).

use forme_pdf_html::{render_html, FontSpec, HtmlOptions};

const HTML: &str =
    "<html><body><p><b>Invoice INV-0042</b></p><p>Total due: $1,250.00</p></body></html>";

fn options(embed: bool, substitutes: bool) -> HtmlOptions {
    let face = |file: &str, weight: u32| FontSpec {
        family: "Liberation Sans".to_string(),
        data: std::fs::read(format!("../packages/fonts-standard/fonts/{file}"))
            .expect("Liberation Sans in the repo"),
        weight,
        italic: false,
    };
    HtmlOptions {
        fonts: if substitutes {
            vec![
                face("LiberationSans-Regular.ttf", 400),
                face("LiberationSans-Bold.ttf", 700),
            ]
        } else {
            vec![]
        },
        embed_standard_fonts: embed,
        ..Default::default()
    }
}

#[test]
fn embeds_regular_and_bold_without_a_claim() {
    let out = render_html(HTML, &options(true, true)).unwrap();
    let raw = String::from_utf8_lossy(&out.pdf);
    assert_eq!(
        raw.matches("/FontFile2").count(),
        2,
        "regular and bold embedded"
    );
    for claim in ["pdfuaid", "pdfaid", "/OutputIntents"] {
        assert!(!raw.contains(claim), "no conformance claim: found {claim}");
    }
}

#[test]
fn off_by_default() {
    let out = render_html(HTML, &options(false, true)).unwrap();
    assert!(!String::from_utf8_lossy(&out.pdf).contains("/FontFile2"));
}

#[test]
fn warns_when_the_substitutes_are_not_registered() {
    let out = render_html(HTML, &options(true, false)).unwrap();
    assert!(
        out.warnings
            .iter()
            .any(|w| w.starts_with("embedStandardFonts:")),
        "{:?}",
        out.warnings
    );
}
