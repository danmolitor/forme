//! The whitespace/fixed-dimensions family — fails-first.
//!
//! Template-compat follow-up (see REPORT.md): the corpus gaps traced to
//! image sizing that measure and layout computed differently, page-sized
//! body declarations overflowing the content box, and blank pages
//! emitted before unfittable atomic table rows. Chrome is the reference
//! for image sizing: percent width honored, max-width clamped, aspect
//! preserved.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

/// 1x1 transparent PNG (the corpus placeholder-logo shape: tiny
/// intrinsic size, styled large).
const PX: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

fn find<'a>(out: &'a HtmlLayoutOutput, node_type: &str) -> Vec<&'a ElementInfo> {
    let mut hits = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == node_type {
                hits.push(el);
            }
        });
    }
    hits
}

fn line_y(out: &HtmlLayoutOutput, needle: &str) -> f64 {
    let mut hit = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if hit.is_none() && el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        hit = Some(el.y);
                    }
                }
            }
        });
    }
    hit.unwrap_or_else(|| panic!("no text line containing {needle:?}"))
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1.0,
        "{what}: expected ~{expected:.2}, got {actual:.2}"
    );
}

// ── Image sizing: the Chrome ladder ────────────────────────────────

#[test]
fn percent_width_with_max_width_sizes_like_chrome() {
    // The sparksuite logo shape: a small image styled
    // `width:100%; max-width:300px`. Chrome: width = min(container,
    // 225pt), height follows the 1:1 aspect → a 225pt square. We
    // previously drew it at intrinsic size (1pt) while MEASURING it at
    // container width — a ~474pt phantom gap (template-compat 01's
    // broken page).
    let out = render(&format!(
        "<html><head><style>body,div,p{{margin:0}}</style></head><body>\
         <div><img src=\"{PX}\" style=\"width: 100%; max-width: 300px\"></div>\
         <p>after the logo</p></body></html>"
    ));
    let images = find(&out, "Image");
    assert_eq!(images.len(), 1);
    assert_close(images[0].width, 225.0, "max-width clamps (300px = 225pt)");
    assert_close(images[0].height, 225.0, "height follows the 1:1 aspect");
    assert_close(
        line_y(&out, "after the logo"),
        54.0 + 225.0,
        "following content sits directly below the image — no phantom",
    );
}

#[test]
fn percent_width_image_fills_its_container() {
    // width:50% on A4 default: content box is 487.28pt wide.
    let out = render(&format!(
        "<html><head><style>body,div{{margin:0}}</style></head><body>\
         <img src=\"{PX}\" style=\"width: 50%\"></body></html>"
    ));
    let images = find(&out, "Image");
    assert_close(images[0].width, 487.28 * 0.5, "percent width honored");
    assert_close(images[0].height, 487.28 * 0.5, "aspect preserved");
}

#[test]
fn explicit_height_derives_width_from_aspect() {
    let out = render(&format!(
        "<html><head><style>body{{margin:0}}</style></head><body>\
         <img src=\"{PX}\" style=\"height: 60pt\"></body></html>"
    ));
    let images = find(&out, "Image");
    assert_close(images[0].height, 60.0, "explicit height");
    assert_close(images[0].width, 60.0, "width from the 1:1 aspect");
}

#[test]
fn unstyled_small_image_keeps_intrinsic_size_without_phantom() {
    // No styles at all: intrinsic size (tiny), and the following text
    // sits right below — measure must agree with layout here too.
    let out = render(&format!(
        "<html><head><style>body,p{{margin:0}}</style></head><body>\
         <img src=\"{PX}\"><p>after</p></body></html>"
    ));
    let images = find(&out, "Image");
    assert!(
        images[0].height <= 2.0,
        "intrinsic 1px image stays tiny, got {}",
        images[0].height
    );
    assert!(
        line_y(&out, "after") < 54.0 + 30.0,
        "no phantom gap after an unstyled small image"
    );
}

// ── Page-sized body declarations ───────────────────────────────────

#[test]
fn page_sized_body_is_clamped_and_warns() {
    // The mPDF idiom: body{width:21cm;height:29.7cm} means "I am the
    // page", not content sizing. 595pt > the 487pt content box —
    // honoring it cut off the right edge and forced a blank first page.
    // Clamp to the content box and say so.
    let out = render(
        "<html><head><style>body { width: 21cm; height: 29.7cm; margin: 0 }</style></head>\
         <body><p style=\"margin:0\">clamped body content</p></body></html>",
    );
    assert_eq!(out.layout.pages.len(), 1, "no forced overflow page");
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("body width") && w.contains("@page")),
        "the clamp must warn and name the remedy: {:?}",
        out.warnings
    );
    // Content starts at the top of the content box, not after a
    // page-height band.
    assert_close(line_y(&out, "clamped body content"), 54.0, "content at top");
}

#[test]
fn body_dimensions_within_the_content_box_are_honored() {
    // A body narrower than the content box is legitimate sizing — no
    // clamp, no warning.
    let out = render(
        "<html><head><style>body { width: 400pt; margin: 0 }</style></head>\
         <body><p style=\"margin:0\">narrow body</p></body></html>",
    );
    assert!(
        !out.warnings.iter().any(|w| w.contains("body width")),
        "in-box body width must not warn: {:?}",
        out.warnings
    );
}

// ── Unfittable atomic rows ─────────────────────────────────────────

#[test]
fn giant_atomic_row_emits_no_blank_pages() {
    // The email-template idiom: everything inside one <tr>. The row is
    // atomic by engine design (placed whole, overflows) — but it must
    // not ALSO emit empty pages before itself (template-compat 11
    // produced two entirely empty pages).
    let mut cells = String::new();
    for i in 0..120 {
        cells.push_str(&format!("line {i}<br>"));
    }
    let out = render(&format!(
        "<html><head><style>body,td{{margin:0;padding:0}}</style></head><body>\
         <table><tr><td>{cells}</td></tr></table></body></html>"
    ));
    for (i, page) in out.layout.pages.iter().enumerate() {
        let mut text_lines = 0;
        walk(&page.elements, &mut |el| {
            if el.node_type == "TextLine" {
                text_lines += 1;
            }
        });
        assert!(
            text_lines > 0,
            "page {} of {} is blank (giant atomic row emitted an empty page)",
            i + 1,
            out.layout.pages.len()
        );
    }
}
