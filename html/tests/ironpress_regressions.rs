//! Regression tests distilled from the ironpress parity corpus
//! (<https://github.com/gastongouron/ironpress>, MIT licensed). Each test is a
//! minimal, hand-reduced repro of a divergence that corpus surfaced against a
//! Forme-claimed feature — NOT a copy of an upstream fixture. The upstream
//! `.html` cases and their oracle PDFs stay in that repo under MIT; only the
//! observed behaviors are re-expressed here as engine-level assertions.

use forme::style::Color;
use forme::Node;
use forme_pdf_html::{html_to_document, render_html_with_layout, HtmlOptions};

fn color_hex(c: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8
    )
}

fn any_node(nodes: &[Node], pred: &impl Fn(&Node) -> bool) -> bool {
    nodes
        .iter()
        .any(|n| pred(n) || any_node(&n.children, pred))
}

/// Finding C — a forced `break-before: page` on the *first* in-flow block has
/// nothing to break from, so it is suppressed at the start of the
/// fragmentation context (exactly as Chrome's print path does) and a single
/// page renders. Migrated wkhtmltopdf-era templates put `page-break-before` on
/// every section including the first; without this suppression the document
/// opens with an embarrassing blank page.
///
/// Reduced from ironpress `paged-media/paged-break-before-page-modern`.
#[test]
fn break_before_page_on_first_block_is_suppressed() {
    let html = r#"<!DOCTYPE html><html><head><style>
      @page { size: 300px 400px; margin: 0; }
      .block { width: 200px; height: 130px; border: 2px solid #0b3d2e; }
      .a { background: #06d6a0; break-before: page; }
      .b { background: #118ab2; }
    </style></head><body>
      <div class="block a"></div>
      <div class="block b"></div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("render");
    assert_eq!(
        out.layout.pages.len(),
        1,
        "break-before:page on the first block must be suppressed — no leading blank page"
    );
}

/// Counter-test to Finding C's fix: a `break-before: page` on a *later* block
/// (with real content before it) must still force a new page. Guards against
/// over-suppression. Reduced from ironpress
/// `paged-media/break-before-page-modern-real`.
#[test]
fn break_before_page_after_content_still_breaks() {
    let html = r#"<!DOCTYPE html><html><head><style>
      @page { size: 300px 400px; margin: 0; }
      .block { width: 200px; height: 130px; border: 2px solid #0b3d2e; }
      .b { background: #118ab2; break-before: page; }
    </style></head><body>
      <div class="block"></div>
      <div class="block b"></div>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("render");
    assert_eq!(
        out.layout.pages.len(),
        2,
        "break-before:page after real content must still force a second page"
    );
}

/// Finding A — an empty `<p>` that carries paint (background / border /
/// padding) must still render its box. A browser paints it, and silently
/// dropping a styled element violates the render contract ("every render
/// either looks right or tells you why not"). Empty `<p>` tags are ubiquitous
/// in WYSIWYG and CMS exports.
///
/// Reduced from ironpress `selectors-cascade/selectors-cascade-type-selector`,
/// where the green target is an empty `<p class="box">`.
#[test]
fn empty_styled_paragraph_renders_its_box() {
    let html = r#"<!DOCTYPE html><html><head><style>
      .box { display: block; width: 100px; height: 60px; background: #bdbdbd; border: 2px solid #113355; }
      p.box { background: #2e9e4f; }
    </style></head><body>
      <div class="box"></div>
      <p class="box"></p>
      <div class="box"></div>
    </body></html>"#;
    let (doc, _warnings) = html_to_document(html, &HtmlOptions::default());
    assert!(
        any_node(&doc.children, &|n| n
            .style
            .background_color
            .map(|c| color_hex(c) == "#2e9e4f")
            .unwrap_or(false)),
        "empty <p class=box> with a green background must render a box — it was silently dropped"
    );
}
