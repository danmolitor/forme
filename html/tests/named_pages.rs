//! Named pages — `@page <name>` + the `page` property (fails-first).
//!
//! Design: a named run STARTS at a forced page break, so its pages may
//! genuinely differ vertically (real cursor config). Horizontally the flow
//! is baked at the base width, so — exactly like `:left`/`:right` — named
//! margins must be mirrored (equal left+right sum) and are applied as a
//! constant x translation. Precedence per CSS Paged Media specificity
//! (f, g, h): a named rule (f) outranks `:first` (g), which outranks
//! parity (h); `@page name:first` composes.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlLayoutOutput, HtmlOptions};

/// 400x400pt page, base margins 40pt vertical / 50pt horizontal (sum 100).
fn doc(extra_css: &str, body: &str) -> String {
    format!(
        "<html><head><style>\
         @page {{ size: 400pt 400pt; margin: 40pt 50pt }} \
         body {{ margin: 0 }} p {{ margin: 0 0 10pt 0 }} \
         {extra_css}</style></head><body>{body}</body></html>"
    )
}

fn many_paragraphs(n: usize) -> String {
    (0..n)
        .map(|i| format!("<p>para{i} filler text that occupies a line</p>"))
        .collect()
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

fn find_on<'a>(out: &'a HtmlLayoutOutput, page: usize, needle: &str) -> &'a ElementInfo {
    let mut found: Option<&ElementInfo> = None;
    walk(&out.layout.pages[page].elements, &mut |el| {
        if found.is_none() {
            if let Some(t) = &el.text_content {
                if t.contains(needle) {
                    found = Some(el);
                }
            }
        }
    });
    found.unwrap_or_else(|| panic!("element containing {needle:?} not found on page {page}"))
}

fn page_of(out: &HtmlLayoutOutput, needle: &str) -> usize {
    for (i, page) in out.layout.pages.iter().enumerate() {
        let mut hit = false;
        walk(&page.elements, &mut |el| {
            if let Some(t) = &el.text_content {
                if t.contains(needle) {
                    hit = true;
                }
            }
        });
        if hit {
            return i;
        }
    }
    panic!("element containing {needle:?} not found on any page");
}

fn first_text_x(out: &HtmlLayoutOutput, page: usize) -> f64 {
    let mut x: Option<f64> = None;
    walk(&out.layout.pages[page].elements, &mut |el| {
        if x.is_none() && el.node_type == "TextLine" {
            x = Some(el.x);
        }
    });
    x.unwrap_or_else(|| panic!("no text line on page {page}"))
}

fn first_text_y(out: &HtmlLayoutOutput, page: usize) -> f64 {
    let mut y: Option<f64> = None;
    walk(&out.layout.pages[page].elements, &mut |el| {
        if y.is_none() && el.node_type == "TextLine" {
            y = Some(el.y);
        }
    });
    y.unwrap_or_else(|| panic!("no text line on page {page}"))
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

// ── Selection: the named run's pages use the named config ──────────

#[test]
fn named_page_selects_config_for_its_run() {
    // Mirrored horizontal (80+20 = base 100) plus a REAL vertical margin:
    // the run starts at a forced break, so top margin may genuinely vary.
    let html = doc(
        "@page cover { margin-left: 80pt; margin-right: 20pt; margin-top: 10pt } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\"><p>coverline alpha</p></div>{}",
            many_paragraphs(40)
        ),
    );
    let out = render(&html);
    assert!(out.warnings.is_empty(), "in-subset: {:?}", out.warnings);
    assert!(out.layout.pages.len() >= 2);

    assert_close(first_text_x(&out, 0), 80.0, "cover page x (translated)");
    assert_close(
        first_text_y(&out, 0),
        10.0,
        "cover page y (real margin-top)",
    );
    assert_close(first_text_x(&out, 1), 50.0, "page 2 back to base x");
    assert_close(first_text_y(&out, 1), 40.0, "page 2 back to base y");
    assert_close(out.layout.pages[0].content_x, 80.0, "cover content_x");
}

#[test]
fn named_region_mid_document_breaks_before_and_after() {
    let html = doc(
        "@page summary { margin-left: 70pt; margin-right: 30pt } \
         .summary { page: summary }",
        &format!(
            "{}<div class=\"summary\"><p>summarywords here</p></div><p>afterwords resume</p>",
            many_paragraphs(3)
        ),
    );
    let out = render(&html);
    let p_before = page_of(&out, "para0");
    let p_named = page_of(&out, "summarywords");
    let p_after = page_of(&out, "afterwords");
    assert!(
        p_named > p_before,
        "named region breaks BEFORE: para0 on {p_before}, summary on {p_named}"
    );
    assert!(
        p_after > p_named,
        "named region breaks AFTER: summary on {p_named}, after on {p_after}"
    );
    assert_close(
        find_on(&out, p_named, "summarywords").x,
        70.0,
        "named page x",
    );
    assert_close(
        find_on(&out, p_after, "afterwords").x,
        50.0,
        "after page back to base x",
    );
}

// ── Precedence: named (f) > :first (g); name:first composes ────────

#[test]
fn named_outranks_first_on_page_one() {
    let html = doc(
        "@page :first { margin-left: 90pt; margin-right: 10pt } \
         @page cover { margin-left: 80pt; margin-right: 20pt } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\"><p>coverline alpha</p></div>{}",
            many_paragraphs(40)
        ),
    );
    let out = render(&html);
    // Page 1 is named: the named rule (specificity f) wins over :first (g).
    assert_close(first_text_x(&out, 0), 80.0, "page 1 uses @page cover");
}

