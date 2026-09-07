//! The opt-in post-render content audit (`HtmlOptions::audit_content` /
//! `--audit-content`): after layout, the engine verifies the pages against
//! the input and reports dropped / fully off-page / invisible /
//! clipped-to-nothing content through the render-defect channel.
//!
//! The design constraint under test is FALSE POSITIVES as much as
//! detection: every "must flag" pin has a sibling "must NOT flag" pin for
//! the legitimate version of the same shape.

use forme_pdf_html::{render_html, HtmlOptions};

fn audit(html: &str) -> Vec<String> {
    let out = render_html(
        html,
        &HtmlOptions {
            audit_content: true,
            ..Default::default()
        },
    )
    .expect("render");
    out.warnings
        .into_iter()
        .filter(|w| w.contains("content audit"))
        .collect()
}

// ── invisible text (colour == background) ─────────────────────────────

#[test]
fn white_text_on_the_default_white_page_flags() {
    let defects = audit("<p style=\"color: #ffffff\">ghost text</p>");
    assert!(
        defects.iter().any(|d| d.contains("invisible")),
        "white-on-white must flag: {defects:?}"
    );
}

#[test]
fn white_text_on_an_explicit_white_background_flags() {
    let defects =
        audit("<div style=\"background-color: #fff\"><p style=\"color: #ffffff\">ghost</p></div>");
    assert!(
        defects.iter().any(|d| d.contains("invisible")),
        "white-on-explicit-white must flag: {defects:?}"
    );
}

#[test]
fn white_text_on_a_dark_background_does_not_flag() {
    let defects = audit(
        "<div style=\"background-color: #222d32\"><p style=\"color: #ffffff\">fine</p></div>",
    );
    assert!(
        defects.is_empty(),
        "legitimate contrast flagged: {defects:?}"
    );
}

#[test]
fn black_text_on_white_does_not_flag() {
    let defects = audit("<p>perfectly ordinary paragraph</p>");
    assert!(defects.is_empty(), "{defects:?}");
}

// ── fully off-page content (the parked-element idiom) ────────────────

#[test]
fn text_parked_fully_off_page_flags() {
    // The admin-shell idiom WITHOUT the body clip that makes it deliberate:
    // absolutely positioned text whose box lies entirely left of the page.
    let defects = audit(
        "<div style=\"position: absolute; left: -3000px; top: 0\"><p>parked</p></div>\
         <p>visible content</p>",
    );
    assert!(
        defects.iter().any(|d| d.contains("off-page")),
        "fully off-page text must flag: {defects:?}"
    );
}

#[test]
fn parked_text_under_the_body_clip_does_not_flag() {
    // With body { overflow-x: hidden } the page-level clip makes the parking
    // deliberate — browsers hide it the same way; the audit must not warn.
    let defects = audit(
        "<html><head><style>body { overflow-x: hidden }</style></head><body>\
         <div style=\"position: absolute; left: -3000px; top: 0\"><p>parked</p></div>\
         <p>visible content</p></body></html>",
    );
    assert!(
        !defects.iter().any(|d| d.contains("off-page")),
        "deliberately clipped parking flagged: {defects:?}"
    );
}

// ── the flag itself ──────────────────────────────────────────────────

#[test]
fn audit_off_emits_nothing_even_for_invisible_text() {
    let out = render_html(
        "<p style=\"color: #ffffff\">ghost text</p>",
        &HtmlOptions::default(),
    )
    .expect("render");
    assert!(
        !out.warnings.iter().any(|w| w.contains("content audit")),
        "audit ran while off: {:?}",
        out.warnings
    );
}

// ── a real document stays quiet ──────────────────────────────────────

#[test]
fn a_normal_invoice_produces_no_audit_defects() {
    let defects = audit(
        "<html><head><style>\
         @page { size: A4; margin: 48pt }\
         table { width: 100%; border-collapse: collapse }\
         th, td { border: 1px solid #333; padding: 6px }\
         h1 { color: #1a365d } .total { text-align: right; font-weight: bold }\
         </style></head><body>\
         <h1>Invoice INV-2048</h1>\
         <p>Due <strong>net 30</strong> days. Fine print in <span style=\"color:#666\">grey</span>.</p>\
         <table><thead><tr><th>Item</th><th>Qty</th><th>Price</th></tr></thead>\
         <tbody><tr><td>Widget</td><td>3</td><td>$30.00</td></tr>\
         <tr><td class=\"total\" colspan=\"2\">Total</td><td>$75.00</td></tr></tbody></table>\
         </body></html>",
    );
    assert!(defects.is_empty(), "clean invoice flagged: {defects:?}");
}

#[test]
fn multipage_with_page_counters_produces_no_audit_defects() {
    // Page-number placeholders are substituted at render; repeated table
    // headers render more text than the source contains; both directions
    // must stay quiet.
    let body: String = (0..80).map(|i| format!("<p>line {i}</p>")).collect();
    let html = format!(
        "<html><head><style>@page {{ margin: 60pt; @bottom-center {{ content: \"Page \" counter(page) \" of \" counter(pages) }} }}</style></head>\
         <body>{body}</body></html>"
    );
    let defects = audit(&html);
    assert!(defects.is_empty(), "{defects:?}");
}
