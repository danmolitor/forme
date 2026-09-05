//! Known-gaps polish: entities × whitespace collapsing (the predicted
//! sneaky interaction), justify (already engine-supported — pinned),
//! border-radius under border-collapse, row atomicity, and the pending
//! warnings for vertical-align and section-level break-inside.

use forme::{Node, NodeKind};
use forme_pdf_html::{html_to_document, render_html, render_html_with_layout, HtmlOptions};

fn text_of(nodes: &[Node], needle: &str) -> Option<String> {
    for n in nodes {
        if let NodeKind::Text { content, runs, .. } = &n.kind {
            let t: String = if runs.is_empty() {
                content.clone()
            } else {
                runs.iter().map(|r| r.content.as_str()).collect()
            };
            if t.contains(needle) {
                return Some(t);
            }
        }
        if let Some(f) = text_of(&n.children, needle) {
            return Some(f);
        }
    }
    None
}

#[test]
fn nbsp_survives_whitespace_collapsing() {
    // THE interaction assertion: the collapser must treat U+00A0 as
    // content, not whitespace. If it had used char::is_whitespace()
    // (which includes U+00A0) instead of is_ascii_whitespace(), every
    // &nbsp; in every template would collapse away silently.
    let html = "<p>a&nbsp;&nbsp;b</p>\n<p>x &nbsp; y</p>";
    let (doc, _) = html_to_document(html, &HtmlOptions::default());

    let t1 = text_of(&doc.children, "a").expect("first para");
    assert_eq!(t1, "a\u{a0}\u{a0}b", "consecutive nbsp both survive");

    let t2 = text_of(&doc.children, "x").expect("second para");
    // ASCII spaces around the nbsp collapse to one each; the nbsp stays.
    assert_eq!(t2, "x \u{a0} y", "nbsp keeps its flanking ASCII spaces");

    // And it renders (WinAnsi has A0).
    let out = render_html(html, &HtmlOptions::default()).expect("render");
    assert!(out.pdf.starts_with(b"%PDF-"));
}

#[test]
fn common_entities_decode_through_to_text() {
    let html = "<p>Em&mdash;dash &copy; AT&amp;T &lt;tag&gt; &#8364;9</p>";
    let (doc, _) = html_to_document(html, &HtmlOptions::default());
    let t = text_of(&doc.children, "dash").expect("para");
    assert_eq!(t, "Em\u{2014}dash \u{a9} AT&T <tag> \u{20ac}9");
}

#[test]
fn text_align_justify_is_already_supported() {
    // Engine-side Knuth-Plass + the PDF Tw operator exist; the mapper
    // parses `justify`. Pin the wiring end to end.
    let html = r#"<p style="text-align: justify">Long enough text to wrap across multiple
        lines so that justification actually distributes inter-word space in the
        rendered output of this paragraph.</p>"#;
    let (doc, warnings) = html_to_document(html, &HtmlOptions::default());
    fn find_p(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if matches!(n.kind, NodeKind::Text { .. }) {
                return Some(n);
            }
            if let Some(f) = find_p(&n.children) {
                return Some(f);
            }
        }
        None
    }
    let p = find_p(&doc.children).expect("para");
    assert!(
        matches!(p.style.text_align, Some(forme::style::TextAlign::Justify)),
        "justify must reach the engine style"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("render");
    assert!(out.pdf.starts_with(b"%PDF-"));
}

