//! Paged-media tests over the report fixture: @page geometry, break-*,
//! break-inside: avoid, and the pending-feature warnings.

use forme::layout::ElementInfo;
use forme_pdf_html::{
    html_to_document, render_html_with_layout, HtmlLayoutOutput, HtmlOptions, PageSize,
};

const FIXTURE: &str = include_str!("fixtures/report.html");

fn rendered() -> HtmlLayoutOutput {
    render_html_with_layout(FIXTURE, &HtmlOptions::default()).expect("fixture must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

/// (page index, element) of the first element whose text contains `needle`.
fn find_on_page<'a>(out: &'a HtmlLayoutOutput, needle: &str) -> Option<(usize, &'a ElementInfo)> {
    for (i, page) in out.layout.pages.iter().enumerate() {
        let mut found = None;
        walk(&page.elements, &mut |el| {
            if found.is_none() {
                if let Some(t) = &el.text_content {
                    if t.contains(needle) {
                        found = Some(el);
                    }
                }
            }
        });
        if let Some(el) = found {
            return Some((i, el));
        }
    }
    None
}

#[test]
fn at_page_geometry_applies() {
    let out = rendered();
    let page = &out.layout.pages[0];
    assert_eq!(page.width, 612.0, "@page size: Letter");
    assert_eq!(page.height, 792.0);
    // :first { margin-top: 120pt } is live — page one gets its own top
    // margin; later pages use the base 72pt.
    assert_eq!(page.content_y, 120.0, "@page :first margin-top 120pt");
    assert_eq!(page.content_x, 60.0, "@page margin left 60pt");
    assert_eq!(
        out.layout.pages[1].content_y, 72.0,
        "later pages use the base @page margin"
    );
}

#[test]
fn bottom_center_counter_footer_on_every_page() {
    // @bottom-center with counter(page)/counter(pages) renders as a
    // footer band occupying the bottom margin strip on EVERY page.
    let out = rendered();
    for (i, page) in out.layout.pages.iter().enumerate() {
        let mut found_y = None;
        walk(&page.elements, &mut |el| {
            if let Some(t) = &el.text_content {
                // Counters are engine sentinels until PDF write; match the
                // literal part of the template.
                if t.contains("Page ") && t.contains(" of ") && found_y.is_none() {
                    found_y = Some(el.y);
                }
            }
        });
        let y = found_y.unwrap_or_else(|| panic!("no footer on page {i}"));
        assert!(
            y >= 792.0 - 72.0,
            "footer must sit in the bottom margin strip on page {i}, got y={y}"
        );
    }
}

#[test]
fn explicit_option_overrides_at_page() {
    let options = HtmlOptions {
        page_size: Some(PageSize::A4),
        ..Default::default()
    };
    let out = render_html_with_layout(FIXTURE, &options).expect("render");
    assert!(
        (out.layout.pages[0].width - 595.28).abs() < 0.01,
        "explicit option must beat @page (print-dialog precedence)"
    );
}

#[test]
fn break_after_starts_next_content_on_new_page() {
    // .summary has break-after: page — Section One's heading must open
    // page 2 even though page 1 has plenty of room.
    let out = rendered();
    let (summary_page, _) = find_on_page(&out, "Executive summary").expect("summary");
    let (s1_page, _) = find_on_page(&out, "Section One").expect("section one");
    assert_eq!(summary_page, 0);
    assert_eq!(s1_page, 1, "break-after: page on .summary");
}

#[test]
fn legacy_page_break_before_alias_works() {
    let out = rendered();
    let (s2_page, s2) = find_on_page(&out, "Section Two").expect("section two");
    let (table_page, _) = find_on_page(&out, "Offshore").expect("table");
    assert!(
        s2_page > table_page,
        "page-break-before: always must start a fresh page after the table"
    );
    // And it sits at the top of its page's content area.
    assert!(
        s2.y <= out.layout.pages[s2_page].content_y + 1.0,
        "section heading must open its page (y={}, content_y={})",
        s2.y,
        out.layout.pages[s2_page].content_y
    );
}

#[test]
fn break_inside_avoid_keeps_table_whole() {
    // The filler paragraphs leave too little room for the 7-row table;
    // break-inside: avoid must move it to the next page intact rather
    // than splitting it.
    let out = rendered();
    let mut row_pages: Vec<usize> = Vec::new();
    for (i, page) in out.layout.pages.iter().enumerate() {
        walk(&page.elements, &mut |el| {
            if el.node_type == "TableRow" {
                row_pages.push(i);
            }
        });
    }
    assert_eq!(row_pages.len(), 7, "1 header + 6 body rows");
    let first = row_pages[0];
    assert!(
        row_pages.iter().all(|p| *p == first),
        "break-inside: avoid table must not split across pages: {row_pages:?}"
    );
    // And it genuinely didn't fit where the filler ended: the last filler
    // paragraph sits on the page BEFORE the table.
    let (filler_page, _) = find_on_page(&out, "pushing the table").expect("last filler");
    assert_eq!(
        first,
        filler_page + 1,
        "table must have moved to the page after the filler"
    );
}

#[test]
fn orphans_widows_reach_engine_style() {
    let (doc, _) = html_to_document(FIXTURE, &HtmlOptions::default());
    fn find_p(nodes: &[forme::Node]) -> Option<&forme::Node> {
        for n in nodes {
            if let forme::NodeKind::Text { content, .. } = &n.kind {
                if content.contains("Paragraph one of the filler") {
                    return Some(n);
                }
            }
            if let Some(f) = find_p(&n.children) {
                return Some(f);
            }
        }
        None
    }
    let p = find_p(&doc.children).expect("filler paragraph");
    assert_eq!(p.style.min_orphan_lines, Some(3), "orphans: 3");
    assert_eq!(p.style.min_widow_lines, Some(3), "widows: 3");
}

#[test]
fn unsupported_paged_features_warn_supported_ones_dont() {
    let out = rendered();
    let has = |needle: &str| out.warnings.iter().any(|w| w.contains(needle));
    assert!(has("'bleed'"), "bleed descriptor: {:?}", out.warnings);
    // Margin boxes and :first are live now — they must NOT warn.
    assert!(
        !has("@bottom-center") && !has(":first"),
        "live features must not warn: {:?}",
        out.warnings
    );
    assert!(out.pdf.starts_with(b"%PDF-"));
}

#[test]
fn report_renders_artifact() {
    let out = rendered();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/report.pdf");
    std::fs::write(path, &out.pdf).expect("write artifact");
    assert!(
        out.layout.pages.len() >= 3,
        "summary, section one + table, section two"
    );
}

// ─── The headline combo: running letterhead + :first suppression ──────

const LETTERHEAD: &str = include_str!("fixtures/letterhead.html");

#[test]
fn first_page_suppresses_letterhead_and_keeps_its_margin() {
    // THE interaction test: margin boxes + :first suppression. The band
    // trick zeroes the engine's top margin globally; filtering the band
    // off page one must RESTORE the real margin there — content must not
    // start at the physical top of the paper.
    let out = render_html_with_layout(LETTERHEAD, &HtmlOptions::default()).expect("render");
    assert!(out.layout.pages.len() >= 3, "title page + two sections");

    let header_on = |i: usize| -> Option<f64> {
        let mut y = None;
        walk(&out.layout.pages[i].elements, &mut |el| {
            if el
                .text_content
                .as_deref()
                .is_some_and(|t| t.contains("CONFIDENTIAL"))
                && y.is_none()
            {
                y = Some(el.y);
            }
        });
        y
    };

    // Page 1: no letterhead, and the restored margin means the first
    // content element starts at (or below) 72pt — never at y=0.
    assert!(
        header_on(0).is_none(),
        "letterhead must be suppressed on page 1"
    );
    assert_eq!(
        out.layout.pages[0].content_y, 72.0,
        "page 1 must carry the restored top margin"
    );
    let min_y = out.layout.pages[0]
        .elements
        .iter()
        .map(|e| e.y)
        .fold(f64::MAX, f64::min);
    assert!(
        min_y >= 72.0 - 0.5,
        "page 1 content must start at the restored margin, got y={min_y}"
    );

    // Pages 2+: letterhead sits IN the top margin strip (above 72pt),
    // body content starts below it.
    for i in 1..out.layout.pages.len() {
        let hy = header_on(i).unwrap_or_else(|| panic!("letterhead missing on page {}", i + 1));
        assert!(
            hy < 72.0,
            "letterhead must sit in the margin strip on page {}, got y={hy}",
            i + 1
        );
    }

    // Section pages open with their headings below the letterhead strip.
    let (bg_page, bg) = find_on_page(&out, "Background").expect("background heading");
    assert!(bg_page >= 1);
    assert!(
        (bg.y - 72.0).abs() < 1.0,
        "inner-page content starts where the margin ends, got y={}",
        bg.y
    );

    // Footer page number on every page.
    for (i, page) in out.layout.pages.iter().enumerate() {
        let mut found = false;
        walk(&page.elements, &mut |el| {
            if el.kind == "Text" && el.y >= 792.0 - 72.0 {
                found = true;
            }
        });
        assert!(found, "footer counter missing on page {}", i + 1);
    }
}
