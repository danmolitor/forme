//! box-sizing — fails-first pins from the certificate template's nested
//! frames (forme-landing report, 2026-09-10).
//!
//! CSS dimensions are content-box by default; the engine's `Fixed` is the
//! full border box. The mapper converts: content-box Pt widths/heights
//! (and min/max) grow by padding + border; a declared
//! `box-sizing: border-box` passes through untouched — the control that
//! proves the property is read, not just padding added everywhere. The
//! Bootstrap-era corpus templates declare border-box resets that were
//! silently ignored; for them today's output is accidentally correct and
//! must not move.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

/// (height, y) of the View elements for the nested-frame probe in DFS
/// order, SKIPPING the body wrapper (the first View): fill, outer, inner.
fn frame_boxes(out: &HtmlLayoutOutput) -> Vec<(f64, f64)> {
    let mut hs = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "View" {
                hs.push((el.height, el.y));
            }
        });
    }
    hs.drain(..1);
    hs
}

/// The certificate's exact shape: a 694.5pt fill wrapping a bordered,
/// padded outer frame (height 684 + 2x0.75 border + 2x4.5 padding =
/// 694.5 border box) wrapping a bordered, padded inner frame (height 615
/// + 2x1.5 border + 33+33 padding = 684 border box).
const CERT_FRAMES: &str = r#"<html><head><style>
  @page { size: Letter; margin: 52.5pt 63pt 45pt; }
  body { margin: 0; }
  .fill  { height: 694.5pt; }
  .outer { height: 684pt; border: 0.75pt solid #7B2233; padding: 4.5pt; }
  .inner { height: 615pt; border: 1.5pt solid #7B2233; padding: 36pt 42pt 30pt; }
</style></head><body>
  <div class="fill"><div class="outer"><div class="inner">certified</div></div></div>
</body></html>"#;

#[test]
fn content_box_heights_grow_by_padding_and_border() {
    let out = render(CERT_FRAMES);
    let hs = frame_boxes(&out);
    assert!(hs.len() >= 3, "three nested frames, got {hs:?}");
    let ((fill, _), (outer, oy), (inner, _)) = (hs[0], hs[1], hs[2]);
    assert!(
        (fill - 694.5).abs() < 0.1,
        "no padding/border: content-box equals border box: {fill}"
    );
    assert!(
        (outer - 694.5).abs() < 0.1,
        ".cert-outer border box is 684 + 9 padding + 1.5 border: {outer}"
    );
    assert!(
        (inner - 684.0).abs() < 0.1,
        ".cert-inner border box is 615 + 66 padding + 3 border: {inner}"
    );
    // The design's whole point: the frame stack fills the 694.5pt content
    // area, so the OUTER frame's border bottom lands on the bottom margin.
    // Letter is 792pt; bottom margin 45pt; layout y is top-origin.
    let bottom = oy + outer;
    assert!(
        (bottom - (792.0 - 45.0)).abs() < 0.6,
        "outer frame bottom sits on the bottom margin: {bottom}"
    );
}

#[test]
fn declared_border_box_passes_through_untouched() {
    // The Bootstrap-reset control: with border-box declared, the CSS value
    // IS the border box and nothing may move. If this fails, the property
    // is being parsed wrong (or padding added unconditionally).
    let html = CERT_FRAMES.replace(
        "body { margin: 0; }",
        "body { margin: 0; } * { box-sizing: border-box; }",
    );
    let out = render(&html);
    let hs = frame_boxes(&out);
    assert!(hs.len() >= 3, "three nested frames, got {hs:?}");
    assert!((hs[0].0 - 694.5).abs() < 0.1, "fill: {}", hs[0].0);
    assert!(
        (hs[1].0 - 684.0).abs() < 0.1,
        "outer stays 684: {}",
        hs[1].0
    );
    assert!(
        (hs[2].0 - 615.0).abs() < 0.1,
        "inner stays 615: {}",
        hs[2].0
    );
}

#[test]
fn content_box_width_grows_too() {
    // Same defect on the horizontal axis: a 200pt content-box width with
    // 10pt padding and 1pt border is a 222pt border box.
    let html = r#"<html><head><style>
      @page { size: Letter; margin: 36pt; }
      body { margin: 0; }
      .w { width: 200pt; padding: 10pt; border: 1pt solid #000; }
    </style></head><body><div class="w">wide</div></body></html>"#;
    let out = render(html);
    let mut views = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "View" {
                views.push(el.width);
            }
        });
    }
    assert!(views.len() >= 2, "body + .w views: {views:?}");
    let w = views[1];
    assert!((w - 222.0).abs() < 0.1, "222pt border box: {w}");
}

#[test]
fn min_and_max_dimensions_convert_like_the_dimensions_they_bound() {
    // max-width bounds the same box the width property sizes, so it
    // converts identically: a 200pt content-box max-width with 10pt
    // padding and 1pt border caps the border box at 222pt.
    let html = r#"<html><head><style>
      @page { size: Letter; margin: 36pt; }
      body { margin: 0; }
      .m { max-width: 200pt; padding: 10pt; border: 1pt solid #000; }
    </style></head><body><div class="m">
      a very long paragraph of certificate prose that would otherwise fill the whole measure of the page and then some
    </div></body></html>"#;
    let out = render(html);
    let mut views = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "View" {
                views.push(el.width);
            }
        });
    }
    assert!(views.len() >= 2, "body + .m views: {views:?}");
    let w = views[1];
    assert!((w - 222.0).abs() < 0.5, "max border box 222pt: {w}");
}
