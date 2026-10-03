use forme::font::{fallback::segment_by_font, FontContext, FontRegistry};
use forme::style::{Color, FontStyle, Hyphens, TextDecoration};
use forme::text::{StyledChar, TextLayout};

#[test]
fn registered_font_covers_supplementary_characters() {
    let mut registry = FontRegistry::new();
    registry.register(
        "Emoji",
        400,
        false,
        include_bytes!("fixtures/fonts/NotoEmoji-Regular.ttf").to_vec(),
    );
    let font = registry.resolve("Emoji", 400, false);
    for ch in ['💻', '🚀', '😀'] {
        assert!(font.has_char(ch), "registered font lost {ch}");
        assert_eq!(
            registry
                .resolve_for_char("Helvetica, Emoji", ch, 400, false)
                .1,
            "Emoji"
        );
    }
}

#[test]
fn fallback_keeps_shaping_controls_with_the_base_font() {
    let registry = FontRegistry::new();
    for text in ["П\u{200d}П\u{fe0f}", "П\u{200c}П\u{e0100}"] {
        let chars: Vec<_> = text.chars().collect();
        let runs = segment_by_font(&chars, "Helvetica, Noto Sans", 400, false, &registry);
        assert_eq!(runs.len(), 1, "split shaping cluster in {text:?}");
        assert_eq!(runs[0].family, "Noto Sans");
        assert_eq!(runs[0].end, chars.len());
    }
}

#[test]
fn styled_fallback_uses_shaped_widths_for_line_breaking() {
    let mut context = FontContext::new();
    context.registry_mut().register(
        "Devanagari",
        400,
        false,
        include_bytes!("fixtures/fonts/NotoSansDevanagari-Regular.ttf").to_vec(),
    );
    let layout = TextLayout::new();
    let measure = |family: &str, width: f64| {
        let chars: Vec<_> = "नमस्तेनमस्ते"
            .chars()
            .map(|ch| StyledChar {
                ch,
                font_family: family.to_string(),
                font_size: 14.0,
                font_weight: 400,
                font_style: FontStyle::Normal,
                color: Color::BLACK,
                href: None,
                text_decoration: TextDecoration::None,
                letter_spacing: 0.0,
                word_spacing: 0.0,
            })
            .collect();
        layout.break_runs_into_lines(&context, &chars, width, Hyphens::None, None)
    };
    let direct = measure("Devanagari", 1000.0);
    let fallback = measure("Noto Sans, Devanagari", 1000.0);
    assert!(
        (direct[0].width - fallback[0].width).abs() < 0.001,
        "same font measured differently: {} vs {}",
        direct[0].width,
        fallback[0].width
    );
    assert_eq!(
        measure("Noto Sans, Devanagari", direct[0].width + 0.001).len(),
        1
    );
}

#[test]
fn plain_and_styled_text_resolve_unencodable_whitespace_alike() {
    // U+202F (narrow no-break space, French number grouping) is whitespace
    // with no WinAnsi byte. Both paths must draw it in builtin Noto Sans,
    // not "?" in Helvetica.
    let text = "Total 1\u{202F}000\u{202F}€ due";
    let plain = serde_json::json!({ "type": "Text", "content": text });
    let styled = serde_json::json!({
        "type": "Text", "content": "", "runs": [{ "content": text }]
    });
    for (path, kind) in [("plain", plain), ("styled", styled)] {
        let doc: forme::Document = serde_json::from_value(serde_json::json!({
            "children": [{ "kind": kind, "style": {}, "children": [] }],
            "metadata": {}
        }))
        .unwrap();
        let (pdf, warnings) = forme::render_with_warnings(&doc).unwrap();
        assert!(
            !warnings.iter().any(|w| w.contains("U+202F")),
            "{path} text drew U+202F as \"?\": {warnings:?}"
        );
        assert!(
            String::from_utf8_lossy(&pdf).contains("NotoSans"),
            "{path} text did not fall back to Noto Sans"
        );
    }
}

#[test]
fn controls_stay_with_a_registered_font_under_pdfa() {
    // A tab or newline has no glyph in any font. Resolving it by coverage
    // pulled styled runs into Helvetica, which PDF/A cannot embed.
    let font = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        include_bytes!("fixtures/fonts/NotoSansDevanagari-Regular.ttf"),
    );
    let text = "नम\tस्ते\nनमस्ते";
    let plain = serde_json::json!({ "type": "Text", "content": text });
    let styled = serde_json::json!({
        "type": "Text", "content": "", "runs": [{ "content": text }]
    });
    for (path, kind) in [("plain", plain), ("styled", styled)] {
        let doc: forme::Document = serde_json::from_value(serde_json::json!({
            "children": [{ "kind": kind, "style": { "fontFamily": "Dev" }, "children": [] }],
            "metadata": {},
            "pdfa": "2b",
            "fonts": [{
                "family": "Dev",
                "src": format!("data:font/ttf;base64,{font}"),
                "weight": 400,
                "italic": false
            }]
        }))
        .unwrap();
        if let Err(e) = forme::render(&doc) {
            panic!("{path} text with controls failed under PDF/A: {e}");
        }
    }
}

#[test]
fn styled_runs_keep_an_unregistered_family_its_font_covers() {
    // A family nobody registered resolves to Helvetica. Styled runs went
    // through per-char resolution, which skips unregistered families and
    // tries builtin Noto Sans first, so a table cell or a <b> in
    // "DejaVu Sans" drew in Noto Sans while plain text stayed Helvetica.
    let text = "Acme Corp";
    let plain = serde_json::json!({ "type": "Text", "content": text });
    let styled = serde_json::json!({
        "type": "Text", "content": "", "runs": [{ "content": text }]
    });
    for (path, kind) in [("plain", plain), ("styled", styled)] {
        let doc: forme::Document = serde_json::from_value(serde_json::json!({
            "children": [{ "kind": kind, "style": { "fontFamily": "DejaVu Sans" }, "children": [] }],
            "metadata": {}
        }))
        .unwrap();
        let pdf = String::from_utf8_lossy(&forme::render(&doc).unwrap()).into_owned();
        assert!(pdf.contains("/Helvetica"), "{path} text left Helvetica");
        assert!(
            !pdf.contains("NotoSans"),
            "{path} text fell back to Noto Sans"
        );
    }
}
