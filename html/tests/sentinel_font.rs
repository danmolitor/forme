//! Page counters in margin boxes must not change the font.
//!
//! `counter(page)` compiles to the engine's `{{pageNumber}}` sentinel.
//! With a provided font (`options.fonts` / `--font`), the sentinel used
//! to fall through coverage-based font fallback into base-14 Helvetica —
//! splitting the running footer across two typefaces and breaking
//! PDF/A eligibility. Same engine bug as the JSX path; this pins the
//! HTML shape (margin box + provided font), which is the common one.

use forme_pdf_html::{render_html, FontSpec, HtmlOptions};

const NOTO: &[u8] = include_bytes!("../../engine/fonts/NotoSans-Regular.ttf");

fn options_with_test_font() -> HtmlOptions {
    HtmlOptions {
        fonts: vec![FontSpec {
            family: "TestSans".to_string(),
            data: NOTO.to_vec(),
            weight: 400,
            italic: false,
        }],
        ..Default::default()
    }
}

fn base_fonts(pdf: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(pdf);
    let mut fonts: Vec<String> = Vec::new();
    for m in text.split("/BaseFont /").skip(1) {
        let name: String = m
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '+' || *c == ',')
            .collect();
        if !fonts.contains(&name) {
            fonts.push(name);
        }
    }
    fonts
}

#[test]
fn margin_box_page_counter_stays_in_the_provided_font() {
    let html = "<html><head><style>\
         @page { size: A4; margin: 50pt; \
                 @bottom-center { content: \"Page \" counter(page) \" of \" counter(pages); \
                                  font-family: TestSans; font-size: 9pt } } \
         body { font-family: TestSans; margin: 0 } p { margin: 0 0 600pt 0 }\
         </style></head><body><p>first page</p><p>second page</p></body></html>";
    let out = render_html(html, &options_with_test_font()).expect("renders");
    assert!(out.warnings.is_empty(), "in-subset: {:?}", out.warnings);
    let fonts = base_fonts(&out.pdf);
    assert!(
        !fonts.iter().any(|f| f.contains("Helvetica")),
        "counter(page) digits must use the margin box's provided font; got {fonts:?}"
    );
    assert!(
        fonts.iter().any(|f| f.contains("TestSans")),
        "the provided font must be present; got {fonts:?}"
    );
}
