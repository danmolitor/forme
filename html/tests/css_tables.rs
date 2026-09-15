//! `display: table` / `table-cell` on divs — the pre-flexbox
//! equal-height-columns idiom (template-compat 09: a dark sidebar cell
//! beside a taller content cell, both full height). Mapped onto the
//! engine's native table machinery.

use forme::layout::ElementInfo;
use forme_pdf_html::{html_to_document, render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

fn cells(out: &HtmlLayoutOutput) -> Vec<(f64, f64, f64, f64)> {
    let mut v = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "TableCell" {
                v.push((el.x, el.y, el.width, el.height));
            }
        });
    }
    v
}

const EQUAL_HEIGHT: &str = "<html><head><style>\
    .row-equal { display: table }\
    .row-equal .col-equal { float: none; display: table-cell; vertical-align: top }\
    .col-4 { width: 33.33333333% }\
    .col-8 { width: 66.66666667% }\
    .data { background-color: #222d32 }\
    </style></head><body>\
    <div class=\"row row-equal\">\
      <div class=\"col-4 data col-equal\"><p>short sidebar</p></div>\
      <div class=\"col-8 col-equal\"><p>line one</p><p>line two</p><p>line three</p>\
        <p>line four</p><p>line five</p></div>\
    </div></body></html>";

#[test]
fn equal_height_columns_share_row_height() {
    // The whole point of the idiom: the short dark cell is as tall as
    // its tall neighbor, and they sit side by side. A single-row CSS
    // table maps to a flex row (breakable — template-compat 09 wraps
    // its whole document in one such row), so the columns land as Views
    // whose widths follow the 1/3 : 2/3 cell definitions.
    let out = render(EQUAL_HEIGHT);
    let mut cols: Vec<(f64, f64, f64, f64)> = Vec::new();
    for page in &out.layout.pages {
        walk(&page.elements, &mut |el| {
            if el.node_type == "View" && el.width > 100.0 && el.width < 400.0 {
                cols.push((el.x, el.y, el.width, el.height));
            }
        });
    }
    assert_eq!(cols.len(), 2, "two column views: {cols:?}");
    let (a, b) = if cols[0].0 < cols[1].0 {
        (cols[0], cols[1])
    } else {
        (cols[1], cols[0])
    };
    assert!(
        (a.1 - b.1).abs() < 0.01,
        "columns share a top edge: {cols:?}"
    );
    assert!(
        (a.3 - b.3).abs() < 0.01,
        "EQUAL heights (the idiom): {cols:?}"
    );
    // Column widths follow the harvested 1/3 : 2/3 definitions.
    let ratio = b.2 / a.2;
    assert!((ratio - 2.0).abs() < 0.1, "1/3 : 2/3 columns, got {ratio}");
}

#[test]
fn table_row_wrappers_and_row_groups_are_rows() {
    let out = render(
        "<html><head><style>\
        .t { display: table } .rg { display: table-row-group }\
        .r { display: table-row } .c { display: table-cell }\
        </style></head><body>\
        <div class=\"t\"><div class=\"rg\">\
          <div class=\"r\"><div class=\"c\"><p>a1</p></div><div class=\"c\"><p>a2</p></div></div>\
          <div class=\"r\"><div class=\"c\"><p>b1</p></div><div class=\"c\"><p>b2</p></div></div>\
        </div></div></body></html>",
    );
    let cs = cells(&out);
    assert_eq!(cs.len(), 4, "2x2 cells: {cs:?}");
    assert!((cs[0].1 - cs[1].1).abs() < 0.01, "row 1 aligned");
    assert!((cs[2].1 - cs[3].1).abs() < 0.01, "row 2 aligned");
    assert!(cs[2].1 > cs[0].1, "second row below first");
}

#[test]
fn non_cell_run_becomes_an_anonymous_single_cell_row() {
    let (doc, warnings) = html_to_document(
        "<html><head><style>.t { display: table } .c { display: table-cell }</style></head>\
         <body><div class=\"t\">\
           <p>loose caption</p>\
           <div class=\"c\"><p>x</p></div><div class=\"c\"><p>y</p></div>\
         </div></body></html>",
        &HtmlOptions::default(),
    );
    use forme::NodeKind;
    fn find_table(nodes: &[forme::Node]) -> Option<&forme::Node> {
        nodes.iter().find_map(|n| {
            if matches!(n.kind, NodeKind::Table { .. }) {
                Some(n)
            } else {
                find_table(&n.children)
            }
        })
    }
    let table = find_table(&doc.children).expect("table node");
    assert_eq!(table.children.len(), 2, "anonymous row + cell row");
    assert_eq!(table.children[0].children.len(), 1, "one anonymous cell");
    assert_eq!(table.children[1].children.len(), 2, "two real cells");
    assert!(
        !warnings.iter().any(|w| w.contains("display")),
        "no display warnings: {warnings:?}"
    );
}

