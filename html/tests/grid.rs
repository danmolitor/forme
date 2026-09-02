//! CSS Grid through the mapper — the documented subset (fails-first).
//!
//! The engine has full grid layout (`layout/grid.rs`); this suite pins the
//! mapper's bounded subset: fr/px/auto tracks, integer `repeat()`, spans,
//! explicit placement, and gaps — the shapes Tailwind emits (`grid-cols-N`
//! is `repeat(N, minmax(0, 1fr))`). Everything the engine cannot express
//! warns by name; nothing silently mislays out.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

/// A 0-margin 400×400pt page so column math is exact.
fn page(css: &str, body: &str) -> String {
    format!(
        "<html><head><style>@page {{ size: 400pt 400pt; margin: 0 }} body {{ margin: 0 }} p {{ margin: 0 }} {css}</style></head><body>{body}</body></html>"
    )
}

fn render(html: &str) -> HtmlLayoutOutput {
    render_html_with_layout(html, &HtmlOptions::default()).expect("must render")
}

fn walk<'a>(elements: &'a [ElementInfo], f: &mut impl FnMut(&'a ElementInfo)) {
    for el in elements {
        f(el);
        walk(&el.children, f);
    }
}

/// First element on page 0 whose text contains `needle`.
fn find<'a>(out: &'a HtmlLayoutOutput, needle: &str) -> &'a ElementInfo {
    let mut found: Option<&ElementInfo> = None;
    walk(&out.layout.pages[0].elements, &mut |el| {
        if found.is_none() {
            if let Some(t) = &el.text_content {
                if t.contains(needle) {
                    found = Some(el);
                }
            }
        }
    });
    found.unwrap_or_else(|| panic!("element containing {needle:?} not found"))
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 0.5,
        "{what}: expected ~{expected}, got {actual}"
    );
}

fn warned(out: &HtmlLayoutOutput, needle: &str) {
    assert!(
        out.warnings.iter().any(|w| w.contains(needle)),
        "expected a warning containing {needle:?}; got {:?}",
        out.warnings
    );
}

// ── Layout: the IN subset ──────────────────────────────────────────

#[test]
fn three_fr_columns_divide_the_width_equally() {
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr 1fr }",
        "<div class='g'><p>alpha</p><p>beta</p><p>gamma</p></div>",
    );
    let out = render(&html);
    assert!(
        out.warnings.is_empty(),
        "no warnings for pure subset: {:?}",
        out.warnings
    );
    let third = 400.0 / 3.0;
    assert_close(find(&out, "alpha").x, 0.0, "col 1 x");
    assert_close(find(&out, "beta").x, third, "col 2 x");
    assert_close(find(&out, "gamma").x, 2.0 * third, "col 3 x");
    // Same row: equal y.
    assert_close(find(&out, "beta").y, find(&out, "alpha").y, "row alignment");
}

#[test]
fn tailwind_grid_cols_shape_renders_equal_columns() {
    // Tailwind's grid-cols-3 emits exactly this. The engine's MinMax track
    // never joins fr distribution, so the mapper must normalize
    // minmax(0, Xfr) -> Xfr or this silently content-sizes the columns.
    let html = page(
        ".g { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)) }",
        "<div class='g'><p>a</p><p>bbbbbbbbbbbbbbbbbbbb</p><p>c</p></div>",
    );
    let out = render(&html);
    assert!(
        out.warnings.is_empty(),
        "tailwind shape is fully in-subset: {:?}",
        out.warnings
    );
    let third = 400.0 / 3.0;
    assert_close(
        find(&out, "bbbb").x,
        third,
        "col 2 x (equal despite long content)",
    );
    assert_close(find(&out, "c").x, 2.0 * third, "col 3 x");
}

#[test]
fn fixed_and_fr_tracks_mix() {
    // 100px = 75pt; remaining 325pt goes to the fr track.
    let html = page(
        ".g { display: grid; grid-template-columns: 100px 1fr }",
        "<div class='g'><p>left</p><p>right</p></div>",
    );
    let out = render(&html);
    assert_close(find(&out, "left").x, 0.0, "fixed col x");
    assert_close(find(&out, "right").x, 75.0, "fr col x after 75pt track");
}

#[test]
fn column_span_consumes_spanned_tracks() {
    // The banner text is ~290pt wide at the default font: it fits one line
    // only if the span grants it both 200pt tracks. A broken span would
    // wrap it and double its height vs the single-line "a".
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr } .wide { grid-column: span 2 }",
        "<div class='g'><p class='wide'>banner banner banner banner banner banner it</p><p>uno</p><p>dos</p></div>",
    );
    let out = render(&html);
    let banner = find(&out, "banner");
    assert_close(banner.x, 0.0, "spanning item x");
    let single = find(&out, "uno");
    assert!(
        banner.height < single.height * 1.5,
        "spanning text stays on one line (span grants both tracks): banner h={} vs single h={}",
        banner.height,
        single.height
    );
    // The two singles land on the next row, one per column.
    assert_close(single.x, 0.0, "row-2 col-1 x");
    assert_close(find(&out, "dos").x, 200.0, "row-2 col-2 x");
    assert!(single.y > banner.y, "singles placed below the spanning row");
}

#[test]
fn explicit_column_placement() {
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr 1fr } .third { grid-column: 3 }",
        "<div class='g'><p class='third'>pinned</p><p>auto</p></div>",
    );
    let out = render(&html);
    assert_close(
        find(&out, "pinned").x,
        2.0 * (400.0 / 3.0),
        "explicit col 3 x",
    );
    assert_close(
        find(&out, "auto").x,
        0.0,
        "auto-placed fills the first free cell",
    );
}

