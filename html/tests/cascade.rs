//! End-to-end cascade tests over the statement fixture: every deliberate
//! conflict in its stylesheet is asserted from the mapped document tree.

use forme::style::{Color, TextAlign, TextDecoration};
use forme::{Node, NodeKind};
use forme_pdf_html::{html_to_document, render_html, HtmlOptions};

const FIXTURE: &str = include_str!("fixtures/statement.html");

fn mapped() -> (forme::Document, Vec<String>) {
    html_to_document(FIXTURE, &HtmlOptions::default())
}

/// Depth-first search for the first node whose rendered text contains
/// `needle` (checks both plain content and run content).
fn find_by_text<'a>(nodes: &'a [Node], needle: &str) -> Option<&'a Node> {
    for node in nodes {
        let text: String = match &node.kind {
            NodeKind::Text { content, runs, .. } | NodeKind::Heading { content, runs, .. } => {
                if runs.is_empty() {
                    content.clone()
                } else {
                    runs.iter().map(|r| r.content.as_str()).collect()
                }
            }
            _ => String::new(),
        };
        if text.contains(needle) {
            return Some(node);
        }
        if let Some(found) = find_by_text(&node.children, needle) {
            return Some(found);
        }
    }
    None
}

fn color_hex(c: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8
    )
}

fn assert_color(node: &Node, hex: &str, what: &str) {
    let c = node
        .style
        .color
        .unwrap_or_else(|| panic!("{what}: no color set"));
    assert_eq!(color_hex(c), hex, "{what}");
}

#[test]
fn type_rule_from_stylesheet_applies() {
    let (doc, _) = mapped();
    let h1 = find_by_text(&doc.children, "Account Statement").expect("h1");
    assert!(matches!(h1.kind, NodeKind::Heading { level: 1, .. }));
    assert_color(h1, "#1a365d", "h1 color from stylesheet");
    assert_eq!(
        h1.style.font_size,
        Some(20.0),
        "h1 font-size from stylesheet"
    );
}

#[test]
fn descendant_specificity_beats_bare_type() {
    // `table td` (0,0,2) must beat `td` (0,0,1) for both padding and color.
    let (doc, _) = mapped();
    let cell = find_by_text(&doc.children, "Widget Pro subscription").expect("cell text");
    // The text node is inside the TableCell; find the cell itself.
    let cell_node = find_cell_containing(&doc.children, "Widget Pro subscription");
    let padding = cell_node.style.padding.expect("cell padding");
    assert_eq!(
        padding.top, 3.0,
        "4px → 3pt from `table td`, not 20px → 15pt"
    );
    assert_eq!(padding.left, 4.5, "6px → 4.5pt");
    let _ = cell;
    assert_color(cell_node, "#333333", "cell color from `table td`");
}

fn find_cell_containing<'a>(nodes: &'a [Node], needle: &str) -> &'a Node {
    fn walk<'a>(nodes: &'a [Node], needle: &str) -> Option<&'a Node> {
        for node in nodes {
            if matches!(node.kind, NodeKind::TableCell { .. })
                && find_by_text(std::slice::from_ref(node), needle).is_some()
            {
                return Some(node);
            }
            if let Some(f) = walk(&node.children, needle) {
                return Some(f);
            }
        }
        None
    }
    walk(nodes, needle).expect("cell not found")
}

#[test]
fn class_rule_applies_and_id_wins() {
    let (doc, _) = mapped();
    // .amount → text-align right on the header cell content.
    let amount_cell = find_cell_containing(&doc.children, "$49.00");
    assert!(
        matches!(amount_cell.style.text_align, Some(TextAlign::Right)),
        ".amount must right-align"
    );
    // #grand-total td → bold beats everything else targeting the cell.
    let total_cell = find_cell_containing(&doc.children, "$74.00");
    assert_eq!(
        total_cell.style.font_weight,
        Some(700),
        "#grand-total td must be bold"
    );
}

#[test]
fn important_beats_inline_style() {
    // `p.status { color: #b45309 !important }` vs inline `color: #00ff00`:
    // important wins.
    let (doc, _) = mapped();
    let p = find_by_text(&doc.children, "Payment overdue").expect("status p");
    assert_color(p, "#b45309", "!important must beat the inline style");
}

#[test]
fn source_order_breaks_specificity_ties() {
    let (doc, _) = mapped();
    let p = find_by_text(&doc.children, "Tie-breaker paragraph").expect("tie p");
    assert_color(p, "#0000ff", "later rule of equal specificity wins");
}

#[test]
fn child_combinator_styles_list_items() {
    let (doc, _) = mapped();
    let li_text = find_by_text(&doc.children, "Autopay is enabled").expect("li text");
    // The color lands on the ListItem (or its anonymous text child
    // inherits) — the li itself carries the rule.
    let li = find_li_containing(&doc.children, "Autopay is enabled");
    assert_color(li, "#444444", "ul > li color");
    let _ = li_text;
}

fn find_li_containing<'a>(nodes: &'a [Node], needle: &str) -> &'a Node {
    fn walk<'a>(nodes: &'a [Node], needle: &str) -> Option<&'a Node> {
        for node in nodes {
            if matches!(node.kind, NodeKind::ListItem)
                && find_by_text(std::slice::from_ref(node), needle).is_some()
            {
                return Some(node);
            }
            if let Some(f) = walk(&node.children, needle) {
                return Some(f);
            }
        }
        None
    }
    walk(nodes, needle).expect("li not found")
}