#[test]
fn orphan_table_cell_warns_and_renders_as_block() {
    let (doc, warnings) = html_to_document(
        "<html><body><div style=\"display: table-cell\"><p>orphan</p></div></body></html>",
        &HtmlOptions::default(),
    );
    assert!(!doc.children.is_empty());
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("outside a display: table parent is treated as block")),
        "{warnings:?}"
    );
}

#[test]
fn bootstrap_print_display_values_no_longer_warn() {
    // Bootstrap's print stylesheet says thead { display: table-header-group }
    // and td { display: table-cell } on REAL table elements — that must
    // not produce 'unsupported display value' noise (tag-driven mapping
    // wins for real table markup).
    let (_, warnings) = html_to_document(
        "<html><head><style>\
         thead { display: table-header-group } td, th { display: table-cell }\
         tr { display: table-row } table { display: table }\
         </style></head><body>\
         <table><thead><tr><th>h</th></tr></thead><tbody><tr><td>d</td></tr></tbody></table>\
         </body></html>",
        &HtmlOptions::default(),
    );
    assert!(
        !warnings.iter().any(|w| w.contains("unsupported display")),
        "{warnings:?}"
    );
}

#[test]
fn page_tall_column_row_continues_as_parallel_columns() {
    // 09's real shape: the single equal-height row is taller than a page.
    // It used to serialize — the wide column took the following pages to
    // itself and the sidebar was displaced past it — and the engine could
    // only report that through the render-defect channel. It now
    // fragments: each column continues on the next page at its own x, so
    // the row's columns appear TOGETHER on every page it spans, and there
    // is no defect left to report.
    let tall: String = (0..120)
        .map(|i| format!("<p>content line {i}</p>"))
        .collect();
    let html = format!(
        "<html><head><style>\
         .t {{ display: table }} .c {{ display: table-cell }}\
         .a {{ width: 33% }} .b {{ width: 67% }}\
         </style></head><body><div class=\"t\">\
         <div class=\"c a\"><p>sidebar</p></div>\
         <div class=\"c b\">{tall}</div>\
         </div></body></html>"
    );
    let out = render(&html);
    assert!(out.layout.pages.len() > 1, "the row must actually split");
    assert!(
        !out.warnings.iter().any(|w| w.contains("sequentially")),
        "a fragmented row is not a sequential split: {:?}",
        out.warnings
    );

    // The proof the columns are parallel rather than serialized: the wide
    // column's text keeps its own x on the continuation page, and it is
    // not the sidebar's x.
    let xs = |page: usize| -> Vec<f64> {
        let mut xs = Vec::new();
        fn walk(els: &[forme::layout::ElementInfo], xs: &mut Vec<f64>) {
            for e in els {
                if e.node_type == "TextLine" {
                    xs.push(e.x.round());
                }
                walk(&e.children, xs);
            }
        }
        walk(&out.layout.pages[page].elements, &mut xs);
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup();
        xs
    };
    // Page-relative on purpose: a row taller than the remaining space
    // still relocates whole today (that is the next phase), so the row may
    // begin on page 2. What this pins is that wherever it begins, both
    // columns are there, and the next page carries them both on too.
    let start = (0..out.layout.pages.len())
        .find(|&i| xs(i).len() >= 2)
        .expect("some page carries both columns side by side");
    let at_start = xs(start);
    let sidebar_x = at_start[0];
    let wide_x = *at_start.last().unwrap();
    assert!(
        wide_x > sidebar_x,
        "the two columns occupy different x: {at_start:?}"
    );
    assert!(
        start + 1 < out.layout.pages.len(),
        "the row spans past its first page"
    );
    assert!(
        xs(start + 1).contains(&wide_x),
        "the wide column continues at its own x on the next page: {:?}",
        xs(start + 1)
    );
}

#[test]
fn row_that_relocates_whole_does_not_report_sequential_split() {
    // The false positive that closed a correct PR: a short flex row pushed
    // past a page boundary moves WHOLE to the next page — every child
    // starts on the same page, side by side. The old warning fired on any
    // page growth during the row's layout; the precise one must stay
    // silent here.
    let filler: String = (0..80).map(|i| format!("<p>line {i}</p>")).collect();
    let html = format!(
        "<html><body>{filler}\
         <div style=\"display: flex\">\
           <div style=\"width: 50%\"><p>left cell</p></div>\
           <div style=\"width: 50%\"><p>right cell</p></div>\
         </div></body></html>"
    );
    let out = render(&html);
    assert!(
        out.layout.pages.len() > 1,
        "the row must actually be pushed across"
    );
    assert!(
        !out.warnings.iter().any(|w| w.contains("sequentially")),
        "a relocated-whole row is not a sequential split: {:?}",
        out.warnings
    );
}