#[test]
fn border_radius_is_ignored_under_collapse_and_kept_otherwise() {
    let html = r#"<style>
        table.a { border-collapse: collapse; border-radius: 8px; border: 1px solid #000 }
        table.a td { border: 1px solid #000; border-radius: 4px }
        div.badge { border: 1px solid #000; border-radius: 6px }
      </style>
      <table class="a"><tr><td>x</td></tr></table>
      <div class="badge">badge</div>"#;
    let (doc, _) = html_to_document(html, &HtmlOptions::default());

    fn find_kind(nodes: &[Node], want_table: bool) -> Option<&Node> {
        for n in nodes {
            match &n.kind {
                NodeKind::Table { .. } if want_table => return Some(n),
                NodeKind::View if !want_table && n.style.border_radius.is_some() => return Some(n),
                _ => {}
            }
            if let Some(f) = find_kind(&n.children, want_table) {
                return Some(f);
            }
        }
        None
    }
    let table = find_kind(&doc.children, true).expect("table");
    assert!(
        table.style.border_radius.is_none(),
        "radius must be dropped on collapsed tables (spec + Chrome)"
    );
    let cell = &table.children[0].children[0];
    assert!(
        cell.style.border_radius.is_none(),
        "cell radius dropped too"
    );
    assert!(
        find_kind(&doc.children, false).is_some(),
        "the badge div keeps its radius"
    );
}

#[test]
fn tr_break_inside_avoid_is_honored_rows_are_atomic() {
    // Rows never split unless taller than a full page — the engine moves
    // a row that doesn't fit to the next page whole. A row-level
    // break-inside: avoid is therefore honored by design; pin it.
    let mut rows = String::new();
    for i in 0..60 {
        rows.push_str(&format!(
            "<tr style=\"break-inside: avoid\"><td>row {i} line one<br>line two<br>line three</td></tr>\n"
        ));
    }
    let html = format!("<table>{rows}</table>");
    let out = render_html_with_layout(&html, &HtmlOptions::default()).expect("render");
    assert!(out.layout.pages.len() > 1, "must paginate");

    // No TableRow may straddle: every row's cells land on one page.
    fn rows_on_page(els: &[forme::layout::ElementInfo], out: &mut Vec<String>) {
        for el in els {
            if el.node_type == "TableRow" {
                if let Some(t) = first_text(&el.children) {
                    out.push(t);
                }
            }
            rows_on_page(&el.children, out);
        }
    }
    fn first_text(els: &[forme::layout::ElementInfo]) -> Option<String> {
        for el in els {
            if let Some(t) = &el.text_content {
                return Some(t.clone());
            }
            if let Some(t) = first_text(&el.children) {
                return Some(t);
            }
        }
        None
    }
    let mut seen = std::collections::HashSet::new();
    for page in &out.layout.pages {
        let mut labels = Vec::new();
        rows_on_page(&page.elements, &mut labels);
        for l in labels {
            assert!(
                seen.insert(l.clone()),
                "row '{l}' appears on more than one page — a row split"
            );
        }
    }
}

#[test]
fn pending_polish_warnings_are_named() {
    let html = r#"<style>
        td { vertical-align: middle }
        thead { break-inside: avoid }
      </style>
      <table><thead><tr><td>h</td></tr></thead><tbody><tr><td>b</td></tr></tbody></table>"#;
    let (doc, warnings) = html_to_document(html, &HtmlOptions::default());
    // vertical-align is live now: it lands on the cell and does NOT warn.
    fn find_cell(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if matches!(n.kind, NodeKind::TableCell { .. }) {
                return Some(n);
            }
            if let Some(f) = find_cell(&n.children) {
                return Some(f);
            }
        }
        None
    }
    let cell = find_cell(&doc.children).expect("cell");
    assert!(
        matches!(
            cell.style.vertical_align,
            Some(forme::style::VerticalAlign::Middle)
        ),
        "vertical-align: middle must reach the cell"
    );
    assert!(
        !warnings.iter().any(|w| w.contains("vertical-align")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("break-inside on <thead> is pending")),
        "{warnings:?}"
    );
}

#[test]
fn stylesheet_links_warn_with_the_remedy_other_links_stay_silent() {
    // The most common template shape on earth: HTML + linked stylesheet.
    // Nothing is fetched (by design) — but the skip must be LOUD, in the
    // same shape as the @import warning. Non-stylesheet links (icons,
    // preloads) don't affect rendering and stay silent.
    let html = r#"<!DOCTYPE html>
      <html><head>
        <link rel="stylesheet" href="brand.css">
        <link rel="icon" href="favicon.ico">
      </head>
      <body><h1>Linked</h1></body></html>"#;
    let out = render_html(html, &HtmlOptions::default()).expect("render");
    assert!(out.pdf.starts_with(b"%PDF-"));
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("stylesheet link 'brand.css' is not fetched")
                && w.contains("--css / options.css")),
        "{:?}",
        out.warnings
    );
    assert!(
        !out.warnings.iter().any(|w| w.contains("favicon")),
        "icon links must stay silent: {:?}",
        out.warnings
    );
}

// ── Floats: out of subset, warned loud with a remedy (Part 2, item 5) ──

#[test]
fn float_renders_as_columns_and_warns_only_for_text_wrap() {
    // Floats are in-subset now (sibling rows); the residual warning
    // covers the one thing still out: text wrapping ALONGSIDE a float.
    // This doc hits exactly that case — a paragraph after an uncleared
    // float — so the narrowed warning must fire; clear is silent.
    let html = r#"<html><head><style>
      .fig { float: left; width: 80px; }
      .foot { clear: both; }
    </style></head><body>
      <div class="fig">logo</div>
      <p>Body text beside the figure.</p>
      <div class="foot">Footer</div>
    </body></html>"#;
    let out = render_html(html, &HtmlOptions::default()).expect("must still render");
    assert!(out.pdf.len() > 100 && &out.pdf[0..5] == b"%PDF-");
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("text wrapping alongside floats is not supported")),
        "the residual text-wrap case must warn: {:?}",
        out.warnings
    );
    assert!(
        !out.warnings.iter().any(|w| w.contains("clear is not")),
        "clear is in-subset now: {:?}",
        out.warnings
    );
}

