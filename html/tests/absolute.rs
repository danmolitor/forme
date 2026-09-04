//! Absolute positioning — fails-first pins from the template corpus.

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

fn line(out: &HtmlLayoutOutput, needle: &str) -> (f64, f64) {
    let mut hit = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if hit.is_none() && el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        hit = Some((el.y, el.height));
                    }
                }
            }
        });
    }
    hit.unwrap_or_else(|| panic!("no text line containing {needle:?}"))
}

#[test]
fn bottom_anchored_margin_box_stays_on_the_page() {
    // The template-compat 15 shape: A5 landscape (content height 419.5pt
    // with zero vertical @page margins), a footer with
    // `position:absolute; bottom:0` AND `margin-top:1rem`. Per CSS,
    // `bottom` positions the MARGIN edge — the margin must not shove the
    // content past the anchor. The bug rendered the footer 12pt below
    // the page bottom, leaving only ascender tips visible ("Thk ft" —
    // the corpus's one silent text defect).
    let out = render(
        "<html><head><style>\
         @page { size: A5 landscape; margin-top: 0; margin-bottom: 0 }\
         body { margin: 0 }\
         </style></head><body>\
         <p style=\"margin:0\">content</p>\
         <div style=\"position:absolute; bottom:0; width:100%; margin-top:12pt\">\
           <p style=\"margin:0\">Thank you for your payment.</p>\
         </div></body></html>",
    );
    let content_h = 419.53; // A5 landscape height, zero vertical margins
    let (y, h) = line(&out, "Thank you");
    assert!(
        y + h <= content_h + 0.5,
        "footer bottom {:.1} must not pass the content bottom {content_h}",
        y + h
    );
    // The margin-box bottom sits at the anchor, so the text bottom sits
    // exactly at the content bottom (zero margin-bottom).
    assert!(
        (y + h - content_h).abs() < 1.5,
        "footer should sit flush at the anchor: bottom {:.1} vs {content_h}",
        y + h
    );
}

#[test]
fn right_anchored_margin_box_respects_its_margin() {
    // Horizontal twin: `right: 0` positions the margin edge, so a
    // margin-left must not shove the box past the right edge.
    let out = render(
        "<html><head><style>body { margin: 0 }</style></head><body>\
         <div style=\"position:relative; width: 400pt; height: 100pt\">\
           <div style=\"position:absolute; right:0; top:0; width:100pt; margin-left:20pt\">\
             <p style=\"margin:0\">badge</p>\
           </div>\
         </div></body></html>",
    );
    let mut badge = None;
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if badge.is_none() && el.node_type == "TextLine" {
                if let Some(t) = &el.text_content {
                    if t.contains("badge") {
                        badge = Some((el.x, el.width));
                    }
                }
            }
        });
    }
    let (x, _) = badge.expect("badge line");
    // Container starts at content x = 54 (A4 default margins), width
    // 400: right edge at 454. A 100pt box anchored right ends at 454,
    // so its text starts at 354 — the margin-left must not push it to
    // 374 (past the anchor).
    assert!(
        x <= 354.5,
        "right-anchored box must end at the anchor; text starts at {x:.1}, expected ~354"
    );
}
