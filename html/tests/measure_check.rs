//! The measure/layout agreement gate.
//!
//! Four shipped bugs shared one shape: `measure_*` computed a height that
//! layout never produced (table column count, table intrinsic width,
//! image phantom height, row-measure percent double-resolution). Each was
//! found by a user or a corpus experiment — two code paths for the same
//! quantity with nothing forcing agreement. This gate enforces the
//! invariant: every committed fixture renders with `FORME_MEASURE_CHECK=1`
//! and any "measure-check:" emission fails the test, so the next
//! divergence announces itself at development time instead of in a
//! rendered document.

use forme_pdf_html::{render_html_with_layout, HtmlOptions};

fn measure_check_warnings(html: &str) -> Vec<String> {
    // Safe under edition 2021; every test in this binary sets the same
    // value, so parallel test threads cannot disagree.
    std::env::set_var("FORME_MEASURE_CHECK", "1");
    let out = render_html_with_layout(html, &HtmlOptions::default()).expect("must render");
    out.warnings
        .into_iter()
        .filter(|w| w.contains("measure-check:"))
        .collect()
}

fn assert_agrees(html: &str, what: &str) {
    let violations = measure_check_warnings(html);
    assert!(
        violations.is_empty(),
        "{what}: measure/layout disagreement — {violations:?}"
    );
}

// ── The committed fixture corpus ───────────────────────────────────

#[test]
fn fixture_corpus_measures_what_it_lays_out() {
    for fixture in [
        "invoice",
        "letterhead",
        "report",
        "statement",
        "zebra-invoice",
        "dashed-borders",
    ] {
        let html =
            std::fs::read_to_string(format!("tests/fixtures/{fixture}.html")).expect("fixture");
        assert_agrees(&html, fixture);
    }
}

// ── The shapes that found the shipped bugs ─────────────────────────

#[test]
fn percent_width_row_children_measure_at_container_width() {
    // Row-measure double percent resolution: the Row measure arm resolved
    // a child's `width: 27%` against the child's own final width (27% of
    // 27%), measured its text at a quarter width — one word per line —
    // and produced a row 2.5-4x taller than layout. The float transform
    // made this shape common (template-compat 04 and 07).
    assert_agrees(
        "<html><head><style>body,div{margin:0}</style></head><body>\
         <div style=\"float:left;width:30%\">Acme Corp<br>1600 Market Street<br>San Francisco<br>United States</div>\
         <div style=\"float:right;width:27%\">Ship To<br>Johnson Ltd<br>Receiving Dept.<br>1200 Industrial Parkway<br>Springfield<br>United States</div>\
         <div style=\"clear:both\">after</div>\
         </body></html>",
        "percent-width float row",
    );
}

#[test]
fn percent_width_flex_row_measures_tight() {
    // Same mechanism without floats: any explicit flex row with
    // percent-width children whose text wraps.
    assert_agrees(
        "<html><head><style>body,div{margin:0}</style></head><body>\
         <div style=\"display:flex\">\
           <div style=\"width:30%\">a long address line that wraps at thirty percent width</div>\
           <div style=\"width:70%\">short</div>\
         </div>\
         </body></html>",
        "percent-width flex row",
    );
}
