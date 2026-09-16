//! `word-spacing` reaches the engine.
//!
//! The property was in neither column of the README's CSS subset table and
//! `word-spacing` appeared zero times in `css.rs`, so a reader could not find
//! out it was unsupported without rendering and reading the warnings.
//!
//! TWO THINGS TO KNOW BEFORE READING THE ASSERTIONS.
//!
//! It is block-level only, deliberately. Both engine read sites take the value
//! from the text node's own resolved style, never from a run, so it is not
//! carried on `RunStyle`. Putting it there would create a field nothing reads,
//! which is the exact defect class this fix closes.
//!
//! And it is applied at PDF-WRITE time, through the `Tw` operator, not during
//! text measurement. `letter_spacing` is threaded through `text/mod.rs` and so
//! changes line breaking and reported widths; `word_spacing` appears nowhere
//! in that file. A word-spaced paragraph therefore renders with the spacing
//! but breaks its lines as though it had none. That asymmetry is pre-existing
//! and engine-side, it applies equally to the JSX path, and it is tracked
//! separately. It is also why these tests read the content stream instead of
//! measuring the layout: the layout genuinely cannot see this property, and a
//! width-based assertion passes vacuously by comparing two identical renders.

use forme_pdf_html::{render_html, HtmlOptions};

/// Every `Tw` operand in the document's content streams.
fn word_spacings(pdf: &[u8]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = find(rest, b"stream") {
        let after = &rest[i + b"stream".len()..];
        let body = match after.first() {
            Some(b'\r') => &after[2..],
            Some(b'\n') => &after[1..],
            _ => after,
        };
        let end = find(body, b"endstream").unwrap_or(body.len());
        if let Ok(text) = miniz_oxide::inflate::decompress_to_vec_zlib(&body[..end]) {
            let text = String::from_utf8_lossy(&text);
            for (idx, _) in text.match_indices(" Tw") {
                let head = &text[..idx];
                if let Some(tok) = head.split_whitespace().next_back() {
                    if let Ok(v) = tok.parse::<f64>() {
                        out.push(v);
                    }
                }
            }
        }
        rest = &rest[i + b"stream".len()..];
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn render(html: &str) -> Vec<u8> {
    render_html(html, &HtmlOptions::default())
        .expect("must render")
        .pdf
}

const LINE: &str = "alpha beta gamma delta epsilon";

#[test]
fn word_spacing_reaches_the_pdf() {
    let with = word_spacings(&render(&format!(
        r#"<p style="word-spacing:5pt">{LINE}</p>"#
    )));
    let without = word_spacings(&render(&format!(r#"<p>{LINE}</p>"#)));
    assert!(
        without.is_empty(),
        "control: no word spacing means no Tw operator, got {without:?}"
    );
    assert!(
        with.iter().any(|v| (v - 5.0).abs() < 0.01),
        "word-spacing:5pt must emit `5 Tw`, got {with:?}"
    );
}

#[test]
fn word_spacing_accepts_relative_units() {
    // Follows the unit handling established for gap and border-radius: the
    // length keeps its unit and resolves against the element's own font size.
    let em = word_spacings(&render(&format!(
        r#"<p style="font-size:16pt; word-spacing:0.5em">{LINE}</p>"#
    )));
    assert!(
        em.iter().any(|v| (v - 8.0).abs() < 0.01),
        "0.5em at font-size 16pt must emit `8 Tw`, got {em:?}"
    );
}

/// Guards the `normal` keyword against being parsed as a length. Note that
/// this one passes with or without the fix, since "no `Tw` at all" also
/// satisfies "every `Tw` is zero". It is here to pin the keyword, not as
/// evidence that the feature works; the other four are the evidence.
#[test]
fn word_spacing_normal_is_zero() {
    let normal = word_spacings(&render(&format!(
        r#"<p style="word-spacing:normal">{LINE}</p>"#
    )));
    assert!(
        normal.iter().all(|v| v.abs() < 0.01),
        "`normal` is the initial value, not an unparsed declaration: {normal:?}"
    );
}

#[test]
fn word_spacing_survives_the_cascade_merge() {
    // `CssStyle::merge` uses a hand-written field list. A property missing
    // from it parses correctly and is then dropped between cascade layers,
    // which would be this same bug one step further along.
    let merged = word_spacings(&render(&format!(
        r#"<style>p {{ word-spacing: 5pt }} p {{ color: #222 }}</style><p>{LINE}</p>"#
    )));
    assert!(
        merged.iter().any(|v| (v - 5.0).abs() < 0.01),
        "word-spacing from a stylesheet rule survives merging with a later \
         rule, got {merged:?}"
    );
}

#[test]
fn word_spacing_no_longer_warns_as_unsupported() {
    let out = render_html(
        r#"<p style="word-spacing:4pt">alpha beta</p>"#,
        &HtmlOptions::default(),
    )
    .expect("renders");
    assert!(
        !out.warnings.iter().any(|w| w.contains("word-spacing")),
        "a supported property must not appear in the warnings: {:?}",
        out.warnings
    );
}

// ─── Measurement (issue #135) ────────────────────────────────────────
//
// `word_spacing` was applied at PDF-write time through `Tw` and appeared
// nowhere in `engine/src/text/mod.rs`, so lines were broken as though it were
// zero and the drawn text came out wider than the box it was measured into.
// `letter_spacing` was threaded through that file in 36 places; this one in
// none. These assert on LAYOUT, which is now the right instrument: before the
// fix the property could not be observed there at all.

fn layout_of(html: &str) -> forme_pdf_html::HtmlLayoutOutput {
    forme_pdf_html::render_html_with_layout(html, &HtmlOptions::default()).expect("renders")
}

/// Every drawn line's width, in order.
fn line_widths(out: &forme_pdf_html::HtmlLayoutOutput) -> Vec<f64> {
    fn walk(el: &forme::layout::ElementInfo, into: &mut Vec<f64>) {
        if el.text_content.is_some() {
            into.push(el.width);
        }
        for c in &el.children {
            walk(c, into);
        }
    }
    let mut v = Vec::new();
    for p in &out.layout.pages {
        for el in &p.elements {
            walk(el, &mut v);
        }
    }
    v
}

const EIGHT_WORDS: &str = "alpha beta gamma delta epsilon zeta eta theta";

#[test]
fn word_spacing_changes_where_lines_break() {
    // The reported shape: text that fits on one line unspaced must wrap once
    // the spacing is counted, instead of being drawn past the box edge.
    let plain = layout_of(&format!(
        r#"<div style="width:260pt; font-size:12pt">{EIGHT_WORDS}</div>"#
    ));
    let spaced = layout_of(&format!(
        r#"<div style="width:260pt; font-size:12pt; word-spacing:12pt">{EIGHT_WORDS}</div>"#
    ));
    assert_eq!(
        line_widths(&plain).len(),
        1,
        "control: unspaced, this fits one line"
    );
    let spaced_lines = line_widths(&spaced);
    assert_eq!(
        spaced_lines.len(),
        2,
        "seven spaces at 12pt add 84pt, which cannot fit: {spaced_lines:?}"
    );
}

#[test]
fn the_measured_width_includes_the_word_spacing() {
    // Asserting that every line fits its box would be VACUOUS here, and the
    // first version of this test did exactly that: before the fix the layout
    // reported ~246pt for a line the PDF drew 74pt wider, so a fits-the-box
    // assertion passed while reading the engine's own wrong belief. The
    // measurable claim is that the reported width now CONTAINS the spacing.
    //
    // A wide box so nothing wraps and the comparison is line-for-line: six
    // words, five spaces, 10pt each, so exactly 50pt wider.
    let words = "alpha beta gamma delta epsilon zeta";
    let plain = layout_of(&format!(
        r#"<div style="width:520pt; font-size:12pt">{words}</div>"#
    ));
    let spaced = layout_of(&format!(
        r#"<div style="width:520pt; font-size:12pt; word-spacing:10pt">{words}</div>"#
    ));
    let (a, b) = (line_widths(&plain)[0], line_widths(&spaced)[0]);
    assert_eq!(line_widths(&spaced).len(), 1, "control: still one line");
    let delta = b - a;
    assert!(
        (delta - 50.0).abs() < 1.0,
        "five spaces at 10pt must widen the measured line by 50pt: \
         {a:.1}pt -> {b:.1}pt, delta {delta:.1}pt"
    );
}

/// Passes with or without the fix, by construction: without it nothing
/// changes at all. It is here to stop the advance being applied per
/// character, which would be a larger and far less obvious error than not
/// applying it. Not evidence that the feature works.
#[test]
fn word_spacing_only_widens_spaces() {
    // A single word has no spaces, so the property must not touch it. Guards
    // against applying the advance per character, which would be a much
    // larger and much less obvious error than not applying it at all.
    let plain = layout_of(r#"<div style="font-size:12pt">antidisestablishmentarianism</div>"#);
    let spaced = layout_of(
        r#"<div style="font-size:12pt; word-spacing:12pt">antidisestablishmentarianism</div>"#,
    );
    let (a, b) = (line_widths(&plain)[0], line_widths(&spaced)[0]);
    assert!(
        (a - b).abs() < 0.5,
        "a word with no spaces is unaffected: {a:.1}pt vs {b:.1}pt"
    );
}
