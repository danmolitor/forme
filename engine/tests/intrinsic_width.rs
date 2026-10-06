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
