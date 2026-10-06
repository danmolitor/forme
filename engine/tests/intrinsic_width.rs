//! A text item sized by its content must hold the line it is sized for.
//!
//! Intrinsic width summed each character's unshaped advance, while both line
//! breakers measure shaped text. With GPOS kerning that widens a word, the
//! box came out a fraction of a point narrower than the line, and the breaker
//! split the word: "Mumbai" in IBM Plex Serif 10pt drew as "Mumba" / "i"
//! (#181), as plain text and as a nested run alike.

use forme::font::FontContext;
use forme::layout::{DrawCommand, LayoutElement, LayoutEngine};
use forme::model::*;

const PLEX: &[u8] = include_bytes!("fixtures/fonts/IBMPlexSerif-Regular.ttf");

/// Every drawn text line on the first page, as its characters.
fn lines(els: &[LayoutElement], out: &mut Vec<String>) {
    for el in els {
        if let DrawCommand::Text { lines: ls, .. } = &el.draw {
            for line in ls {
                out.push(line.glyphs.iter().map(|g| g.char_value).collect());
            }
        }
        lines(&el.children, out);
    }
}

/// Lay out `word` beside "Acme" in a 200pt space-between row, as plain text
/// or as one nested run, and return the lines it was drawn on.
fn layout_row(word: &str, runs: bool) -> Vec<String> {
    let second = if runs {
        serde_json::json!({ "type": "Text", "content": "", "runs": [{ "content": word }] })
    } else {
        serde_json::json!({ "type": "Text", "content": word })
    };
    let text_style = serde_json::json!({ "fontFamily": "Plex", "fontSize": 10 });
    let doc: Document = serde_json::from_value(serde_json::json!({
        "children": [{
            "kind": { "type": "View" },
            "style": { "flexDirection": "Row", "justifyContent": "SpaceBetween", "width": { "Pt": 200 } },
            "children": [
                { "kind": { "type": "Text", "content": "Acme" }, "style": text_style, "children": [] },
                { "kind": second, "style": text_style, "children": [] }
            ]
        }],
        "metadata": {}
    }))
    .unwrap();
    let mut font_context = FontContext::new();
    font_context
        .registry_mut()
        .register("Plex", 400, false, PLEX.to_vec());
    let pages = LayoutEngine::new().layout(&doc, &font_context);
    let mut out = Vec::new();
    lines(&pages[0].elements, &mut out);
    out.retain(|l| l != "Acme");
    out
}

#[test]
fn content_sized_text_holds_its_shaped_line() {
    for word in ["Mumbai", "Mumbai,", "Bengaluru"] {
        for runs in [false, true] {
            let path = if runs { "nested run" } else { "plain text" };
            assert_eq!(
                layout_row(word, runs),
                vec![word.to_string()],
                "{word:?} as {path} must stay on one line"
            );
        }
    }
}

/// The width of the text box a row lays out for `kind`.
fn text_box_width(kind: serde_json::Value) -> f64 {
    let doc: Document = serde_json::from_value(serde_json::json!({
        "children": [{
            "kind": { "type": "View" },
            "style": { "flexDirection": "Row" },
            "children": [{ "kind": kind, "style": { "fontFamily": "Plex", "fontSize": 10 }, "children": [] }]
        }],
        "metadata": {}
    }))
    .unwrap();
    let mut font_context = FontContext::new();
    font_context
        .registry_mut()
        .register("Plex", 400, false, PLEX.to_vec());
    let pages = LayoutEngine::new().layout(&doc, &font_context);
    // The Text box, not one of the TextLine elements inside it.
    fn find(els: &[LayoutElement]) -> Option<f64> {
        els.iter().find_map(|el| {
            if el.node_type.as_deref() == Some("Text") {
                Some(el.width)
            } else {
                find(&el.children)
            }
        })
    }
    find(&pages[0].elements).expect("a text element")
}

/// A hard break restarts the line, so content-sized text with two lines is
/// as wide as its wider line. Runs summed every run's width instead, so an
/// HTML cell like `<span>Website Design</span><br><span>…</span>` measured
/// as both lines laid end to end and took column width from its neighbours.
#[test]
fn styled_text_with_a_hard_break_is_as_wide_as_its_wider_line() {
    let long = "Homepage and landing page design";
    let two_lines = text_box_width(serde_json::json!({
        "type": "Text", "content": "",
        "runs": [{ "content": "Website Design" }, { "content": "\n" }, { "content": long }]
    }));
    let one_line = text_box_width(serde_json::json!({ "type": "Text", "content": long }));
    assert!(
        (two_lines - one_line).abs() < 0.001,
        "two lines measured {two_lines:.2}pt, the wider line alone {one_line:.2}pt"
    );
}