// ── position: relative offsets via the mapper (item 7a) ──

#[test]
fn position_relative_offset_shifts_paint_through_the_mapper() {
    let doc = |top_left: &str| {
        format!(
            r#"<html><head><style>
              div {{ height: 20px; width: 60px; }}
              .rel {{ position: relative; {top_left} }}
            </style></head><body>
              <div class="a">a</div>
              <div class="rel">r</div>
              <div class="b">b</div>
            </body></html>"#
        )
    };
    let find = |els: &[forme::layout::ElementInfo], needle: &str| -> (f64, f64) {
        fn walk(els: &[forme::layout::ElementInfo], needle: &str) -> Option<(f64, f64)> {
            for e in els {
                if e.text_content
                    .as_deref()
                    .is_some_and(|t| t.contains(needle))
                {
                    return Some((e.x, e.y));
                }
                if let Some(p) = walk(&e.children, needle) {
                    return Some(p);
                }
            }
            None
        }
        walk(els, needle).expect("element")
    };

    let off =
        render_html_with_layout(&doc("top: 10px; left: 8px;"), &HtmlOptions::default()).unwrap();
    let base = render_html_with_layout(&doc(""), &HtmlOptions::default()).unwrap();
    let (ox, oy) = find(&off.layout.pages[0].elements, "r");
    let (bx, by) = find(&base.layout.pages[0].elements, "r");
    // 8px → 6pt, 10px → 7.5pt.
    assert!(
        (ox - (bx + 6.0)).abs() < 0.01,
        "left:8px → +6pt: {ox} vs {bx}"
    );
    assert!(
        (oy - (by + 7.5)).abs() < 0.01,
        "top:10px → +7.5pt: {oy} vs {by}"
    );
    // Following sibling flow unchanged.
    let (_, ob_y) = find(&off.layout.pages[0].elements, "b");
    let (_, bb_y) = find(&base.layout.pages[0].elements, "b");
    assert_eq!(ob_y, bb_y, "sibling flow must not move");
    assert!(
        off.warnings.is_empty(),
        "no offset warning for relative: {:?}",
        off.warnings
    );
}

// ── border-style: dashed/dotted per side; unsupported → solid (item 4) ──

#[test]
fn border_style_maps_per_side_and_double_falls_back_to_solid() {
    use forme::style::BorderStyle;
    let (doc, _) = html_to_document(
        r#"<div style="border-top: 3px dashed #000; border-right: 3px dotted #000; border-bottom: 3px solid #000; border-left: 3px double #000">x</div>"#,
        &HtmlOptions::default(),
    );
    fn find(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if n.style.border_style.is_some() {
                return Some(n);
            }
            if let Some(f) = find(&n.children) {
                return Some(f);
            }
        }
        None
    }
    let bs = find(&doc.children)
        .expect("bordered node")
        .style
        .border_style
        .expect("border_style set");
    assert_eq!(bs.top, BorderStyle::Dashed);
    assert_eq!(bs.right, BorderStyle::Dotted);
    assert_eq!(bs.bottom, BorderStyle::Solid);
    // `double` is out of subset → falls back to solid, no warning, still renders.
    assert_eq!(bs.left, BorderStyle::Solid, "double → solid");

    let out = render_html(
        r#"<div style="border: 4px double #333">x</div>"#,
        &HtmlOptions::default(),
    )
    .expect("double still renders");
    assert!(out.pdf.len() > 100 && &out.pdf[0..5] == b"%PDF-");
}

#[test]
fn running_position_removes_the_element_from_flow() {
    // Per CSS GCPM, `position: running(name)` takes the element OUT of
    // normal flow; it appears only where a margin box says
    // `content: element(name)`, and simply doesn't display otherwise.
    // We support neither half, warned by name — but the element still
    // rendered in-flow, producing stray header text at the top of the
    // page (template-compat 02's "Page of"). Suppression IS the
    // spec-conformant floor here.
    let (doc, warnings) = html_to_document(
        "<html><head><style>.h { position: running(header) }</style></head><body>\
         <div class=\"h\">Page of</div>\
         <p>real content</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(
        warnings.iter().any(|w| w.contains("running()")),
        "still warned by name: {warnings:?}"
    );
    assert!(
        text_of(&doc.children, "Page of").is_none(),
        "running element must not render in flow"
    );
    assert!(
        text_of(&doc.children, "real content").is_some(),
        "siblings unaffected"
    );
}
