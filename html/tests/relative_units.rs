//! `em` and `rem` on gap, border-radius and border-width.
//!
//! These four parse sites shared one shape: `if let Some(Length::Pt(v)) =
//! parse_length(p)`, with no `else` and no warning. The value parsed
//! correctly and then fell off the end of the `if`, so the declaration did
//! nothing and reported nothing. Their fields were typed `Option<f64>` —
//! already-resolved points — so a relative unit had nowhere to go, while
//! `width` and `padding` next door stored a `Length` and resolved it against
//! the element's font size later.
//!
//! The `border` shorthand was worse than a drop: its `width` local kept its
//! `medium` default, so `border: 0.5em solid red` painted 2.25pt instead of
//! 6pt — a wrong value rather than a missing one.
//!
//! Each test pins the equivalence that matters: `Xem` at a known font size
//! must equal the same measurement written in points.

use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

/// The y-gap between the first two child boxes of the flex column.
fn first_gap(out: &HtmlLayoutOutput) -> f64 {
    let mut boxes: Vec<(f64, f64)> = Vec::new();
    fn walk(el: &forme::layout::ElementInfo, into: &mut Vec<(f64, f64)>) {
        if el.style.background_color.is_some() {
            into.push((el.y, el.height));
        }
        for c in &el.children {
            walk(c, into);
        }
    }
    for page in &out.layout.pages {
        for el in &page.elements {
            walk(el, &mut boxes);
        }
    }
    boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert!(
        boxes.len() >= 2,
        "needs two painted boxes, got {}",
        boxes.len()
    );
    boxes[1].0 - (boxes[0].0 + boxes[0].1)
}

/// The widest resolved border width anywhere in the document.
fn border_width(out: &HtmlLayoutOutput) -> f64 {
    fn walk(el: &forme::layout::ElementInfo, max: &mut f64) {
        let b = &el.style.border_width;
        for w in [b.top, b.right, b.bottom, b.left] {
            if w > *max {
                *max = w;
            }
        }
        for c in &el.children {
            walk(c, max);
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

fn radius(out: &HtmlLayoutOutput) -> f64 {
    fn walk(el: &forme::layout::ElementInfo, max: &mut f64) {
        let c = &el.style.border_radius;
        for r in [c.top_left, c.top_right, c.bottom_right, c.bottom_left] {
            if r > *max {
                *max = r;
            }
        }
        for c in &el.children {
            walk(c, max);
        }
    }
    let mut r = 0.0;
    for page in &out.layout.pages {
        for el in &page.elements {
            walk(el, &mut r);
        }
    }
    r
}

const TWO_BOXES: &str = r#"<div style="display:flex; flex-direction:column; font-size:16pt; gap:GAP">
      <div style="background-color:#eee; height:20pt"></div>
      <div style="background-color:#ddd; height:20pt"></div>
    </div>"#;

#[test]
fn gap_in_rem_matches_the_same_gap_in_points() {
    // 1rem is the 12pt root font size.
    let in_rem = render(&TWO_BOXES.replace("GAP", "1rem"));
    let in_pt = render(&TWO_BOXES.replace("GAP", "12pt"));
    let (a, b) = (first_gap(&in_rem), first_gap(&in_pt));
    assert!(
        (b - 12.0).abs() < 0.5,
        "control: a 12pt gap measures 12pt ({b:.1})"
    );
    assert!(
        (a - b).abs() < 0.5,
        "gap: 1rem must equal gap: 12pt — got {a:.1}pt vs {b:.1}pt"
    );
}

#[test]
fn gap_in_em_resolves_against_the_elements_own_font_size() {
    // font-size is 16pt on the flex container, so 1em is 16pt.
    let in_em = render(&TWO_BOXES.replace("GAP", "1em"));
    let in_pt = render(&TWO_BOXES.replace("GAP", "16pt"));
    let (a, b) = (first_gap(&in_em), first_gap(&in_pt));
    assert!(
        (b - 16.0).abs() < 0.5,
        "control: a 16pt gap measures 16pt ({b:.1})"
    );
    assert!(
        (a - b).abs() < 0.5,
        "gap: 1em at font-size 16pt must equal 16pt — got {a:.1}pt vs {b:.1}pt"
    );
}

#[test]
fn border_radius_in_rem_is_not_dropped() {
    // Tailwind's `rounded-lg` is 0.5rem.
    let out = render(r#"<div style="border-radius:0.5rem; background-color:#eee">x</div>"#);
    let pt = render(r#"<div style="border-radius:6pt; background-color:#eee">x</div>"#);
    let (a, b) = (radius(&out), radius(&pt));
    assert!(b > 0.0, "control: a 6pt radius is reported ({b:.1})");
    assert!(
        (a - b).abs() < 0.5,
        "border-radius: 0.5rem must equal 6pt — got {a:.1}pt vs {b:.1}pt"
    );
}

#[test]
fn border_width_in_em_is_not_dropped() {
    let out =
        render(r#"<div style="font-size:16pt; border-width:0.25em; border-style:solid">x</div>"#);
    let pt = render(r#"<div style="font-size:16pt; border-width:4pt; border-style:solid">x</div>"#);
    let (a, b) = (border_width(&out), border_width(&pt));
    assert!(
        (b - 4.0).abs() < 0.5,
        "control: a 4pt border measures 4pt ({b:.1})"
    );
    assert!(
        (a - b).abs() < 0.5,
        "border-width: 0.25em at font-size 16pt must equal 4pt — got {a:.1}pt vs {b:.1}pt"
    );
}

#[test]
fn the_border_shorthand_honours_a_relative_width() {
    // The wrong-value case: this used to paint at the `medium` default
    // (2.25pt) rather than the 6pt asked for, silently.
    let out = render(r#"<div style="font-size:12pt; border:0.5em solid red">x</div>"#);
    let pt = render(r#"<div style="font-size:12pt; border:6pt solid red">x</div>"#);
    let (a, b) = (border_width(&out), border_width(&pt));
    assert!(
        (b - 6.0).abs() < 0.5,
        "control: a 6pt border measures 6pt ({b:.1})"
    );
    assert!(
        (a - b).abs() < 0.5,
        "border: 0.5em at font-size 12pt must equal 6pt — got {a:.1}pt vs {b:.1}pt"
    );
}
