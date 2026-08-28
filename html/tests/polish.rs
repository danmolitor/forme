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
    let (_, warnings) = html_to_document(html, &HtmlOptions::default());
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("vertical-align is pending")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("break-inside on <thead> is pending")),
        "{warnings:?}"
    );
}