#[test]
fn unsupported_selector_skipped_but_group_partner_applies() {
    let (doc, warnings) = mapped();
    let p = find_by_text(&doc.children, "Kept selector paragraph").expect("kept p");
    assert!(
        matches!(p.style.text_decoration, Some(TextDecoration::Underline)),
        ".kept from the `td:hover, .kept` group must still apply"
    );
    assert!(
        warnings.iter().any(|w| w.contains("unsupported selector")),
        "td:hover must be reported: {warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("@media")),
        "@media must be reported: {warnings:?}"
    );
}

#[test]
fn options_css_appends_after_document_styles() {
    let options = HtmlOptions {
        // Same specificity as the document's `.tie` rules — appended
        // origin must win the tie.
        css: Some(".tie { color: #00aa00 }".to_string()),
        ..Default::default()
    };
    let (doc, _) = html_to_document(FIXTURE, &options);
    let p = find_by_text(&doc.children, "Tie-breaker paragraph").expect("tie p");
    assert_color(p, "#00aa00", "options.css must win equal-specificity ties");
}

#[test]
fn statement_renders_to_pdf() {
    let out = render_html(FIXTURE, &HtmlOptions::default()).expect("render");
    assert!(out.pdf.starts_with(b"%PDF-"));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/statement.pdf");
    std::fs::write(path, &out.pdf).expect("write artifact");
}

// ─── Pre-launch round: centered column + middle-aligned cells ─────────

#[test]
fn max_width_column_centers_in_the_layout() {
    let out =
        forme_pdf_html::render_html_with_layout(FIXTURE, &HtmlOptions::default()).expect("render");
    let page = &out.layout.pages[0];

    // body > .sheet — the centered column.
    fn find_sheet(els: &[forme::layout::ElementInfo]) -> Option<&forme::layout::ElementInfo> {
        // The first nested View whose width is well below the content
        // width is the clamped sheet.
        for el in els {
            if el.node_type == "View" {
                for child in &el.children {
                    if child.node_type == "View" && child.width < 450.0 {
                        return Some(child);
                    }
                }
            }
            if let Some(f) = find_sheet(&el.children) {
                return Some(f);
            }
        }
        None
    }
    let sheet = find_sheet(&page.elements).expect("sheet column");
    assert!(
        (sheet.width - 420.0).abs() < 0.01,
        "max-width must clamp the sheet to 420pt exactly, got {}",
        sheet.width
    );
    // body has the UA 6pt margin on each side inside the page content box.
    let body_content_w = page.content_width - 12.0;
    let expected_x = page.content_x + 6.0 + (body_content_w - 420.0) / 2.0;
    assert!(
        (sheet.x - expected_x).abs() < 0.01,
        "auto margins must center it: x {} != {}",
        sheet.x,
        expected_x
    );
}

#[test]
fn middle_aligned_totals_center_and_valign_attr_works() {
    let out =
        forme_pdf_html::render_html_with_layout(FIXTURE, &HtmlOptions::default()).expect("render");

    fn find_cell_with<'a>(
        els: &'a [forme::layout::ElementInfo],
        needle: &str,
    ) -> Option<&'a forme::layout::ElementInfo> {
        for el in els {
            if el.node_type == "TableCell" {
                let mut has = false;
                walk_text(&el.children, &mut |t| {
                    if t.contains(needle) {
                        has = true;
                    }
                });
                if has {
                    return Some(el);
                }
            }
            if let Some(f) = find_cell_with(&el.children, needle) {
                return Some(f);
            }
        }
        None
    }
    fn walk_text(els: &[forme::layout::ElementInfo], f: &mut impl FnMut(&str)) {
        for el in els {
            if let Some(t) = &el.text_content {
                f(t);
            }
            walk_text(&el.children, f);
        }
    }
    fn text_box(el: &forme::layout::ElementInfo) -> (f64, f64) {
        // (y, height) of the inner Text container.
        fn first_text(els: &[forme::layout::ElementInfo]) -> Option<(f64, f64)> {
            for el in els {
                if el.node_type == "Text" {
                    return Some((el.y, el.height));
                }
                if let Some(t) = first_text(&el.children) {
                    return Some(t);
                }
            }
            None
        }
        first_text(&el.children).expect("cell text")
    }

    let page0 = &out.layout.pages[0];

    // CSS vertical-align: middle — the single-line amount centers against
    // the 3-line note in the grand-total row. Exact center match.
    let amount = find_cell_with(&page0.elements, "$74.00").expect("amount cell");
    let (ty, th) = text_box(amount);
    let cell_center = amount.y + amount.height / 2.0;
    let text_center = ty + th / 2.0;
    assert!(
        (text_center - cell_center).abs() < 0.01,
        "middle-aligned amount: text center {text_center} != cell center {cell_center}"
    );

    // Legacy valign="middle" attribute — the single-line cell centers
    // against its 2-line neighbor, exactly.
    let legacy = find_cell_with(&page0.elements, "legacy valign attr").expect("legacy cell");
    let (ly, lh) = text_box(legacy);
    let lcc = legacy.y + legacy.height / 2.0;
    let ltc = ly + lh / 2.0;
    assert!(
        (ltc - lcc).abs() < 0.01,
        "valign attr must center: text center {ltc} != cell center {lcc}"
    );
    // And its unaligned neighbor ($0.00) top-aligns in the same row.
    let unaligned = find_cell_with(&page0.elements, "$0.00").expect("unaligned cell");
    let (uy, uh) = text_box(unaligned);
    let ucc = unaligned.y + unaligned.height / 2.0;
    assert!(uy + uh / 2.0 < ucc, "no-valign neighbor stays top-aligned");
}