#[test]
fn named_first_composes_on_page_one_of_the_run() {
    // A cover run spanning pages 1-2: page 1 matches cover:first (f+g),
    // page 2 matches plain cover (f). :first means the document's first
    // page, per spec.
    let html = doc(
        "@page cover { margin-left: 80pt; margin-right: 20pt } \
         @page cover:first { margin-left: 60pt; margin-right: 40pt } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\">{}</div><p>afterwords resume</p>",
            many_paragraphs(40)
        ),
    );
    let out = render(&html);
    assert!(out.layout.pages.len() >= 3, "cover spans 2+ pages");
    assert_close(first_text_x(&out, 0), 60.0, "page 1 uses cover:first");
    assert_close(first_text_x(&out, 1), 80.0, "page 2 uses cover");
    let p_after = page_of(&out, "afterwords");
    assert_close(
        find_on(&out, p_after, "afterwords").x,
        50.0,
        "after the run: base x",
    );
}

// ── Margin boxes resolve per name ──────────────────────────────────

#[test]
fn named_page_margin_boxes_override_and_suppress() {
    // Base header on every page; cover replaces it. The cover band shows
    // COVER, other pages show BASE, and BASE never leaks onto the cover.
    let html = doc(
        "@page { @top-center { content: \"BASE\" } } \
         @page cover { @top-center { content: \"COVER\" } } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\"><p>coverline alpha</p></div>{}",
            many_paragraphs(40)
        ),
    );
    let out = render(&html);
    find_on(&out, 0, "COVER");
    find_on(&out, 1, "BASE");
    let mut base_on_cover = false;
    walk(&out.layout.pages[0].elements, &mut |el| {
        if let Some(t) = &el.text_content {
            if t.contains("BASE") {
                base_on_cover = true;
            }
        }
    });
    assert!(!base_on_cover, "cover's @top-center replaces the base box");
}

#[test]
fn named_page_suppressing_band_restores_margin() {
    // The classic cover: no running header, and content must start at the
    // REAL top margin (40pt), not at the physical paper edge where the
    // zeroed band-margin would put it.
    let html = doc(
        "@page { @top-center { content: \"BASE\" } } \
         @page cover { @top-center { content: none } } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\"><p>coverline alpha</p></div>{}",
            many_paragraphs(40)
        ),
    );
    let out = render(&html);
    let mut base_on_cover = false;
    walk(&out.layout.pages[0].elements, &mut |el| {
        if let Some(t) = &el.text_content {
            if t.contains("BASE") {
                base_on_cover = true;
            }
        }
    });
    assert!(!base_on_cover, "content: none suppresses the band on cover");
    find_on(&out, 1, "BASE");
    assert_close(
        first_text_y(&out, 0),
        40.0,
        "cover content starts at the restored margin",
    );
}

// ── The honest boundary: warnings by name ──────────────────────────

#[test]
fn unequal_sum_on_named_page_warns_and_normalizes() {
    let html = doc(
        "@page cover { margin-left: 30pt; margin-right: 80pt } \
         .cover { page: cover }",
        &format!(
            "<div class=\"cover\"><p>coverline alpha</p></div>{}",
            many_paragraphs(10)
        ),
    );
    let out = render(&html);
    warned(&out, "must sum equally");
    assert_close(first_text_x(&out, 0), 50.0, "cover normalized to base x");
}

#[test]
fn vertical_margins_on_named_parity_slots_warn() {
    // Vertical variation is real for the named BASE slot (the run starts
    // at a break) but :left/:right selection happens mid-run per parity —
    // translation only, so vertical overrides there warn and normalize.
    let html = doc(
        "@page cover { margin-left: 80pt; margin-right: 20pt } \
         @page cover:left { margin-left: 20pt; margin-right: 80pt; margin-top: 80pt } \
         .cover { page: cover }",
        &format!("<div class=\"cover\">{}</div>", many_paragraphs(40)),
    );
    let out = render(&html);
    warned(&out, "top/bottom margins");
    // The mirrored horizontal still applies: page 2 of the run is a left
    // page (doc parity), x = 20.
    assert_close(first_text_x(&out, 1), 20.0, "run page 2 keeps mirrored x");
}

#[test]
fn unknown_page_name_still_breaks_with_base_config() {
    // `page: chapter` with no `@page chapter` rule is valid CSS: the name
    // still forces breaks between differently named boxes; the base
    // config applies.
    let html = doc(
        ".ch { page: chapter }",
        &format!(
            "{}<div class=\"ch\"><p>chapterwords here</p></div>",
            many_paragraphs(3)
        ),
    );
    let out = render(&html);
    let p_named = page_of(&out, "chapterwords");
    assert!(
        p_named > page_of(&out, "para0"),
        "break before still happens"
    );
    assert_close(
        find_on(&out, p_named, "chapterwords").x,
        50.0,
        "base config on the unmatched named page",
    );
}

#[test]
fn out_of_scope_paged_features_warn_by_name() {
    // Each named-by-spec feature outside the subset warns with its name.
    let html = doc(
        "@page :blank { margin-top: 99pt } \
         @page :nth(2) { margin-top: 99pt } \
         h1 { string-set: title content() } \
         .run { position: running(header) } \
         @page { @top-center { content: string(title) } }",
        &format!("<div class=\"run\">runner</div>{}", many_paragraphs(3)),
    );
    let out = render(&html);
    warned(&out, ":blank");
    warned(&out, ":nth");
    warned(&out, "string-set");
    warned(&out, "running");
    warned(&out, "string(");
}