#[test]
fn start_slash_end_line_placement() {
    // ~150pt of text: fits the 200pt two-track span on one line, but would
    // wrap in a single 100pt track — the height proves the end line held.
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr 1fr 1fr } .mid { grid-column: 2 / 4 }",
        "<div class='g'><p class='mid'>middle middle middle x</p><p>solo</p></div>",
    );
    let out = render(&html);
    let mid = find(&out, "middle");
    assert_close(mid.x, 100.0, "line 2 starts at 100");
    let solo = find(&out, "solo");
    assert!(
        mid.height < solo.height * 1.5,
        "two-track span keeps the text on one line: mid h={} vs solo h={}",
        mid.height,
        solo.height
    );
}

#[test]
fn gap_two_value_and_longhands() {
    // gap: <row> <col>. 20px col gap = 15pt: cols at 0 and 192.5+15.
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 20px }",
        "<div class='g'><p>a</p><p>b</p><p>c</p><p>d</p></div>",
    );
    let out = render(&html);
    let col_w = (400.0 - 15.0) / 2.0;
    assert_close(find(&out, "b").x, col_w + 15.0, "column gap applied");
    let (a, c) = (find(&out, "a"), find(&out, "c"));
    assert!(
        c.y >= a.y + a.height + 7.5 - 0.5,
        "row gap applied: c.y={} vs a bottom={}",
        c.y,
        a.y + a.height
    );

    // Longhands override the shorthand.
    let html2 = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; column-gap: 40px }",
        "<div class='g'><p>a</p><p>b</p></div>",
    );
    let out2 = render(&html2);
    let col_w2 = (400.0 - 30.0) / 2.0;
    assert_close(
        find(&out2, "b").x,
        col_w2 + 30.0,
        "column-gap longhand wins over gap",
    );
}

#[test]
fn repeat_with_multi_track_pattern_expands() {
    let html = page(
        ".g { display: grid; grid-template-columns: repeat(2, 50pt 1fr) }",
        "<div class='g'><p>a</p><p>b</p><p>c</p><p>d</p></div>",
    );
    let out = render(&html);
    // Tracks: 50, fr, 50, fr => fr = (400-100)/2 = 150.
    assert_close(find(&out, "b").x, 50.0, "first fr col x");
    assert_close(find(&out, "c").x, 200.0, "second fixed col x");
    assert_close(find(&out, "d").x, 250.0, "second fr col x");
}

#[test]
fn grid_template_rows_and_auto_rows() {
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr; grid-template-rows: 100pt; grid-auto-rows: 50pt }",
        "<div class='g'><p>first</p><p>second</p><p>third</p></div>",
    );
    let out = render(&html);
    assert_close(find(&out, "second").y, 100.0, "explicit 100pt row 1");
    assert_close(find(&out, "third").y, 150.0, "implicit 50pt row 2");
}

// ── Warnings: everything OUT is named, never silent ────────────────

#[test]
fn negative_line_numbers_warn_instead_of_clamping() {
    // The engine clamps negatives to line 1 — CSS counts from the end.
    // Mapping would silently mislay out; warning is the contract.
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr } .x { grid-column: -1 }",
        "<div class='g'><p class='x'>end</p></div>",
    );
    let out = render(&html);
    warned(&out, "negative grid line");
}

#[test]
fn grid_template_areas_warns() {
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr 1fr; grid-template-areas: \"a b\" }",
        "<div class='g'><p>x</p></div>",
    );
    warned(&render(&html), "grid-template-areas");
}

#[test]
fn grid_auto_flow_warns() {
    let html = page(
        ".g { display: grid; grid-template-columns: 1fr; grid-auto-flow: column }",
        "<div class='g'><p>x</p></div>",
    );
    warned(&render(&html), "grid-auto-flow");
}

#[test]
fn percentage_tracks_warn() {
    let html = page(
        ".g { display: grid; grid-template-columns: 50% 50% }",
        "<div class='g'><p>x</p></div>",
    );
    warned(&render(&html), "percentage");
}

#[test]
fn auto_fit_and_auto_fill_warn() {
    let html = page(
        ".g { display: grid; grid-template-columns: repeat(auto-fill, 100px) }",
        "<div class='g'><p>x</p></div>",
    );
    warned(&render(&html), "auto-fill");
}

#[test]
fn named_lines_warn() {
    let html = page(
        ".g { display: grid; grid-template-columns: [main-start] 1fr [main-end] }",
        "<div class='g'><p>x</p></div>",
    );
    warned(&render(&html), "named grid line");
}

#[test]
fn minmax_with_flexible_max_and_nonzero_min_warns() {
    // The engine's MinMax never joins fr distribution — "at least 40px,
    // flex the rest" cannot be expressed and would content-size silently.
    let html = page(
        ".g { display: grid; grid-template-columns: minmax(40px, 1fr) 1fr }",
        "<div class='g'><p>x</p><p>y</p></div>",
    );
    warned(&render(&html), "minmax");
}

#[test]
fn display_grid_without_template_warns_and_stacks() {
    let html = page(
        ".g { display: grid }",
        "<div class='g'><p>one</p><p>two</p></div>",
    );
    let out = render(&html);
    warned(&out, "grid-template-columns");
    // Block-stacking behavior: both at x 0, stacked vertically.
    assert_close(find(&out, "one").x, 0.0, "stacked x");
    assert_close(find(&out, "two").x, 0.0, "stacked x");
    assert!(find(&out, "two").y > find(&out, "one").y, "vertical stack");
}

#[test]
fn content_sized_track_keywords_warn() {
    let html = page(
        ".g { display: grid; grid-template-columns: min-content 1fr }",
        "<div class='g'><p>x</p><p>y</p></div>",
    );
    warned(&render(&html), "min-content");
}
