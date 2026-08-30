//! Regression tests distilled from the ironpress parity corpus
//! (<https://github.com/gastongouron/ironpress>, MIT licensed). Each test is a
//! minimal, hand-reduced repro of a divergence that corpus surfaced against a
//! Forme-claimed feature — NOT a copy of an upstream fixture. The upstream
//! `.html` cases and their oracle PDFs stay in that repo under MIT; only the
//! observed behaviors are re-expressed here as engine-level assertions.

use forme::layout::ElementInfo;
use forme::style::Color;
use forme::Node;
use forme_pdf_html::{html_to_document, render_html_with_layout, HtmlOptions};

fn collect<'a>(els: &'a [ElementInfo], out: &mut Vec<&'a ElementInfo>) {
    for e in els {
        out.push(e);
        collect(&e.children, out);
    }
}

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

/// Finding B (bounded) — with cell `height` honored as a minimum, the
/// just-shipped `vertical-align: top/middle/bottom` on table cells has slack
/// to act on, so a marker lands at the top, centre, and bottom of three
/// equal-height cells respectively. Fails-first: before the cell-height fix
/// the cells collapse to marker height and all three markers sit at the top.
///
/// Reduced from ironpress `tables/tables-cell-vertical-align`.
#[test]
fn table_cell_vertical_align_positions_markers() {
    let html = r#"<!DOCTYPE html><html><head><style>
      @page { size: 520px 200px; margin: 0; }
      table { border-collapse: collapse; }
      td { width: 120px; height: 120px; border: 3px solid #2f3e46; background: #cad2c5; }
      .marker { width: 60px; height: 24px; background: #52796f; }
      .top { vertical-align: top; }
      .middle { vertical-align: middle; }
      .bottom { vertical-align: bottom; }
    </style></head><body>
      <table><tr>
        <td class="top"><div class="marker"></div></td>
        <td class="middle"><div class="marker"></div></td>
        <td class="bottom"><div class="marker"></div></td>
      </tr></table>
    </body></html>"#;
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("render");
    let mut all = Vec::new();
    for page in &out.layout.pages {
        collect(&page.elements, &mut all);
    }

    // Cells and their markers, ordered left→right = top/middle/bottom.
    let mut cells: Vec<&ElementInfo> = all.iter().copied().filter(|e| e.node_type == "TableCell").collect();
    let mut markers: Vec<&ElementInfo> = all
        .iter()
        .copied()
        .filter(|e| e.node_type == "View" && e.height > 15.0 && e.height < 22.0)
        .collect();
    cells.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    markers.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    assert_eq!(cells.len(), 3, "expected 3 cells");
    assert_eq!(markers.len(), 3, "expected 3 markers (18pt each)");

    let (top, mid, bot) = (markers[0], markers[1], markers[2]);
    // Distinct, correctly ordered vertical positions.
    assert!(
        top.y < mid.y && mid.y < bot.y,
        "markers must stack top < middle < bottom; got {:.2}, {:.2}, {:.2}",
        top.y, mid.y, bot.y
    );
    // Middle marker centred in its cell (±0.01pt).
    let marker_centre = mid.y + mid.height / 2.0;
    let cell_centre = cells[1].y + cells[1].height / 2.0;
    assert!(
        (marker_centre - cell_centre).abs() < 0.01,
        "middle marker must be centred: marker centre {:.4} vs cell centre {:.4}",
        marker_centre, cell_centre
    );
    // Top-aligned top-gap equals bottom-aligned bottom-gap (both == the border),
    // proving symmetric top/bottom placement without hard-coding the border.
    let top_gap = top.y - cells[0].y;
    let bottom_gap = (cells[2].y + cells[2].height) - (bot.y + bot.height);
    assert!(
        (top_gap - bottom_gap).abs() < 0.01,
        "top gap {:.4} must equal bottom gap {:.4}",
        top_gap, bottom_gap
    );
}
