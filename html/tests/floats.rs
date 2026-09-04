//! Float support, document subset — fails-first.
//!
//! Sibling floats form rows (the shape 8 of 15 real templates use, per
//! template-compat/REPORT.md); text never wraps AROUND a float. Runs of
//! consecutive floated siblings become flex rows in the mapper. (Two
//! small engine fixes rode along: a 0.01pt flex-wrap epsilon for f32
//! percent noise, and Table arms in the intrinsic-width measurers.)
//! A4 default: content x starts at 54, content width 487.28pt.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

const W: f64 = 487.28;
const X0: f64 = 54.0;

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

/// (x, y, width) of the first TextLine containing `needle`.
fn line(out: &HtmlLayoutOutput, needle: &str) -> (f64, f64, f64) {
    let mut hit: Option<(f64, f64, f64)> = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if hit.is_none() && el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        hit = Some((el.x, el.y, el.width));
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

fn doc(body: &str) -> String {
    format!(
        "<html><head><style>body {{ margin: 0 }} div, p, h5 {{ margin: 0 }}</style></head><body>{body}</body></html>"
    )
}

// ── Side-by-side placement ─────────────────────────────────────────

#[test]
fn two_half_width_left_floats_sit_side_by_side() {
    let out = render(&doc(
        "<div style=\"float: left; width: 50%\">alpha column</div>\
         <div style=\"float: left; width: 50%\">beta column</div>",
    ));
    assert!(out.warnings.is_empty(), "in-subset: {:?}", out.warnings);
    let (ax, ay, _) = line(&out, "alpha");
    let (bx, by, _) = line(&out, "beta");
    assert_close(ax, X0, "first float at content left");
    assert_close(bx, X0 + W * 0.5, "second float at the 50% boundary");
    assert_close(ay, by, "same row (same y)");
}

#[test]
fn right_floats_stack_right_to_left() {
    // The subtle CSS rule with its own pin: successive float:right
    // elements stack RIGHT-TO-LEFT — the first in markup ends up
    // rightmost.
    let out = render(&doc(
        "<div style=\"float: right; width: 30%\">first-markup</div>\
         <div style=\"float: right; width: 30%\">second-markup</div>",
    ));
    let (fx, fy, _) = line(&out, "first-markup");
    let (sx, sy, _) = line(&out, "second-markup");
    assert_close(fx, X0 + W * 0.7, "first in markup sits RIGHTMOST");
    assert_close(sx, X0 + W * 0.4, "second in markup sits to its left");
    assert_close(fy, sy, "same row");
}

#[test]
fn left_right_pair_splits_the_row() {
    // The corpus's dominant shape (crater 30%/35%, invoiceplane 55%/40%).
    let out = render(&doc("<div style=\"float: left; width: 30%\">leftcol</div>\
         <div style=\"float: right; width: 35%\">rightcol</div>"));
    let (lx, ly, _) = line(&out, "leftcol");
    let (rx, ry, _) = line(&out, "rightcol");
    assert_close(lx, X0, "left column at content left");
    assert_close(rx, X0 + W * 0.65, "right column flush to the right edge");
    assert_close(ly, ry, "one row");
}

#[test]
fn lone_float_right_is_right_aligned() {
    let out = render(&doc(
        "<div style=\"float: right; width: 40%\">pulled right</div>",
    ));
    let (x, _, _) = line(&out, "pulled right");
    assert_close(x, X0 + W * 0.6, "float:right block starts at 60%");
}

#[test]
fn overwide_run_wraps_like_float_lines() {
    // 100% + 60% + 40%: CSS drops the second float to a new line and the
    // third sits beside it. flex-wrap gives the same shape.
    let out = render(&doc(
        "<div style=\"float: left; width: 100%\">full row</div>\
         <div style=\"float: left; width: 60%\">sixty</div>\
         <div style=\"float: left; width: 40%\">forty</div>",
    ));
    let (_, fy, _) = line(&out, "full row");
    let (sx, sy, _) = line(&out, "sixty");
    let (qx, qy, _) = line(&out, "forty");
    assert!(sy > fy, "second line drops below the full-width float");
    assert_close(sx, X0, "wrapped line starts at content left");
    assert_close(qx, X0 + W * 0.6, "forty sits beside sixty");
    assert_close(sy, qy, "sixty and forty share the wrapped line");
}

// ── Clear + auto width ─────────────────────────────────────────────

#[test]
fn clear_places_content_below_the_row() {
    let out = render(&doc("<div style=\"float: left; width: 50%\">floated</div>\
         <div style=\"clear: both\">cleared content</div>"));
    assert!(
        out.warnings.is_empty(),
        "clear is in-subset now: {:?}",
        out.warnings
    );
    let (_, fy, _) = line(&out, "floated");
    let (cx, cy, _) = line(&out, "cleared content");
    assert!(cy > fy, "cleared content starts below the float row");
    assert_close(cx, X0, "cleared content back at content left");
}

#[test]
fn auto_width_pair_shares_a_baseline() {
    // The ERPNext statement shape: <h5 float:left> + <h5 float:right>,
    // no widths — shrink-to-fit on both, one row.
    let out = render(&doc("<h5 style=\"float: left\">Customer: Johnson</h5>\
         <h5 style=\"float: right\">Date: 01-01-2026</h5>"));
    let (lx, ly, _) = line(&out, "Customer");
    let (rx, ry, rw) = line(&out, "Date:");
    assert_close(ly, ry, "left and right share the row");
    assert_close(lx, X0, "left at content left");
    assert_close(rx + rw, X0 + W, "right text ends flush at the right edge");
}

// ── The honest boundary ────────────────────────────────────────────

#[test]
fn uncleared_following_content_warns_and_renders_below() {
    // The true text-wrap case: a non-floated sibling after an uncleared
    // float would flow BESIDE it in a browser. We place it below and say
    // so — this warning must never go silent.
    let out = render(&doc(
        "<div style=\"float: left; width: 40%\">floated block</div>\
         <p>following paragraph that a browser would wrap alongside</p>",
    ));
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("text wrapping alongside floats is not supported")),
        "the residual case must warn: {:?}",
        out.warnings
    );
    let (_, fy, _) = line(&out, "floated block");
    let (_, py, _) = line(&out, "following paragraph");
    assert!(py > fy, "following content placed below, not overlapped");
}

#[test]
fn float_on_flex_items_is_ignored_silently() {
    // CSS ignores float on flex items; so do we, with no warning.
    let out = render(&doc("<div style=\"display: flex\">\
           <div style=\"float: right; width: 30%\">flex child a</div>\
           <div style=\"width: 30%\">flex child b</div>\
         </div>"));
    assert!(
        !out.warnings.iter().any(|w| w.contains("float")),
        "no float warning inside flex: {:?}",
        out.warnings
    );
    let (ax, _, _) = line(&out, "flex child a");
    let (bx, _, _) = line(&out, "flex child b");
    assert!(
        ax < bx,
        "flex order preserved — float:right did not reorder"
    );
}

#[test]
fn floats_inside_table_cells_form_rows() {
    let out = render(&doc("<table style=\"width: 100%\"><tr><td>\
           <div style=\"float: left; width: 50%\">cell left</div>\
           <div style=\"float: right; width: 40%\">cell right</div>\
         </td></tr></table>"));
    let (ly, ry) = (line(&out, "cell left").1, line(&out, "cell right").1);
    assert_close(ly, ry, "floats inside a cell share a row");
}
