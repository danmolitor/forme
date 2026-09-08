//! Margin-box borders and backgrounds must actually paint. The declared
//! style used to land on the band's TEXT node, and the engine paints
//! borders only on views — the running header's hairline rule silently
//! vanished (Northmoor batch 1 shipped without it).

use forme_pdf_html::{render_html_with_layout, HtmlOptions};

#[test]
fn margin_box_border_bottom_paints_on_the_band_cell() {
    let out = render_html_with_layout(
        "<html><head><style>@page { size: Letter; margin: 72pt;
           @top-left { content: \"L\"; border-bottom: 1pt solid #111111 }
           @top-center { content: \"C\"; border-bottom: 1pt solid #111111 }
           @top-right { content: \"R\"; border-bottom: 1pt solid #111111 }
         }</style></head><body><p>x</p></body></html>",
        &HtmlOptions::default(),
    )
    .expect("render");
    fn bordered(els: &[forme::layout::ElementInfo]) -> usize {
        els.iter()
            .map(|e| {
                let own = (e.style.border_width.bottom > 0.0) as usize;
                own + bordered(&e.children)
            })
            .sum()
    }
    let n = bordered(&out.layout.pages[0].elements);
    assert!(
        n >= 3,
        "all three top margin boxes must carry a painted border-bottom, found {n}"
    );
}
