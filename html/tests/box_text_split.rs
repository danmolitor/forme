//! A paragraph that also has a border, padding or a background must keep the
//! rest of its style.
//!
//! `split_box_and_text_style` was two hand-written allow-lists: eight
//! properties for the wrapping View, eight for the inner Text node, and
//! everything else in the engine's `Style` dropped on the floor. The lists
//! were written when sixteen properties were most of the struct, so every
//! property added afterwards landed in neither — while the README documents
//! all of them as supported.
//!
//! The split only happens when a block needs a wrapping box, which is what
//! made this so quiet: the SAME declaration works on a bare `<p>` and
//! disappears the moment you add a border. Each test below therefore renders
//! the property twice, with and without a border, and asserts the two agree.
//! That framing is the point — the bug is not "property X is broken", it is
//! "property X is broken only in the presence of an unrelated property".

use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

/// All text drawn in the document, in layout order.
fn text_of(out: &HtmlLayoutOutput) -> String {
    fn walk(el: &forme::layout::ElementInfo, into: &mut String) {
        if let Some(t) = &el.text_content {
            into.push_str(t);
            into.push('\n');
        }
        for child in &el.children {
            walk(child, into);
        }
    }
    let mut s = String::new();
    for page in &out.layout.pages {
        for el in &page.elements {
            walk(el, &mut s);
        }
    }
    s
}

/// Width of the widest line of text drawn.
fn widest_text(out: &HtmlLayoutOutput) -> f64 {
    fn walk(el: &forme::layout::ElementInfo, max: &mut f64) {
        if el.text_content.is_some() && el.width > *max {
            *max = el.width;
        }
        for child in &el.children {
            walk(child, max);
        }
    }
    let mut w = 0.0;
    for page in &out.layout.pages {
        for el in &page.elements {
            walk(el, &mut w);
        }
    }
    w
}

#[test]
fn text_transform_survives_a_border() {
    let bare = render(r#"<p style="text-transform:uppercase">hello there</p>"#);
    let boxed = render(
        r#"<p style="text-transform:uppercase; border-bottom:1pt solid red; padding:4pt">hello there</p>"#,
    );
    assert!(
        text_of(&bare).contains("HELLO THERE"),
        "control: uppercase works without a border"
    );
    assert!(
        text_of(&boxed).contains("HELLO THERE"),
        "a border must not cancel text-transform; got: {:?}",
        text_of(&boxed)
    );
}

#[test]
fn letter_spacing_survives_a_background() {
    let bare = render(r#"<p style="letter-spacing:4pt">wide</p>"#);
    let boxed = render(r#"<p style="letter-spacing:4pt; background-color:#eee">wide</p>"#);
    let (a, b) = (widest_text(&bare), widest_text(&boxed));
    assert!(a > 0.0 && b > 0.0, "both rendered text");
    assert!(
        (a - b).abs() < 0.5,
        "a background must not cancel letter-spacing: {a:.1}pt bare vs {b:.1}pt boxed"
    );
}

#[test]
fn max_width_survives_a_border() {
    // The reference is a <div>, NOT a bare <p>. A <p> carrying only
    // `max-width` needs no wrapping box, so it maps to a bare Text node and
    // the constraint is ignored there — measured at full width, 352pt
    // against a 120pt cap. That is a SEPARATE bug of the same family
    // (max_width mapped onto a node that never consults it) and it is not
    // fixed here; using it as the control would have made this test fail for
    // the wrong reason, which is how it was written the first time.
    let long = "the quick brown fox jumps over the lazy dog and keeps on running";
    let reference = render(&format!(r#"<div style="max-width:120pt">{long}</div>"#));
    let boxed = render(&format!(
        r#"<p style="max-width:120pt; border:1pt solid red">{long}</p>"#
    ));
    let (a, b) = (widest_text(&reference), widest_text(&boxed));
    assert!(
        a <= 121.0,
        "control: max-width constrains a block-level div ({a:.1}pt)"
    );
    assert!(
        b <= 121.0,
        "a border must not cancel max-width: {b:.1}pt wide against a 120pt cap"
    );
}

#[test]
fn a_dashed_border_stays_dashed() {
    // border_style was in neither list, so the engine defaulted to Solid and
    // every dashed or dotted rule in the HTML path painted solid.
    let out = render(r#"<p style="border:2pt dashed red; padding:4pt">boxed</p>"#);
    let mut found = None;
    fn walk(el: &forme::layout::ElementInfo, found: &mut Option<String>) {
        // The bordered box is the element whose border has width; read the
        // style the engine actually resolved for it.
        if el.style.border_width.top > 0.0 || el.style.border_width.bottom > 0.0 {
            *found = Some(format!("{:?}", el.style.border_style));
        }
        for c in &el.children {
            walk(c, found);
        }
    }
    for page in &out.layout.pages {
        for el in &page.elements {
            walk(el, &mut found);
        }
    }
    let found = found.expect("a bordered paragraph reports a border style");
    assert!(
        found.to_lowercase().contains("dash"),
        "border-style: dashed must reach the box, got {found}"
    );
}
