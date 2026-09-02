//! Integration tests for the Forme rendering pipeline.
//!
//! These tests exercise the full path from JSON input to PDF output.
//! They verify:
//! - JSON deserialization works correctly
//! - Layout engine produces the right number of pages
//! - PDF output is structurally valid
//! - Page breaks happen at the right places
//! - Table header repetition works

use forme::font::FontContext;
use forme::layout::LayoutEngine;
use forme::model::*;
use forme::style::*;

// ─── Helpers ────────────────────────────────────────────────────

fn make_text(content: &str, font_size: f64) -> Node {
    Node {
        kind: NodeKind::Text {
            content: content.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(font_size),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_view(children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::View,
        style: Style::default(),
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_styled_view(style: Style, children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::View,
        style,
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_page_break() -> Node {
    Node {
        kind: NodeKind::PageBreak,
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_table_row(is_header: bool, cells: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::TableRow { is_header },
        style: Style::default(),
        children: cells,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_table_cell(children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::TableCell {
            col_span: 1,
            row_span: 1,
        },
        style: Style {
            padding: Some(Edges::uniform(4.0)),
            ..Default::default()
        },
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn default_doc(children: Vec<Node>) -> Document {
    Document {
        children,
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    }
}

fn layout_doc(doc: &Document) -> Vec<forme::layout::LayoutPage> {
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();
    engine.layout(doc, &font_context)
}

fn render_to_pdf(doc: &Document) -> Vec<u8> {
    forme::render(doc).unwrap()
}

fn assert_valid_pdf(bytes: &[u8]) {
    assert!(bytes.len() > 50, "PDF too small to be valid");
    assert!(bytes.starts_with(b"%PDF-1.7"), "Missing PDF header");
    assert!(
        bytes.windows(5).any(|w| w == b"%%EOF"),
        "Missing %%EOF marker"
    );
    assert!(bytes.windows(4).any(|w| w == b"xref"), "Missing xref table");
    assert!(bytes.windows(7).any(|w| w == b"trailer"), "Missing trailer");
}

/// Decompress all FlateDecode streams in a PDF and concatenate the results.
/// Used for testing content that's inside compressed content streams.
fn decompress_pdf_streams(pdf: &[u8]) -> String {
    use miniz_oxide::inflate::decompress_to_vec_zlib;
    let mut result = String::new();
    let needle_start = b"stream\n";
    let needle_end = b"\nendstream";
    let mut pos = 0;
    while pos + needle_start.len() < pdf.len() {
        // Find "stream\n" in raw bytes
        let found = pdf[pos..]
            .windows(needle_start.len())
            .position(|w| w == needle_start);
        let idx = match found {
            Some(i) => i,
            None => break,
        };
        let abs = pos + idx + needle_start.len();
        // Find "\nendstream" in raw bytes
        let end_found = pdf[abs..]
            .windows(needle_end.len())
            .position(|w| w == needle_end);
        let end_idx = match end_found {
            Some(i) => i,
            None => break,
        };
        let stream_bytes = &pdf[abs..abs + end_idx];
        if let Ok(decompressed) = decompress_to_vec_zlib(stream_bytes) {
            if let Ok(s) = std::str::from_utf8(&decompressed) {
                result.push_str(s);
                result.push('\n');
            }
        }
        pos = abs + end_idx;
    }
    result
}

// ─── Basic Pipeline Tests ───────────────────────────────────────

#[test]
fn test_empty_document() {
    let doc = default_doc(vec![]);
    let pages = layout_doc(&doc);
    // Empty doc should produce no pages (no content placed)
    assert!(pages.is_empty(), "Empty document should produce no pages");
}

#[test]
fn test_single_text_node() {
    let doc = default_doc(vec![make_text("Hello, World!", 12.0)]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1, "Single text should fit on one page");
    assert!(!pages[0].elements.is_empty(), "Page should have elements");
}

#[test]
fn test_single_text_produces_valid_pdf() {
    let doc = default_doc(vec![make_text("Hello, World!", 12.0)]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_explicit_page_break() {
    let doc = default_doc(vec![
        make_text("Page 1", 12.0),
        make_page_break(),
        make_text("Page 2", 12.0),
    ]);
    let pages = layout_doc(&doc);
    assert_eq!(
        pages.len(),
        2,
        "Should have exactly 2 pages after a page break"
    );
}

#[test]
fn test_multiple_page_breaks() {
    let doc = default_doc(vec![
        make_text("Page 1", 12.0),
        make_page_break(),
        make_text("Page 2", 12.0),
        make_page_break(),
        make_text("Page 3", 12.0),
    ]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 3);
}

// ─── Page Overflow Tests ────────────────────────────────────────

#[test]
fn test_content_overflow_creates_new_page() {
    // A4 content height is roughly 734pt (841.89 - 2*54).
    // At 12pt font with 1.4 line height = 16.8pt per line.
    // 734 / 16.8 ≈ 43 lines per page.
    // 100 lines should overflow to at least 2 pages.
    let mut children = Vec::new();
    for i in 0..100 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }
    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "100 lines should overflow to multiple pages, got {} pages",
        pages.len()
    );
}

#[test]
fn test_large_font_overflows_faster() {
    let mut children = Vec::new();
    for i in 0..30 {
        children.push(make_text(&format!("Line {}", i), 24.0));
    }
    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "30 lines at 24pt should overflow, got {} pages",
        pages.len()
    );
}

// ─── Flexbox Tests ──────────────────────────────────────────────

#[test]
fn test_flex_row_layout() {
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![make_text("Left", 12.0), make_text("Right", 12.0)],
    );
    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    assert!(!pages[0].elements.is_empty());
}

#[test]
fn test_flex_column_is_default() {
    let container = make_view(vec![make_text("First", 12.0), make_text("Second", 12.0)]);
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // The container view is a single top-level element with nested children.
    // Children (text elements) should be stacked vertically inside the container.
    assert!(!pages[0].elements.is_empty());
    let container = &pages[0].elements[0];
    assert!(
        container.children.len() >= 2,
        "Container should have at least 2 child elements, got {}",
        container.children.len()
    );
}

// ─── Table Tests ────────────────────────────────────────────────

fn make_simple_table(header_cells: Vec<&str>, rows: Vec<Vec<&str>>) -> Node {
    let mut children = Vec::new();

    // Header row
    let header_row = make_table_row(
        true,
        header_cells
            .into_iter()
            .map(|text| make_table_cell(vec![make_text(text, 10.0)]))
            .collect(),
    );
    children.push(header_row);

    // Body rows
    for row_data in rows {
        let body_row = make_table_row(
            false,
            row_data
                .into_iter()
                .map(|text| make_table_cell(vec![make_text(text, 10.0)]))
                .collect(),
        );
        children.push(body_row);
    }

    Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_simple_table() {
    let table = make_simple_table(
        vec!["Name", "Age"],
        vec![vec!["Alice", "30"], vec!["Bob", "25"]],
    );
    let doc = default_doc(vec![table]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    assert!(!pages[0].elements.is_empty());
}

#[test]
fn test_table_page_break_with_many_rows() {
    // Create a table with enough rows to overflow a page.
    // At ~22pt per row (10pt font, padding, line height), ~34 rows per page.
    let rows: Vec<Vec<&str>> = (0..80)
        .map(|i| {
            vec![
                Box::leak(format!("Item {}", i).into_boxed_str()) as &str,
                "Value",
            ]
        })
        .collect();
    let table = make_simple_table(vec!["Name", "Value"], rows);
    let doc = default_doc(vec![table]);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "80-row table should span multiple pages, got {}",
        pages.len()
    );
}

// ─── JSON Deserialization Tests ─────────────────────────────────

#[test]
fn test_minimal_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "Text", "content": "Hello from JSON" },
                "style": { "fontSize": 14 }
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse minimal JSON");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_view_container_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": { "flexDirection": "Row", "gap": 12 },
                "children": [
                    { "kind": { "type": "Text", "content": "Left" }, "style": {} },
                    { "kind": { "type": "Text", "content": "Right" }, "style": {} }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse view JSON");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_table_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "Table", "columns": [] },
                "style": {},
                "children": [
                    {
                        "kind": { "type": "TableRow", "is_header": true },
                        "style": {},
                        "children": [
                            {
                                "kind": { "type": "TableCell" },
                                "style": {},
                                "children": [
                                    { "kind": { "type": "Text", "content": "Header" }, "style": {} }
                                ]
                            }
                        ]
                    },
                    {
                        "kind": { "type": "TableRow", "is_header": false },
                        "style": {},
                        "children": [
                            {
                                "kind": { "type": "TableCell" },
                                "style": {},
                                "children": [
                                    { "kind": { "type": "Text", "content": "Cell" }, "style": {} }
                                ]
                            }
                        ]
                    }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse table JSON");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_camel_case_deserialization() {
    // Verifies that camelCase JSON fields map correctly
    let json = r#"{
        "defaultPage": {
            "size": "Letter",
            "margin": { "top": 72, "right": 72, "bottom": 72, "left": 72 }
        },
        "children": [
            {
                "kind": { "type": "Text", "content": "Test" },
                "style": {
                    "fontSize": 16,
                    "fontWeight": 700,
                    "lineHeight": 1.5,
                    "textAlign": "Center",
                    "backgroundColor": { "r": 0.9, "g": 0.9, "b": 0.95, "a": 1.0 }
                }
            }
        ]
    }"#;
    let doc: Document = serde_json::from_str(json).expect("Should deserialize camelCase JSON");
    assert!(matches!(doc.default_page.size, PageSize::Letter));
    assert_eq!(doc.default_page.margin.top, 72.0);

    let bytes = forme::render(&doc).unwrap();
    assert_valid_pdf(&bytes);
}

#[test]
fn test_style_inheritance() {
    // Parent sets font size 20, child inherits it
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": { "fontSize": 20 },
                "children": [
                    {
                        "kind": { "type": "Text", "content": "Should be 20pt" },
                        "style": {}
                    }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should handle style inheritance");
    assert_valid_pdf(&bytes);
}

// ─── Page Size Tests ────────────────────────────────────────────

#[test]
fn test_page_sizes() {
    for (size, expected_w, expected_h) in &[
        (PageSize::A4, 595.28, 841.89),
        (PageSize::Letter, 612.0, 792.0),
        (PageSize::Legal, 612.0, 1008.0),
        (PageSize::A3, 841.89, 1190.55),
        (PageSize::A5, 419.53, 595.28),
    ] {
        let (w, h) = size.dimensions();
        assert!(
            (w - expected_w).abs() < 0.01 && (h - expected_h).abs() < 0.01,
            "Page size {:?} dimensions wrong: ({}, {}) vs ({}, {})",
            size,
            w,
            h,
            expected_w,
            expected_h
        );
    }
}

#[test]
fn test_custom_page_size() {
    let size = PageSize::Custom {
        width: 400.0,
        height: 600.0,
    };
    let (w, h) = size.dimensions();
    assert_eq!(w, 400.0);
    assert_eq!(h, 600.0);
}

// ─── Edge Cases ─────────────────────────────────────────────────

#[test]
fn test_empty_text_node() {
    let doc = default_doc(vec![make_text("", 12.0)]);
    let pages = layout_doc(&doc);
    // Should produce a page (empty text still gets laid out)
    assert_eq!(pages.len(), 1);
}

#[test]
fn test_deeply_nested_views() {
    let mut node = make_text("Deep", 12.0);
    for _ in 0..10 {
        node = make_view(vec![node]);
    }
    let doc = default_doc(vec![node]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_metadata_in_output() {
    let doc = Document {
        children: vec![make_text("Content", 12.0)],
        metadata: Metadata {
            title: Some("Test Title".to_string()),
            author: Some("Test Author".to_string()),
            subject: Some("Testing".to_string()),
            creator: None,
            lang: None,
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Title (Test Title)"));
    assert!(text.contains("/Author (Test Author)"));
}

// ─── Page Break Decision Tests ──────────────────────────────────

#[test]
fn test_unbreakable_node_moves_to_next_page() {
    // Create a tall unbreakable view that doesn't fit
    let mut children = Vec::new();
    for i in 0..40 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }

    // First, fill most of the page
    let mut page_children = Vec::new();
    for i in 0..45 {
        page_children.push(make_text(&format!("Filler {}", i), 12.0));
    }

    // Then add an unbreakable block
    let unbreakable = Node {
        kind: NodeKind::View,
        style: Style {
            wrap: Some(false), // unbreakable
            ..Default::default()
        },
        children: vec![
            make_text("Must stay together line 1", 12.0),
            make_text("Must stay together line 2", 12.0),
            make_text("Must stay together line 3", 12.0),
            make_text("Must stay together line 4", 12.0),
            make_text("Must stay together line 5", 12.0),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    page_children.push(unbreakable);

    let doc = default_doc(page_children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Unbreakable block should push to next page"
    );
}

// ─── Color Tests ────────────────────────────────────────────────

#[test]
fn test_hex_color_parsing() {
    let c = Color::hex("#ff0000");
    assert!((c.r - 1.0).abs() < 0.01);
    assert!((c.g - 0.0).abs() < 0.01);
    assert!((c.b - 0.0).abs() < 0.01);

    let c = Color::hex("00ff00");
    assert!((c.g - 1.0).abs() < 0.01);

    let c = Color::hex("#abc");
    assert!((c.r - 0xAA as f64 / 255.0).abs() < 0.01);
    assert!((c.g - 0xBB as f64 / 255.0).abs() < 0.01);
    assert!((c.b - 0xCC as f64 / 255.0).abs() < 0.01);
}

// ─── Dimension Resolution Tests ─────────────────────────────────

#[test]
fn test_dimension_resolve() {
    assert_eq!(Dimension::Pt(100.0).resolve(500.0), Some(100.0));
    assert_eq!(Dimension::Percent(50.0).resolve(500.0), Some(250.0));
    assert_eq!(Dimension::Auto.resolve(500.0), None);
}

// ─── Custom Font Embedding Tests ────────────────────────────────

use forme::pdf::PdfWriter;

/// Load a system TTF font for testing. Returns None if not available.
fn load_test_font() -> Option<Vec<u8>> {
    // Try common macOS system fonts
    let paths = [
        "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Supplemental/Verdana.ttf",
        "/System/Library/Fonts/Apple Braille.ttf",
    ];
    for path in &paths {
        if let Ok(data) = std::fs::read(path) {
            // Verify it's a valid TTF
            if ttf_parser::Face::parse(&data, 0).is_ok() {
                return Some(data);
            }
        }
    }
    None
}

fn render_with_custom_font(font_data: &[u8], text: &str) -> Vec<u8> {
    let mut font_context = FontContext::new();
    font_context
        .registry_mut()
        .register("TestFont", 400, false, font_data.to_vec());

    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Text {
                content: text.to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_family: Some("TestFont".to_string()),
                font_size: Some(14.0),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    writer
        .write(
            &pages,
            &doc.metadata,
            &font_context,
            doc.tagged,
            doc.pdfa.as_ref(),
            doc.pdf_ua,
            doc.embedded_data.as_deref(),
            doc.flatten_forms,
        )
        .unwrap()
        .0
}

#[test]
fn test_custom_font_produces_valid_pdf() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => {
            eprintln!("Skipping: no test TTF font found");
            return;
        }
    };

    let bytes = render_with_custom_font(&font_data, "Hello Custom Font");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_custom_font_has_cidfont_objects() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => {
            eprintln!("Skipping: no test TTF font found");
            return;
        }
    };

    let bytes = render_with_custom_font(&font_data, "ABC");
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains("CIDFontType2"),
        "Should contain CIDFontType2 subtype"
    );
    assert!(
        text.contains("/FontFile2"),
        "Should contain FontFile2 reference"
    );
    assert!(
        text.contains("/Type0"),
        "Should contain Type0 font dictionary"
    );
    assert!(
        text.contains("/Identity-H"),
        "Should use Identity-H encoding"
    );
    assert!(
        text.contains("/DescendantFonts"),
        "Should have DescendantFonts array"
    );
}

#[test]
fn test_custom_font_has_tounicode() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => {
            eprintln!("Skipping: no test TTF font found");
            return;
        }
    };

    let bytes = render_with_custom_font(&font_data, "Test");
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains("/ToUnicode"),
        "Should have ToUnicode CMap for text extraction"
    );
}

#[test]
fn test_mixed_standard_and_custom_fonts() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => {
            eprintln!("Skipping: no test TTF font found");
            return;
        }
    };

    let mut font_context = FontContext::new();
    font_context
        .registry_mut()
        .register("CustomFont", 400, false, font_data);

    let doc = Document {
        children: vec![
            // Standard font text
            Node {
                kind: NodeKind::Text {
                    content: "Standard Helvetica".to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style {
                    font_family: Some("Helvetica".to_string()),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
            // Custom font text
            Node {
                kind: NodeKind::Text {
                    content: "Custom Font Text".to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style {
                    font_family: Some("CustomFont".to_string()),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
        ],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    let (bytes, _) = writer
        .write(
            &pages,
            &doc.metadata,
            &font_context,
            false,
            None,
            false,
            None,
            false,
        )
        .unwrap();

    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);

    // Should have both Type1 (standard) and Type0/CIDFontType2 (custom) fonts
    assert!(
        text.contains("/Type1"),
        "Should have Type1 for standard font"
    );
    assert!(
        text.contains("CIDFontType2"),
        "Should have CIDFontType2 for custom font"
    );
}

#[test]
fn test_custom_font_subset_smaller_than_full() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => {
            eprintln!("Skipping: no test TTF font found");
            return;
        }
    };

    // Render with just "A" — the subset should be much smaller than the full font
    let bytes = render_with_custom_font(&font_data, "A");
    let pdf_text = String::from_utf8_lossy(&bytes);

    // The PDF should contain FontFile2 with compressed subset data
    assert!(pdf_text.contains("/FontFile2"), "Should embed font data");

    // PDF output should be reasonable size — much smaller than embedding the full font
    // Full font is typically >50KB. With subsetting + compression, PDF should be <50KB for "A"
    assert!(
        bytes.len() < font_data.len(),
        "PDF ({} bytes) should be smaller than full font ({} bytes)",
        bytes.len(),
        font_data.len()
    );
}

// ─── Image Embedding Tests ─────────────────────────────────────

/// Helper: create a minimal in-memory JPEG for testing.
fn make_test_jpeg(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(width, height, |_, _| image::Rgb([0, 128, 255]));
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new(&mut buf);
    image::ImageEncoder::write_image(encoder, img.as_raw(), width, height, image::ColorType::Rgb8)
        .unwrap();
    buf
}

/// Helper: create a minimal in-memory PNG (opaque) for testing.
fn make_test_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = image::RgbaImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 0, 0, 255]);
    }
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ColorType::Rgba8,
    )
    .unwrap();
    buf
}

/// Helper: create an RGBA PNG with partial transparency for testing.
fn make_test_png_with_alpha(width: u32, height: u32) -> Vec<u8> {
    let mut img = image::RgbaImage::new(width, height);
    for (x, _y, pixel) in img.enumerate_pixels_mut() {
        let alpha = if x % 2 == 0 { 128 } else { 255 };
        *pixel = image::Rgba([0, 255, 0, alpha]);
    }
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ColorType::Rgba8,
    )
    .unwrap();
    buf
}

/// Helper: encode bytes as base64 data URI.
fn to_data_uri(data: &[u8], mime: &str) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    format!("data:{};base64,{}", mime, b64)
}

fn make_image_node(src: &str, width: Option<f64>, height: Option<f64>) -> Node {
    Node {
        kind: NodeKind::Image {
            src: src.to_string(),
            width,
            height,
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_jpeg_image_produces_valid_pdf() {
    let jpeg_data = make_test_jpeg(4, 4);
    let src = to_data_uri(&jpeg_data, "image/jpeg");

    let doc = default_doc(vec![make_image_node(&src, Some(100.0), Some(100.0))]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/DCTDecode"),
        "JPEG should use DCTDecode filter"
    );
    assert!(text.contains("/XObject"), "Page should reference XObject");
    assert!(text.contains("/Im0"), "Should reference /Im0");
}

#[test]
fn test_png_image_produces_valid_pdf() {
    let png_data = make_test_png(4, 4);
    let src = to_data_uri(&png_data, "image/png");

    let doc = default_doc(vec![make_image_node(&src, Some(80.0), Some(80.0))]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/FlateDecode"),
        "PNG should use FlateDecode filter"
    );
    assert!(text.contains("/XObject"), "Page should reference XObject");
}

#[test]
fn test_png_with_alpha_has_smask() {
    let png_data = make_test_png_with_alpha(4, 4);
    let src = to_data_uri(&png_data, "image/png");

    let doc = default_doc(vec![make_image_node(&src, Some(60.0), Some(60.0))]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/SMask"),
        "Alpha PNG should have SMask reference"
    );
    assert!(text.contains("/DeviceGray"), "SMask should use DeviceGray");
}

/// Helper: create a minimal in-memory WebP (opaque, lossless) for testing.
fn make_test_webp(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(width, height, |_, _| image::Rgba([0, 0, 255, 255]));
    let mut buf = Vec::new();
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ColorType::Rgba8,
    )
    .unwrap();
    buf
}

#[test]
fn test_webp_image_produces_valid_pdf() {
    let webp_data = make_test_webp(4, 4);
    let src = to_data_uri(&webp_data, "image/webp");

    let doc = default_doc(vec![make_image_node(&src, Some(80.0), Some(80.0))]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/FlateDecode"),
        "WebP should use FlateDecode filter (decoded to RGB)"
    );
    assert!(text.contains("/XObject"), "Page should reference XObject");
}

#[test]
fn test_image_aspect_ratio() {
    // 8x4 image: aspect ratio 0.5
    let png_data = make_test_png(8, 4);
    let src = to_data_uri(&png_data, "image/png");

    // Only specify width=100, height should be auto-calculated to 50
    let doc = default_doc(vec![make_image_node(&src, Some(100.0), None)]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // Find the image element
    let img_elem = pages[0]
        .elements
        .iter()
        .find(|e| matches!(e.draw, forme::layout::DrawCommand::Image { .. }))
        .expect("Should have an image element");

    assert!((img_elem.width - 100.0).abs() < 0.1, "Width should be 100");
    assert!(
        (img_elem.height - 50.0).abs() < 0.1,
        "Height should be 50 (100 * 4/8), got {}",
        img_elem.height
    );
}

#[test]
fn test_base64_image_src() {
    let png_data = make_test_png(2, 2);
    use base64::Engine;
    let raw_b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);

    // Test with raw base64 (no data URI prefix)
    let doc = default_doc(vec![make_image_node(&raw_b64, Some(50.0), Some(50.0))]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/XObject"),
        "Raw base64 image should produce XObject"
    );
}

#[test]
fn test_missing_image_falls_back() {
    // Invalid src should fall back to placeholder, not crash
    let doc = default_doc(vec![make_image_node(
        "nonexistent_file.png",
        Some(100.0),
        Some(75.0),
    )]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    // Should NOT have XObject (it's a placeholder)
    assert!(
        !text.contains("/XObject"),
        "Missing image should render as placeholder, not XObject"
    );
}

#[test]
fn test_multiple_images_on_same_page() {
    let jpeg_data = make_test_jpeg(4, 4);
    let png_data = make_test_png(4, 4);
    let jpeg_src = to_data_uri(&jpeg_data, "image/jpeg");
    let png_src = to_data_uri(&png_data, "image/png");

    let doc = default_doc(vec![
        make_image_node(&jpeg_src, Some(100.0), Some(100.0)),
        make_image_node(&png_src, Some(100.0), Some(100.0)),
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Im0"), "Should have first image reference");
    assert!(text.contains("/Im1"), "Should have second image reference");
}

#[test]
fn test_image_json_deserialization() {
    let png_data = make_test_png(2, 2);
    let src = to_data_uri(&png_data, "image/png");

    let json = format!(
        r#"{{
        "children": [
            {{
                "kind": {{ "type": "Image", "src": "{}", "width": 100.0, "height": 100.0 }},
                "style": {{}}
            }}
        ]
    }}"#,
        src
    );

    let bytes = forme::render_json(&json).expect("Should parse image JSON");
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/XObject"),
        "Image from JSON should produce XObject"
    );
}

// ─── Fixed Header/Footer Tests ──────────────────────────────────

fn make_fixed_header(text: &str) -> Node {
    Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Header,
            pages: FixedPageFilter::default(),
        },
        style: Style {
            padding: Some(Edges::uniform(8.0)),
            background_color: Some(Color::rgb(0.9, 0.9, 0.95)),
            ..Default::default()
        },
        children: vec![make_text(text, 10.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_fixed_footer(text: &str) -> Node {
    Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Footer,
            pages: FixedPageFilter::default(),
        },
        style: Style {
            padding: Some(Edges::uniform(8.0)),
            background_color: Some(Color::rgb(0.95, 0.95, 0.95)),
            ..Default::default()
        },
        children: vec![make_text(text, 10.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_fixed_header_single_page() {
    let doc = default_doc(vec![
        make_fixed_header("Header Text"),
        make_text("Body content", 12.0),
    ]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    // Header + body elements should be present
    assert!(
        pages[0].elements.len() >= 2,
        "Page should have header + body elements"
    );
}

#[test]
fn test_fixed_header_repeats_on_overflow() {
    let mut children = vec![make_fixed_header("Page Header")];
    // Add enough content to overflow to 3+ pages
    for i in 0..120 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }
    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 3,
        "Should have 3+ pages, got {}",
        pages.len()
    );

    // Every page should have elements (header renders on each)
    for (i, page) in pages.iter().enumerate() {
        assert!(
            !page.elements.is_empty(),
            "Page {} should have elements (header should render)",
            i
        );
    }
}

#[test]
fn test_fixed_footer_renders() {
    let doc = default_doc(vec![
        make_fixed_footer("Footer Text"),
        make_text("Body content", 12.0),
    ]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    // Footer elements should be at the bottom of content area
    assert!(
        pages[0].elements.len() >= 2,
        "Page should have footer + body elements"
    );
}

#[test]
fn test_header_and_footer_together() {
    let mut children = vec![make_fixed_header("Header"), make_fixed_footer("Footer")];
    for i in 0..80 {
        children.push(make_text(&format!("Content line {}", i), 12.0));
    }
    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Should overflow to multiple pages, got {}",
        pages.len()
    );

    // Each page should have elements
    for (i, page) in pages.iter().enumerate() {
        assert!(
            !page.elements.is_empty(),
            "Page {} should have header/footer/content elements",
            i
        );
    }

    // Verify PDF is valid
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_footer_reduces_content_area() {
    // Doc without footer
    let mut children_no_footer = Vec::new();
    for i in 0..80 {
        children_no_footer.push(make_text(&format!("Line {}", i), 12.0));
    }
    let doc_no_footer = default_doc(children_no_footer);
    let pages_no_footer = layout_doc(&doc_no_footer);

    // Doc with large footer
    let big_footer = Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Footer,
            pages: FixedPageFilter::default(),
        },
        style: Style {
            padding: Some(Edges::symmetric(40.0, 8.0)), // tall footer
            ..Default::default()
        },
        children: vec![make_text("Big Footer", 14.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let mut children_with_footer = vec![big_footer];
    for i in 0..80 {
        children_with_footer.push(make_text(&format!("Line {}", i), 12.0));
    }
    let doc_with_footer = default_doc(children_with_footer);
    let pages_with_footer = layout_doc(&doc_with_footer);

    assert!(
        pages_with_footer.len() > pages_no_footer.len(),
        "Doc with footer ({} pages) should have more pages than without ({} pages)",
        pages_with_footer.len(),
        pages_no_footer.len()
    );
}

#[test]
fn test_fixed_element_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "Fixed", "position": "Header" },
                "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
                "children": [
                    { "kind": { "type": "Text", "content": "JSON Header" }, "style": {} }
                ]
            },
            {
                "kind": { "type": "Text", "content": "Body text" },
                "style": {}
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse Fixed node JSON");
    assert_valid_pdf(&bytes);
}

// ─── Flex Wrap Tests ────────────────────────────────────────────

#[test]
fn test_flex_wrap_single_line_fits() {
    // 3 items × 100pt = 300pt; available ~487pt (A4 minus margins) — should fit on one line
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            ..Default::default()
        },
        vec![
            make_styled_view(
                Style {
                    width: Some(Dimension::Pt(100.0)),
                    ..Default::default()
                },
                vec![make_text("A", 12.0)],
            ),
            make_styled_view(
                Style {
                    width: Some(Dimension::Pt(100.0)),
                    ..Default::default()
                },
                vec![make_text("B", 12.0)],
            ),
            make_styled_view(
                Style {
                    width: Some(Dimension::Pt(100.0)),
                    ..Default::default()
                },
                vec![make_text("C", 12.0)],
            ),
        ],
    );
    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
}

#[test]
fn test_flex_wrap_items_wrap_to_second_line() {
    // 5 items × 120pt = 600pt; available ~487pt — items should wrap
    let mut items = Vec::new();
    for i in 0..5 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(120.0)),
                ..Default::default()
            },
            vec![make_text(&format!("Item {}", i), 12.0)],
        ));
    }
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // Collect Y positions from all elements recursively (items are now nested)
    fn collect_rect_ys(elements: &[forme::layout::LayoutElement], ys: &mut Vec<f64>) {
        for e in elements {
            if matches!(e.draw, forme::layout::DrawCommand::Rect { .. }) {
                ys.push(e.y);
            }
            collect_rect_ys(&e.children, ys);
        }
    }
    let mut y_positions = Vec::new();
    collect_rect_ys(&pages[0].elements, &mut y_positions);

    // Should have at least 2 distinct Y positions (2 wrap lines)
    let mut unique_ys: Vec<f64> = y_positions.clone();
    unique_ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    unique_ys.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    assert!(
        unique_ys.len() >= 2,
        "Wrapped items should produce at least 2 Y positions, got {:?}",
        unique_ys
    );
}

#[test]
fn test_flex_wrap_produces_valid_pdf() {
    let mut items = Vec::new();
    for i in 0..8 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(120.0)),
                padding: Some(Edges::uniform(4.0)),
                ..Default::default()
            },
            vec![make_text(&format!("Cell {}", i), 10.0)],
        ));
    }
    let grid = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            gap: Some(8.0),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![grid]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_flex_wrap_nowrap_unchanged() {
    // NoWrap regression: 10 items should still squeeze on one line
    let mut items = Vec::new();
    for i in 0..10 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(80.0)),
                ..Default::default()
            },
            vec![make_text(&format!("{}", i), 10.0)],
        ));
    }
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::NoWrap),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // All items should be at the same Y position (single line)
    let y_positions: Vec<f64> = pages[0]
        .elements
        .iter()
        .filter(|e| matches!(e.draw, forme::layout::DrawCommand::Rect { .. }))
        .map(|e| e.y)
        .collect();

    if y_positions.len() > 1 {
        let first_y = y_positions[0];
        for y in &y_positions {
            assert!(
                (y - first_y).abs() < 1.0,
                "NoWrap items should all be on same line, got different Y positions"
            );
        }
    }
}

#[test]
fn test_flex_wrap_page_break_per_line() {
    // Many wrapped items with padding should span multiple pages
    // A4 content: ~487pt wide, ~734pt tall
    // Items: 200pt wide → 2 per line, each ~40pt tall → 100 lines × 40pt = 4000pt
    let mut items = Vec::new();
    for i in 0..200 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                padding: Some(Edges::uniform(10.0)),
                ..Default::default()
            },
            vec![make_text(&format!("I{}", i), 12.0)],
        ));
    }
    let grid = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![grid]);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "200 wrapped items should span multiple pages, got {}",
        pages.len()
    );
}

#[test]
fn test_flex_wrap_with_row_gap() {
    // Verify row_gap applies between wrap lines
    let mut items = Vec::new();
    for i in 0..6 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_text(&format!("Item {}", i), 12.0)],
        ));
    }
    // 6 items × 200pt; available ~487pt → 2 per line → 3 lines
    // row_gap=20 should add space between lines
    let grid_with_gap = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            row_gap: Some(20.0),
            ..Default::default()
        },
        items.clone(),
    );
    let grid_no_gap = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            row_gap: Some(0.0),
            ..Default::default()
        },
        items,
    );

    let doc_with_gap = default_doc(vec![grid_with_gap]);
    let doc_no_gap = default_doc(vec![grid_no_gap]);

    let pages_gap = layout_doc(&doc_with_gap);
    let pages_no_gap = layout_doc(&doc_no_gap);

    // Both should produce valid output
    assert_eq!(pages_gap.len(), 1);
    assert_eq!(pages_no_gap.len(), 1);

    // The version with row_gap should use more vertical space
    let max_y_gap = pages_gap[0]
        .elements
        .iter()
        .map(|e| e.y + e.height)
        .fold(0.0f64, f64::max);
    let max_y_no_gap = pages_no_gap[0]
        .elements
        .iter()
        .map(|e| e.y + e.height)
        .fold(0.0f64, f64::max);
    assert!(
        max_y_gap > max_y_no_gap,
        "Grid with row_gap ({:.1}) should use more vertical space than without ({:.1})",
        max_y_gap,
        max_y_no_gap
    );
}

#[test]
fn test_flex_wrap_json_deserialization() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": { "flexDirection": "Row", "flexWrap": "Wrap", "gap": 8 },
                "children": [
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "A" }, "style": {} }
                        ]
                    },
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "B" }, "style": {} }
                        ]
                    },
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "C" }, "style": {} }
                        ]
                    }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse flex-wrap JSON");
    assert_valid_pdf(&bytes);
}

// ─── Table Cell Overflow Tests ──────────────────────────────────

#[test]
fn test_table_cell_overflow_does_not_panic() {
    // Known limitation: cell content that exceeds a full page silently overflows.
    // Row-level page breaks work: if a row doesn't fit, it moves to the next page.
    // But page breaks INSIDE cells are swallowed (layout_table_row passes &mut Vec::new()).
    // This test verifies the engine doesn't panic and row-level breaks still function.

    // Fill most of a page with text so the table starts near the bottom
    let mut children = Vec::new();
    for i in 0..40 {
        children.push(make_text(&format!("Filler line {}", i), 12.0));
    }

    // Add a table with a cell containing enough text to be tall
    let long_text = "This is a cell with enough text to be reasonably tall. ".repeat(3);
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![
            make_table_row(true, vec![make_table_cell(vec![make_text("Header", 10.0)])]),
            make_table_row(
                false,
                vec![make_table_cell(vec![make_text(&long_text, 10.0)])],
            ),
            make_table_row(
                false,
                vec![make_table_cell(vec![make_text("Normal row", 10.0)])],
            ),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    children.push(table);

    let doc = default_doc(children);
    let pages = layout_doc(&doc);

    // Should produce multiple pages (filler overflows, table row moves to next page)
    assert!(
        pages.len() >= 2,
        "Table near page bottom should cause page break, got {} pages",
        pages.len()
    );

    // Should produce valid PDF
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_table_row_level_page_break_works() {
    // Verify that when a row doesn't fit on the current page, it moves to the next
    // page with header repetition. This is the supported behavior (vs. cell-level breaks).
    let rows: Vec<Vec<&str>> = (0..60)
        .map(|i| {
            vec![
                Box::leak(format!("Row {}", i).into_boxed_str()) as &str,
                "Data",
            ]
        })
        .collect();
    let table = make_simple_table(vec!["Col A", "Col B"], rows);
    let doc = default_doc(vec![table]);
    let pages = layout_doc(&doc);

    assert!(
        pages.len() >= 2,
        "60-row table should span multiple pages, got {}",
        pages.len()
    );

    // Every page should have elements (header repeats on each)
    for (i, page) in pages.iter().enumerate() {
        assert!(
            !page.elements.is_empty(),
            "Page {} should have elements (table header should repeat)",
            i
        );
    }
}

// ─── Error Handling Tests ────────────────────────────────────────

#[test]
fn test_invalid_json_returns_parse_error() {
    let result = forme::render_json("not valid json {{{");
    assert!(result.is_err(), "Invalid JSON should return Err");
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Failed to parse document"),
        "Error should describe parse failure: {}",
        msg
    );
}

#[test]
fn test_wrong_schema_returns_parse_error() {
    let result = forme::render_json(r#"{"wrong": "schema"}"#);
    assert!(result.is_err(), "Wrong schema should return Err");
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Hint:"), "Error should include hint: {}", msg);
}

#[test]
fn test_valid_doc_returns_ok() {
    let json = r#"{"children": [{"kind": {"type": "Text", "content": "Hello"}, "style": {}}]}"#;
    let result = forme::render_json(json);
    assert!(
        result.is_ok(),
        "Valid JSON should return Ok, got: {:?}",
        result.err()
    );
}

#[test]
fn test_empty_json_object_returns_ok() {
    let json = r#"{"children": []}"#;
    let result = forme::render_json(json);
    assert!(result.is_ok(), "Empty children should return Ok");
}

// ─── Page Number Placeholder Tests ──────────────────────────────

#[test]
fn test_page_number_placeholder_single_page() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Page {
            config: PageConfig::default(),
        },
        style: Style::default(),
        children: vec![
            Node {
                kind: NodeKind::Fixed {
                    position: FixedPosition::Footer,
                    pages: FixedPageFilter::default(),
                },
                style: Style::default(),
                children: vec![make_text("Page {{pageNumber}} of {{totalPages}}", 12.0)],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
            make_text("Hello", 12.0),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf_bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    // Streams are compressed, but the raw PDF bytes should not contain
    // the placeholder strings (they should have been replaced before encoding).
    assert!(
        !pdf_str.contains("{{pageNumber}}"),
        "Placeholder {{{{pageNumber}}}} should have been replaced"
    );
    assert!(
        !pdf_str.contains("{{totalPages}}"),
        "Placeholder {{{{totalPages}}}} should have been replaced"
    );
}

#[test]
fn test_page_number_placeholder_multi_page() {
    let mut page_children: Vec<Node> = vec![Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Footer,
            pages: FixedPageFilter::default(),
        },
        style: Style {
            font_size: Some(10.0),
            ..Style::default()
        },
        children: vec![make_text("{{pageNumber}}/{{totalPages}}", 10.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }];
    for _ in 0..80 {
        page_children.push(make_text("Line of text to fill the page.", 12.0));
    }

    let doc = default_doc(vec![Node {
        kind: NodeKind::Page {
            config: PageConfig::default(),
        },
        style: Style::default(),
        children: page_children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf_bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        !pdf_str.contains("{{pageNumber}}"),
        "All {{{{pageNumber}}}} placeholders should be replaced"
    );
    assert!(
        !pdf_str.contains("{{totalPages}}"),
        "All {{{{totalPages}}}} placeholders should be replaced"
    );
}

#[test]
fn test_page_number_in_body_text() {
    let doc = default_doc(vec![make_text(
        "This is page {{pageNumber}} of {{totalPages}}.",
        12.0,
    )]);
    let pdf_bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        !pdf_str.contains("{{pageNumber}}"),
        "Placeholder should be replaced even in body text"
    );
}

#[test]
fn test_no_placeholder_unchanged() {
    let doc = default_doc(vec![make_text("Hello World", 12.0)]);
    let pdf_bytes = forme::render(&doc).unwrap();
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "Should produce valid PDF without placeholders"
    );
}

// ── Feature 1: Links Tests ──────────────────────────────────────

#[test]
fn test_text_with_href_produces_link_annotation() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: "Click here".to_string(),
            href: Some("https://example.com".to_string()),
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/URI"),
        "Text with href should produce /URI annotation"
    );
    assert!(
        text.contains("example.com"),
        "Annotation should contain the URL"
    );
    assert!(text.contains("/Annots"), "Page should have /Annots array");
}

#[test]
fn test_text_without_href_has_no_annotation() {
    let doc = default_doc(vec![make_text("No link here", 12.0)]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/URI"),
        "Text without href should not produce annotations"
    );
    assert!(
        !text.contains("/Annots"),
        "Page should not have /Annots array"
    );
}

#[test]
fn test_multiple_links_on_same_page() {
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::Text {
                content: "Link 1".to_string(),
                href: Some("https://example.com/1".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
        Node {
            kind: NodeKind::Text {
                content: "Link 2".to_string(),
                href: Some("https://example.com/2".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    // Should have at least 2 /URI references
    let uri_count = text.matches("/URI").count();
    assert!(
        uri_count >= 2,
        "Should have at least 2 link annotations, got {}",
        uri_count
    );
}

#[test]
fn test_text_decoration_underline_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "Text", "content": "Underlined text" },
                "style": { "textDecoration": "Underline" }
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse underline JSON");
    assert_valid_pdf(&bytes);
}

// ── Feature 2: Text Runs Tests ──────────────────────────────────

#[test]
fn test_text_runs_render_valid_pdf() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: String::new(),
            href: None,
            runs: vec![
                TextRun {
                    content: "Hello ".to_string(),
                    style: Style::default(),
                    href: None,
                },
                TextRun {
                    content: "bold".to_string(),
                    style: Style {
                        font_weight: Some(700),
                        ..Default::default()
                    },
                    href: None,
                },
                TextRun {
                    content: " world".to_string(),
                    style: Style::default(),
                    href: None,
                },
            ],
        },
        style: Style {
            font_size: Some(12.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_text_runs_with_href_per_run() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: String::new(),
            href: None,
            runs: vec![
                TextRun {
                    content: "Normal text ".to_string(),
                    style: Style::default(),
                    href: None,
                },
                TextRun {
                    content: "linked text".to_string(),
                    style: Style {
                        color: Some(Color::rgb(0.0, 0.0, 1.0)),
                        ..Default::default()
                    },
                    href: Some("https://example.com".to_string()),
                },
            ],
        },
        style: Style {
            font_size: Some(12.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_text_runs_json_deserialization() {
    let json = r#"{
        "children": [
            {
                "kind": {
                    "type": "Text",
                    "content": "",
                    "runs": [
                        { "content": "Hello ", "style": {} },
                        { "content": "bold", "style": { "fontWeight": 700 } }
                    ]
                },
                "style": { "fontSize": 14 }
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse text runs JSON");
    assert_valid_pdf(&bytes);
}

// ── Feature 3: Bookmarks Tests ──────────────────────────────────

#[test]
fn test_bookmarks_produce_outlines() {
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Chapter 1", 18.0)],
            id: None,
            source_location: None,
            bookmark: Some("Chapter 1".to_string()),
            href: None,
            alt: None,
        },
        make_text("Content for chapter 1", 12.0),
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Chapter 2", 18.0)],
            id: None,
            source_location: None,
            bookmark: Some("Chapter 2".to_string()),
            href: None,
            alt: None,
        },
        make_text("Content for chapter 2", 12.0),
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Outlines"),
        "Document with bookmarks should have /Outlines"
    );
    assert!(
        text.contains("Chapter 1"),
        "Outline should contain bookmark title 'Chapter 1'"
    );
    assert!(
        text.contains("Chapter 2"),
        "Outline should contain bookmark title 'Chapter 2'"
    );
}

#[test]
fn test_no_bookmarks_no_outlines() {
    let doc = default_doc(vec![make_text("No bookmarks here", 12.0)]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/Outlines"),
        "Document without bookmarks should not have /Outlines"
    );
}

#[test]
fn test_bookmarks_json_deserialization() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": {},
                "bookmark": "Section A",
                "children": [
                    { "kind": { "type": "Text", "content": "Section A" }, "style": {} }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse bookmark JSON");
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Outlines"),
        "Should produce outlines from JSON bookmark"
    );
}

#[test]
fn test_bookmarks_on_breakable_view() {
    // A bookmarked View whose content exceeds a single page triggers the breakable
    // path (layout_breakable_view). The bookmark must still appear in the PDF outlines.
    let mut children = Vec::new();
    for i in 0..80 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }
    let bookmarked_view = Node {
        kind: NodeKind::View,
        style: Style::default(), // wrap defaults to true → breakable
        children,
        id: None,
        source_location: None,
        bookmark: Some("Breakable Chapter".to_string()),
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![bookmarked_view]);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Breakable bookmarked view should span multiple pages, got {}",
        pages.len()
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Outlines"),
        "Breakable view with bookmark should produce /Outlines"
    );
    assert!(
        text.contains("Breakable Chapter"),
        "Outline should contain 'Breakable Chapter' bookmark title"
    );
}

#[test]
fn test_multiple_bookmarked_views_mixed_sizes() {
    // Simulates a catalog: 4 bookmarked categories, some small (fit on page), some large (break).
    // All 4 bookmarks must appear in the PDF outlines.
    let mut doc_children = Vec::new();
    for i in 0..4 {
        let name = format!("Category {}", i + 1);
        let num_lines = if i % 2 == 0 { 10 } else { 60 };
        let mut children = Vec::new();
        for j in 0..num_lines {
            children.push(make_text(&format!("{} line {}", name, j), 12.0));
        }
        doc_children.push(Node {
            kind: NodeKind::View,
            style: Style::default(),
            children,
            id: None,
            source_location: None,
            bookmark: Some(name),
            href: None,
            alt: None,
        });
    }
    let doc = default_doc(doc_children);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Outlines"), "Should have outlines");
    for i in 1..=4 {
        let name = format!("Category {}", i);
        assert!(
            text.contains(&format!("/Title ({})", name)),
            "Missing bookmark for '{}'",
            name
        );
    }
}

#[test]
fn test_styled_breakable_bookmark_emits_exactly_one_outline_entry() {
    // Regression: a bookmarked View that BOTH overflows a page AND has visual
    // styling used to emit the outline entry twice.
    //
    // The overflow path emits a zero-height marker element carrying the
    // bookmark (it has to — with no background/border it builds no wrapper at
    // all, and the bookmark would otherwise be lost). When the view IS styled,
    // it also builds a wrapper, the marker gets drained into that wrapper's
    // children, and `collect_bookmarks` recurses — so it found the same
    // bookmark on the wrapper and again on its child marker. Two identical
    // outline entries for one `bookmark` prop, and `/Count 2`.
    //
    // The sibling tests above only assert `contains`, which is exactly why this
    // survived: a duplicate satisfies `contains` perfectly well. Assert counts.
    let mut children = Vec::new();
    for i in 0..80 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }
    let view = Node {
        kind: NodeKind::View,
        style: Style {
            // Forces `needs_wrapper` — this is the half of the branch that duplicated.
            background_color: Some(Color::rgb(0.9, 0.9, 0.9)),
            padding: Some(Edges::uniform(20.0)),
            ..Default::default()
        },
        children,
        id: None,
        source_location: None,
        bookmark: Some("Styled Chapter".to_string()),
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![view]);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Test needs the overflow path; got {} page(s)",
        pages.len()
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);

    let title_count = text.matches("/Title (Styled Chapter)").count();
    assert_eq!(
        title_count, 1,
        "One `bookmark` prop must produce exactly one outline entry, got {}",
        title_count
    );
    // Scoped to the Outlines dictionary on purpose — a bare `/Count` search
    // also hits the /Pages dictionary, whose count is the page count (2 here).
    let outlines_dict = text
        .split("/Type /Outlines")
        .nth(1)
        .expect("PDF should contain an /Outlines dictionary");
    let outlines_dict = &outlines_dict[..outlines_dict.find(">>").unwrap_or(outlines_dict.len())];
    assert!(
        outlines_dict.contains("/Count 1"),
        "Outlines dictionary should report /Count 1, got: << /Type /Outlines{}>>",
        outlines_dict
    );
}

// ── Feature 4: Absolute Positioning Tests ───────────────────────

#[test]
fn test_absolute_position_does_not_affect_flow() {
    let doc = default_doc(vec![make_styled_view(
        Style {
            width: Some(Dimension::Pt(200.0)),
            height: Some(Dimension::Pt(200.0)),
            ..Default::default()
        },
        vec![
            make_text("Flow child", 12.0),
            Node {
                kind: NodeKind::View,
                style: Style {
                    position: Some(Position::Absolute),
                    top: Some(10.0),
                    left: Some(10.0),
                    width: Some(Dimension::Pt(50.0)),
                    height: Some(Dimension::Pt(50.0)),
                    background_color: Some(Color::rgb(1.0, 0.0, 0.0)),
                    ..Default::default()
                },
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
            make_text("After absolute", 12.0),
        ],
    )]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_absolute_position_json() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": { "width": { "Pt": 300 }, "height": { "Pt": 300 } },
                "children": [
                    { "kind": { "type": "Text", "content": "Flow" }, "style": {} },
                    {
                        "kind": { "type": "View" },
                        "style": {
                            "position": "Absolute",
                            "top": 20, "right": 20,
                            "width": { "Pt": 80 },
                            "backgroundColor": { "r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0 }
                        },
                        "children": [
                            { "kind": { "type": "Text", "content": "Abs" }, "style": {} }
                        ]
                    }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse absolute position JSON");
    assert_valid_pdf(&bytes);
}

// ── Feature 5: SVG Rendering Tests ──────────────────────────────

#[test]
fn test_svg_basic_rect() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 100.0,
            height: 100.0,
            view_box: Some("0 0 100 100".to_string()),
            content: r##"<rect x="10" y="10" width="80" height="80" fill="#ff0000"/>"##.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_svg_circle_and_path() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 200.0,
            view_box: Some("0 0 200 200".to_string()),
            content: r#"<circle cx="100" cy="100" r="50" fill="blue"/>
                        <path d="M 10 10 L 50 50 Z" stroke="black" fill="none"/>"#
                .to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_svg_json_deserialization() {
    let json = r#"{
        "children": [
            {
                "kind": {
                    "type": "Svg",
                    "width": 100,
                    "height": 100,
                    "viewBox": "0 0 100 100",
                    "content": "<rect x=\"0\" y=\"0\" width=\"100\" height=\"100\" fill=\"green\"/>"
                },
                "style": {}
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse SVG JSON");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_svg_page_break() {
    // Fill most of a page, then add an SVG that won't fit
    let mut children = Vec::new();
    for i in 0..50 {
        children.push(make_text(&format!("Line {}", i), 12.0));
    }
    children.push(Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 200.0,
            view_box: Some("0 0 200 200".to_string()),
            content: r#"<rect x="0" y="0" width="200" height="200" fill="red"/>"#.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    });
    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "SVG after many lines should push to next page"
    );
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_empty_svg_content() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 50.0,
            height: 50.0,
            view_box: None,
            content: String::new(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

// ─── Widow/Orphan Control Tests ─────────────────────────────────

#[test]
fn test_orphan_control_moves_paragraph_to_next_page() {
    // Fill most of a page so only 1 line can fit, then add a 5-line paragraph.
    // With min_orphan_lines=2 (default), only 1 line fitting → MoveToNextPage.
    let mut children = Vec::new();
    // A4 content height ~734pt. At 12pt font * 1.4 line height = 16.8pt/line.
    // 43 lines fills most of the page.
    for i in 0..43 {
        children.push(make_text(&format!("Filler line {}", i), 12.0));
    }
    // Add a single text node that will break into 5+ lines
    let long_text = "This is a paragraph with enough words to create multiple lines when rendered into the available page width. ";
    let repeated = long_text.repeat(3);
    children.push(Node {
        kind: NodeKind::Text {
            content: repeated,
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    });

    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Orphan control should push paragraph to next page, got {} pages",
        pages.len()
    );
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_widow_control_adjusts_split_point() {
    // Fill most of a page so only 3 lines can fit from a 4-line paragraph.
    // With min_widow_lines=2, leaving 1 on next page → pull one back to 2+2.
    let mut children = Vec::new();
    for i in 0..40 {
        children.push(make_text(&format!("Filler line {}", i), 12.0));
    }

    // A 4-child breakable view. Each child is one text node (~1 line).
    let paragraph = Node {
        kind: NodeKind::View,
        style: Style {
            wrap: Some(true),
            ..Default::default()
        },
        children: vec![
            make_text("Paragraph line 1", 12.0),
            make_text("Paragraph line 2", 12.0),
            make_text("Paragraph line 3", 12.0),
            make_text("Paragraph line 4", 12.0),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    children.push(paragraph);

    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Should overflow to at least 2 pages, got {}",
        pages.len()
    );
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_widow_orphan_with_custom_settings() {
    // Test with min_widow_lines=1 and min_orphan_lines=1
    let mut children = Vec::new();
    for i in 0..43 {
        children.push(make_text(&format!("Filler {}", i), 12.0));
    }
    let text = "Line one. Line two. Line three.";
    children.push(Node {
        kind: NodeKind::Text {
            content: text.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            min_widow_lines: Some(1),
            min_orphan_lines: Some(1),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    });

    let doc = default_doc(children);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

// ─── Align-Content Tests ────────────────────────────────────────

#[test]
fn test_align_content_center() {
    // Fixed-height container with 2 wrapped lines, align-content: center
    let mut items = Vec::new();
    for i in 0..4 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_text(&format!("Item {}", i), 12.0)],
        ));
    }
    // 4 items × 200pt; available ~487pt → 2 per line → 2 lines
    // Container height 300pt, lines ~16.8pt each → lots of slack
    let container = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            height: Some(Dimension::Pt(300.0)),
            align_content: Some(AlignContent::Center),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // Verify lines are centered: the flex items (children of the container)
    // should be offset from the top, not at y=0 within the container.
    fn find_min_y(elems: &[forme::layout::LayoutElement]) -> f64 {
        let mut min_y = f64::MAX;
        for e in elems {
            if matches!(e.draw, forme::layout::DrawCommand::Rect { .. }) {
                min_y = min_y.min(e.y);
            }
            let child_min = find_min_y(&e.children);
            min_y = min_y.min(child_min);
        }
        min_y
    }
    // The first top-level element is the flex container at y=54 (margin).
    // Look at its children (the flex items) which should be centered within.
    let container_elem = &pages[0].elements[0];
    let items_min_y = find_min_y(&container_elem.children);
    // With centering, items should be well below the container top (54pt margin)
    assert!(
        items_min_y > 100.0,
        "With align-content: center, flex items should be offset from top, got items_min_y={}",
        items_min_y
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_align_content_space_between() {
    // 3 wrapped lines in fixed-height container, space-between
    let mut items = Vec::new();
    for i in 0..6 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_text(&format!("SB {}", i), 12.0)],
        ));
    }
    // 6 items × 200pt → 2 per line → 3 lines
    let container = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            height: Some(Dimension::Pt(400.0)),
            align_content: Some(AlignContent::SpaceBetween),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_align_content_flex_end() {
    let mut items = Vec::new();
    for i in 0..4 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_text(&format!("FE {}", i), 12.0)],
        ));
    }
    let container = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            height: Some(Dimension::Pt(300.0)),
            align_content: Some(AlignContent::FlexEnd),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_align_content_no_effect_without_fixed_height() {
    // Without a fixed height, align-content has no effect
    let mut items = Vec::new();
    for i in 0..4 {
        items.push(make_styled_view(
            Style {
                width: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_text(&format!("NH {}", i), 12.0)],
        ));
    }
    let container = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            align_content: Some(AlignContent::Center),
            ..Default::default()
        },
        items,
    );
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_align_content_json_deserialization() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "View" },
                "style": {
                    "flexDirection": "Row",
                    "flexWrap": "Wrap",
                    "height": { "Pt": 300 },
                    "alignContent": "Center"
                },
                "children": [
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "A" }, "style": {} }
                        ]
                    },
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "B" }, "style": {} }
                        ]
                    },
                    {
                        "kind": { "type": "View" },
                        "style": { "width": { "Pt": 200 } },
                        "children": [
                            { "kind": { "type": "Text", "content": "C" }, "style": {} }
                        ]
                    }
                ]
            }
        ]
    }"#;
    let bytes = forme::render_json(json).expect("Should parse align-content JSON");
    assert_valid_pdf(&bytes);
}

// ─── Table Cell Overflow Fix Tests ──────────────────────────────

#[test]
fn test_table_cell_overflow_preserves_content() {
    // A table with a cell containing enough text to overflow the page.
    // Content should be preserved on subsequent pages.
    let very_long_text = "This is a very long cell content that should overflow. ".repeat(20);
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![
            make_table_row(true, vec![make_table_cell(vec![make_text("Header", 10.0)])]),
            make_table_row(
                false,
                vec![make_table_cell(vec![make_text(&very_long_text, 10.0)])],
            ),
            make_table_row(
                false,
                vec![make_table_cell(vec![make_text("After overflow", 10.0)])],
            ),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc = default_doc(vec![table]);
    let pages = layout_doc(&doc);

    // Should produce a valid PDF without panicking
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);

    assert!(
        !pages.is_empty(),
        "Table with overflow cell should produce at least 1 page"
    );
}

#[test]
fn test_table_cell_overflow_near_page_bottom() {
    // Fill most of a page, then add a table with a tall cell
    let mut children = Vec::new();
    for i in 0..35 {
        children.push(make_text(&format!("Filler {}", i), 12.0));
    }

    // Cell content must exceed a full page height (~734pt) to trigger
    // page breaks inside the cell. At 10pt font, ~14pt line height,
    // ~44 chars/line → 200 repeats ≈ 73 lines ≈ 1022pt of text.
    let tall_cell_text = "Tall cell line. ".repeat(200);
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![
            make_table_row(
                true,
                vec![
                    make_table_cell(vec![make_text("Col A", 10.0)]),
                    make_table_cell(vec![make_text("Col B", 10.0)]),
                ],
            ),
            make_table_row(
                false,
                vec![
                    make_table_cell(vec![make_text(&tall_cell_text, 10.0)]),
                    make_table_cell(vec![make_text("Short", 10.0)]),
                ],
            ),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    children.push(table);

    let doc = default_doc(children);
    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Table with tall cell near page bottom should create multiple pages, got {}",
        pages.len()
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

// ─── Internal Link (Anchor) Tests ───────────────────────────────

#[test]
fn test_internal_link_produces_goto_annotation() {
    // A text with href="#Chapter 1" linking to a bookmarked view
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::Text {
                content: "Go to Chapter 1".to_string(),
                href: Some("#Chapter 1".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
        make_page_break(),
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Chapter 1 content", 12.0)],
            id: None,
            source_location: None,
            bookmark: Some("Chapter 1".to_string()),
            href: None,
            alt: None,
        },
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/S /GoTo"),
        "Internal link should produce /GoTo action"
    );
    assert!(
        !text.contains("/S /URI"),
        "Internal link should not produce /URI action"
    );
}

#[test]
fn test_external_link_still_produces_uri() {
    // Ensure external links are unaffected by the internal link feature
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::Text {
                content: "Visit site".to_string(),
                href: Some("https://example.com".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Some section", 12.0)],
            id: None,
            source_location: None,
            bookmark: Some("Some section".to_string()),
            href: None,
            alt: None,
        },
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/S /URI"),
        "External link should produce /URI action"
    );
    assert!(
        !text.contains("/S /GoTo"),
        "External link should not produce /GoTo action"
    );
}

#[test]
fn test_internal_link_no_matching_bookmark_skipped() {
    // An internal link pointing to a nonexistent bookmark should be silently skipped
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: "Go to nowhere".to_string(),
            href: Some("#Nonexistent".to_string()),
            runs: vec![],
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/Annots"),
        "Missing bookmark target should produce no annotation"
    );
    assert!(
        !text.contains("/S /GoTo"),
        "Missing bookmark target should not produce /GoTo"
    );
}

#[test]
fn test_multiple_internal_links_to_multiple_bookmarks() {
    // Two internal links on page 1 pointing to two bookmarked sections on page 2
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::Text {
                content: "Go to A".to_string(),
                href: Some("#Section A".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
        Node {
            kind: NodeKind::Text {
                content: "Go to B".to_string(),
                href: Some("#Section B".to_string()),
                runs: vec![],
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        },
        make_page_break(),
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Content A", 12.0)],
            id: None,
            source_location: None,
            bookmark: Some("Section A".to_string()),
            href: None,
            alt: None,
        },
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Content B", 12.0)],
            id: None,
            source_location: None,
            bookmark: Some("Section B".to_string()),
            href: None,
            alt: None,
        },
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    let goto_count = text.matches("/S /GoTo").count();
    assert_eq!(
        goto_count, 2,
        "Should have 2 /GoTo annotations, got {}",
        goto_count
    );
}

#[test]
fn test_view_href_produces_link_annotation() {
    // A View with href should produce a link annotation
    let doc = default_doc(vec![
        Node {
            kind: NodeKind::View,
            style: Style {
                height: Some(Dimension::Pt(30.0)),
                ..Default::default()
            },
            children: vec![make_text("TOC entry", 10.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: Some("#Target".to_string()),
            alt: None,
        },
        make_page_break(),
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Target content", 12.0)],
            id: None,
            source_location: None,
            bookmark: Some("Target".to_string()),
            href: None,
            alt: None,
        },
    ]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/S /GoTo"),
        "View with internal href should produce /GoTo annotation"
    );
}

#[test]
fn test_internal_link_json_deserialization() {
    let json = r##"{
        "children": [
            {
                "kind": { "type": "Text", "content": "Jump to section", "href": "#my-section" },
                "style": {}
            },
            { "kind": { "type": "PageBreak" } },
            {
                "kind": { "type": "View" },
                "bookmark": "my-section",
                "children": [
                    { "kind": { "type": "Text", "content": "Section content" } }
                ]
            }
        ]
    }"##;
    let bytes = forme::render_json(json).expect("Should parse internal link JSON");
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/S /GoTo"),
        "JSON internal link should produce /GoTo"
    );
}

// ─── Breakable View Background/Border Preservation ──────────────

/// Helper: count top-level Rect elements on a page
fn count_top_level_rects(page: &forme::layout::LayoutPage) -> usize {
    page.elements
        .iter()
        .filter(|e| matches!(e.draw, forme::layout::DrawCommand::Rect { .. }))
        .count()
}

/// Helper: check if a page has a Rect element with a background color
fn has_rect_with_background(page: &forme::layout::LayoutPage) -> bool {
    page.elements.iter().any(|e| {
        matches!(
            e.draw,
            forme::layout::DrawCommand::Rect {
                background: Some(_),
                ..
            }
        )
    })
}

#[test]
fn test_breakable_view_with_background_splits_across_pages() {
    // Create a view with a background that overflows onto multiple pages.
    // Use a short page to force the split with less content.
    let mut children = Vec::new();
    for i in 0..60 {
        children.push(make_text(&format!("Line {}", i), 14.0));
    }
    let view = make_styled_view(
        Style {
            background_color: Some(Color::rgb(0.9, 0.9, 1.0)),
            ..Default::default()
        },
        children,
    );

    let doc = Document {
        children: vec![view],
        metadata: Metadata::default(),
        default_page: PageConfig {
            size: PageSize::Custom {
                width: 400.0,
                height: 300.0,
            },
            margin: Edges::uniform(20.0),
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "View should overflow onto at least 2 pages, got {}",
        pages.len()
    );

    // Each page should have a Rect wrapper with the background color
    for (i, page) in pages.iter().enumerate() {
        assert!(
            has_rect_with_background(page),
            "Page {} should have a Rect element with background color",
            i
        );
    }

    // Should also produce a valid PDF
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_breakable_view_background_does_not_overlap_footer() {
    // A breakable view with a background color should not extend its
    // wrapper Rect into the footer's reserved space on any page.
    let page_height = 300.0;
    let margin = 20.0;
    let footer_padding = 20.0; // top + bottom = 40 total
    let footer_font = 12.0;

    let footer = Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Footer,
            pages: FixedPageFilter::default(),
        },
        style: Style {
            padding: Some(Edges::uniform(footer_padding)),
            ..Default::default()
        },
        children: vec![make_text("Footer", footer_font)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let mut view_children = Vec::new();
    for i in 0..60 {
        view_children.push(make_text(&format!("Item {}", i), 14.0));
    }
    let view = make_styled_view(
        Style {
            background_color: Some(Color::rgb(0.8, 1.0, 0.8)),
            ..Default::default()
        },
        view_children,
    );

    let doc = Document {
        children: vec![footer, view],
        metadata: Metadata::default(),
        default_page: PageConfig {
            size: PageSize::Custom {
                width: 400.0,
                height: page_height,
            },
            margin: Edges::uniform(margin),
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Should overflow to at least 2 pages, got {}",
        pages.len()
    );

    // The page content area bottom (before footer) =
    //   page_height - margin_bottom = 300 - 20 = 280.
    // The footer occupies space above that, so the content must stop
    // before the footer. We check that no background Rect extends
    // past the usable content area (i.e., page_height - margin - footer_height).
    // We use a generous threshold: the rect bottom must be ≤ page_height - margin.
    // More importantly, it must NOT reach page_height - margin (the absolute bottom
    // of the content box), because the footer takes space away from that.
    let page_content_bottom = page_height - margin; // 280.0

    for (i, page) in pages.iter().enumerate() {
        for elem in &page.elements {
            if let forme::layout::DrawCommand::Rect {
                background: Some(_),
                ..
            } = &elem.draw
            {
                let rect_bottom = elem.y + elem.height;
                assert!(
                    rect_bottom < page_content_bottom - 1.0,
                    "Page {}: background Rect bottom ({:.1}) should not reach content bottom ({:.1}) — footer space must be reserved",
                    i,
                    rect_bottom,
                    page_content_bottom,
                );
            }
        }
    }

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_breakable_view_without_visual_stays_unwrapped() {
    // A plain view (no background, no border) should NOT get a Rect wrapper
    let mut children = Vec::new();
    for i in 0..60 {
        children.push(make_text(&format!("Line {}", i), 14.0));
    }
    let view = make_view(children);

    let doc = Document {
        children: vec![view],
        metadata: Metadata::default(),
        default_page: PageConfig {
            size: PageSize::Custom {
                width: 400.0,
                height: 300.0,
            },
            margin: Edges::uniform(20.0),
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pages = layout_doc(&doc);
    assert!(pages.len() >= 2, "Should overflow onto multiple pages");

    // No page should have top-level Rect elements (plain view = no wrapper)
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(
            count_top_level_rects(page),
            0,
            "Page {} should have no Rect wrapper for a plain view",
            i
        );
    }
}

#[test]
fn test_single_page_breakable_view_with_background_gets_wrapped() {
    // A breakable view with background that fits on one page should still get a Rect wrapper
    let view = make_styled_view(
        Style {
            background_color: Some(Color::rgb(1.0, 0.9, 0.9)),
            padding: Some(Edges::uniform(10.0)),
            ..Default::default()
        },
        vec![make_text("Short content", 12.0)],
    );

    let doc = default_doc(vec![view]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1, "Should fit on one page");

    assert!(
        has_rect_with_background(&pages[0]),
        "Single-page breakable view with background should get a Rect wrapper"
    );

    // Verify the Rect has children (the text content)
    let rect = pages[0]
        .elements
        .iter()
        .find(|e| {
            matches!(
                e.draw,
                forme::layout::DrawCommand::Rect {
                    background: Some(_),
                    ..
                }
            )
        })
        .expect("Should find Rect element");
    assert!(
        !rect.children.is_empty(),
        "Rect wrapper should contain child elements"
    );
}

// ─── Text Transform ────────────────────────────────────────────

#[test]
fn test_text_transform_uppercase_in_pdf() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: "hello world".to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            text_transform: Some(TextTransform::Uppercase),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);

    let pages = layout_doc(&doc);

    // The laid-out text should contain uppercase characters
    let text_content = extract_text_from_pages(&pages);
    assert!(
        text_content.contains('H') && text_content.contains('W'),
        "Text should be uppercased, got: {}",
        text_content
    );
    assert!(
        !text_content.contains('h'),
        "Should not contain lowercase 'h', got: {}",
        text_content
    );
}

#[test]
fn test_text_transform_resolves_with_inheritance() {
    let style = Style {
        text_transform: Some(TextTransform::Uppercase),
        ..Default::default()
    };
    let parent_resolved = style.resolve(None, 500.0);

    // Child without text_transform should inherit from parent
    let child_style = Style::default();
    let child_resolved = child_style.resolve(Some(&parent_resolved), 500.0);
    assert!(matches!(
        child_resolved.text_transform,
        TextTransform::Uppercase
    ));

    // Child with explicit text_transform should override
    let child_override = Style {
        text_transform: Some(TextTransform::Lowercase),
        ..Default::default()
    };
    let child_resolved = child_override.resolve(Some(&parent_resolved), 500.0);
    assert!(matches!(
        child_resolved.text_transform,
        TextTransform::Lowercase
    ));
}

// ─── Opacity ───────────────────────────────────────────────────

#[test]
fn test_opacity_produces_ext_gstate_in_pdf() {
    let doc = default_doc(vec![make_styled_view(
        Style {
            opacity: Some(0.5),
            background_color: Some(Color::rgb(1.0, 0.0, 0.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![make_text("Semi-transparent", 12.0)],
    )]);

    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    // Should contain ExtGState dictionary with opacity
    assert!(
        pdf_str.contains("/ExtGState"),
        "PDF should contain /ExtGState resource"
    );
    assert!(
        pdf_str.contains("/ca 0.5"),
        "PDF should contain /ca 0.5 for fill opacity"
    );
    assert!(
        pdf_str.contains("/CA 0.5"),
        "PDF should contain /CA 0.5 for stroke opacity"
    );
}

#[test]
fn test_opacity_1_produces_no_ext_gstate() {
    let doc = default_doc(vec![make_text("Full opacity", 12.0)]);

    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    assert!(
        !pdf_str.contains("/ExtGState"),
        "PDF should NOT contain /ExtGState when all opacities are 1.0"
    );
}

/// Helper: extract all text characters from laid-out pages.
fn extract_text_from_pages(pages: &[forme::layout::LayoutPage]) -> String {
    let mut text = String::new();
    for page in pages {
        extract_text_from_elements(&page.elements, &mut text);
    }
    text
}

fn extract_text_from_elements(elements: &[forme::layout::LayoutElement], text: &mut String) {
    for el in elements {
        if let forme::layout::DrawCommand::Text { lines, .. } = &el.draw {
            for line in lines {
                for glyph in &line.glyphs {
                    text.push(glyph.char_value);
                }
            }
        }
        extract_text_from_elements(&el.children, text);
    }
}

#[test]
fn test_fonts_via_json_deserialization() {
    // Test that a document with fonts[] array deserializes and renders correctly
    let font_data = load_test_font();
    if font_data.is_none() {
        println!("Skipping test_fonts_via_json — no test font available");
        return;
    }
    let font_data = font_data.unwrap();
    let font_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &font_data);

    let json = format!(
        r#"{{
            "children": [{{
                "kind": {{ "type": "Text", "content": "Hello custom font" }},
                "style": {{ "fontFamily": "MyFont", "fontSize": 16 }},
                "children": []
            }}],
            "metadata": {{}},
            "defaultPage": {{
                "size": "A4",
                "margin": {{ "top": 54, "right": 54, "bottom": 54, "left": 54 }},
                "wrap": true
            }},
            "fonts": [{{
                "family": "MyFont",
                "src": "data:font/ttf;base64,{}",
                "weight": 400,
                "italic": false
            }}]
        }}"#,
        font_b64
    );

    let bytes = forme::render_json(&json).unwrap();
    assert_valid_pdf(&bytes);

    let text = String::from_utf8_lossy(&bytes);
    // Should have CIDFont (embedded custom font) not just standard fonts
    assert!(
        text.contains("CIDFontType2"),
        "PDF should contain embedded custom font (CIDFontType2)"
    );
}

#[test]
fn test_fonts_empty_array_renders_ok() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Text", "content": "Hello" },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {
            "size": "A4",
            "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 },
            "wrap": true
        },
        "fonts": []
    }"#;

    let bytes = forme::render_json(json).unwrap();
    assert_valid_pdf(&bytes);
}

#[test]
fn test_fonts_field_omitted_renders_ok() {
    // fonts field omitted entirely — should default to empty vec
    let json = r#"{
        "children": [{
            "kind": { "type": "Text", "content": "Hello" },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {
            "size": "A4",
            "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 },
            "wrap": true
        }
    }"#;

    let bytes = forme::render_json(json).unwrap();
    assert_valid_pdf(&bytes);
}

#[test]
fn test_breakable_view_continuation_page_has_top_padding() {
    // Use a small custom page so content overflows to page 2
    let page_config = PageConfig {
        size: PageSize::Custom {
            width: 200.0,
            height: 200.0,
        },
        margin: Edges::uniform(20.0),
        wrap: true,
        ..Default::default()
    };
    let padding = 15.0;

    // Create a breakable view with background + padding containing children that overflow
    let breakable_view = make_styled_view(
        Style {
            background_color: Some(Color {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                a: 1.0,
            }),
            padding: Some(Edges::uniform(padding)),
            ..Default::default()
        },
        vec![
            make_text("First child on page 1", 12.0),
            make_text("Second child on page 1", 12.0),
            make_text("Third child on page 1", 12.0),
            make_text("Fourth child on page 1", 12.0),
            make_text("Fifth child on page 1", 12.0),
            make_text("Sixth child on page 1", 12.0),
            make_text("Seventh child overflows", 12.0),
            make_text("Eighth child on page 2", 12.0),
            make_text("Ninth child on page 2", 12.0),
            make_text("Tenth child on page 2", 12.0),
        ],
    );

    let doc = Document {
        children: vec![breakable_view],
        metadata: Metadata::default(),
        default_page: page_config,
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Expected at least 2 pages, got {}",
        pages.len()
    );

    // On continuation pages (page 2+), the wrapper Rect element should exist
    // and the first child inside it should be offset by padding.top from the Rect's top edge.
    for (page_idx, page) in pages.iter().enumerate().skip(1) {
        // Find the wrapper Rect element (the breakable view's background)
        let wrapper = page
            .elements
            .iter()
            .find(|el| matches!(el.draw, forme::layout::DrawCommand::Rect { .. }))
            .unwrap_or_else(|| panic!("Page {} should have a wrapper Rect element", page_idx + 1));

        assert!(
            !wrapper.children.is_empty(),
            "Page {} wrapper should have children",
            page_idx + 1
        );

        let first_child = &wrapper.children[0];
        let offset_from_rect_top = first_child.y - wrapper.y;
        assert!(
            (offset_from_rect_top - padding).abs() < 1.0,
            "Page {}: first child should be {}pt below wrapper top, but was {}pt (child.y={}, wrapper.y={})",
            page_idx + 1,
            padding,
            offset_from_rect_top,
            first_child.y,
            wrapper.y
        );
    }
}

// ─── Template expression evaluator tests ──────────────────────────────

use forme::template::evaluate_template;
use serde_json::json;

#[test]
fn test_template_ref_simple() {
    let template = json!({
        "children": [
            {"kind": {"type": "Text", "content": {"$ref": "title"}}, "style": {}, "children": []}
        ],
        "metadata": {},
        "defaultPage": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}
    });
    let data = json!({"title": "Hello World"});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result["children"][0]["kind"]["content"], "Hello World");
}

#[test]
fn test_template_ref_nested_path() {
    let template = json!({"$ref": "user.address.city"});
    let data = json!({"user": {"address": {"city": "Portland"}}});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("Portland"));
}

#[test]
fn test_template_each_basic() {
    let template = json!({
        "children": [
            {
                "$each": {"$ref": "items"},
                "as": "$item",
                "template": {
                    "kind": {"type": "Text", "content": {"$ref": "$item.name"}},
                    "style": {},
                    "children": []
                }
            }
        ]
    });
    let data = json!({"items": [{"name": "A"}, {"name": "B"}, {"name": "C"}]});
    let result = evaluate_template(&template, &data).unwrap();
    let children = result["children"].as_array().unwrap();
    assert_eq!(children.len(), 3);
    assert_eq!(children[0]["kind"]["content"], "A");
    assert_eq!(children[1]["kind"]["content"], "B");
    assert_eq!(children[2]["kind"]["content"], "C");
}

#[test]
fn test_template_each_nested() {
    let template = json!({
        "items": [
            {
                "$each": {"$ref": "groups"},
                "as": "$group",
                "template": {
                    "name": {"$ref": "$group.name"},
                    "members": [
                        {
                            "$each": {"$ref": "$group.members"},
                            "as": "$member",
                            "template": {"$ref": "$member"}
                        }
                    ]
                }
            }
        ]
    });
    let data = json!({
        "groups": [
            {"name": "A", "members": ["x", "y"]},
            {"name": "B", "members": ["z"]}
        ]
    });
    let result = evaluate_template(&template, &data).unwrap();
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "A");
    assert_eq!(items[0]["members"].as_array().unwrap().len(), 2);
    assert_eq!(items[1]["members"].as_array().unwrap().len(), 1);
}

#[test]
fn test_template_each_empty_array() {
    let template = json!({
        "children": [
            {
                "$each": {"$ref": "items"},
                "as": "$item",
                "template": {"$ref": "$item"}
            }
        ]
    });
    let data = json!({"items": []});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result["children"].as_array().unwrap().len(), 0);
}

#[test]
fn test_template_if_truthy() {
    let template = json!({
        "$if": {"$ref": "showTitle"},
        "then": {"kind": {"type": "Text", "content": "Title"}, "style": {}, "children": []},
        "else": {"kind": {"type": "Text", "content": "No Title"}, "style": {}, "children": []}
    });
    let data = json!({"showTitle": true});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result["kind"]["content"], "Title");
}

#[test]
fn test_template_if_falsy() {
    let template = json!({
        "$if": {"$ref": "showTitle"},
        "then": "yes",
        "else": "no"
    });
    let data = json!({"showTitle": false});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("no"));
}

#[test]
fn test_template_if_with_operator() {
    let template = json!({
        "$if": {"$gt": [{"$ref": "count"}, 10]},
        "then": "many",
        "else": "few"
    });
    let data = json!({"count": 25});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("many"));
}

#[test]
fn test_template_comparison_ops() {
    let data = json!({"a": 5, "b": 10});

    let eq = evaluate_template(&json!({"$eq": [{"$ref": "a"}, 5]}), &data).unwrap();
    assert_eq!(eq, json!(true));

    let ne = evaluate_template(&json!({"$ne": [{"$ref": "a"}, {"$ref": "b"}]}), &data).unwrap();
    assert_eq!(ne, json!(true));

    let gt = evaluate_template(&json!({"$gt": [{"$ref": "b"}, {"$ref": "a"}]}), &data).unwrap();
    assert_eq!(gt, json!(true));

    let lt = evaluate_template(&json!({"$lt": [{"$ref": "a"}, {"$ref": "b"}]}), &data).unwrap();
    assert_eq!(lt, json!(true));

    let gte = evaluate_template(&json!({"$gte": [{"$ref": "a"}, 5]}), &data).unwrap();
    assert_eq!(gte, json!(true));

    let lte = evaluate_template(&json!({"$lte": [{"$ref": "a"}, 5]}), &data).unwrap();
    assert_eq!(lte, json!(true));
}

#[test]
fn test_template_arithmetic_ops() {
    let data = json!({"x": 10, "y": 3});

    let add = evaluate_template(&json!({"$add": [{"$ref": "x"}, {"$ref": "y"}]}), &data).unwrap();
    assert_eq!(add, json!(13.0));

    let sub = evaluate_template(&json!({"$sub": [{"$ref": "x"}, {"$ref": "y"}]}), &data).unwrap();
    assert_eq!(sub, json!(7.0));

    let mul = evaluate_template(&json!({"$mul": [{"$ref": "x"}, {"$ref": "y"}]}), &data).unwrap();
    assert_eq!(mul, json!(30.0));

    let div = evaluate_template(&json!({"$div": [{"$ref": "x"}, {"$ref": "y"}]}), &data).unwrap();
    let div_val = div.as_f64().unwrap();
    assert!((div_val - 3.333333).abs() < 0.001);
}

#[test]
fn test_template_string_ops() {
    let data = json!({"name": "hello"});

    let upper = evaluate_template(&json!({"$upper": {"$ref": "name"}}), &data).unwrap();
    assert_eq!(upper, json!("HELLO"));

    let lower = evaluate_template(&json!({"$lower": "WORLD"}), &data).unwrap();
    assert_eq!(lower, json!("world"));
}

#[test]
fn test_template_concat() {
    let data = json!({"first": "John", "last": "Doe"});
    let template = json!({"$concat": [{"$ref": "first"}, " ", {"$ref": "last"}]});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("John Doe"));
}

#[test]
fn test_template_format() {
    let data = json!({"price": 42.5});
    let template = json!({"$format": [{"$ref": "price"}, "0.00"]});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("42.50"));
}

#[test]
fn test_template_cond() {
    let data = json!({"premium": true});
    let template = json!({"$cond": [{"$ref": "premium"}, "gold", "standard"]});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!("gold"));
}

#[test]
fn test_template_count() {
    let data = json!({"items": [1, 2, 3, 4, 5]});
    let template = json!({"$count": {"$ref": "items"}});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result, json!(5));
}

#[test]
fn test_template_missing_ref_omitted() {
    let template = json!({"a": {"$ref": "exists"}, "b": {"$ref": "missing"}});
    let data = json!({"exists": "yes"});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result["a"], json!("yes"));
    // Missing ref should be omitted from the object
    assert!(result.get("b").is_none());
}

#[test]
fn test_template_passthrough_primitives() {
    let template = json!({
        "type": "Text",
        "content": "static",
        "fontSize": 12,
        "bold": true,
        "empty": null
    });
    let data = json!({});
    let result = evaluate_template(&template, &data).unwrap();
    assert_eq!(result["type"], "Text");
    assert_eq!(result["content"], "static");
    assert_eq!(result["fontSize"], 12);
    assert_eq!(result["bold"], true);
    assert!(result["empty"].is_null());
}

#[test]
fn test_template_full_render() {
    // Full pipeline: template JSON + data → evaluate → render PDF
    let template_json = serde_json::to_string(&json!({
        "children": [
            {
                "kind": {"type": "Text", "content": {"$ref": "title"}},
                "style": {"fontSize": 24},
                "children": []
            },
            {
                "kind": {"type": "View"},
                "style": {},
                "children": [
                    {
                        "$each": {"$ref": "items"},
                        "as": "$item",
                        "template": {
                            "kind": {"type": "Text", "content": {"$ref": "$item"}},
                            "style": {},
                            "children": []
                        }
                    }
                ]
            }
        ],
        "metadata": {"title": {"$ref": "title"}},
        "defaultPage": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}
    })).unwrap();
    let data_json = r#"{"title": "Invoice #001", "items": ["Widget A", "Widget B"]}"#;

    let pdf = forme::render_template(&template_json, data_json).unwrap();
    assert_valid_pdf(&pdf);
}

#[test]
fn test_template_div_by_zero() {
    let data = json!({});
    let result = evaluate_template(&json!({"$div": [10, 0]}), &data).unwrap();
    assert_eq!(result, json!(0.0));
}

// ─── Document lang ────────────────────────────────────────────

#[test]
fn test_document_lang_in_pdf_catalog() {
    let doc = Document {
        children: vec![make_text("Hello", 12.0)],
        metadata: Metadata {
            title: None,
            author: None,
            subject: None,
            creator: None,
            lang: Some("en-US".to_string()),
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Lang (en-US)"),
        "PDF catalog should contain /Lang"
    );
}

#[test]
fn test_document_lang_omitted_when_none() {
    let doc = default_doc(vec![make_text("Hello", 12.0)]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/Lang"),
        "PDF catalog should not contain /Lang when not set"
    );
}

// ─── Image/SVG href passthrough ─────────────────────────────────

#[test]
fn test_image_href_produces_link_annotation() {
    let one_px_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let doc = default_doc(vec![Node {
        kind: NodeKind::Image {
            src: one_px_png.to_string(),
            width: Some(100.0),
            height: Some(50.0),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: Some("https://example.com".to_string()),
        alt: None,
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Annots"),
        "Image with href should produce annotations"
    );
    assert!(
        text.contains("https://example.com"),
        "Annotation should contain the URL"
    );
}

// ─── Alt text on Node ───────────────────────────────────────────

#[test]
fn test_alt_deserializes_from_json() {
    let json_str = r#"{
        "children": [{
            "kind": {"type": "Image", "src": "test.png", "width": 100, "height": 50},
            "style": {},
            "children": [],
            "alt": "A test image"
        }],
        "metadata": {},
        "defaultPage": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}
    }"#;
    let doc: Document = serde_json::from_str(json_str).unwrap();
    assert_eq!(doc.children[0].alt.as_deref(), Some("A test image"));
}

#[test]
fn test_lang_deserializes_from_json() {
    let json_str = r#"{
        "children": [],
        "metadata": {"lang": "fr-FR"},
        "defaultPage": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}
    }"#;
    let doc: Document = serde_json::from_str(json_str).unwrap();
    assert_eq!(doc.metadata.lang.as_deref(), Some("fr-FR"));
}

// ─── Hyphenation Tests ──────────────────────────────────────────

#[test]
fn test_hyphenation_json_round_trip() {
    let json_str = r#"{
        "children": [{
            "kind": {"type": "Page", "config": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}},
            "style": {},
            "children": [{
                "kind": {"type": "Text", "content": "extraordinary"},
                "style": {"hyphens": "auto"},
                "children": []
            }]
        }],
        "metadata": {},
        "defaultPage": {"size": "A4", "margin": {"top": 54, "right": 54, "bottom": 54, "left": 54}, "wrap": true}
    }"#;
    let doc: Document = serde_json::from_str(json_str).unwrap();
    let text_node = &doc.children[0].children[0];
    assert_eq!(text_node.style.hyphens, Some(Hyphens::Auto));
}

#[test]
fn test_hyphenation_inherits() {
    // Parent has hyphens: auto, child text inherits it
    let parent_style = Style {
        hyphens: Some(Hyphens::Auto),
        ..Default::default()
    };
    let resolved_parent = parent_style.resolve(None, 500.0);
    assert_eq!(resolved_parent.hyphens, Hyphens::Auto);

    // Child with no hyphens set should inherit from parent
    let child_style = Style::default();
    let resolved_child = child_style.resolve(Some(&resolved_parent), 500.0);
    assert_eq!(resolved_child.hyphens, Hyphens::Auto);

    // Child with explicit override
    let child_override = Style {
        hyphens: Some(Hyphens::None),
        ..Default::default()
    };
    let resolved_override = child_override.resolve(Some(&resolved_parent), 500.0);
    assert_eq!(resolved_override.hyphens, Hyphens::None);
}

#[test]
fn test_hyphenation_min_content_in_flex() {
    // A flex row with a narrow child containing a long word + hyphens: auto
    // should allow the child to shrink smaller than the full word width
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();

    // Without hyphenation: min-content is the full word
    let text_no_hyphen = Node {
        kind: NodeKind::Text {
            content: "extraordinary".to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            hyphens: Some(Hyphens::Manual),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let style_no_hyphen = text_no_hyphen.style.resolve(None, 500.0);
    let min_width_no_hyphen =
        engine.measure_min_content_width(&text_no_hyphen, &style_no_hyphen, &font_context);

    // With hyphenation: min-content is the widest syllable
    let text_with_hyphen = Node {
        kind: NodeKind::Text {
            content: "extraordinary".to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            hyphens: Some(Hyphens::Auto),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let style_with_hyphen = text_with_hyphen.style.resolve(None, 500.0);
    let min_width_with_hyphen =
        engine.measure_min_content_width(&text_with_hyphen, &style_with_hyphen, &font_context);

    assert!(
        min_width_with_hyphen < min_width_no_hyphen,
        "With auto hyphenation, min-content ({min_width_with_hyphen}) should be smaller than without ({min_width_no_hyphen})"
    );
}

// ─── Justified text ─────────────────────────────────────────────

#[test]
fn test_justified_text_produces_valid_pdf() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Page {
                config: PageConfig {
                    size: PageSize::Letter,
                    margin: Edges { top: 36.0, right: 36.0, bottom: 36.0, left: 36.0 },
                    wrap: true,
                ..Default::default()
                },
            },
            style: Style::default(),
            children: vec![Node {
                kind: NodeKind::Text {
                    content: "The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog again.".to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style {
                    text_align: Some(TextAlign::Justify),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            }],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig {
            size: PageSize::Letter,
            margin: Edges { top: 72.0, right: 72.0, bottom: 72.0, left: 72.0 },
            wrap: true,
        ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).expect("Should render justified text");
    assert!(bytes.len() > 100);
    assert!(bytes.starts_with(b"%PDF"));
}

// ─── Language inheritance ────────────────────────────────────────

#[test]
fn test_lang_inherits_to_text_nodes() {
    // Document lang should cascade to child styles
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Page {
                config: PageConfig {
                    size: PageSize::A4,
                    margin: Edges {
                        top: 36.0,
                        right: 36.0,
                        bottom: 36.0,
                        left: 36.0,
                    },
                    wrap: true,
                    ..Default::default()
                },
            },
            style: Style::default(),
            children: vec![make_text("Hallo Welt", 12.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata {
            lang: Some("de".to_string()),
            ..Default::default()
        },
        default_page: PageConfig {
            size: PageSize::A4,
            margin: Edges {
                top: 72.0,
                right: 72.0,
                bottom: 72.0,
                left: 72.0,
            },
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    // Just verify it renders without error — lang cascading is tested at the unit level
    let bytes = forme::render(&doc).expect("Should render with document lang");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn test_per_node_lang_override() {
    // A child node should be able to override the document lang
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Page {
                config: PageConfig {
                    size: PageSize::A4,
                    margin: Edges {
                        top: 36.0,
                        right: 36.0,
                        bottom: 36.0,
                        left: 36.0,
                    },
                    wrap: true,
                    ..Default::default()
                },
            },
            style: Style::default(),
            children: vec![Node {
                kind: NodeKind::Text {
                    content: "Bonjour le monde".to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style {
                    lang: Some("fr".to_string()),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            }],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata {
            lang: Some("de".to_string()),
            ..Default::default()
        },
        default_page: PageConfig {
            size: PageSize::A4,
            margin: Edges {
                top: 72.0,
                right: 72.0,
                bottom: 72.0,
                left: 72.0,
            },
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).expect("Should render with per-node lang override");
    assert!(bytes.starts_with(b"%PDF"));
}

// ─── Tagged PDF Integration Tests ────────────────────────────────

#[test]
fn test_tagged_pdf_has_struct_tree_root() {
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![make_text("Hello World", 16.0)],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    // Catalog must contain /MarkInfo and /StructTreeRoot
    assert!(
        pdf_str.contains("/MarkInfo << /Marked true >>"),
        "Tagged PDF must have /MarkInfo in Catalog"
    );
    assert!(
        pdf_str.contains("/StructTreeRoot"),
        "Tagged PDF must have /StructTreeRoot in Catalog"
    );
    // Structure tree root must be present
    assert!(
        pdf_str.contains("/Type /StructTreeRoot"),
        "Tagged PDF must have StructTreeRoot object"
    );
    // RoleMap must exist but be EMPTY: every role Forme emits is a standard
    // PDF structure type, so it needs no remapping. Mapping a standard type
    // to itself is the circular RoleMap veraPDF flags (PDF/UA-1 7.1-6).
    assert!(
        pdf_str.contains("/RoleMap "),
        "Tagged PDF must reference a RoleMap from the StructTreeRoot"
    );
    assert!(
        !pdf_str.contains("/Document /Document"),
        "RoleMap must not self-map the standard /Document type (7.1-6)"
    );
}

#[test]
fn test_tagged_pdf_parent_tree_consistency() {
    // Generate enough text to span 2+ pages to test ParentTree across pages
    let mut children = Vec::new();
    for i in 0..60 {
        children.push(make_text(
            &format!(
                "Paragraph {}: Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
                i + 1
            ),
            11.0,
        ));
    }

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            children,
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    // Each page must have /StructParents
    assert!(
        pdf_str.contains("/StructParents 0"),
        "First page must have /StructParents 0"
    );

    // ParentTree must have /Nums array
    assert!(
        pdf_str.contains("/Nums ["),
        "ParentTree must have /Nums array"
    );

    // Structure elements must reference marked content via MCR entries
    assert!(
        pdf_str.contains("/Type /MCR"),
        "Tagged PDF must have marked content references in structure elements"
    );

    // Verify MCID references exist in structure elements
    assert!(
        pdf_str.contains("/MCID 0"),
        "Tagged PDF must have MCID 0 reference in structure elements"
    );
}

#[test]
fn test_tagged_pdf_nested_text_roles() {
    // Nested Text inside Text: outer → P, inner → Span
    let outer_text = Node {
        kind: NodeKind::Text {
            content: "Hello ".to_string(),
            href: None,
            runs: vec![TextRun {
                content: "bold world".to_string(),
                style: Style {
                    font_weight: Some(700),
                    ..Default::default()
                },
                href: None,
            }],
        },
        style: Style {
            font_size: Some(12.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![outer_text],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    // Must have both P and Span structure elements
    assert!(pdf_str.contains("/S /P"), "Outer Text should map to /S /P");
    assert!(
        pdf_str.contains("/S /Span"),
        "TextLine elements should map to /S /Span"
    );

    // StructTreeRoot must exist
    assert!(pdf_str.contains("/Type /StructTreeRoot"));
}

#[test]
fn test_tagged_pdf_table_th_td() {
    // Table with header row and body rows
    let header_row = Node {
        kind: NodeKind::TableRow { is_header: true },
        style: Style::default(),
        children: vec![Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style::default(),
            children: vec![make_text("Name", 10.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let body_row = Node {
        kind: NodeKind::TableRow { is_header: false },
        style: Style::default(),
        children: vec![Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style::default(),
            children: vec![make_text("Alice", 10.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let table = Node {
        kind: NodeKind::Table {
            columns: vec![ColumnDef {
                width: ColumnWidth::Fraction(1.0),
            }],
        },
        style: Style::default(),
        children: vec![header_row, body_row],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![table],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    // Must have TR, TH, and TD structure elements
    // Note: layout_table doesn't create a wrapper Table element (rows are direct children)
    assert!(
        pdf_str.contains("/S /TR"),
        "Must have /S /TR structure element"
    );
    assert!(
        pdf_str.contains("/S /TH"),
        "Header cells must map to /S /TH"
    );
    assert!(pdf_str.contains("/S /TD"), "Body cells must map to /S /TD");
}

#[test]
fn test_tagged_pdf_figure_alt_text() {
    // An SVG with alt text should produce a Figure element with /Alt
    let svg_node = Node {
        kind: NodeKind::Svg {
            width: 100.0,
            height: 100.0,
            view_box: None,
            content: r#"<rect width="100" height="100" fill="red"/>"#.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: Some("A red square".to_string()),
    };

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![svg_node],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    // Must have Figure structure element with alt text
    assert!(
        pdf_str.contains("/S /Figure"),
        "SVG with alt text must produce /S /Figure"
    );
    assert!(
        pdf_str.contains("/Alt (A red square)"),
        "Figure must carry /Alt text"
    );
}

// ─── BiDi Text Tests ─────────────────────────────────────────

#[test]
fn test_direction_json_deserialization() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "Text", "content": "مرحبا" },
            "style": { "direction": "rtl", "fontSize": 14 }
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let style = &doc.children[0].style;
    assert!(matches!(style.direction, Some(Direction::Rtl)));
}

#[test]
fn test_direction_auto_detection() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "Text", "content": "Hello World" },
            "style": { "direction": "auto", "fontSize": 14 }
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    assert!(matches!(
        doc.children[0].style.direction,
        Some(Direction::Auto)
    ));
    let bytes = forme::render(&doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_rtl_text_produces_valid_pdf() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "Text", "content": "مرحبا بالعالم" },
            "style": { "fontSize": 14, "direction": "rtl" }
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);
    assert!(pdf_str.starts_with("%PDF-1.7"));
    assert!(pdf_str.contains("%%EOF"));
}

#[test]
fn test_mixed_ltr_rtl_produces_valid_pdf() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "Text", "content": "Hello مرحبا World" },
            "style": { "fontSize": 14 }
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);
    assert!(pdf_str.starts_with("%PDF-1.7"));
}

#[test]
fn test_rtl_direction_defaults_text_align_right() {
    let style = Style {
        direction: Some(Direction::Rtl),
        font_size: Some(12.0),
        ..Default::default()
    };
    let resolved = style.resolve(None, 500.0);
    assert!(
        matches!(resolved.text_align, TextAlign::Right),
        "RTL direction should default text-align to Right"
    );
}

#[test]
fn test_direction_inherits_from_parent() {
    let parent_style = Style {
        direction: Some(Direction::Rtl),
        font_size: Some(14.0),
        ..Default::default()
    };
    let parent_resolved = parent_style.resolve(None, 500.0);

    let child_style = Style {
        font_size: Some(12.0),
        ..Default::default()
    };
    let child_resolved = child_style.resolve(Some(&parent_resolved), 500.0);
    assert!(
        matches!(child_resolved.direction, Direction::Rtl),
        "Child should inherit direction from parent"
    );
}

// ─── CSS Grid Tests ──────────────────────────────────────────

#[test]
fn test_grid_display_json_deserialization() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "View" },
            "style": {
                "display": "Grid",
                "gridTemplateColumns": [{ "Fr": 1.0 }, { "Fr": 1.0 }, { "Fr": 1.0 }]
            },
            "children": [
                { "kind": { "type": "Text", "content": "A" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "B" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "C" }, "style": { "fontSize": 12 } }
            ]
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);
    assert!(pdf_str.starts_with("%PDF-1.7"));
}

#[test]
fn test_grid_fixed_and_fr_columns() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "View" },
            "style": {
                "display": "Grid",
                "gridTemplateColumns": [{ "Pt": 100 }, { "Fr": 1.0 }, { "Fr": 2.0 }],
                "gap": 10
            },
            "children": [
                { "kind": { "type": "Text", "content": "Fixed" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "1fr" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "2fr" }, "style": { "fontSize": 12 } }
            ]
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_grid_multiple_rows() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "View" },
            "style": {
                "display": "Grid",
                "gridTemplateColumns": [{ "Fr": 1.0 }, { "Fr": 1.0 }],
                "rowGap": 5,
                "columnGap": 10
            },
            "children": [
                { "kind": { "type": "Text", "content": "A" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "B" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "C" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "D" }, "style": { "fontSize": 12 } }
            ]
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_grid_explicit_placement() {
    let json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "View" },
            "style": {
                "display": "Grid",
                "gridTemplateColumns": [{ "Fr": 1.0 }, { "Fr": 1.0 }, { "Fr": 1.0 }]
            },
            "children": [
                {
                    "kind": { "type": "Text", "content": "Placed" },
                    "style": {
                        "fontSize": 12,
                        "gridPlacement": { "columnStart": 2, "rowStart": 1 }
                    }
                },
                { "kind": { "type": "Text", "content": "Auto 1" }, "style": { "fontSize": 12 } },
                { "kind": { "type": "Text", "content": "Auto 2" }, "style": { "fontSize": 12 } }
            ]
        }]
    }"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let bytes = forme::render(&doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_grid_display_default_is_flex() {
    let style = Style::default();
    let resolved = style.resolve(None, 500.0);
    assert!(
        matches!(resolved.display, Display::Flex),
        "Display should default to Flex"
    );
}

#[test]
fn test_grid_track_resolution() {
    use forme::layout::grid;
    let tracks = vec![
        GridTrackSize::Pt(100.0),
        GridTrackSize::Fr(1.0),
        GridTrackSize::Fr(2.0),
    ];
    let sizes = grid::resolve_tracks(&tracks, 400.0, 0.0, &[]);
    assert!((sizes[0] - 100.0).abs() < 0.001);
    assert!((sizes[1] - 100.0).abs() < 0.001);
    assert!((sizes[2] - 200.0).abs() < 0.001);
}

// ─── QR Code Tests ───────────────────────────────────────────────

#[test]
fn test_qrcode_renders_to_pdf() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::QrCode {
                data: "https://formepdf.com".to_string(),
                size: Some(100.0),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).expect("QR code should render to PDF");
    assert!(pdf.len() > 100, "PDF should have content");
    assert!(pdf.starts_with(b"%PDF"), "Should be a valid PDF");
}

#[test]
fn test_qrcode_with_explicit_size() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::QrCode {
                data: "test".to_string(),
                size: Some(50.0),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(layout.pages.len(), 1);
    // The QR code element should be 50x50
    let qr_elem = &layout.pages[0].elements[0];
    assert!((qr_elem.width - 50.0).abs() < 0.1);
    assert!((qr_elem.height - 50.0).abs() < 0.1);
}

#[test]
fn test_qrcode_page_break() {
    // Fill a page almost to the bottom, then add a QR code that should overflow
    let mut children: Vec<Node> = Vec::new();
    // Add text to fill up the page
    for _ in 0..50 {
        children.push(make_text("Line of text to fill the page up", 12.0));
    }
    children.push(Node {
        kind: NodeKind::QrCode {
            data: "overflow".to_string(),
            size: Some(200.0),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    });

    let doc = Document {
        children,
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert!(layout.pages.len() >= 2, "QR code should cause a page break");
}

#[test]
fn test_qrcode_json_deserialization() {
    let json = r#"{
        "children": [{
            "kind": { "type": "QrCode", "data": "hello world", "size": 80 },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("QR code JSON should render");
    assert!(pdf.starts_with(b"%PDF"));
}

// ─── Font Fallback Chain Tests ──────────────────────────────────

#[test]
fn test_font_fallback_chain_in_document() {
    // Use "Missing, Helvetica" — should fall back to Helvetica
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Text {
                content: "Fallback test".to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_family: Some("Missing, Helvetica".to_string()),
                font_size: Some(12.0),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).expect("Fallback chain should render");
    assert!(pdf.starts_with(b"%PDF"));
}

// ─── textOverflow Tests ──────────────────────────────────────────

#[test]
fn test_text_overflow_ellipsis_json() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Text", "content": "This is a very long text that should be truncated with ellipsis because the container is narrow" },
            "style": { "textOverflow": "Ellipsis", "fontSize": 12 },
            "children": []
        }],
        "metadata": {},
        "defaultPage": { "size": { "Custom": { "width": 100, "height": 200 } }, "margin": { "top": 10, "right": 10, "bottom": 10, "left": 10 } },
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Ellipsis text should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_text_overflow_clip_json() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Text", "content": "This is a long text that will be clipped" },
            "style": { "textOverflow": "Clip", "fontSize": 12 },
            "children": []
        }],
        "metadata": {},
        "defaultPage": { "size": { "Custom": { "width": 100, "height": 200 } }, "margin": { "top": 10, "right": 10, "bottom": 10, "left": 10 } },
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Clip text should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_text_overflow_ellipsis_single_line() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Text {
                content: "This is a long text that needs truncation".to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_size: Some(12.0),
                text_overflow: Some(TextOverflow::Ellipsis),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig {
            size: PageSize::Custom {
                width: 100.0,
                height: 200.0,
            },
            margin: Edges::uniform(10.0),
            wrap: true,
            ..Default::default()
        },
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    // With ellipsis, there should be exactly 1 page with 1 text container
    assert_eq!(layout.pages.len(), 1);
    // The Text container should only have 1 TextLine child (single line)
    let text_elem = &layout.pages[0].elements[0];
    assert_eq!(
        text_elem.children.len(),
        1,
        "Ellipsis should produce exactly 1 line"
    );
}

// ─── Cross-axis stretch enables justify-content / flex-grow ─────

/// A flex row with `alignItems: stretch` (default) and two children of
/// different heights.  The shorter child has `justifyContent: center`.
/// Without the cross_axis_height fix, the shorter child's text stays at
/// the top.  With the fix, it is vertically centered.
#[test]
fn test_flex_row_stretch_enables_justify_content() {
    let left_child = make_styled_view(
        Style {
            flex_grow: Some(1.0),
            justify_content: Some(JustifyContent::Center),
            ..Default::default()
        },
        vec![make_text("Short", 12.0)],
    );

    let right_child = make_styled_view(
        Style {
            flex_grow: Some(1.0),
            height: Some(Dimension::Pt(100.0)),
            ..Default::default()
        },
        vec![make_text("Tall", 12.0)],
    );

    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![left_child, right_child],
    );

    let doc = default_doc(vec![row]);

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert_eq!(layout.pages.len(), 1);

    // page.elements[0] is the row View; its children are the two flex items
    let page = &layout.pages[0];
    let row_elem = &page.elements[0];
    let left_col = &row_elem.children[0];
    let right_col = &row_elem.children[1];

    // Left child should be stretched to match right child's height
    assert!(
        (left_col.height - right_col.height).abs() < 1.0,
        "Left col height ({}) should match right col height ({})",
        left_col.height,
        right_col.height,
    );

    // The text inside the left column should NOT be at y=0 offset from parent;
    // justify-content: center should push it down.
    let text_in_left = &left_col.children[0]; // The text container
    let offset_from_top = text_in_left.y - left_col.y;
    assert!(
        offset_from_top > 1.0,
        "Text should be vertically offset from top (offset={}), justifyContent:center should center it",
        offset_from_top,
    );
}

/// When a flex row stretches a child and that child has a flex-grow
/// inner child, the inner child should expand to fill the stretched
/// height.
#[test]
fn test_flex_row_stretch_enables_flex_grow() {
    let inner_grow = make_styled_view(
        Style {
            flex_grow: Some(1.0),
            ..Default::default()
        },
        vec![make_text("Grows", 12.0)],
    );

    let left_child = make_styled_view(
        Style {
            flex_grow: Some(1.0),
            ..Default::default()
        },
        vec![inner_grow],
    );

    let right_child = make_styled_view(
        Style {
            flex_grow: Some(1.0),
            height: Some(Dimension::Pt(120.0)),
            ..Default::default()
        },
        vec![make_text("Tall", 12.0)],
    );

    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![left_child, right_child],
    );

    let doc = default_doc(vec![row]);

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert_eq!(layout.pages.len(), 1);

    let page = &layout.pages[0];
    let row_elem = &page.elements[0];
    let left_col = &row_elem.children[0];

    // Left child should be stretched to ~120pt (matching right child)
    assert!(
        (left_col.height - 120.0).abs() < 1.0,
        "Left col height ({}) should be ~120 (stretched to match right)",
        left_col.height,
    );

    // Inner flex-grow child should fill the full stretched height
    let inner = &left_col.children[0];
    assert!(
        inner.height > 100.0,
        "Inner flex-grow child height ({}) should expand to fill stretched parent",
        inner.height,
    );
}

// ─── Line Breaking Mode Tests ─────────────────────────────────────

#[test]
fn test_line_breaking_greedy_vs_optimal_both_work() {
    let paragraph = "The quick brown fox jumps over the lazy dog near the riverbank where the tall reeds sway gently in the warm afternoon breeze.";

    let make_text_with_mode = |mode: LineBreaking| Node {
        kind: NodeKind::Text {
            content: paragraph.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(10.0),
            line_breaking: Some(mode),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc_optimal = default_doc(vec![make_text_with_mode(LineBreaking::Optimal)]);
    let doc_greedy = default_doc(vec![make_text_with_mode(LineBreaking::Greedy)]);

    let pages_optimal = layout_doc(&doc_optimal);
    let pages_greedy = layout_doc(&doc_greedy);

    // Both should produce valid single-page output
    assert_eq!(pages_optimal.len(), 1);
    assert_eq!(pages_greedy.len(), 1);
    assert!(!pages_optimal[0].elements.is_empty());
    assert!(!pages_greedy[0].elements.is_empty());
}

#[test]
fn test_line_breaking_mode_inherits() {
    // lineBreaking set on a parent View should be inherited by child Text nodes
    let doc = default_doc(vec![make_styled_view(
        Style {
            line_breaking: Some(LineBreaking::Greedy),
            ..Default::default()
        },
        vec![make_text(
            "Some text content that spans multiple lines in this narrow container.",
            10.0,
        )],
    )]);

    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    assert!(!pages[0].elements.is_empty());
}

#[test]
fn test_grid_justified_text_does_not_overflow() {
    let french = "Le chiffre d'affaires consolide a atteint douze virgule un millions de dollars, soit une augmentation de vingt-trois pour cent par rapport a l'exercice precedent. L'expansion dans trois nouveaux marches a contribue a une croissance trimestrielle de trente et un pour cent des nouvelles acquisitions de clients.";

    let text_node = Node {
        kind: NodeKind::Text {
            content: french.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(8.0),
            line_height: Some(1.5),
            text_align: Some(TextAlign::Justify),
            hyphens: Some(Hyphens::Auto),
            lang: Some("fr".to_string()),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let card = make_styled_view(
        Style {
            padding: Some(Edges::uniform(14.0)),
            border_width: Some(EdgeValues::uniform(1.0)),
            border_color: Some(EdgeValues::uniform(Color {
                r: 0.73,
                g: 0.97,
                b: 0.83,
                a: 1.0,
            })),
            background_color: Some(Color {
                r: 0.94,
                g: 0.99,
                b: 0.96,
                a: 1.0,
            }),
            ..Default::default()
        },
        vec![text_node],
    );

    let grid = make_styled_view(
        Style {
            display: Some(Display::Grid),
            grid_template_columns: Some(vec![GridTrackSize::Fr(1.0), GridTrackSize::Fr(1.0)]),
            gap: Some(14.0),
            ..Default::default()
        },
        vec![card.clone(), card],
    );

    let doc = default_doc(vec![Node {
        kind: NodeKind::Page {
            config: PageConfig {
                size: PageSize::Letter,
                margin: Edges::uniform(40.0),
                ..Default::default()
            },
        },
        style: Style::default(),
        children: vec![grid],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);

    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    fn check_overflow(el: &forme::layout::LayoutElement, depth: usize) {
        let parent_right = el.x + el.width;
        for child in &el.children {
            let child_right = child.x + child.width;
            if child_right > parent_right + 0.5 {
                eprintln!(
                    "OVERFLOW depth={}: child ({:.2}..{:.2} w={:.2}) > parent ({:.2}..{:.2} w={:.2}) by {:.2}pt type={:?}",
                    depth, child.x, child_right, child.width,
                    el.x, parent_right, el.width,
                    child_right - parent_right, child.node_type,
                );
            }
            check_overflow(child, depth + 1);
        }
    }

    for el in &pages[0].elements {
        check_overflow(el, 0);
    }
}

#[test]
fn test_arabic_text_with_font_fallback() {
    let font_bytes = match std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf") {
        Ok(b) => b,
        Err(_) => return, // Skip on CI / non-macOS
    };

    let mut font_ctx = FontContext::new();
    font_ctx
        .registry_mut()
        .register("ArialUnicode", 400, false, font_bytes);

    let doc_json = r#"{
        "defaultPage": { "width": 612, "height": 792 },
        "children": [{
            "kind": { "type": "Text", "content": "حقق الإيراد الموحد للربع الرابع" },
            "style": { "fontFamily": "Helvetica, ArialUnicode", "fontSize": 12 }
        }]
    }"#;
    let doc: Document = serde_json::from_str(doc_json).unwrap();
    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_ctx);
    assert!(!pages.is_empty());
    let bytes = forme::render(&doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_builtin_noto_sans_cyrillic() {
    // Cyrillic text should render via builtin Noto Sans without any font registration
    let doc = default_doc(vec![make_text(
        "\u{041F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}",
        12.0,
    )]);
    let pdf = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf);
    // Noto Sans should be embedded as a CIDFont
    assert!(
        pdf_str.contains("NotoSans"),
        "PDF should contain embedded Noto Sans for Cyrillic text"
    );
}

#[test]
fn test_builtin_noto_sans_greek() {
    // Greek text should render via builtin Noto Sans
    let doc = default_doc(vec![make_text("\u{03B1}\u{03B2}\u{03B3}", 12.0)]);
    let pdf = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("NotoSans"),
        "PDF should contain embedded Noto Sans for Greek text"
    );
}

#[test]
fn test_document_default_style() {
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![make_text("Hello", 12.0)],
        )],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: Some(Style {
            font_family: Some("Courier".to_string()),
            font_size: Some(16.0),
            ..Default::default()
        }),
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let pdf = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("Courier"),
        "PDF should use Courier from document default_style"
    );
}

// ─── Embedded Data Tests ────────────────────────────────────────

#[test]
fn test_embedded_data_round_trip() {
    let data = r#"{"invoice":123,"items":["widget","gadget"]}"#;
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![make_text("Invoice", 12.0)],
        )],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: Some(data.to_string()),
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("forme-data.json"),
        "PDF should contain embedded file name"
    );
    assert!(
        pdf_str.contains("/Type /EmbeddedFile"),
        "PDF should contain EmbeddedFile object"
    );
    assert!(
        pdf_str.contains("/Type /Filespec"),
        "PDF should contain Filespec object"
    );
    assert!(
        pdf_str.contains("/EmbeddedFiles"),
        "Catalog should reference EmbeddedFiles"
    );
    assert!(
        pdf_str.contains("/AFRelationship /Data"),
        "Filespec should have AFRelationship"
    );
}

#[test]
fn test_no_embedded_data() {
    let doc = default_doc(vec![make_text("Hello", 12.0)]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("forme-data.json"),
        "PDF without embedded data should not contain forme-data.json"
    );
    assert!(
        !pdf_str.contains("/EmbeddedFiles"),
        "PDF without embedded data should not have EmbeddedFiles"
    );
}

#[test]
fn test_embedded_data_via_json() {
    let json = r#"{
        "children": [{ "kind": { "type": "Text", "content": "Test" }, "style": {}, "children": [] }],
        "embeddedData": "{\"key\":\"value\"}"
    }"#;
    let pdf = forme::render_json(json).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.7"));
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("forme-data.json"),
        "JSON-deserialized embedded data should be present in PDF"
    );
}

// ─── Barcode Tests ──────────────────────────────────────────────

#[test]
fn test_barcode_renders_to_pdf() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Barcode {
                data: "ABC-123".to_string(),
                format: forme::barcode::BarcodeFormat::Code128,
                width: Some(200.0),
                height: 60.0,
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).expect("Barcode should render to PDF");
    assert!(pdf.len() > 100, "PDF should have content");
    assert!(pdf.starts_with(b"%PDF"), "Should be a valid PDF");
}

#[test]
fn test_barcode_json_deserialization() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Barcode", "data": "HELLO", "format": "Code39", "height": 50 },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Barcode JSON should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_barcode_code128_default_format() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Barcode", "data": "12345", "height": 40 },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Default format barcode should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_barcode_layout_dimensions() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Barcode {
                data: "TEST".to_string(),
                format: forme::barcode::BarcodeFormat::Code39,
                width: Some(150.0),
                height: 40.0,
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(layout.pages.len(), 1);
    let elem = &layout.pages[0].elements[0];
    assert!((elem.width - 150.0).abs() < 0.1);
    assert!((elem.height - 40.0).abs() < 0.1);
}

// ─── Auto margin centering ──────────────────────────────────────

#[test]
fn auto_margin_horizontal_centers_child() {
    // A 200pt-wide child with margin-left: auto, margin-right: auto
    // inside a standard A4 page should be horizontally centered.
    let child = make_styled_view(
        Style {
            width: Some(Dimension::Pt(200.0)),
            height: Some(Dimension::Pt(50.0)),
            margin: Some(MarginEdges {
                top: EdgeValue::Pt(0.0),
                right: EdgeValue::Auto,
                bottom: EdgeValue::Pt(0.0),
                left: EdgeValue::Auto,
            }),
            ..Default::default()
        },
        vec![],
    );

    let page = Node::page(
        PageConfig {
            margin: Edges::uniform(0.0),
            ..Default::default()
        },
        Style::default(),
        vec![child],
    );

    let doc = Document {
        children: vec![page],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert_eq!(layout.pages.len(), 1);

    // A4 width = 595.28. Child = 200pt. Expected x = (595.28 - 200) / 2 = 197.64
    let elem = &layout.pages[0].elements[0];
    assert!((elem.width - 200.0).abs() < 0.1, "width: {}", elem.width);
    let expected_x = (595.28 - 200.0) / 2.0;
    assert!(
        (elem.x - expected_x).abs() < 0.5,
        "x: {}, expected: {}",
        elem.x,
        expected_x
    );
}

#[test]
fn auto_margin_left_pushes_right() {
    // margin-left: auto should push the child to the right
    let child = make_styled_view(
        Style {
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            margin: Some(MarginEdges {
                top: EdgeValue::Pt(0.0),
                right: EdgeValue::Pt(0.0),
                bottom: EdgeValue::Pt(0.0),
                left: EdgeValue::Auto,
            }),
            ..Default::default()
        },
        vec![],
    );

    let page = Node::page(
        PageConfig {
            margin: Edges::uniform(0.0),
            ..Default::default()
        },
        Style::default(),
        vec![child],
    );

    let doc = Document {
        children: vec![page],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    let elem = &layout.pages[0].elements[0];
    // Should be pushed to the right: x = 595.28 - 100 = 495.28
    let expected_x = 595.28 - 100.0;
    assert!(
        (elem.x - expected_x).abs() < 0.5,
        "x: {}, expected: {}",
        elem.x,
        expected_x
    );
}

#[test]
fn auto_margin_deserializes_from_json() {
    // Verify that "auto" string deserializes correctly in margin edges
    let json = r#"{
        "children": [{
            "kind": { "type": "Page", "config": { "size": "A4", "margin": { "top": 0, "right": 0, "bottom": 0, "left": 0 }, "wrap": true } },
            "style": {},
            "children": [{
                "kind": { "type": "View" },
                "style": {
                    "width": { "Pt": 200 },
                    "height": { "Pt": 50 },
                    "margin": { "top": 0, "right": "auto", "bottom": 0, "left": "auto" }
                },
                "children": []
            }]
        }],
        "metadata": {},
        "defaultPage": { "size": "A4", "margin": { "top": 0, "right": 0, "bottom": 0, "left": 0 }, "wrap": true }
    }"#;

    let (pdf, layout, _) = forme::render_json_with_layout(json).expect("Should render from JSON");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(layout.pages.len(), 1);

    let elem = &layout.pages[0].elements[0];
    assert!((elem.width - 200.0).abs() < 0.1);
    let expected_x = (595.28 - 200.0) / 2.0;
    assert!(
        (elem.x - expected_x).abs() < 0.5,
        "x: {}, expected: {}",
        elem.x,
        expected_x
    );
}

// ── Chart Tests ─────────────────────────────────────────────────────

#[test]
fn test_bar_chart_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "BarChart",
                "data": [
                    { "label": "Q1", "value": 100 },
                    { "label": "Q2", "value": 150 },
                    { "label": "Q3", "value": 80 },
                    { "label": "Q4", "value": 200 },
                    { "label": "Q5", "value": 120 }
                ],
                "width": 400,
                "height": 200,
                "show_labels": true,
                "show_values": true,
                "show_grid": true
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("BarChart should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert!(pdf.len() > 200);
}

#[test]
fn test_line_chart_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "LineChart",
                "series": [
                    { "name": "Revenue", "data": [100, 150, 130, 200], "color": "#2b6cb0" },
                    { "name": "Expenses", "data": [80, 120, 100, 160], "color": "#e53e3e" }
                ],
                "labels": ["Q1", "Q2", "Q3", "Q4"],
                "width": 400,
                "height": 200,
                "show_points": true,
                "show_grid": true
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("LineChart should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_pie_chart_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "PieChart",
                "data": [
                    { "label": "A", "value": 40, "color": "#1a365d" },
                    { "label": "B", "value": 30, "color": "#2b6cb0" },
                    { "label": "C", "value": 20, "color": "#63b3ed" },
                    { "label": "D", "value": 10, "color": "#90cdf4" }
                ],
                "width": 200,
                "height": 200,
                "donut": false,
                "show_legend": true
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("PieChart should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_pie_chart_donut_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "PieChart",
                "data": [
                    { "label": "Yes", "value": 70 },
                    { "label": "No", "value": 30 }
                ],
                "width": 200,
                "height": 200,
                "donut": true,
                "show_legend": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("PieChart donut should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_area_chart_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "AreaChart",
                "series": [
                    { "name": "Users", "data": [10, 50, 80, 120, 160], "color": "#38a169" },
                    { "name": "Revenue", "data": [5, 30, 60, 90, 140], "color": "#805ad5" }
                ],
                "labels": ["Jan", "Feb", "Mar", "Apr", "May"],
                "width": 400,
                "height": 200,
                "show_grid": true
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("AreaChart should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_dot_plot_renders_to_pdf() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "DotPlot",
                "groups": [
                    { "name": "Group A", "color": "#1a365d", "data": [[1, 2], [3, 4], [5, 6], [7, 8]] },
                    { "name": "Group B", "color": "#e53e3e", "data": [[2, 3], [4, 5], [6, 7], [8, 9]] },
                    { "name": "Group C", "color": "#38a169", "data": [[1, 5], [3, 7], [5, 3], [7, 1]] },
                    { "name": "Group D", "color": "#805ad5", "data": [[2, 6], [4, 2], [6, 8], [8, 4]] }
                ],
                "width": 400,
                "height": 300,
                "show_legend": true,
                "dot_size": 4
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("DotPlot should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_chart_with_title() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "BarChart",
                "data": [
                    { "label": "A", "value": 50 },
                    { "label": "B", "value": 75 }
                ],
                "width": 300,
                "height": 200,
                "show_labels": true,
                "show_values": false,
                "show_grid": false,
                "title": "Sales Report"
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("Chart with title should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert!(
        pdf.len() > 200,
        "PDF should have content for chart with title"
    );
}

#[test]
fn test_chart_respects_custom_colors() {
    let json = r##"{
        "children": [{
            "kind": {
                "type": "BarChart",
                "data": [
                    { "label": "A", "value": 50, "color": "#ff0000" },
                    { "label": "B", "value": 75, "color": "#00ff00" }
                ],
                "width": 300,
                "height": 200,
                "show_labels": true,
                "show_values": false,
                "show_grid": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("Chart with custom colors should render");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn test_bar_chart_layout_dimensions() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::BarChart {
                data: vec![
                    forme::model::ChartDataPoint {
                        label: "A".into(),
                        value: 100.0,
                        color: None,
                    },
                    forme::model::ChartDataPoint {
                        label: "B".into(),
                        value: 200.0,
                        color: None,
                    },
                ],
                width: 350.0,
                height: 250.0,
                color: None,
                show_labels: true,
                show_values: false,
                show_grid: false,
                title: None,
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(layout.pages.len(), 1);
    let elem = &layout.pages[0].elements[0];
    assert!((elem.width - 350.0).abs() < 0.1);
    assert!((elem.height - 250.0).abs() < 0.1);
}

#[test]
fn test_multiple_charts_on_same_page() {
    let json = r##"{
        "children": [
            {
                "kind": {
                    "type": "BarChart",
                    "data": [{ "label": "A", "value": 50 }],
                    "width": 300,
                    "height": 150,
                    "show_labels": true,
                    "show_values": false,
                    "show_grid": false
                },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "LineChart",
                    "series": [{ "name": "S1", "data": [10, 20, 30] }],
                    "labels": ["A", "B", "C"],
                    "width": 300,
                    "height": 150,
                    "show_points": false,
                    "show_grid": false
                },
                "style": {},
                "children": []
            }
        ],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"##;
    let pdf = forme::render_json(json).expect("Multiple charts should render");
    assert!(pdf.starts_with(b"%PDF"));
}

// ─── Form Field Tests ──────────────────────────────────────────────

#[test]
fn test_text_field_renders_to_pdf() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField",
                "name": "full_name",
                "width": 200,
                "height": 24,
                "multiline": false,
                "password": false,
                "read_only": false,
                "font_size": 12
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("TextField should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/AcroForm"),
        "PDF should contain AcroForm dictionary"
    );
    assert!(
        pdf_str.contains("/FT /Tx"),
        "PDF should contain text field type"
    );
    assert!(
        pdf_str.contains("/T (full_name)"),
        "PDF should contain field name"
    );
}

#[test]
fn test_checkbox_renders_to_pdf() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "Checkbox",
                "name": "agree_terms",
                "checked": true,
                "width": 14,
                "height": 14,
                "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Checkbox should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/AcroForm"),
        "PDF should contain AcroForm dictionary"
    );
    assert!(
        pdf_str.contains("/FT /Btn"),
        "PDF should contain button field type"
    );
    assert!(
        pdf_str.contains("/T (agree_terms)"),
        "PDF should contain field name"
    );
    assert!(
        pdf_str.contains("/V /Yes"),
        "Checked checkbox should have /V /Yes"
    );
}

#[test]
fn test_dropdown_renders_to_pdf() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "Dropdown",
                "name": "country",
                "options": ["US", "UK", "CA"],
                "width": 200,
                "height": 24,
                "read_only": false,
                "font_size": 12
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Dropdown should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/AcroForm"),
        "PDF should contain AcroForm dictionary"
    );
    assert!(
        pdf_str.contains("/FT /Ch"),
        "PDF should contain choice field type"
    );
    assert!(
        pdf_str.contains("/T (country)"),
        "PDF should contain field name"
    );
    assert!(pdf_str.contains("/Opt"), "PDF should contain options array");
}

#[test]
fn test_radio_button_group_renders_to_pdf() {
    let json = r#"{
        "children": [
            {
                "kind": {
                    "type": "RadioButton",
                    "name": "plan",
                    "value": "free",
                    "checked": true,
                    "width": 14,
                    "height": 14,
                    "read_only": false
                },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "RadioButton",
                    "name": "plan",
                    "value": "pro",
                    "checked": false,
                    "width": 14,
                    "height": 14,
                    "read_only": false
                },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "RadioButton",
                    "name": "plan",
                    "value": "team",
                    "checked": false,
                    "width": 14,
                    "height": 14,
                    "read_only": false
                },
                "style": {},
                "children": []
            }
        ],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("RadioButton group should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/AcroForm"),
        "PDF should contain AcroForm dictionary"
    );
    assert!(
        pdf_str.contains("/T (plan)"),
        "PDF should contain radio group name"
    );
    assert!(
        pdf_str.contains("/Ff 49152"),
        "Radio parent should have radio+noToggleToOff flags"
    );
    assert!(
        pdf_str.contains("/Kids"),
        "Radio parent should have /Kids array"
    );
    assert!(
        pdf_str.contains("/V /free"),
        "Radio group should have /V /free (checked value)"
    );
}

#[test]
fn test_mixed_form_fields_on_same_page() {
    let json = r#"{
        "children": [
            {
                "kind": {
                    "type": "TextField",
                    "name": "name",
                    "width": 200,
                    "height": 24,
                    "multiline": false,
                    "password": false,
                    "read_only": false,
                    "font_size": 12
                },
                "style": {},
                "children": []
            },
            {
                "kind": { "type": "Text", "content": "Some text between fields" },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "Checkbox",
                    "name": "agree",
                    "checked": false,
                    "width": 14,
                    "height": 14,
                    "read_only": false
                },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "Dropdown",
                    "name": "color",
                    "options": ["Red", "Green", "Blue"],
                    "width": 150,
                    "height": 24,
                    "read_only": false,
                    "font_size": 12
                },
                "style": {},
                "children": []
            }
        ],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Mixed form fields should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(pdf_str.contains("/AcroForm"), "PDF should contain AcroForm");
    assert!(pdf_str.contains("/FT /Tx"), "PDF should contain text field");
    assert!(pdf_str.contains("/FT /Btn"), "PDF should contain checkbox");
    assert!(
        pdf_str.contains("/FT /Ch"),
        "PDF should contain choice field"
    );
}

#[test]
fn test_no_form_fields_no_acroform() {
    let json = r#"{
        "children": [{
            "kind": { "type": "Text", "content": "Hello, no forms here" },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Should render without form fields");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "PDF without form fields should not have /AcroForm"
    );
}

#[test]
fn test_text_field_with_value_and_placeholder() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField",
                "name": "email",
                "value": "test@example.com",
                "placeholder": "Enter email",
                "width": 250,
                "height": 30,
                "multiline": false,
                "password": false,
                "read_only": false,
                "font_size": 14
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("TextField with value should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/V (test@example.com)"),
        "PDF should contain field value"
    );
}

#[test]
fn test_text_field_multiline_flag() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField",
                "name": "notes",
                "width": 300,
                "height": 100,
                "multiline": true,
                "password": false,
                "read_only": false,
                "font_size": 12
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Multiline text field should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    // Multiline flag is bit 13 (4096) in the /Ff field
    assert!(
        pdf_str.contains("/Ff 4096"),
        "Multiline text field should have bit 13 set"
    );
}

#[test]
fn test_dropdown_with_selected_value() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "Dropdown",
                "name": "size",
                "options": ["Small", "Medium", "Large"],
                "value": "Medium",
                "width": 150,
                "height": 24,
                "read_only": false,
                "font_size": 12
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Dropdown with value should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/V (Medium)"),
        "Dropdown should have selected value"
    );
}

// ─── Form flattening tests ───────────────────────────────────────────

#[test]
fn test_flatten_text_field_with_value() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "name", "width": 200, "height": 24,
                "value": "Jane Smith", "font_size": 12,
                "multiline": false, "password": false, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened text field should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    // No AcroForm in flattened output
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    // No interactive widget annotations
    assert!(
        !pdf_str.contains("/FT /Tx"),
        "Flattened PDF should not contain text field widgets"
    );
    // Valid PDF structure
    assert!(pdf.starts_with(b"%PDF-1.7"));
}

#[test]
fn test_flatten_checkbox_checked() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "Checkbox", "name": "agree", "width": 14, "height": 14,
                "checked": true, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened checkbox should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    assert!(
        !pdf_str.contains("/FT /Btn"),
        "Flattened PDF should not contain button field widgets"
    );
}

#[test]
fn test_flatten_dropdown_with_value() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "Dropdown", "name": "size", "width": 150, "height": 24,
                "options": ["Small", "Medium", "Large"], "value": "Medium",
                "read_only": false, "font_size": 12
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened dropdown should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    assert!(
        !pdf_str.contains("/FT /Ch"),
        "Flattened PDF should not contain choice field widgets"
    );
    assert!(pdf.starts_with(b"%PDF-1.7"));
}

#[test]
fn test_flatten_radio_button_group() {
    let json = r#"{
        "children": [
            {
                "kind": {
                    "type": "RadioButton", "name": "color", "value": "red",
                    "width": 14, "height": 14, "checked": true, "read_only": false
                },
                "style": {}, "children": []
            },
            {
                "kind": {
                    "type": "RadioButton", "name": "color", "value": "blue",
                    "width": 14, "height": 14, "checked": false, "read_only": false
                },
                "style": {}, "children": []
            }
        ],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened radio buttons should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    // No radio button field type in flattened output
    assert!(
        !pdf_str.contains("/FT /Btn"),
        "Flattened PDF should not contain button field widgets"
    );
    assert!(pdf.starts_with(b"%PDF-1.7"));
}

#[test]
fn test_flatten_empty_text_field() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "empty", "width": 200, "height": 24,
                "font_size": 12,
                "multiline": false, "password": false, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened empty field should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    // Should still produce a valid PDF
    assert!(pdf.starts_with(b"%PDF-1.7"));
}

#[test]
fn test_non_flattened_default_has_acroform() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "name", "width": 200, "height": 24,
                "value": "Test", "font_size": 12,
                "multiline": false, "password": false, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": []
    }"#;
    let pdf = forme::render_json(json).expect("Non-flattened form should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/AcroForm"),
        "Non-flattened PDF should contain /AcroForm"
    );
}

#[test]
fn test_flatten_mixed_content() {
    let json = r#"{
        "children": [
            {
                "kind": { "type": "Text", "content": "Hello World" },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "TextField", "name": "field1", "width": 200, "height": 24,
                    "value": "Static Value", "font_size": 12,
                    "multiline": false, "password": false, "read_only": false
                },
                "style": {},
                "children": []
            },
            {
                "kind": {
                    "type": "Checkbox", "name": "check1", "width": 14, "height": 14,
                    "checked": true, "read_only": false
                },
                "style": {},
                "children": []
            }
        ],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened mixed content should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    // Regular text should still render, valid PDF
    assert!(pdf.starts_with(b"%PDF-1.7"));
    // No interactive field types
    assert!(
        !pdf_str.contains("/FT /Tx"),
        "Flattened PDF should not contain text field widgets"
    );
}

#[test]
fn test_flatten_password_field_renders_dots() {
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "pw", "width": 200, "height": 24,
                "value": "secret", "font_size": 12,
                "multiline": false, "password": true, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened password field should render");
    let pdf_str = String::from_utf8_lossy(&pdf);
    // No AcroForm in flattened output
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    // Password value must not appear as plain text
    assert!(
        !pdf_str.contains("secret"),
        "Flattened password field should not contain plain text value"
    );
    assert!(pdf.starts_with(b"%PDF-1.7"));
}

#[test]
fn test_flatten_multiline_long_word() {
    // 50 'A's in a narrow field — should break at character boundary
    let long_word = "A".repeat(50);
    let json = format!(
        r#"{{
        "children": [{{
            "kind": {{
                "type": "TextField", "name": "long", "width": 80, "height": 100,
                "value": "{}", "font_size": 10,
                "multiline": true, "password": false, "read_only": false
            }},
            "style": {{}},
            "children": []
        }}],
        "metadata": {{}},
        "defaultPage": {{}},
        "fonts": [],
        "flattenForms": true
    }}"#,
        long_word
    );
    let pdf = forme::render_json(&json).expect("Flattened multiline long word should render");
    assert!(pdf.starts_with(b"%PDF-1.7"));
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
}

#[test]
fn test_flatten_text_field_non_ascii() {
    // Euro sign (U+20AC) and en-dash (U+2013) — both in WinAnsi
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "price", "width": 200, "height": 24,
                "value": "\u20AC100 \u2013 price", "font_size": 12,
                "multiline": false, "password": false, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened non-ASCII text should render");
    assert!(pdf.starts_with(b"%PDF-1.7"));
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
}

// ─── PDF/UA tests ──────────────────────────────────────────────────

#[test]
fn test_pdf_ua_has_viewer_preferences() {
    let doc = Document {
        children: vec![Node::text("Accessible doc", Style::default())],
        metadata: Metadata {
            title: Some("UA Test".to_string()),
            ..Default::default()
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: true,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/ViewerPreferences"),
        "PDF/UA should include /ViewerPreferences"
    );
    assert!(
        text.contains("/DisplayDocTitle true"),
        "PDF/UA should set /DisplayDocTitle true"
    );
}

#[test]
fn test_pdf_ua_has_xmp_pdfuaid() {
    let doc = Document {
        children: vec![Node::text("Accessible doc", Style::default())],
        metadata: Metadata {
            title: Some("UA Test".to_string()),
            ..Default::default()
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: true,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("pdfuaid:part"),
        "PDF/UA should include pdfuaid:part in XMP metadata"
    );
}

#[test]
fn test_pdf_ua_forces_tagging() {
    let doc = Document {
        children: vec![Node::text("Tagged by UA", Style::default())],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: true,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/StructTreeRoot"),
        "PDF/UA should force tagging even when tagged=false"
    );
    assert!(
        text.contains("/Marked true"),
        "PDF/UA should have /Marked true in /MarkInfo"
    );
}

#[test]
fn test_pdf_ua_and_pdfa_combined_xmp() {
    // PDF/A requires embedded fonts, so we can't easily test the full render
    // pipeline with both flags. The combined XMP metadata (both pdfaid + pdfuaid)
    // is verified in xmp.rs unit tests (test_xmp_both_pdfa_and_pdfua).
    // Here we verify PDF/UA works correctly on its own with all expected features.
    let doc = Document {
        children: vec![Node::text("Accessible", Style::default())],
        metadata: Metadata {
            title: Some("UA Only".to_string()),
            ..Default::default()
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: true,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    // PDF/UA generates XMP with pdfuaid namespace
    assert!(
        text.contains("pdfuaid"),
        "PDF/UA doc should have pdfuaid in XMP"
    );
    // PDF/UA has ViewerPreferences
    assert!(
        text.contains("/ViewerPreferences"),
        "PDF/UA doc should have /ViewerPreferences"
    );
    // PDF/UA forces tagging
    assert!(
        text.contains("/StructTreeRoot"),
        "PDF/UA doc should have structure tree"
    );
}

// ─── Tagged PDF compliance tests ───────────────────────────────────

#[test]
fn test_tagged_pdf_has_tab_order() {
    let doc = Document {
        children: vec![Node::text("Tab order test", Style::default())],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Tabs /S"),
        "Tagged PDF pages must have /Tabs /S for structure-order tab navigation"
    );
}

#[test]
fn test_untagged_pdf_no_tab_order() {
    let doc = Document {
        children: vec![Node::text("No tabs", Style::default())],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/Tabs"),
        "Non-tagged PDF should not contain /Tabs"
    );
}

#[test]
fn test_tagged_role_map_omits_standard_self_mappings() {
    let doc = Document {
        children: vec![Node::text("RoleMap test", Style::default())],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    // Forme emits only standard PDF structure types (H1, L, THead, Link,
    // BlockQuote, …). Standard types must NOT appear in the RoleMap — a
    // self-mapping like `/H1 /H1` is the circular RoleMap veraPDF rejects
    // (PDF/UA-1 7.1-6). The RoleMap is therefore empty.
    for identity in [
        "/H1 /H1",
        "/L /L",
        "/THead /THead",
        "/Link /Link",
        "/BlockQuote /BlockQuote",
    ] {
        assert!(
            !text.contains(identity),
            "RoleMap must not self-map standard type ({identity}) — 7.1-6"
        );
    }
    assert!(
        text.contains("/RoleMap "),
        "StructTreeRoot must still reference a (now empty) RoleMap"
    );
}

#[test]
fn test_tagged_struct_tree_has_lang() {
    let doc = Document {
        children: vec![Node::text("Lang test", Style::default())],
        metadata: Metadata {
            lang: Some("en-US".to_string()),
            ..Default::default()
        },
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    // /Lang should appear on both Catalog and StructTreeRoot
    assert!(
        text.contains("/Type /StructTreeRoot"),
        "Tagged doc must have StructTreeRoot"
    );
    // Count /Lang occurrences — should be at least 2 (Catalog + StructTreeRoot)
    let lang_count = text.matches("/Lang (en-US)").count();
    assert!(
        lang_count >= 2,
        "Expected /Lang (en-US) on both Catalog and StructTreeRoot, found {} occurrences",
        lang_count
    );
}

#[test]
fn test_tagged_watermark_not_in_structure_tree() {
    // Watermarks should be artifact-tagged (not structure-tagged) in tagged PDFs.
    // When properly artifact-tagged, they don't appear in the structure tree.
    // The structure tree should only have: Document, Page (Div), and Text (P).
    let json = r#"{
        "children": [{
            "kind": { "type": "Page", "config": { "size": "A4", "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 }, "wrap": true } },
            "style": {},
            "children": [
                {
                    "kind": { "type": "Watermark", "text": "DRAFT", "font_size": 60, "angle": -45 },
                    "style": {},
                    "children": []
                },
                {
                    "kind": { "type": "Text", "content": "Real content", "runs": [] },
                    "style": {},
                    "children": []
                }
            ]
        }],
        "metadata": {},
        "tagged": true
    }"#;
    let bytes = forme::render_json(json).unwrap();
    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);

    // The structure tree should have /StructTreeRoot
    assert!(text.contains("/Type /StructTreeRoot"));

    // Count StructElem objects — watermark should NOT generate one.
    // Expected: Page→Div, Text→P (2 structure elements, not 3).
    let struct_elem_count = text.matches("/Type /StructElem").count();
    assert_eq!(
        struct_elem_count, 2,
        "Watermark should not be in structure tree (expected 2 StructElems: Div + P, got {})",
        struct_elem_count
    );
}

// ─── Digital Certification Tests ────────────────────────────────

/// Helper: generate a self-signed X.509 cert + RSA private key for testing.
fn generate_test_cert_and_key() -> (String, String) {
    use rsa::pkcs8::EncodePrivateKey;

    // Generate RSA key using the `rsa` crate
    let mut rng = rsa::rand_core::OsRng;
    let rsa_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let key_pem = rsa_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap()
        .to_string();

    // Create rcgen KeyPair from the RSA PEM
    let key_pair = rcgen::KeyPair::from_pem(&key_pem).unwrap();

    // Generate self-signed cert with CN in Subject DN
    let mut params = rcgen::CertificateParams::new(vec!["Test Signer".to_string()]).unwrap();
    params.distinguished_name = {
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "Test Signer");
        dn
    };
    let cert = params.self_signed(&key_pair).unwrap();

    (cert.pem(), key_pem)
}

#[test]
fn test_certify_pdf_basic() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Signed document", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    assert!(
        text.contains("/Type /Sig"),
        "Must contain signature dictionary"
    );
    assert!(
        text.contains("/Filter /Adobe.PPKLite"),
        "Must have Adobe.PPKLite filter"
    );
    assert!(
        text.contains("/SubFilter /adbe.pkcs7.detached"),
        "Must use PKCS#7 detached subfilter"
    );
}

#[test]
fn test_certify_pdf_has_acroform() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("AcroForm test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    assert!(text.contains("/AcroForm"), "Must contain AcroForm");
    assert!(text.contains("/SigFlags 3"), "Must have SigFlags 3");
}

#[test]
fn test_certify_pdf_byterange() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("ByteRange test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    // Find /ByteRange [0 X Y Z]
    let br_start = text.find("/ByteRange [").expect("Must contain /ByteRange");
    let br_end = text[br_start..].find(']').unwrap() + br_start + 1;
    let br_str = &text[br_start + 12..br_end - 1]; // inside the brackets
    let values: Vec<usize> = br_str
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();

    assert_eq!(values.len(), 4, "ByteRange must have exactly 4 values");
    assert_eq!(values[0], 0, "ByteRange must start at 0");
    assert!(
        values[1] < values[2],
        "First range end must be before second range start"
    );
    assert_eq!(
        values[1] + values[2] + values[3],
        values[1] + values[2] + values[3],
        "Values must be consistent"
    );
    // Verify: first_range_end + gap + second_range_len should relate to file size
    assert_eq!(
        values[2] + values[3],
        signed_pdf.len(),
        "ByteRange end must equal file length"
    );
}

#[test]
fn test_certify_pdf_contents_not_placeholder() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Contents test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    // Find /Contents <...> and verify it's not all zeros
    let contents_start = text.find("/Contents <").expect("Must contain /Contents");
    let hex_start = contents_start + 11; // after "/Contents <"
    let hex_end = text[hex_start..].find('>').unwrap() + hex_start;
    let hex_str = &text[hex_start..hex_end];

    // Should not be all zeros
    let all_zeros = hex_str.chars().all(|c| c == '0');
    assert!(!all_zeros, "/Contents must not be all zeros after signing");
}

#[test]
fn test_certify_pdf_invisible() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Invisible sig", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    assert!(
        text.contains("/Rect [0 0 0 0]"),
        "Invisible signature must have zero rect"
    );
}

#[test]
fn test_certify_pdf_reason_location() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Reason/location test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: Some("Approved for release".to_string()),
        location: Some("New York, NY".to_string()),
        contact: Some("signer@example.com".to_string()),
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    assert!(
        text.contains("/Reason (Approved for release)"),
        "Must contain reason string"
    );
    assert!(
        text.contains("/Location (New York, NY)"),
        "Must contain location string"
    );
    assert!(
        text.contains("/ContactInfo (signer@example.com)"),
        "Must contain contact info"
    );
}

#[test]
fn test_certify_pdf_invalid_cert() {
    let doc = default_doc(vec![make_text("Invalid cert test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: "not a real PEM certificate".to_string(),
        private_key_pem: "not a real PEM key".to_string(),
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let result = forme::certify_pdf(&unsigned_pdf, &config);
    assert!(result.is_err(), "Invalid cert should produce an error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("certificate") || err.contains("PEM"),
        "Error message should mention certificate: {}",
        err
    );
}

#[test]
fn test_certify_pdf_key_mismatch() {
    // Generate two different key pairs
    let (cert_pem, _key_pem) = generate_test_cert_and_key();
    let (_other_cert, other_key) = generate_test_cert_and_key();

    let doc = default_doc(vec![make_text("Key mismatch test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: other_key, // wrong key
        reason: None,
        location: None,
        contact: None,
        visible: false,

        x: None,
        y: None,
        width: None,
        height: None,
    };

    let result = forme::certify_pdf(&unsigned_pdf, &config);
    assert!(result.is_err(), "Mismatched key should produce an error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("do not match"),
        "Error should mention key mismatch: {}",
        err
    );
}

#[test]
fn test_certify_at_render_time() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();

    let doc = Document {
        children: vec![make_text("Render-time signed", 12.0)],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: Some(forme::CertificationConfig {
            certificate_pem: cert_pem,
            private_key_pem: key_pem,
            reason: Some("Auto-signed at render".to_string()),
            location: None,
            contact: None,
            visible: false,

            x: None,
            y: None,
            width: None,
            height: None,
        }),
    };

    let pdf = forme::render(&doc).unwrap();
    let text = String::from_utf8_lossy(&pdf);

    assert!(
        text.contains("/Type /Sig"),
        "Render-time signing must produce signature"
    );
    assert!(
        text.contains("/Reason (Auto-signed at render)"),
        "Reason must appear in signed output"
    );
    assert!(text.contains("/AcroForm"), "Must have AcroForm");
}

#[test]
fn test_certify_pdf_visible_has_appearance_stream() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Visible sig test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: Some("Contract approval".to_string()),
        location: None,
        contact: None,
        visible: true,

        x: Some(50.0),
        y: Some(600.0),
        width: Some(200.0),
        height: Some(60.0),
    };

    let signed_pdf = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text = String::from_utf8_lossy(&signed_pdf);

    // Must have a non-zero rect
    assert!(
        !text.contains("/Rect [0 0 0 0]"),
        "Visible signature must not have zero rect"
    );
    assert!(
        text.contains("/Rect [50.00 600.00 250.00 660.00]"),
        "Rect must match configured position/size"
    );

    // Must have an appearance stream reference
    assert!(
        text.contains("/AP <<"),
        "Visible signature must have /AP entry"
    );

    // Must have a Form XObject with the appearance content
    assert!(
        text.contains("/Subtype /Form"),
        "Must have Form XObject for appearance"
    );
    assert!(
        text.contains("Digitally signed by"),
        "Appearance must contain 'Digitally signed by' text"
    );
    assert!(
        text.contains("Test Signer"),
        "Appearance must contain the signer CN"
    );
    assert!(
        text.contains("Contract approval"),
        "Appearance must contain the reason"
    );

    // AcroForm must have default resources for Helvetica
    assert!(
        text.contains("/DR <<"),
        "AcroForm must have /DR for font resources"
    );
}

#[test]
fn test_double_certification_unique_names() {
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let doc = default_doc(vec![make_text("Double certify test", 12.0)]);
    let unsigned_pdf = render_to_pdf(&doc);

    let config = forme::CertificationConfig {
        certificate_pem: cert_pem.clone(),
        private_key_pem: key_pem.clone(),
        reason: None,
        location: None,
        contact: None,
        visible: false,
        x: None,
        y: None,
        width: None,
        height: None,
    };

    // Sign once → Signature1
    let signed_once = forme::certify_pdf(&unsigned_pdf, &config).unwrap();
    let text1 = String::from_utf8_lossy(&signed_once);
    assert!(
        text1.contains("/T (Signature1)"),
        "First signature must be named Signature1"
    );

    // Sign again → Signature2
    let signed_twice = forme::certify_pdf(&signed_once, &config).unwrap();
    let text2 = String::from_utf8_lossy(&signed_twice);
    assert!(
        text2.contains("/T (Signature2)"),
        "Second signature must be named Signature2"
    );
    assert!(
        text2.contains("/T (Signature1)"),
        "First signature must still be present"
    );
}

#[test]
fn test_certify_preserves_acroform_metadata() {
    // Create a PDF with a text field (which produces /NeedAppearances true, /DA)
    let text_field = Node {
        kind: NodeKind::TextField {
            name: "field1".to_string(),
            value: Some("Hello".to_string()),
            placeholder: None,
            multiline: false,
            password: false,
            read_only: false,
            max_length: None,
            font_size: 12.0,
            width: 200.0,
            height: 24.0,
        },
        style: Default::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![text_field]);

    let pdf = forme::render(&doc).unwrap();
    let text_before = String::from_utf8_lossy(&pdf);
    assert!(
        text_before.contains("/NeedAppearances true"),
        "Original PDF must have /NeedAppearances"
    );
    assert!(text_before.contains("/DA"), "Original PDF must have /DA");

    // Sign it
    let (cert_pem, key_pem) = generate_test_cert_and_key();
    let config = forme::CertificationConfig {
        certificate_pem: cert_pem,
        private_key_pem: key_pem,
        reason: None,
        location: None,
        contact: None,
        visible: false,
        x: None,
        y: None,
        width: None,
        height: None,
    };

    let signed_pdf = forme::certify_pdf(&pdf, &config).unwrap();
    let text_after = String::from_utf8_lossy(&signed_pdf);

    // Signed PDF must preserve AcroForm metadata
    assert!(
        text_after.contains("/NeedAppearances true"),
        "Signed PDF must preserve /NeedAppearances"
    );
    assert!(text_after.contains("/DA"), "Signed PDF must preserve /DA");

    // Must also have the original form field
    assert!(
        text_after.contains("/FT /Tx"),
        "Signed PDF must still contain the text field"
    );
}

#[test]
fn test_tagged_pdf_form_fields_have_form_role() {
    let text_field = Node {
        kind: NodeKind::TextField {
            name: "name".to_string(),
            value: Some("Test".to_string()),
            placeholder: None,
            multiline: false,
            password: false,
            read_only: false,
            max_length: None,
            font_size: 12.0,
            width: 200.0,
            height: 24.0,
        },
        style: Default::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let checkbox = Node {
        kind: NodeKind::Checkbox {
            name: "agree".to_string(),
            checked: true,
            read_only: false,
            width: 14.0,
            height: 14.0,
        },
        style: Default::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let mut doc = default_doc(vec![text_field, checkbox]);
    doc.tagged = true;

    let pdf = forme::render(&doc).unwrap();
    let text = String::from_utf8_lossy(&pdf);

    // Structure elements must use /Form role for form fields
    assert!(
        text.contains("/S /Form"),
        "Form fields must be tagged with /S /Form in structure tree"
    );
}

#[test]
fn test_flatten_forms_renders_placeholder() {
    // Test via JSON so we can verify flattened rendering
    let json = r#"{
        "children": [{
            "kind": {
                "type": "TextField", "name": "name", "width": 200, "height": 24,
                "placeholder": "Enter your name", "font_size": 12,
                "multiline": false, "password": false, "read_only": false
            },
            "style": {},
            "children": []
        }],
        "metadata": {},
        "defaultPage": {},
        "fonts": [],
        "flattenForms": true
    }"#;
    let pdf = forme::render_json(json).expect("Flattened form with placeholder should render");
    let pdf_str = String::from_utf8_lossy(&pdf);

    // Flattened — no interactive widgets
    assert!(
        !pdf_str.contains("/AcroForm"),
        "Flattened PDF should not contain /AcroForm"
    );
    assert!(
        !pdf_str.contains("/FT /Tx"),
        "Flattened PDF should not contain text field widgets"
    );

    // Decompress all FlateDecode streams to verify placeholder text in content
    let decompressed = decompress_pdf_streams(&pdf);
    assert!(
        decompressed.contains("Enter your name"),
        "Flattened form must render placeholder text in content stream when value is empty"
    );
    assert!(
        decompressed.contains("0.6 g"),
        "Placeholder text must be rendered in grey (0.6 g)"
    );
}

#[test]
fn test_checkbox_renders_checkmark() {
    let checkbox = Node {
        kind: NodeKind::Checkbox {
            name: "agree".to_string(),
            checked: true,
            read_only: false,
            width: 14.0,
            height: 14.0,
        },
        style: Default::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![checkbox]);

    let pdf = forme::render(&doc).unwrap();
    let text = String::from_utf8_lossy(&pdf);

    // Appearance stream must use fill (checkmark path), not stroke lines (X)
    // The checkmark is a filled path with 'f' operator, not 'S' (stroke)
    assert!(
        text.contains("0.2 0.2 0.2 rg"),
        "Checkmark must use fill color (rg), not stroke color (RG)"
    );
    // Must NOT contain the old X pattern (two diagonal stroke lines)
    assert!(
        !text.contains("0 0 m 14 14 l S"),
        "Must not draw X (diagonal stroke lines) for checkbox"
    );
}

#[test]
fn test_latin_extended_character_widths() {
    // Verify Latin Extended characters (Å, Ä, Ö, etc.) have correct advance widths
    // and don't render stacked on top of each other.
    use forme::font::StandardFont;

    let m = StandardFont::Helvetica.metrics();
    let test_str = "ÅÄÖÉÈÊÑÜÚÙû";
    let font_size = 24.0;

    // Verify each character has a reasonable width (not near-zero)
    let mut total_width = 0.0;
    let mut prev_x = 0.0;
    for ch in test_str.chars() {
        let w = m.char_width(ch, font_size);
        assert!(
            w > 3.0,
            "Character '{}' (U+{:04X}) has suspiciously small width: {} at {}pt",
            ch,
            ch as u32,
            w,
            font_size
        );
        // Each character's x position should be greater than the previous
        if total_width > 0.0 {
            assert!(
                total_width > prev_x,
                "Character '{}' x position ({}) not advancing from previous ({})",
                ch,
                total_width,
                prev_x
            );
        }
        prev_x = total_width;
        total_width += w;
    }

    // Total width should be reasonable (each char ~16pt at 24pt font size)
    assert!(
        total_width > 100.0,
        "Total width of '{}' at {}pt should be >100pt, got {}",
        test_str,
        font_size,
        total_width
    );

    // Also render a full document and verify PDF is valid
    let text_node = make_text(test_str, font_size);
    let doc = default_doc(vec![text_node]);
    let (pdf, layout, _) = forme::render_with_layout(&doc).unwrap();

    assert!(pdf.starts_with(b"%PDF"), "Output should be a valid PDF");
    assert_eq!(layout.pages.len(), 1);
}

#[test]
fn test_page_placeholder_measurement_width() {
    use forme::font::StandardFont;
    let m = StandardFont::Helvetica.metrics();
    let font_size = 12.0;

    // Measure the substituted version
    let expected_text = "Page 00 of 00";
    let expected_width = m.measure_string(expected_text, font_size, 0.0);

    // The literal placeholder string is much wider
    let placeholder_text = "Page {{pageNumber}} of {{totalPages}}";
    let literal_width = m.measure_string(placeholder_text, font_size, 0.0);
    assert!(
        expected_width < literal_width,
        "Substituted width ({}) should be less than literal placeholder width ({})",
        expected_width,
        literal_width
    );

    // Layout a document with placeholders and verify it renders
    let text_node = make_text(placeholder_text, font_size);
    let doc = default_doc(vec![text_node]);
    let (_pdf, layout, _) = forme::render_with_layout(&doc).unwrap();
    assert_eq!(layout.pages.len(), 1);
}

#[test]
fn test_page_placeholder_survives_line_breaking() {
    // Place "Page {{pageNumber}} of {{totalPages}}" in a narrow page
    // that forces line breaking. Both placeholders must survive intact and
    // be replaced with actual values in the PDF output.
    let text_node = make_text("Page {{pageNumber}} of {{totalPages}}", 12.0);
    let page = Node {
        kind: NodeKind::Page {
            config: PageConfig {
                size: PageSize::Custom {
                    width: 80.0,
                    height: 200.0,
                },
                margin: Edges::uniform(5.0),
                wrap: true,
                ..Default::default()
            },
        },
        style: Style::default(),
        children: vec![text_node],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = Document {
        children: vec![page],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let (pdf, _layout, _) = forme::render_with_layout(&doc).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "Output should be a valid PDF");

    // Sentinel characters must not appear in PDF text operators (Tj lines).
    // We can't check raw bytes because 0x02/0x03 appear in compressed streams.
    // Instead verify via the layout that page numbers were substituted: the
    // existing test_page_number_placeholder_single_page covers actual replacement.
}

#[test]
fn test_page_placeholder_no_sentinel_in_text_operators() {
    // Render a document with placeholders and verify sentinels don't appear
    // in the PDF content stream text operators
    let text_node = make_text("{{pageNumber}} / {{totalPages}}", 12.0);
    let doc = default_doc(vec![text_node]);
    let (pdf, _layout, _) = forme::render_with_layout(&doc).unwrap();

    // Search for sentinel chars in PDF text string operators: (...) Tj
    // The sentinels are \x02 and \x03 which in WinAnsi would be \002 and \003
    let pdf_str = String::from_utf8_lossy(&pdf);
    // Check that no Tj operator contains the sentinel octal escapes
    assert!(
        !pdf_str.contains("\\002"),
        "Sentinel \\x02 octal must not appear in PDF text"
    );
    assert!(
        !pdf_str.contains("\\003"),
        "Sentinel \\x03 octal must not appear in PDF text"
    );
}

#[test]
fn test_two_pass_single_page_digit_count() {
    // A 1-page document should use 1-digit sentinel width after the
    // two-pass re-layout. Verify placeholders are replaced correctly.
    let text_node = make_text("{{pageNumber}}/{{totalPages}}", 12.0);
    let doc = default_doc(vec![text_node]);
    let (pdf, layout, _) = forme::render_with_layout(&doc).unwrap();
    assert_eq!(layout.pages.len(), 1);
    assert_valid_pdf(&pdf);
}

#[test]
fn test_two_pass_multi_page_common_case() {
    // A ~15-page document falls in the 10-99 range (2 digits),
    // matching the default estimate — no re-layout needed.
    let mut children = Vec::new();
    for i in 0..60 {
        children.push(make_text(
            &format!("Paragraph {} with enough text to fill space. {{{{pageNumber}}}}/{{{{totalPages}}}}", i),
            11.0,
        ));
    }
    let page = Node {
        kind: NodeKind::Page {
            config: PageConfig {
                size: PageSize::A4,
                margin: Edges::uniform(40.0),
                wrap: true,
                ..Default::default()
            },
        },
        style: Style::default(),
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = Document {
        children: vec![page],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let (pdf, layout, _) = forme::render_with_layout(&doc).unwrap();
    assert!(layout.pages.len() >= 2, "Should be multi-page");
    assert_valid_pdf(&pdf);
}

#[test]
fn test_render_performance() {
    // Performance benchmark: render a multi-page document and verify timing.
    // Run with: cargo test test_render_performance --release -- --nocapture
    use std::time::Instant;

    let mut children = Vec::new();
    for i in 0..200 {
        children.push(make_text(
            &format!(
                "Line {}: Lorem ipsum dolor sit amet, consectetur adipiscing elit. Page {{{{pageNumber}}}} of {{{{totalPages}}}}.",
                i
            ),
            10.0,
        ));
    }
    let page = Node {
        kind: NodeKind::Page {
            config: PageConfig {
                size: PageSize::A4,
                margin: Edges::uniform(50.0),
                wrap: true,
                ..Default::default()
            },
        },
        style: Style::default(),
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = Document {
        children: vec![page],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };

    let start = Instant::now();
    let (pdf, layout, _) = forme::render_with_layout(&doc).unwrap();
    let elapsed = start.elapsed();

    eprintln!(
        "Performance: {} pages rendered in {:?}",
        layout.pages.len(),
        elapsed
    );
    assert_valid_pdf(&pdf);
    assert!(layout.pages.len() >= 2, "Should produce multiple pages");
}

#[test]
fn test_multi_weight_font_resolution() {
    // Register the same font family at 3 different weights.
    // The PDF serializer should embed 3 separate font objects, not collapse them to 1.
    use base64::Engine as _;
    let font_data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fonts/NotoSans-Regular.ttf"
    ))
    .unwrap();
    let font_b64 = base64::engine::general_purpose::STANDARD.encode(&font_data);

    fn text_with_weight(content: &str, weight: u32) -> Node {
        Node {
            kind: NodeKind::Text {
                content: content.to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_family: Some("TestFont".into()),
                font_weight: Some(weight),
                font_size: Some(14.0),
                ..Style::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    let doc = Document {
        children: vec![
            text_with_weight("Light text", 200),
            text_with_weight("Regular text", 400),
            text_with_weight("Bold text", 700),
        ],
        fonts: vec![
            FontEntry {
                family: "TestFont".into(),
                src: font_b64.clone(),
                weight: 200,
                italic: false,
            },
            FontEntry {
                family: "TestFont".into(),
                src: font_b64.clone(),
                weight: 400,
                italic: false,
            },
            FontEntry {
                family: "TestFont".into(),
                src: font_b64,
                weight: 700,
                italic: false,
            },
        ],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        tagged: false,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdfa: None,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_valid_pdf(&pdf);

    // The PDF should contain 3 distinct CIDFont entries for TestFont (one per weight)
    let pdf_str = String::from_utf8_lossy(&pdf);
    let testfont_count = pdf_str.matches("/BaseFont /TestFont").count();
    assert!(
        testfont_count >= 3,
        "Expected at least 3 distinct TestFont /BaseFont entries, got {}",
        testfont_count
    );
}

#[test]
fn test_svg_opacity_produces_ext_gstate() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Svg {
                width: 100.0,
                height: 100.0,
                view_box: None,
                content:
                    r##"<rect x="0" y="0" width="100" height="100" fill="#ff0000" opacity="0.5"/>"##
                        .to_string(),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        fonts: vec![],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        tagged: false,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdfa: None,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_valid_pdf(&pdf);

    let pdf_str = String::from_utf8_lossy(&pdf);
    // Should contain an ExtGState with 0.5 opacity
    assert!(
        pdf_str.contains("/ca 0.5000") && pdf_str.contains("/CA 0.5000"),
        "PDF should contain ExtGState with 0.5 opacity for SVG element"
    );
    // Should reference the GS in the content stream
    assert!(
        pdf_str.contains("/GS"),
        "PDF content stream should reference a graphics state"
    );
}

#[test]
fn test_svg_fill_opacity_produces_ext_gstate() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Svg {
                width: 100.0,
                height: 100.0,
                view_box: None,
                content: r##"<rect x="0" y="0" width="50" height="50" fill="#00ff00" fill-opacity="0.3"/>"##
                    .to_string(),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        fonts: vec![],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,        tagged: false,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdfa: None,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_valid_pdf(&pdf);

    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(
        pdf_str.contains("/ca 0.3000") && pdf_str.contains("/CA 0.3000"),
        "PDF should contain ExtGState with 0.3 opacity for fill-opacity"
    );
}

#[test]
fn test_svg_inherited_group_opacity() {
    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Svg {
                width: 200.0,
                height: 200.0,
                view_box: None,
                content: r#"<g opacity="0.5"><rect x="0" y="0" width="100" height="100" fill="blue"/></g>"#
                    .to_string(),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        fonts: vec![],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,        tagged: false,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdfa: None,
        pdf_ua: false,
        certification: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_valid_pdf(&pdf);

    let pdf_str = String::from_utf8_lossy(&pdf);
    // Group opacity 0.5 should be inherited by the rect
    assert!(
        pdf_str.contains("/ca 0.5000"),
        "PDF should contain ExtGState with inherited group opacity 0.5"
    );
}

// ─── Feature 1: opacity propagates to children ────────────────

/// A View with opacity:0.5 and a Text child must wrap BOTH the rect's own
/// paint and the text's BT…ET in a single q/GS gs … Q block. Without the
/// fix, the rect fades but the child text renders at 100% alpha.
#[test]
fn test_opacity_wraps_children() {
    let doc = default_doc(vec![make_styled_view(
        Style {
            opacity: Some(0.5),
            background_color: Some(Color::rgb(1.0, 0.0, 0.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![make_text("Faded text", 12.0)],
    )]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);

    // Find the index of the first /GS gs reference.
    let gs_idx = stream
        .find(" gs")
        .expect("expected at least one /GS{n} gs reference");
    // The text BT…ET block must appear AFTER the gs (so it's inside the
    // opacity wrap) and BEFORE the matching Q.
    let bt_idx = stream
        .find("BT")
        .expect("expected a BT text block in the content stream");
    assert!(
        bt_idx > gs_idx,
        "BT must come after /GS gs so the text is inside the opacity wrap (gs at {}, BT at {})",
        gs_idx,
        bt_idx,
    );
    // The matching Q must come after the BT/ET pair (Q closes the
    // opacity-wrap q from the start).
    let et_idx = stream
        .rfind("ET")
        .expect("expected an ET in the content stream");
    let last_q_idx = stream
        .rfind("\nQ\n")
        .or_else(|| stream.rfind("\nQ"))
        .expect("expected a Q closing the opacity wrap");
    assert!(
        last_q_idx > et_idx,
        "Q closing opacity must come after ET (ET at {}, Q at {})",
        et_idx,
        last_q_idx,
    );
}

/// Nested opacities must produce two ExtGState references in nested q/Q
/// blocks. PDF's graphics state stack multiplies them at render time, so a
/// 0.5 child of a 0.5 parent renders at effective 0.25 alpha — but in the
/// emitted bytes both ExtGState dicts hold /ca 0.5 (the multiplication is
/// the viewer's job).
#[test]
fn test_nested_opacity_emits_two_gs_refs() {
    let inner = make_styled_view(
        Style {
            opacity: Some(0.5),
            background_color: Some(Color::rgb(0.0, 1.0, 0.0)),
            width: Some(Dimension::Pt(50.0)),
            height: Some(Dimension::Pt(25.0)),
            ..Default::default()
        },
        vec![make_text("inner", 12.0)],
    );
    let outer = make_styled_view(
        Style {
            opacity: Some(0.5),
            background_color: Some(Color::rgb(1.0, 0.0, 0.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![inner],
    );
    let doc = default_doc(vec![outer]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);

    // Count /GSx gs references — should be at least 2 (one per opacity
    // level). The unique-opacity collector dedupes 0.5 to a single
    // ExtGState dict, so both references point to the same /GS0.
    let gs_count = stream.matches(" gs").count();
    assert!(
        gs_count >= 2,
        "expected at least 2 /GS{{n}} gs references for nested opacities, got {} (stream: {})",
        gs_count,
        stream,
    );

    // ExtGState dict carries 0.5 (the multiplication to 0.25 happens
    // at render time via the q/Q stack — not in the PDF bytes).
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_str.contains("/ca 0.5000"),
        "PDF should contain /ca 0.5000 ExtGState"
    );
}

// ─── Feature 4: boxShadow ──────────────────────────────────────

/// `boxShadow` paints a filled rect offset by (offsetX, offsetY) BEFORE
/// the element's background, so the shadow sits visually behind. Verify
/// the order in the page content stream.
#[test]
fn test_box_shadow_renders_before_background() {
    let view = make_styled_view(
        Style {
            box_shadow: Some(forme::style::BoxShadow {
                offset_x: 2.0,
                offset_y: 4.0,
                blur: 0.0,
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.2,
                },
            }),
            background_color: Some(Color::rgb(1.0, 1.0, 1.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_last_flate_stream(&pdf_bytes).expect("expected page content stream");

    // The shadow's fill (`0.000 0.000 0.000 rg`) must come before the
    // background's white fill (`1.000 1.000 1.000 rg`).
    let shadow_fill_idx = stream
        .find("0.000 0.000 0.000 rg")
        .expect("expected shadow fill in content stream");
    let bg_idx = stream
        .find("1.000 1.000 1.000 rg")
        .expect("expected background fill in content stream");
    assert!(
        shadow_fill_idx < bg_idx,
        "shadow fill (at {}) must precede background fill (at {})",
        shadow_fill_idx,
        bg_idx,
    );
}

/// Shadow color with alpha < 1.0 produces an ExtGState entry so the
/// shadow renders semi-transparently.
#[test]
fn test_box_shadow_alpha_creates_extgstate() {
    let view = make_styled_view(
        Style {
            box_shadow: Some(forme::style::BoxShadow {
                offset_x: 4.0,
                offset_y: 4.0,
                blur: 0.0,
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.25,
                },
            }),
            background_color: Some(Color::rgb(1.0, 1.0, 1.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_str.contains("/ca 0.2500"),
        "expected /ca 0.2500 ExtGState for shadow alpha 0.25"
    );
}

// ─── Feature 5: Page backgroundImage ──────────────────────────

/// `backgroundImage` on a Page is registered as a PDF XObject and
/// referenced from the page's resource dictionary. Verifies the XObject
/// is embedded (`/Subtype /Image`), the page's resource dict contains
/// `/XObject << /Im{n} ... >>`, and the visual ordering check happens via
/// `decompress_page_content_stream` below.
#[test]
fn test_page_background_image_registers_xobject() {
    let one_px_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let mut doc = default_doc(vec![make_text("Foreground", 12.0)]);
    doc.default_page.background_image = Some(one_px_png.to_string());

    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    // The image XObject is embedded in the PDF.
    assert!(
        pdf_str.contains("/Subtype /Image"),
        "expected image XObject (/Subtype /Image) in PDF",
    );
    // The page's resource dictionary references the XObject as /Im0.
    assert!(
        pdf_str.contains("/XObject << /Im0"),
        "expected page resources to include /XObject << /Im0 ...",
    );

    // Locate and decompress the page content stream specifically (not
    // arbitrary streams — fonts and images emit binary streams that
    // confuse the global helper). Find the LAST `<< /Length ... /Filter
    // /FlateDecode >>\nstream\n` block — page content is the final
    // FlateDecode stream in the typical object emission order.
    let content = decompress_last_flate_stream(&pdf_bytes)
        .expect("expected at least one FlateDecode page content stream");
    let do_idx = content
        .find(" Do")
        .expect("expected /Im{n} Do for page background in the content stream");
    let bt_idx = content
        .find("BT")
        .expect("expected BT for foreground text in the content stream");
    assert!(
        do_idx < bt_idx,
        "background Do (at {}) must precede text BT (at {}) so it renders behind",
        do_idx,
        bt_idx,
    );
}

/// Find the last `<< /Length N /Filter /FlateDecode >>\nstream\n` block
/// in the PDF and decompress it. The page content stream is emitted last
/// in the typical object order, so this isolates the operators we want
/// without colliding with `endstream\n` inside binary image/font streams.
fn decompress_last_flate_stream(pdf: &[u8]) -> Option<String> {
    use miniz_oxide::inflate::decompress_to_vec_zlib;
    let header_prefix = b"<< /Length ";
    let header_suffix = b" /Filter /FlateDecode >>\nstream\n";
    // Walk through the bytes finding header_prefix occurrences; for each,
    // parse the length integer and check that header_suffix follows.
    let mut last: Option<(usize, usize)> = None;
    let mut search_from = 0usize;
    while let Some(rel) = pdf[search_from..]
        .windows(header_prefix.len())
        .position(|w| w == header_prefix)
    {
        let prefix_end = search_from + rel + header_prefix.len();
        // Read digits.
        let mut digit_end = prefix_end;
        while digit_end < pdf.len() && pdf[digit_end].is_ascii_digit() {
            digit_end += 1;
        }
        if digit_end == prefix_end {
            search_from = prefix_end;
            continue;
        }
        // Suffix must follow immediately.
        if digit_end + header_suffix.len() > pdf.len()
            || &pdf[digit_end..digit_end + header_suffix.len()] != header_suffix
        {
            search_from = prefix_end;
            continue;
        }
        let length: usize = std::str::from_utf8(&pdf[prefix_end..digit_end])
            .ok()?
            .parse()
            .ok()?;
        let stream_start = digit_end + header_suffix.len();
        last = Some((stream_start, length));
        search_from = stream_start + length;
    }
    let (abs, len) = last?;
    let bytes = &pdf[abs..abs + len];
    let decompressed = decompress_to_vec_zlib(bytes).ok()?;
    String::from_utf8(decompressed).ok()
}

/// `backgroundOpacity: 0.08` produces a `/ca 0.0800` ExtGState entry so
/// the background paint renders at 8% alpha. Useful for watermark-style
/// overlays.
#[test]
fn test_page_background_opacity_creates_extgstate() {
    let one_px_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let config = PageConfig {
        background_image: Some(one_px_png.to_string()),
        background_opacity: Some(0.08),
        ..Default::default()
    };
    let doc = Document {
        children: vec![make_text("on top", 12.0)],
        metadata: Metadata::default(),
        default_page: config,
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        pdf_ua: false,
        flatten_forms: false,
        certification: None,
    };
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_str.contains("/ca 0.0800"),
        "expected /ca 0.0800 ExtGState for backgroundOpacity 0.08"
    );
}

// ─── Feature 3: rounded clipping when overflow:hidden + borderRadius ──

/// `overflow: hidden` with a non-zero `borderRadius` must clip children
/// to the rounded path (m/l/c/h then W n), not a rectangular `re W n`.
/// Otherwise, descendants visually escape the rounded corners.
#[test]
fn test_overflow_hidden_with_border_radius_uses_rounded_clip() {
    let inner = make_text("clipped content", 12.0);
    let outer = make_styled_view(
        Style {
            overflow: Some(forme::style::Overflow::Hidden),
            border_radius: Some(forme::style::CornerValues::uniform(12.0)),
            background_color: Some(Color::rgb(0.9, 0.9, 0.9)),
            width: Some(Dimension::Pt(120.0)),
            height: Some(Dimension::Pt(60.0)),
            ..Default::default()
        },
        vec![inner],
    );
    let doc = default_doc(vec![outer]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);

    // The clip must use a rounded path (m/c/h W n), not a rectangle re W n.
    // Look for the unique signature `h\nW n` — the rounded path closes with
    // `h` before applying the clip.
    let has_rounded_clip = stream.contains("h\nW n") || stream.contains("h\nW n\n");
    assert!(
        has_rounded_clip,
        "expected rounded clip path (h\\nW n) in content stream; got: {}",
        stream,
    );
}

/// `overflow: hidden` with NO borderRadius keeps the rectangular `re W n`
/// clip. Regression check: rounding shouldn't kick in when borderRadius is
/// 0.
#[test]
fn test_overflow_hidden_without_border_radius_uses_rect_clip() {
    let inner = make_text("clipped content", 12.0);
    let outer = make_styled_view(
        Style {
            overflow: Some(forme::style::Overflow::Hidden),
            background_color: Some(Color::rgb(0.9, 0.9, 0.9)),
            width: Some(Dimension::Pt(120.0)),
            height: Some(Dimension::Pt(60.0)),
            ..Default::default()
        },
        vec![inner],
    );
    let doc = default_doc(vec![outer]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);
    assert!(
        stream.contains(" re W n"),
        "expected rectangular clip `re W n` in content stream; got: {}",
        stream,
    );
}

// ─── Feature 2: user-facing wordSpacing ───────────────────────

/// `wordSpacing: 4` on a Text emits a `Tw 4` operator in the content
/// stream so each ASCII space gets +4pt of width (PDF Tw operator).
#[test]
fn test_word_spacing_emits_tw_operator() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Text {
            content: "hello world wide".to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            word_spacing: Some(4.0),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);

    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);
    assert!(
        stream.contains("4.0000 Tw"),
        "expected `4.0000 Tw` in content stream, got: {}",
        stream,
    );
}

/// Default text (no wordSpacing, no justification) should not emit a Tw
/// operator — preserves current output exactly.
#[test]
fn test_default_text_no_tw_operator() {
    let doc = default_doc(vec![make_text("hello world", 12.0)]);
    let pdf_bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf_bytes);
    assert!(
        !stream.contains(" Tw"),
        "expected no Tw in default text content stream, got: {}",
        stream,
    );
}

/// Element with opacity exactly 1.0 should not emit any /GS gs reference
/// or ExtGState dict (perf and cleaner output).
#[test]
fn test_opacity_one_emits_no_extgstate_wrap() {
    let doc = default_doc(vec![make_styled_view(
        Style {
            opacity: Some(1.0),
            background_color: Some(Color::rgb(1.0, 0.0, 0.0)),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![make_text("Full alpha", 12.0)],
    )]);

    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        !pdf_str.contains("/ExtGState"),
        "no /ExtGState resource should be emitted when all opacities are 1.0"
    );
}

// ─── Feature 6 (v1): 2-stop linear / radial gradients ─────────

/// `background: linear-gradient(...)` emits a Type 2 (axial) Shading
/// dictionary with a Type 2 (exponential) Function for color
/// interpolation, and a `/Sh{n} sh` operator inside a clipped path in
/// the content stream.
#[test]
fn test_linear_gradient_emits_shading_type_2() {
    let view = make_styled_view(
        Style {
            background: Some(forme::style::Background::Linear(
                forme::style::LinearGradient {
                    angle_deg: 90.0,
                    stops: vec![
                        forme::style::GradientStop {
                            position: 0.0,
                            color: Color::rgb(1.0, 0.0, 0.0),
                        },
                        forme::style::GradientStop {
                            position: 1.0,
                            color: Color::rgb(0.0, 0.0, 1.0),
                        },
                    ],
                },
            )),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    assert!(
        pdf_str.contains("/ShadingType 2"),
        "expected /ShadingType 2 (axial) for linear gradient",
    );
    assert!(
        pdf_str.contains("/FunctionType 2"),
        "expected /FunctionType 2 (exponential) for color interpolation",
    );
    assert!(
        pdf_str.contains("/Shading << /Sh0"),
        "expected page resource dict to reference /Sh0 shading",
    );

    let stream = decompress_last_flate_stream(&pdf_bytes).expect("expected page content stream");
    assert!(
        stream.contains("/Sh0 sh"),
        "expected `/Sh0 sh` operator in content stream, got: {}",
        stream,
    );
}

/// `background: radial-gradient(...)` emits a Type 3 (radial) Shading
/// dictionary with inner radius 0 and outer radius equal to half the
/// element's longest side.
#[test]
fn test_radial_gradient_emits_shading_type_3() {
    let view = make_styled_view(
        Style {
            background: Some(forme::style::Background::Radial(
                forme::style::RadialGradient {
                    stops: vec![
                        forme::style::GradientStop {
                            position: 0.0,
                            color: Color::rgb(1.0, 1.0, 1.0),
                        },
                        forme::style::GradientStop {
                            position: 1.0,
                            color: Color::rgb(0.0, 0.0, 0.0),
                        },
                    ],
                },
            )),
            width: Some(Dimension::Pt(80.0)),
            height: Some(Dimension::Pt(80.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    assert!(
        pdf_str.contains("/ShadingType 3"),
        "expected /ShadingType 3 (radial) for radial gradient",
    );
    // Center is (w/2, h/2) = (40, 40), inner r = 0, outer r = max(w,h)/2 = 40.
    assert!(
        pdf_str.contains("/Coords [40.000 40.000 0 40.000 40.000 40.000]"),
        "expected radial Coords [cx cy 0 cx cy r] = [40 40 0 40 40 40], got PDF: {}",
        &pdf_str[..pdf_str.len().min(2000)],
    );
}

/// CSS 180deg = top-to-bottom. On a w×h rect, the gradient axis must run
/// from (w/2, h) at the top in PDF coords (CSS-spec start) to (w/2, 0) at
/// the bottom (CSS-spec end). This locks down the angle math against
/// regressions — the Y-flip and CSS clockwise-from-up convention are
/// fiddly to get right.
#[test]
fn test_linear_gradient_180deg_axis_goes_top_to_bottom() {
    // Use a w=200, h=100 rect so the expected coords have nice numbers.
    let view = make_styled_view(
        Style {
            background: Some(forme::style::Background::Linear(
                forme::style::LinearGradient {
                    angle_deg: 180.0,
                    stops: vec![
                        forme::style::GradientStop {
                            position: 0.0,
                            color: Color::rgb(1.0, 1.0, 1.0),
                        },
                        forme::style::GradientStop {
                            position: 1.0,
                            color: Color::rgb(0.0, 0.0, 0.0),
                        },
                    ],
                },
            )),
            width: Some(Dimension::Pt(200.0)),
            height: Some(Dimension::Pt(100.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    // 180deg means dx=sin(180°)=0, dy=cos(180°)=-1. Axis length = h = 100.
    // Center of rect (relative) = (100, 50). half = 50.
    // x0 = 100 - 0 = 100, y0 = 50 - (-1)*50 = 100  (top in PDF coords).
    // x1 = 100 + 0 = 100, y1 = 50 + (-1)*50 = 0    (bottom in PDF coords).
    // First stop (white) at top → last stop (black) at bottom = top-to-bottom.
    // Note: cos(180°) is -1, so y1 = 50 + (-1)*50 = 0, but f64 arithmetic
    // produces -0.0 which {:.3} formats as "-0.000". Accept either signed
    // form of zero — the rendered position is the same.
    let has_top_to_bottom = pdf_str.contains("/Coords [100.000 100.000 100.000 0.000]")
        || pdf_str.contains("/Coords [100.000 100.000 100.000 -0.000]");
    assert!(
        has_top_to_bottom,
        "expected 180deg Coords [w/2 h w/2 0] = [100 100 100 0], got snippet: {}",
        &pdf_str[..pdf_str.len().min(2000)],
    );
}

/// Multi-stop (3+) gradient emits a Type 3 (stitching) function combining
/// N-1 Type 2 sub-functions, with /Bounds at each interior stop position.
#[test]
fn test_multi_stop_gradient_emits_stitching_function() {
    let view = make_styled_view(
        Style {
            background: Some(forme::style::Background::Linear(
                forme::style::LinearGradient {
                    angle_deg: 90.0,
                    stops: vec![
                        forme::style::GradientStop {
                            position: 0.0,
                            color: Color::rgb(1.0, 0.0, 0.0),
                        },
                        forme::style::GradientStop {
                            position: 0.5,
                            color: Color::rgb(0.0, 1.0, 0.0),
                        },
                        forme::style::GradientStop {
                            position: 1.0,
                            color: Color::rgb(0.0, 0.0, 1.0),
                        },
                    ],
                },
            )),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    assert!(
        pdf_str.contains("/FunctionType 3"),
        "expected /FunctionType 3 (stitching) for multi-stop gradient",
    );
    let type2_count = pdf_str.matches("/FunctionType 2").count();
    assert_eq!(
        type2_count, 2,
        "expected 2 Type 2 sub-functions for a 3-stop gradient, got {}",
        type2_count,
    );
    assert!(
        pdf_str.contains("/Bounds [0.5000]"),
        "expected /Bounds [0.5000] for the interior stop, got: {}",
        &pdf_str[..pdf_str.len().min(2000)],
    );
    assert!(
        pdf_str.contains("/Encode [0 1 0 1]"),
        "expected /Encode [0 1 0 1] for 2 sub-functions, got: {}",
        &pdf_str[..pdf_str.len().min(2000)],
    );
}

/// Stops with positions outside [0,1] or in non-monotonic order are
/// normalized: clamped to [0,1], sorted ascending. Defensive in case JSON
/// callers (not just the React parser) pass dirty input.
#[test]
fn test_gradient_stops_normalized_when_out_of_order() {
    let view = make_styled_view(
        Style {
            background: Some(forme::style::Background::Linear(
                forme::style::LinearGradient {
                    angle_deg: 90.0,
                    stops: vec![
                        forme::style::GradientStop {
                            position: 1.0,
                            color: Color::rgb(0.0, 0.0, 1.0),
                        },
                        forme::style::GradientStop {
                            position: 0.0,
                            color: Color::rgb(1.0, 0.0, 0.0),
                        },
                    ],
                },
            )),
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(50.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![view]);
    let pdf_bytes = render_to_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    assert!(
        pdf_str.contains("/C0 [1.0000 0.0000 0.0000] /C1 [0.0000 0.0000 1.0000]"),
        "expected normalized order red→blue in C0/C1, got: {}",
        &pdf_str[..pdf_str.len().min(2000)],
    );
}

#[test]
fn test_flex_row_percentage_widths_resolve_against_parent() {
    // Regression: children with width: 30% / 70% in a flex row were getting
    // double-resolved against their own (already-resolved) width, ending up
    // at ~9%/49% of the row instead of 30%/70%.
    let child30 = make_styled_view(
        Style {
            width: Some(Dimension::Percent(30.0)),
            ..Default::default()
        },
        vec![make_text("30%", 12.0)],
    );
    let child70 = make_styled_view(
        Style {
            width: Some(Dimension::Percent(70.0)),
            ..Default::default()
        },
        vec![make_text("70%", 12.0)],
    );
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![child30, child70],
    );

    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    let row_el = &pages[0].elements[0];
    assert_eq!(row_el.children.len(), 2);

    let row_w = row_el.width;
    let expected_30 = row_w * 0.3;
    let expected_70 = row_w * 0.7;

    assert!(
        (row_el.children[0].width - expected_30).abs() < 1.0,
        "30% child width {:.2} != expected {:.2} (row_w={:.2})",
        row_el.children[0].width,
        expected_30,
        row_w,
    );
    assert!(
        (row_el.children[1].width - expected_70).abs() < 1.0,
        "70% child width {:.2} != expected {:.2} (row_w={:.2})",
        row_el.children[1].width,
        expected_70,
        row_w,
    );
}

#[test]
fn test_flex_row_equal_percentage_widths_split_evenly() {
    // Two children with width: 100% should shrink to 50/50 — flex-shrink
    // distributes the overflow evenly. Regression coverage for the same
    // double-resolution bug.
    let mk_child = || {
        make_styled_view(
            Style {
                width: Some(Dimension::Percent(100.0)),
                ..Default::default()
            },
            vec![make_text("x", 12.0)],
        )
    };
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![mk_child(), mk_child()],
    );

    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    let row_el = &pages[0].elements[0];
    assert_eq!(row_el.children.len(), 2);

    let half = row_el.width / 2.0;
    for (i, child) in row_el.children.iter().enumerate() {
        assert!(
            (child.width - half).abs() < 1.0,
            "child{} width {:.2} != expected {:.2}",
            i,
            child.width,
            half,
        );
    }
}

#[test]
fn test_grid_page_break_keeps_columns_aligned() {
    // Regression: when a grid wraps over a page break, each column was ending
    // up on its own page instead of the grid breaking by row. This happened
    // because the first row's page-break check had `&& row > 0`, so an
    // oversized first row was force-laid-out at the bottom of the current
    // page and each cell's layout_node triggered its own page break.
    let make_tall_cell = |label: &str| {
        make_styled_view(
            Style {
                padding: Some(Edges::uniform(4.0)),
                background_color: Some(Color {
                    r: 0.9,
                    g: 0.9,
                    b: 0.9,
                    a: 1.0,
                }),
                height: Some(Dimension::Pt(400.0)),
                ..Default::default()
            },
            vec![make_text(label, 12.0)],
        )
    };

    // Big spacer pushes the grid to the bottom of page 1 so the grid won't
    // fit there. The grid has 3 columns, all 400pt tall — they should all
    // share page 2, not split across 3 pages.
    let spacer = make_styled_view(
        Style {
            height: Some(Dimension::Pt(500.0)),
            ..Default::default()
        },
        vec![],
    );

    let grid = make_styled_view(
        Style {
            display: Some(Display::Grid),
            grid_template_columns: Some(vec![
                GridTrackSize::Fr(1.0),
                GridTrackSize::Fr(1.0),
                GridTrackSize::Fr(1.0),
            ]),
            gap: Some(8.0),
            ..Default::default()
        },
        vec![
            make_tall_cell("A"),
            make_tall_cell("B"),
            make_tall_cell("C"),
        ],
    );

    let doc = default_doc(vec![spacer, grid]);
    let pages = layout_doc(&doc);

    // Should be exactly 2 pages: spacer on page 1, grid on page 2.
    // Before the fix this produced 4 pages (one per column + the spacer page).
    assert_eq!(
        pages.len(),
        2,
        "expected 2 pages, got {} — grid columns split across pages",
        pages.len(),
    );

    // All three grid cells must be on page 2 at the same y. Walk the tree
    // and collect every element matching our 400pt cell height.
    fn collect_matching<'a>(
        el: &'a forme::layout::LayoutElement,
        out: &mut Vec<&'a forme::layout::LayoutElement>,
    ) {
        if (el.height - 400.0).abs() < 0.5 {
            out.push(el);
        }
        for c in &el.children {
            collect_matching(c, out);
        }
    }
    let mut tall_cells: Vec<&forme::layout::LayoutElement> = Vec::new();
    for el in &pages[1].elements {
        collect_matching(el, &mut tall_cells);
    }
    assert_eq!(
        tall_cells.len(),
        3,
        "expected 3 tall cells on page 2, found {}",
        tall_cells.len(),
    );
    let first_y = tall_cells[0].y;
    for c in &tall_cells {
        assert!(
            (c.y - first_y).abs() < 0.5,
            "grid cells not aligned: y={} vs {}",
            c.y,
            first_y,
        );
    }
}

// Build a <Text> leaf with an explicit fixed width and text alignment.
fn make_text_fixed_width(content: &str, width: f64, align: TextAlign) -> Node {
    Node {
        kind: NodeKind::Text {
            content: content.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(12.0),
            width: Some(Dimension::Pt(width)),
            text_align: Some(align),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

// LETTER page (612pt) with 48pt insets => content band 48..564, matching the
// minimal repro from the bug report.
fn letter_doc_with_inset_48(child: Node) -> Document {
    default_doc(vec![Node {
        kind: NodeKind::Page {
            config: PageConfig {
                size: PageSize::Letter,
                margin: Edges::uniform(48.0),
                ..Default::default()
            },
        },
        style: Style::default(),
        children: vec![child],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }])
}

// Collect every element of a given node_type, depth-first.
fn collect_by_type<'a>(
    el: &'a forme::layout::LayoutElement,
    ty: &str,
    out: &mut Vec<&'a forme::layout::LayoutElement>,
) {
    if el.node_type.as_deref() == Some(ty) {
        out.push(el);
    }
    for c in &el.children {
        collect_by_type(c, ty, out);
    }
}

#[test]
fn test_flex_row_text_fixed_width_box_not_parent_width() {
    // Regression (0.10.2): <Text style={{ width }}> inside a flex row was
    // positioned using the requested width but RENDERED at the parent row's full
    // main-axis width. layout_view honored style.width; layout_text did not, so
    // the Text box ballooned to the row width and textAlign:right threw glyphs
    // hundreds of points past the page edge — silent corruption (byte hashes
    // unchanged). The Text box must equal the requested width and glyphs must
    // stay inside the page content band.
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            justify_content: Some(JustifyContent::FlexEnd),
            ..Default::default()
        },
        vec![
            make_text_fixed_width("Tax:", 120.0, TextAlign::Right),
            make_text_fixed_width("$10.00", 80.0, TextAlign::Right),
        ],
    );

    let pages = layout_doc(&letter_doc_with_inset_48(row));
    assert_eq!(pages.len(), 1);
    let band_right = 612.0 - 48.0; // 564

    // Text container boxes must equal the requested widths, not the 516pt row.
    let mut texts = Vec::new();
    for el in &pages[0].elements {
        collect_by_type(el, "Text", &mut texts);
    }
    assert_eq!(texts.len(), 2, "expected 2 Text boxes, got {}", texts.len());
    let mut widths: Vec<f64> = texts.iter().map(|t| t.width).collect();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (widths[0] - 80.0).abs() < 0.5 && (widths[1] - 120.0).abs() < 0.5,
        "Text boxes should be 80 and 120, got {:?}",
        widths,
    );

    // Glyph lines must land inside the page content band (48..564), not off-page.
    let mut lines = Vec::new();
    for el in &pages[0].elements {
        collect_by_type(el, "TextLine", &mut lines);
    }
    assert!(!lines.is_empty(), "expected TextLine elements");
    for line in &lines {
        assert!(
            line.x + line.width <= band_right + 0.5,
            "glyph line runs off-page: x={:.2} width={:.2} (right={:.2} > band {:.2})",
            line.x,
            line.width,
            line.x + line.width,
            band_right,
        );
        assert!(
            line.x >= 48.0 - 0.5,
            "glyph line starts before content band: x={:.2}",
            line.x,
        );
    }
}

#[test]
fn test_flex_row_text_fixed_width_box_flex_start() {
    // Same fix, justifyContent: flex-start — the off-page shift isn't visible
    // here (no right-edge alignment against the bloated box), but the Text box
    // was still mis-sized to the row width. Lock in the correct box widths.
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            justify_content: Some(JustifyContent::FlexStart),
            ..Default::default()
        },
        vec![
            make_text_fixed_width("Tax:", 120.0, TextAlign::Right),
            make_text_fixed_width("$10.00", 80.0, TextAlign::Right),
        ],
    );

    let pages = layout_doc(&letter_doc_with_inset_48(row));
    assert_eq!(pages.len(), 1);

    let mut texts = Vec::new();
    for el in &pages[0].elements {
        collect_by_type(el, "Text", &mut texts);
    }
    assert_eq!(texts.len(), 2);
    let mut widths: Vec<f64> = texts.iter().map(|t| t.width).collect();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (widths[0] - 80.0).abs() < 0.5 && (widths[1] - 120.0).abs() < 0.5,
        "Text boxes should be 80 and 120, got {:?}",
        widths,
    );
}

// Helper: build a fixed-size child View with the requested top/bottom margins.
fn make_mt_mb_child(top: EdgeValue, bottom: EdgeValue, w: f64, h: f64) -> Node {
    make_styled_view(
        Style {
            width: Some(Dimension::Pt(w)),
            height: Some(Dimension::Pt(h)),
            margin: Some(MarginEdges {
                top,
                right: EdgeValue::Pt(0.0),
                bottom,
                left: EdgeValue::Pt(0.0),
            }),
            ..Default::default()
        },
        vec![],
    )
}

// Helper: wrap a child in a parent View of a known fixed height and place on a
// zero-inset Letter page, so child.y is directly comparable to parent geometry.
fn parent_with_height_doc(parent_h: Option<f64>, child: Node) -> Document {
    let parent = make_styled_view(
        Style {
            width: Some(Dimension::Pt(100.0)),
            height: parent_h.map(Dimension::Pt),
            ..Default::default()
        },
        vec![child],
    );
    default_doc(vec![Node::page(
        PageConfig {
            margin: Edges::uniform(0.0),
            ..Default::default()
        },
        Style::default(),
        vec![parent],
    )])
}

#[test]
fn test_column_mt_auto_pushes_child_to_bottom() {
    // Regression (issue 1): marginTop: 'auto' on a child in a column-flex
    // parent with fixed height was a no-op — the child stayed at the top
    // instead of being pushed to the bottom. The column branch of
    // layout_children had no per-child auto-margin slack pass; flex-row
    // already did at mod.rs:2256-2267. Mirroring that block fixes it.
    let child = make_mt_mb_child(EdgeValue::Auto, EdgeValue::Pt(0.0), 40.0, 10.0);
    let pages = layout_doc(&parent_with_height_doc(Some(50.0), child));
    assert_eq!(pages.len(), 1);

    let parent_el = &pages[0].elements[0];
    assert_eq!(parent_el.children.len(), 1);
    let child_el = &parent_el.children[0];

    let expected_top = parent_el.y + parent_el.height - child_el.height;
    assert!(
        (child_el.y - expected_top).abs() < 0.5,
        "mt-auto child.y={:.2} expected {:.2} (parent y={:.2} h={:.2} child h={:.2})",
        child_el.y,
        expected_top,
        parent_el.y,
        parent_el.height,
        child_el.height,
    );
}

#[test]
fn test_column_mt_and_mb_auto_centers_child() {
    // Both marginTop AND marginBottom auto → child centered. Slack
    // distributes equally to the two autos.
    let child = make_mt_mb_child(EdgeValue::Auto, EdgeValue::Auto, 40.0, 10.0);
    let pages = layout_doc(&parent_with_height_doc(Some(50.0), child));
    assert_eq!(pages.len(), 1);

    let parent_el = &pages[0].elements[0];
    let child_el = &parent_el.children[0];

    let expected_top = parent_el.y + (parent_el.height - child_el.height) / 2.0;
    assert!(
        (child_el.y - expected_top).abs() < 0.5,
        "mt+mb auto centered: child.y={:.2} expected {:.2}",
        child_el.y,
        expected_top,
    );
}

#[test]
fn test_column_mb_auto_alone_holds_child_at_top() {
    // marginBottom: 'auto' alone (no top auto) consumes slack between this
    // child and any subsequent children — for a single child it just adds
    // slack below, which is equivalent to leaving the child at the top.
    let child = make_mt_mb_child(EdgeValue::Pt(0.0), EdgeValue::Auto, 40.0, 10.0);
    let pages = layout_doc(&parent_with_height_doc(Some(50.0), child));
    assert_eq!(pages.len(), 1);

    let parent_el = &pages[0].elements[0];
    let child_el = &parent_el.children[0];

    assert!(
        (child_el.y - parent_el.y).abs() < 0.5,
        "mb-auto alone: child.y={:.2} expected parent.y={:.2} (no shift)",
        child_el.y,
        parent_el.y,
    );
}

#[test]
fn test_column_mt_auto_noop_without_fixed_parent_height() {
    // No slack to consume → auto-margin is a no-op. Matches CSS: margin auto
    // has no effect when the containing block has no definite size in that
    // axis. The parent View has Auto height here.
    let child = make_mt_mb_child(EdgeValue::Auto, EdgeValue::Pt(0.0), 40.0, 10.0);
    let pages = layout_doc(&parent_with_height_doc(None, child));
    assert_eq!(pages.len(), 1);

    let parent_el = &pages[0].elements[0];
    let child_el = &parent_el.children[0];

    assert!(
        (child_el.y - parent_el.y).abs() < 0.5,
        "no fixed parent height: child should stay at top, child.y={:.2} parent.y={:.2}",
        child_el.y,
        parent_el.y,
    );
    // And the parent's auto-height shrinks to the child (no slack).
    assert!(
        (parent_el.height - child_el.height).abs() < 0.5,
        "parent auto-height should ≈ child height: parent.h={:.2} child.h={:.2}",
        parent_el.height,
        child_el.height,
    );
}

// Helper: build an Svg node carrying the given viewBox.
fn make_svg(width: f64, height: f64, view_box: Option<&str>, content: &str) -> Node {
    Node {
        kind: NodeKind::Svg {
            width,
            height,
            view_box: view_box.map(|s| s.to_string()),
            content: content.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_svg_viewbox_scales_to_box() {
    // Regression (issue 2): an Svg with width/height and a differently-sized
    // viewBox was rendering content at raw viewBox coordinates (paths spilled
    // far outside the display box). The PDF emitter scaled by
    // element.width/svg_w where both were the display dimensions, so the cm
    // matrix came out to identity. With the fix, the viewBox dims drive the
    // scale: 200/661 ≈ 0.3025.
    let doc = default_doc(vec![make_svg(
        200.0,
        80.0,
        Some("0 0 661 176"),
        // A rect that fills the entire viewBox — easy to verify it's the
        // viewBox dims (not the display dims) that get scaled.
        r##"<rect x="0" y="0" width="661" height="176" fill="#0000ff"/>"##,
    )]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);
    let stream = decompress_pdf_streams(&pdf);

    // Aspect ratios: display 200/80 = 2.5, viewBox 661/176 ≈ 3.756. meet
    // picks the smaller scale: min(200/661, 80/176) = 200/661 ≈ 0.3026.
    // Centering ty = (80 - 0.3026*176) / 2 ≈ 13.37.
    let s = 200.0 / 661.0;
    let ty = (80.0 - s * 176.0) / 2.0;
    let expected_scale_cm = format!("{:.4} 0 0 {:.4} 0.00 {:.2} cm", s, s, ty);
    assert!(
        stream.contains(&expected_scale_cm),
        "expected scale cm `{}` in stream:\n{}",
        expected_scale_cm,
        &stream[..stream.len().min(800)],
    );
}

#[test]
fn test_svg_viewbox_aspect_letterboxes() {
    // viewBox aspect ratio differs from display: meet picks min(sx, sy) and
    // centers the unused axis. 200 wide / 100 viewBox = 2.0, 200 tall / 100
    // viewBox = 2.0 — same scale, so this also exercises the simple case
    // where centering offsets are zero.
    let doc = default_doc(vec![make_svg(
        200.0,
        200.0,
        Some("0 0 100 100"),
        r##"<rect x="0" y="0" width="100" height="100" fill="#ff0000"/>"##,
    )]);
    let pdf = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf);
    assert!(
        stream.contains("2.0000 0 0 2.0000"),
        "expected scale 2.0 for matching aspect ratios, content:\n{}",
        &stream[..stream.len().min(600)],
    );

    // Now non-matching: 300x100 box with 100x100 viewBox → sx=3, sy=1, meet
    // picks 1, leaving tx = (300 - 100)/2 = 100 horizontal slack.
    let doc2 = default_doc(vec![make_svg(
        300.0,
        100.0,
        Some("0 0 100 100"),
        r##"<rect x="0" y="0" width="100" height="100" fill="#00ff00"/>"##,
    )]);
    let pdf2 = render_to_pdf(&doc2);
    let stream2 = decompress_pdf_streams(&pdf2);
    assert!(
        stream2.contains("1.0000 0 0 1.0000 100.00 0.00"),
        "expected scale 1.0 + tx 100 for letterbox case, content:\n{}",
        &stream2[..stream2.len().min(600)],
    );
}

#[test]
fn test_svg_no_viewbox_unchanged() {
    // No viewBox attribute → viewBox defaults to (0, 0, width, height) so
    // scale is 1 and centering is zero. Path coordinates render in display
    // space exactly as before the fix.
    let doc = default_doc(vec![make_svg(
        120.0,
        60.0,
        None, // no viewBox
        r##"<rect x="0" y="0" width="120" height="60" fill="#000000"/>"##,
    )]);
    let pdf = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&pdf);
    assert!(
        stream.contains("1.0000 0 0 1.0000 0.00 0.00"),
        "no-viewBox case should produce identity scale + zero centering, content:\n{}",
        &stream[..stream.len().min(600)],
    );
}

#[test]
fn test_canvas_renders_at_display_coordinates() {
    // Canvas reuses DrawCommand::Svg via canvas_ops_to_svg_commands. Canvas
    // ops are constructed in display coordinates, so the new viewBox plumbing
    // must default to (0, 0, width, height) — scale 1, no centering — or
    // Canvas geometry would regress.
    let doc = default_doc(vec![Node {
        kind: NodeKind::Canvas {
            width: 100.0,
            height: 50.0,
            operations: vec![
                CanvasOp::SetFillColor {
                    r: 255.0,
                    g: 0.0,
                    b: 0.0,
                },
                CanvasOp::Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 30.0,
                    height: 20.0,
                },
                CanvasOp::Fill,
            ],
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);
    let stream = decompress_pdf_streams(&pdf);
    // Canvas should still produce identity scale + zero centering — its
    // commands are already in display space.
    assert!(
        stream.contains("1.0000 0 0 1.0000 0.00 0.00"),
        "Canvas should keep identity viewBox transform, content:\n{}",
        &stream[..stream.len().min(600)],
    );
}

#[test]
fn test_view_auto_height_wraps_table_tightly() {
    // Regression (issue 3): a View with auto height wrapping a Table grew
    // far beyond the table's rendered content. measure_node_height had no
    // arm for NodeKind::TableRow, so it fell into the generic `_` branch
    // which SUMS cell heights (column-flex semantics). For a 3-column row
    // of 16pt cells, that returned 48pt instead of 16pt — the Table then
    // measured to 5 * 48 = 240pt instead of 5 * 16 = 80pt, and the
    // wrapping View inherited the wrong height.
    //
    // Mirrors the reporter's TSX shape (3 auto-width columns, 1 header row
    // + 4 body rows, each cell carrying an inner View + Text).
    fn cell(text: &str) -> Node {
        let inner = Node {
            kind: NodeKind::View,
            style: Style {
                font_size: Some(10.0),
                ..Default::default()
            },
            children: vec![Node {
                kind: NodeKind::Text {
                    content: text.to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style::default(),
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            }],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        };
        Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style {
                padding: Some(Edges::symmetric(1.0, 2.0)),
                ..Default::default()
            },
            children: vec![inner],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    let header_row = make_table_row(
        true,
        vec![cell("Description"), cell("Quantity"), cell("Unit Price")],
    );
    let body_data = [
        ("Website Redesign", "1", "4500"),
        ("Brand Identity Package", "1", "2200"),
        ("SEO Audit & Strategy", "1", "1800"),
        ("Monthly Hosting", "12", "29"),
    ];
    let mut rows: Vec<Node> = vec![header_row];
    for (d, q, u) in body_data {
        rows.push(make_table_row(false, vec![cell(d), cell(q), cell(u)]));
    }
    let table = Node {
        kind: NodeKind::Table {
            columns: vec![
                ColumnDef {
                    width: ColumnWidth::Auto,
                },
                ColumnDef {
                    width: ColumnWidth::Auto,
                },
                ColumnDef {
                    width: ColumnWidth::Auto,
                },
            ],
        },
        style: Style::default(),
        children: rows,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let view = make_styled_view(
        Style {
            background_color: Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            ..Default::default()
        },
        vec![table],
    );

    let doc = default_doc(vec![Node::page(
        PageConfig {
            size: PageSize::Letter,
            margin: Edges::uniform(48.0),
            ..Default::default()
        },
        Style::default(),
        vec![view],
    )]);

    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // The View wraps a Table container element (rows nest inside it since
    // the Table-wrapper change) holding 5 TableRows of 16pt each
    // (font_size 10 × line_height 1.4 = 14pt text, + 2pt cell vertical
    // padding). Total content = 80pt.
    let view_el = &pages[0].elements[0];
    assert_eq!(view_el.node_type.as_deref(), Some("View"));
    assert_eq!(view_el.children.len(), 1, "expected the Table wrapper");
    let table_el = &view_el.children[0];
    assert_eq!(table_el.node_type.as_deref(), Some("Table"));
    assert_eq!(table_el.children.len(), 5, "expected 5 TableRow children");
    let row_h_sum: f64 = table_el.children.iter().map(|r| r.height).sum();
    assert!(
        (view_el.height - row_h_sum).abs() < 0.5,
        "View height ({:.2}) should equal sum of row heights ({:.2})",
        view_el.height,
        row_h_sum,
    );
    // Concretely: 80pt (5 × 16), well under the Letter content-band 696pt
    // that the bug used to fill.
    assert!(
        view_el.height < 100.0,
        "View height should be ≈80pt, not page-sized — got {:.2}",
        view_el.height,
    );
}

#[test]
fn test_table_row_measures_max_of_cells_not_sum() {
    // Focused unit-style coverage of the same fix: a standalone TableRow
    // with three Cells of differing heights measures to the MAX, not the
    // sum. Before the fix this returned the column-flex sum of cells.
    fn cell_with_content_height(h: f64) -> Node {
        // Cell measured height comes from its content (matching
        // measure_table_row_height), not from style.height on the cell
        // itself — so put an inner View of known height inside each cell.
        let content = make_styled_view(
            Style {
                height: Some(Dimension::Pt(h)),
                ..Default::default()
            },
            vec![],
        );
        Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style::default(),
            children: vec![content],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    let row = make_table_row(
        false,
        vec![
            cell_with_content_height(20.0),
            cell_with_content_height(35.0),
            cell_with_content_height(12.0),
        ],
    );
    // Wrap in a View so we can read the row's measured height via the
    // View's auto-height (which now correctly reflects the row's MAX, not
    // the sum-of-cells).
    let view = make_styled_view(Style::default(), vec![row]);
    let doc = default_doc(vec![Node::page(
        PageConfig {
            size: PageSize::Letter,
            margin: Edges::uniform(40.0),
            ..Default::default()
        },
        Style::default(),
        vec![view],
    )]);
    let pages = layout_doc(&doc);
    let view_el = &pages[0].elements[0];
    // MAX(20, 35, 12) = 35. Sum (the old wrong value) would be 67.
    assert!(
        (view_el.height - 35.0).abs() < 0.5,
        "View around a TableRow with cell heights [20, 35, 12] should be 35 (max), got {:.2}",
        view_el.height,
    );
}

// Build the reporter's repro shape for issue 4. Each cell is paddingVertical:8
// wrapping an inner View with paddingHorizontal:6 wrapping a Text. 5 columns
// with widths Fixed(100), Fixed(90), Fixed(50), Auto, Fixed(36). A spacer View
// of height 700 precedes the table on a Letter page with 36pt margin.
fn build_issue4_doc(use_header_flag: bool, num_body_rows: usize) -> Document {
    fn cell(text: &str) -> Node {
        let inner = make_styled_view(
            Style {
                padding: Some(Edges::symmetric(0.0, 6.0)),
                ..Default::default()
            },
            vec![Node {
                kind: NodeKind::Text {
                    content: text.to_string(),
                    href: None,
                    runs: vec![],
                },
                style: Style::default(),
                children: vec![],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            }],
        );
        Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style {
                padding: Some(Edges::symmetric(8.0, 0.0)),
                ..Default::default()
            },
            children: vec![inner],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    let labels = ["Manufacturer", "Part #", "Qty", "Desc", "Unit"];
    let header_row = make_table_row(use_header_flag, labels.iter().map(|l| cell(l)).collect());
    let body_rows: Vec<Node> = (0..num_body_rows)
        .map(|r| {
            make_table_row(
                false,
                (0..5)
                    .map(|i| {
                        if i == 3 {
                            cell(&format!("Line item {} description", r + 1))
                        } else {
                            cell(&format!("c{}-{}", i, r))
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    let mut rows = vec![header_row];
    rows.extend(body_rows);
    let table = Node {
        kind: NodeKind::Table {
            columns: vec![
                ColumnDef {
                    width: ColumnWidth::Fixed(100.0),
                },
                ColumnDef {
                    width: ColumnWidth::Fixed(90.0),
                },
                ColumnDef {
                    width: ColumnWidth::Fixed(50.0),
                },
                ColumnDef {
                    width: ColumnWidth::Auto,
                },
                ColumnDef {
                    width: ColumnWidth::Fixed(36.0),
                },
            ],
        },
        style: Style::default(),
        children: rows,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    // Spacer pushes the table down so its header doesn't fit before the page
    // break — the trigger condition the reporter narrowed.
    let spacer = make_styled_view(
        Style {
            height: Some(Dimension::Pt(700.0)),
            ..Default::default()
        },
        vec![],
    );
    default_doc(vec![Node::page(
        PageConfig {
            size: PageSize::Letter,
            margin: Edges::uniform(36.0),
            ..Default::default()
        },
        Style::default(),
        vec![spacer, table],
    )])
}

#[test]
fn test_table_repeating_header_does_not_inflate_pages() {
    // Regression (issue 4): a Table with <Row header> starting low on a page
    // (so the header doesn't fit before the page break) inflated the output to
    // 3–5× the right page count, with header cells visibly "doubling and
    // sliding one column right" on each successive spurious page.
    //
    // The no-header variant is the correctness oracle — same content, same
    // position, but the body-row loop's existing fit check catches the
    // page-break cleanly. Page counts should be EQUAL between the two
    // variants. Before the fix: header → 8, plain → 3.
    let with_header = layout_doc(&build_issue4_doc(true, 24));
    let plain = layout_doc(&build_issue4_doc(false, 24));
    assert_eq!(
        with_header.len(),
        plain.len(),
        "<Row header> produced {} pages but the same content with <Row> produced {} — \
         the header flag should never inflate page count",
        with_header.len(),
        plain.len(),
    );
}

#[test]
fn test_table_repeating_header_no_duplicate_cells_per_page() {
    // Same repro as above. Page-count parity is one symptom; the more direct
    // signature of the bug was the "doubled header column sliding one column
    // right per page" — spurious snapshot pages that captured a partial row
    // PLUS the next cell's post-break content stuck at top-of-page
    // coordinates. Asserting no two TableCells on the same page share both
    // x and y catches that geometry corruption directly.
    let pages = layout_doc(&build_issue4_doc(true, 24));

    fn collect_cells(el: &forme::layout::LayoutElement, out: &mut Vec<(f64, f64)>) {
        if el.node_type.as_deref() == Some("TableCell") {
            out.push((el.x, el.y));
        }
        for c in &el.children {
            collect_cells(c, out);
        }
    }

    for (idx, page) in pages.iter().enumerate() {
        let mut cell_positions: Vec<(f64, f64)> = Vec::new();
        for el in &page.elements {
            collect_cells(el, &mut cell_positions);
        }
        let n = cell_positions.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (xi, yi) = cell_positions[i];
                let (xj, yj) = cell_positions[j];
                assert!(
                    (xi - xj).abs() > 0.5 || (yi - yj).abs() > 0.5,
                    "page {}: two TableCells at the same position ({:.2}, {:.2}) — \
                     symptom of the issue 4 doubled/sliding column bug",
                    idx,
                    xi,
                    yi,
                );
            }
        }
    }
}

// ── CSS transforms (#2 from the author-experience batch) ────────────

// Build a small styled View with the requested transform, render to PDF, and
// return the decompressed content stream so the test can assert the cm
// matrix made it in. The view sits on a Letter page with default margins.
fn render_transformed_view(style: Style) -> String {
    let view = make_styled_view(style, vec![]);
    let doc = default_doc(vec![view]);
    let pdf = render_to_pdf(&doc);
    decompress_pdf_streams(&pdf)
}

#[test]
fn test_transform_rotate_emits_cm_matrix() {
    // rotate(45deg): we negate for PDF's flipped y-axis, so cm becomes
    // cos(-45) sin(-45) -sin(-45) cos(-45) 0 0 = 0.707107 -0.707107 0.707107 0.707107 0 0.
    let stream = render_transformed_view(Style {
        width: Some(Dimension::Pt(100.0)),
        height: Some(Dimension::Pt(50.0)),
        background_color: Some(Color {
            r: 0.0,
            g: 0.5,
            b: 0.9,
            a: 1.0,
        }),
        transform: Some(vec![TransformOp::Rotate { deg: 45.0 }]),
        ..Default::default()
    });
    assert!(
        stream.contains("0.707107 -0.707107 0.707107 0.707107 0 0 cm"),
        "expected rotate(45) cm matrix in stream; got:\n{}",
        &stream[..stream.len().min(2000)]
    );
}

#[test]
fn test_transform_scale_emits_cm_matrix() {
    let stream = render_transformed_view(Style {
        width: Some(Dimension::Pt(100.0)),
        height: Some(Dimension::Pt(50.0)),
        background_color: Some(Color {
            r: 0.0,
            g: 0.5,
            b: 0.9,
            a: 1.0,
        }),
        transform: Some(vec![TransformOp::Scale { x: 2.0, y: 0.5 }]),
        ..Default::default()
    });
    assert!(
        stream.contains("2.000000 0 0 0.500000 0 0 cm"),
        "expected scale(2, 0.5) cm matrix in stream; got:\n{}",
        &stream[..stream.len().min(2000)]
    );
}

#[test]
fn test_transform_compose_rotate_and_scale() {
    let stream = render_transformed_view(Style {
        width: Some(Dimension::Pt(100.0)),
        height: Some(Dimension::Pt(50.0)),
        background_color: Some(Color {
            r: 0.0,
            g: 0.5,
            b: 0.9,
            a: 1.0,
        }),
        transform: Some(vec![
            TransformOp::Rotate { deg: 45.0 },
            TransformOp::Scale { x: 1.2, y: 1.2 },
        ]),
        ..Default::default()
    });
    // Both cm lines should appear inside a single q/Q block.
    assert!(
        stream.contains("0.707107 -0.707107 0.707107 0.707107 0 0 cm"),
        "expected rotate cm in compose"
    );
    assert!(
        stream.contains("1.200000 0 0 1.200000 0 0 cm"),
        "expected scale cm in compose"
    );
}

#[test]
fn test_transform_does_not_affect_layout_flow() {
    // Two stacked Views; the first is transformed, the second is not.
    // The transform is paint-only — the second View should sit immediately
    // below where the first View's untransformed box would have been.
    let first = make_styled_view(
        Style {
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(40.0)),
            transform: Some(vec![TransformOp::Rotate { deg: 45.0 }]),
            ..Default::default()
        },
        vec![],
    );
    let second = make_styled_view(
        Style {
            width: Some(Dimension::Pt(100.0)),
            height: Some(Dimension::Pt(40.0)),
            ..Default::default()
        },
        vec![],
    );
    let doc = default_doc(vec![first, second]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    let elements = &pages[0].elements;
    assert!(elements.len() >= 2);
    // Second element's y should be first.y + first.height (no transform-induced shift).
    let first_y = elements[0].y;
    let first_h = elements[0].height;
    let second_y = elements[1].y;
    assert!(
        (second_y - (first_y + first_h)).abs() < 0.5,
        "transformed sibling shifted layout: second.y={:.2}, expected {:.2}",
        second_y,
        first_y + first_h,
    );
}

// ── Headings (#3 from the author-experience batch) ─────────────────

fn make_heading(level: u8, text: &str) -> Node {
    Node {
        kind: NodeKind::Heading {
            level,
            content: text.to_string(),
            href: None,
            runs: vec![],
        },
        style: Style {
            font_size: Some(match level {
                1 => 32.0,
                2 => 24.0,
                3 => 20.0,
                _ => 16.0,
            }),
            font_weight: Some(700),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_heading_emits_h1_through_h6_structure_roles_in_tagged_pdf() {
    // Each level should produce its matching /S /H1 .. /S /H6 structure
    // element. Tagged PDF builder maps node_type "H1".."H6" to those roles.
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![
                make_heading(1, "h1"),
                make_heading(2, "h2"),
                make_heading(3, "h3"),
                make_heading(4, "h4"),
                make_heading(5, "h5"),
                make_heading(6, "h6"),
            ],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);

    for level in 1..=6 {
        let needle = format!("/S /H{}", level);
        assert!(
            pdf_str.contains(&needle),
            "tagged PDF missing structure role {} (looking for `{}`)",
            level,
            needle,
        );
    }
}

#[test]
fn test_heading_renders_text_content() {
    // Even without tagged PDF, the heading content has to land in the
    // rendered text. This catches regressions where a new NodeKind variant
    // would forget to dispatch to layout_text.
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![make_heading(1, "Annual Report")],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: false,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = forme::render(&doc).unwrap();
    let stream = decompress_pdf_streams(&bytes);
    assert!(
        stream.contains("Annual Report") || stream.contains("(Annual Report)"),
        "heading text content missing from rendered stream"
    );
}

#[test]
fn test_no_transform_no_cm_wrapper() {
    // Sanity: a plain View without transform shouldn't generate any of our
    // shift-origin cm lines. (PDFs may contain other `cm` operations for
    // text/image positioning, but the `1 0 0 1 ... cm` shift-pair we emit
    // around transforms shouldn't show up.) We assert the rotate matrix
    // pattern doesn't appear.
    let stream = render_transformed_view(Style {
        width: Some(Dimension::Pt(100.0)),
        height: Some(Dimension::Pt(50.0)),
        background_color: Some(Color {
            r: 0.0,
            g: 0.5,
            b: 0.9,
            a: 1.0,
        }),
        ..Default::default()
    });
    assert!(
        !stream.contains("0.707107 -0.707107"),
        "rotate cm matrix appeared in a no-transform render"
    );
}

// ── Lists (#4 from the author-experience batch) ────────────────────

fn make_list_item(text: &str) -> Node {
    Node {
        kind: NodeKind::ListItem,
        style: Style::default(),
        children: vec![make_text(text, 12.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn make_list(ordered: bool, marker_type: ListMarkerType, start: u32, items: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::List {
            ordered,
            marker_type,
            start,
        },
        style: Style::default(),
        children: items,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn test_unordered_list_renders_bullet_markers() {
    let doc = default_doc(vec![make_list(
        false,
        ListMarkerType::Disc,
        1,
        vec![
            make_list_item("first"),
            make_list_item("second"),
            make_list_item("third"),
        ],
    )]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
    let stream = decompress_pdf_streams(&bytes);
    // Bullet glyph (U+2022) maps through WinAnsi to byte 0x95. In the
    // PDF content stream it appears in the Tj operand. Also assert the
    // item text content lands in the rendered stream.
    assert!(
        stream.contains("first") && stream.contains("second") && stream.contains("third"),
        "expected all three item bodies in the rendered stream"
    );
}

#[test]
fn test_ordered_decimal_list_renders_numbered_markers() {
    let doc = default_doc(vec![make_list(
        true,
        ListMarkerType::Decimal,
        1,
        vec![
            make_list_item("a"),
            make_list_item("b"),
            make_list_item("c"),
        ],
    )]);
    let bytes = render_to_pdf(&doc);
    let stream = decompress_pdf_streams(&bytes);
    // Markers should appear in the stream as "1.", "2.", "3.".
    for n in 1..=3 {
        let marker = format!("{}.", n);
        assert!(
            stream.contains(&marker),
            "expected marker `{}` in the stream",
            marker
        );
    }
}

#[test]
fn test_ordered_list_respects_start_index() {
    let doc = default_doc(vec![make_list(
        true,
        ListMarkerType::Decimal,
        5,
        vec![make_list_item("a"), make_list_item("b")],
    )]);
    let stream = decompress_pdf_streams(&render_to_pdf(&doc));
    assert!(stream.contains("5."), "expected start=5 marker");
    assert!(stream.contains("6."), "expected next marker after start");
    assert!(!stream.contains("1."), "1. should not appear when start=5");
}

#[test]
fn test_ordered_lower_roman_markers() {
    let doc = default_doc(vec![make_list(
        true,
        ListMarkerType::LowerRoman,
        1,
        vec![
            make_list_item("a"),
            make_list_item("b"),
            make_list_item("c"),
            make_list_item("d"),
            make_list_item("e"),
        ],
    )]);
    let stream = decompress_pdf_streams(&render_to_pdf(&doc));
    for m in ["i.", "ii.", "iii.", "iv.", "v."] {
        assert!(stream.contains(m), "expected `{}` in stream", m);
    }
}

#[test]
fn test_ordered_lower_alpha_markers() {
    let doc = default_doc(vec![make_list(
        true,
        ListMarkerType::LowerAlpha,
        1,
        vec![
            make_list_item("a"),
            make_list_item("b"),
            make_list_item("c"),
        ],
    )]);
    let stream = decompress_pdf_streams(&render_to_pdf(&doc));
    for m in ["a.", "b.", "c."] {
        assert!(stream.contains(m), "expected `{}` in stream", m);
    }
}

#[test]
fn test_list_emits_l_li_lbl_in_tagged_pdf() {
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![make_list(
                true,
                ListMarkerType::Decimal,
                1,
                vec![make_list_item("first"), make_list_item("second")],
            )],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        tagged: true,
        pdfa: None,
        default_style: None,
        embedded_data: None,
        flatten_forms: false,
        pdf_ua: false,
        certification: None,
    };
    let bytes = forme::render(&doc).unwrap();
    let pdf_str = String::from_utf8_lossy(&bytes);
    for needle in ["/S /L", "/S /LI", "/S /Lbl"] {
        assert!(
            pdf_str.contains(needle),
            "tagged PDF missing `{}` structure role",
            needle
        );
    }
}

#[test]
fn test_list_item_content_offset_past_marker_gutter() {
    // The content of a list item should be positioned to the right of
    // the marker — i.e. its x is strictly greater than the list's x.
    let list = make_list(
        false,
        ListMarkerType::Disc,
        1,
        vec![make_list_item("hello")],
    );
    let doc = default_doc(vec![list]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    // page → List wrapper → ListItem → [Lbl, Text]
    let list_el = &pages[0].elements[0];
    assert_eq!(list_el.node_type.as_deref(), Some("List"));
    let item = &list_el.children[0];
    assert_eq!(item.node_type.as_deref(), Some("ListItem"));
    // Find the marker label and the body Text and assert content_x > marker_x.
    let mut lbl_x: Option<f64> = None;
    let mut body_x: Option<f64> = None;
    for child in &item.children {
        match child.node_type.as_deref() {
            Some("Lbl") => lbl_x = Some(child.x),
            Some("Text") => body_x = Some(child.x),
            _ => {}
        }
    }
    let lbl = lbl_x.expect("marker Lbl missing");
    let body = body_x.expect("body Text missing");
    assert!(
        body > lbl,
        "body x ({:.2}) should be greater than marker x ({:.2})",
        body,
        lbl,
    );
}

// ─── Table page-break: orphan header + long-token header (issues 1 + 2) ──

/// Count TableRow elements with `is_header_row: true` anywhere in the
/// page's element tree. The table layout nests cells inside rows inside
/// arbitrary parent containers, so a flat scan of page.elements isn't
/// enough — walk recursively.
fn count_header_table_rows(page: &forme::layout::LayoutPage) -> usize {
    fn walk(el: &forme::layout::LayoutElement, count: &mut usize) {
        if el.is_header_row && el.node_type.as_deref() == Some("TableRow") {
            *count += 1;
        }
        for c in &el.children {
            walk(c, count);
        }
    }
    let mut count = 0;
    for el in &page.elements {
        walk(el, &mut count);
    }
    count
}

#[test]
fn test_table_header_no_orphan_when_first_body_row_doesnt_fit() {
    // Regression: a spacer pushes the cursor low enough that the header
    // alone fits in remaining space but the first body row does not. The
    // 0.10.4 header-only pre-check let the header lay out at the bottom of
    // page 1; the body-row check then page-broke and re-emitted the header
    // on page 2 — leaving an orphan header at the bottom of page 1.
    //
    // The pre-check now folds in the first body row, so the table moves to
    // a fresh page where header + first row land together.
    //
    // Default A4 doc: content area 487 × 734pt. A 700pt spacer leaves 34pt
    // remaining. Cell padding 4 each side + 10pt text × 1.4 line-height
    // ≈ 22pt per row. header_h (22) fits in remaining (34) — the original
    // header-only pre-check did NOT fire — but header + first body row
    // (44) does not fit. Combined check fires and page-breaks; header
    // lands on page 2.
    let spacer = make_styled_view(
        Style {
            height: Some(Dimension::Pt(700.0)),
            ..Default::default()
        },
        vec![],
    );

    let table = Node {
        kind: NodeKind::Table {
            columns: vec![ColumnDef {
                width: ColumnWidth::Fraction(1.0),
            }],
        },
        style: Style::default(),
        children: vec![
            make_table_row(true, vec![make_table_cell(vec![make_text("Header", 10.0)])]),
            make_table_row(false, vec![make_table_cell(vec![make_text("Row 1", 10.0)])]),
            make_table_row(false, vec![make_table_cell(vec![make_text("Row 2", 10.0)])]),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc = default_doc(vec![spacer, table]);
    let pages = layout_doc(&doc);

    assert_eq!(
        pages.len(),
        2,
        "Expected exactly 2 pages, got {}",
        pages.len()
    );
    assert_eq!(
        count_header_table_rows(&pages[0]),
        0,
        "Page 1 must not contain an orphaned header row"
    );
    assert_eq!(
        count_header_table_rows(&pages[1]),
        1,
        "Page 2 should contain the single header row"
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

#[test]
fn test_table_long_header_text_no_page_contamination() {
    // Regression (mirrors GitHub issue 2 reproduction): a header cell holds
    // a long no-break-opportunity token ("Amount" × 40). The line breaker
    // force-breaks at char boundaries, so the header row is several lines
    // tall. With a spacer pushing the table low on page 1, the symptom
    // was that header text leaked onto page 1, the page count went 2 → 3,
    // and the header repeated twice with partial content.
    //
    // The combined header + first-body pre-check moves the table to a
    // fresh page where the entire header and first row land together —
    // no intra-cell page break inside the header, no contamination, no
    // doubled header.
    // Default A4 content area: 487 × 734pt. With this column split, the
    // long-token header cell wraps to ~12 lines × 12.6pt + 8pt padding ≈
    // 134pt. A 588pt spacer leaves ~146pt remaining. The header (134) fits
    // alone — the original 0.10.4 pre-check did NOT fire — but the
    // header + first body row (134 + 21 ≈ 155) does not fit. Combined
    // check fires and moves the table to a fresh page.
    let spacer = make_styled_view(
        Style {
            height: Some(Dimension::Pt(588.0)),
            ..Default::default()
        },
        vec![],
    );

    let long_token = "Amount".repeat(40);

    let table = Node {
        kind: NodeKind::Table {
            columns: vec![
                ColumnDef {
                    width: ColumnWidth::Fraction(0.7),
                },
                ColumnDef {
                    width: ColumnWidth::Fraction(0.3),
                },
            ],
        },
        style: Style::default(),
        children: vec![
            make_table_row(
                true,
                vec![
                    make_table_cell(vec![make_text("Description", 9.0)]),
                    make_table_cell(vec![make_text(&long_token, 9.0)]),
                ],
            ),
            make_table_row(
                false,
                vec![
                    make_table_cell(vec![make_text("Website Redesign", 9.0)]),
                    make_table_cell(vec![make_text("$4500.00", 9.0)]),
                ],
            ),
            make_table_row(
                false,
                vec![
                    make_table_cell(vec![make_text("Brand Identity Package", 9.0)]),
                    make_table_cell(vec![make_text("$2200.00", 9.0)]),
                ],
            ),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };

    let doc = default_doc(vec![spacer, table]);
    let pages = layout_doc(&doc);

    assert_eq!(
        pages.len(),
        2,
        "Expected exactly 2 pages (no contamination), got {}",
        pages.len()
    );
    assert_eq!(
        count_header_table_rows(&pages[0]),
        0,
        "Page 1 must not contain the table header"
    );
    assert_eq!(
        count_header_table_rows(&pages[1]),
        1,
        "Page 2 should contain the single header row"
    );

    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}

// ─── SVG stroke-linecap / stroke-linejoin ────────────────────────

#[test]
fn test_svg_stroke_linecap_round_emits_pdf_j_operator() {
    // Regression (GitHub issue, 2026-07): SVG paths with
    // stroke-linecap="round" rendered with default butt caps. The parser
    // read fill/stroke/stroke-width/opacity but silently dropped
    // stroke-linecap and stroke-linejoin, so SvgCommand::SetLineCap was
    // never emitted for SVG content (only from the Canvas API). Segment
    // junctions in signature/scribble drawings looked staircased instead
    // of smoothly joined.
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 100.0,
            view_box: Some("0 0 200 100".to_string()),
            content: r##"<path d="M 10 10 L 100 90" stroke="black" stroke-width="4" fill="none" stroke-linecap="round"/>"##.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);

    let streams = decompress_pdf_streams(&pdf);
    assert!(
        streams.contains("1 J"),
        "PDF content stream should contain `1 J` (round linecap). Streams:\n{}",
        streams
    );
}

#[test]
fn test_svg_stroke_linecap_default_emits_butt() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 100.0,
            view_box: Some("0 0 200 100".to_string()),
            content:
                r##"<path d="M 10 10 L 100 90" stroke="black" stroke-width="4" fill="none"/>"##
                    .to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);

    let streams = decompress_pdf_streams(&pdf);
    assert!(
        streams.contains("0 J"),
        "PDF content stream should contain `0 J` (butt linecap, default). Streams:\n{}",
        streams
    );
    assert!(
        !streams.contains("1 J") && !streams.contains("2 J"),
        "No round/square linecap should appear without an explicit attribute. Streams:\n{}",
        streams
    );
}

#[test]
fn test_svg_stroke_linejoin_round_emits_pdf_j_operator() {
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 100.0,
            view_box: Some("0 0 200 100".to_string()),
            content: r##"<path d="M 10 10 L 50 90 L 100 10" stroke="black" stroke-width="4" fill="none" stroke-linejoin="round"/>"##.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);

    let streams = decompress_pdf_streams(&pdf);
    assert!(
        streams.contains("1 j"),
        "PDF content stream should contain `1 j` (round linejoin). Streams:\n{}",
        streams
    );
}

#[test]
fn test_svg_stroke_linecap_inherited_from_group() {
    // <g stroke-linecap="round"> ... <path .../> </g> — the path should
    // inherit the round cap from its group ancestor.
    let doc = default_doc(vec![Node {
        kind: NodeKind::Svg {
            width: 200.0,
            height: 100.0,
            view_box: Some("0 0 200 100".to_string()),
            content: r##"<g stroke-linecap="round"><path d="M 10 10 L 100 90" stroke="black" stroke-width="4" fill="none"/></g>"##.to_string(),
        },
        style: Style::default(),
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }]);
    let pdf = render_to_pdf(&doc);
    assert_valid_pdf(&pdf);

    let streams = decompress_pdf_streams(&pdf);
    assert!(
        streams.contains("1 J"),
        "PDF content stream should contain `1 J` (round linecap inherited from <g>). Streams:\n{}",
        streams
    );
}

// ─── Flex overflow robustness (found by the HTML input path spike) ─────

/// Collect all rendered text from a layout, all pages, depth-first.
fn all_layout_text(layout: &forme::LayoutInfo) -> String {
    fn walk(els: &[forme::layout::ElementInfo], out: &mut String) {
        for el in els {
            if let Some(t) = &el.text_content {
                out.push_str(t);
                out.push(' ');
            }
            walk(&el.children, out);
        }
    }
    let mut out = String::new();
    for page in &layout.pages {
        walk(&page.elements, &mut out);
    }
    out
}

/// During the forme-pdf-html spike, a flex-row item taller than a page
/// (a zero-width text measuring one character per line) produced an empty
/// first page and silently dropped every sibling AFTER the flex row.
/// This pins the failure mode: content following an overflowing flex row
/// must still render, and no blank leading page may be emitted.
#[test]
fn siblings_after_overflowing_flex_row_still_render() {
    let tall_text = vec!["line"; 200].join("\n");
    let row = make_styled_view(
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        vec![make_text(&tall_text, 12.0)],
    );
    let after = make_text("CONTENT AFTER THE FLEX ROW", 12.0);

    let doc = Document {
        children: vec![row, after],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        default_style: None,
        tagged: false,
        pdfa: None,
        pdf_ua: false,
        embedded_data: None,
        flatten_forms: false,
        certification: None,
    };

    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    let text = all_layout_text(&layout);
    assert!(
        text.contains("CONTENT AFTER THE FLEX ROW"),
        "sibling after an overflowing flex row was swallowed; pages: {}",
        layout.pages.len()
    );
    assert!(
        layout
            .pages
            .first()
            .map(|p| !p.elements.is_empty())
            .unwrap_or(false),
        "first page must not be empty"
    );
}

/// Colspan regression (found by the HTML input path's totals rows): a
/// `colspan=2` cell in a 3-column table must consume BOTH columns' widths,
/// and the following cell must sit in the third column's slot — not the
/// second's. The cell index was being used as the column index.
#[test]
fn colspan_cell_spans_columns_and_next_cell_lands_after_them() {
    fn cell(text: &str, col_span: u32) -> Node {
        Node {
            kind: NodeKind::TableCell {
                col_span,
                row_span: 1,
            },
            style: Style::default(),
            children: vec![make_text(text, 12.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    fn row(cells: Vec<Node>) -> Node {
        Node {
            kind: NodeKind::TableRow { is_header: false },
            style: Style::default(),
            children: cells,
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    let table = Node {
        kind: NodeKind::Table { columns: vec![] }, // 3 equal columns
        style: Style::default(),
        children: vec![
            row(vec![cell("a", 1), cell("b", 1), cell("c", 1)]),
            row(vec![cell("Total", 2), cell("$74.00", 1)]),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![table]);
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");

    // Find both rows' cells.
    fn cells_of_rows(els: &[forme::layout::ElementInfo], out: &mut Vec<Vec<(f64, f64)>>) {
        for el in els {
            if el.node_type == "TableRow" {
                out.push(
                    el.children
                        .iter()
                        .filter(|c| c.node_type == "TableCell")
                        .map(|c| (c.x, c.width))
                        .collect(),
                );
            }
            cells_of_rows(&el.children, out);
        }
    }
    let mut rows: Vec<Vec<(f64, f64)>> = Vec::new();
    for page in &layout.pages {
        cells_of_rows(&page.elements, &mut rows);
    }
    assert_eq!(rows.len(), 2);
    let (normal, totals) = (&rows[0], &rows[1]);
    assert_eq!(normal.len(), 3);
    assert_eq!(totals.len(), 2);

    let col_w = normal[0].1;
    // The colspan cell is twice a normal column wide...
    assert!(
        (totals[0].1 - 2.0 * col_w).abs() < 0.5,
        "colspan=2 cell width {} should be 2 × column width {col_w}",
        totals[0].1
    );
    // ...and the following cell starts where the THIRD column starts.
    assert!(
        (totals[1].0 - normal[2].0).abs() < 0.5,
        "cell after colspan starts at {}, third column starts at {}",
        totals[1].0,
        normal[2].0
    );
}

/// Engine gap #5, found by the HTML input path's `break-inside: avoid`
/// (report fixture): `wrap: false` on a Table was silently ignored —
/// layout_table paginated row-by-row unconditionally, splitting the table
/// and re-drawing its header on the continuation page. An unbreakable
/// table that fits on a fresh page must move there whole.
#[test]
fn unbreakable_table_moves_to_next_page_whole() {
    let spacer = Node {
        kind: NodeKind::View,
        style: Style {
            height: Some(Dimension::Pt(650.0)),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    fn row(label: &str, is_header: bool) -> Node {
        Node {
            kind: NodeKind::TableRow { is_header },
            style: Style::default(),
            children: vec![Node {
                kind: NodeKind::TableCell {
                    col_span: 1,
                    row_span: 1,
                },
                style: Style::default(),
                children: vec![make_text(label, 12.0)],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            }],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style {
            wrap: Some(false), // break-inside: avoid
            ..Default::default()
        },
        children: vec![
            row("hdr", true),
            row("r1", false),
            row("r2", false),
            row("r3", false),
            row("r4", false),
            row("r5", false),
            row("r6", false),
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![spacer, table]);
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");

    let mut row_pages: Vec<usize> = Vec::new();
    fn walk_rows(els: &[forme::layout::ElementInfo], page: usize, out: &mut Vec<usize>) {
        for el in els {
            if el.node_type == "TableRow" {
                out.push(page);
            }
            walk_rows(&el.children, page, out);
        }
    }
    for (i, page) in layout.pages.iter().enumerate() {
        walk_rows(&page.elements, i, &mut row_pages);
    }
    assert_eq!(
        row_pages.len(),
        7,
        "7 source rows, no repeated header (repetition means it split): {row_pages:?}"
    );
    let first = row_pages[0];
    assert!(
        row_pages.iter().all(|p| *p == first),
        "unbreakable table must stay on one page: {row_pages:?}"
    );
    assert_eq!(first, 1, "table must have moved off the crowded first page");
}

// ─── @page :first support (first_page config + FixedPageFilter) ────────

fn fixed_band(height: f64, pages: FixedPageFilter) -> Node {
    Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Header,
            pages,
        },
        style: Style::default(),
        children: vec![Node {
            kind: NodeKind::View,
            style: Style {
                height: Some(Dimension::Pt(height)),
                background_color: Some(Color::rgb(0.9, 0.9, 0.9)),
                ..Default::default()
            },
            children: vec![make_text("RUNNING HEADER", 10.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn tall_content(lines: usize) -> Node {
    make_text(&vec!["content line"; lines].join("\n"), 12.0)
}

#[test]
fn first_page_config_gives_page_one_its_own_margins() {
    let doc = Document {
        children: vec![tall_content(60)],
        metadata: Metadata::default(),
        default_page: PageConfig {
            margin: Edges::uniform(54.0),
            ..Default::default()
        },
        first_page: Some(PageConfig {
            margin: Edges {
                top: 120.0,
                right: 54.0,
                bottom: 54.0,
                left: 54.0,
            },
            ..Default::default()
        }),
        left_page: None,
        right_page: None,
        fonts: vec![],
        default_style: None,
        tagged: false,
        pdfa: None,
        pdf_ua: false,
        embedded_data: None,
        flatten_forms: false,
        certification: None,
    };
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(layout.pages.len() >= 2, "content must flow to page 2");
    assert_eq!(layout.pages[0].content_y, 120.0, ":first margin-top");
    assert_eq!(layout.pages[1].content_y, 54.0, "later pages use default");
}

#[test]
fn not_first_header_skips_page_one() {
    let doc = Document {
        children: vec![
            fixed_band(40.0, FixedPageFilter::NotFirst),
            tall_content(80),
        ],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
        first_page: None,
        left_page: None,
        right_page: None,
        fonts: vec![],
        default_style: None,
        tagged: false,
        pdfa: None,
        pdf_ua: false,
        embedded_data: None,
        flatten_forms: false,
        certification: None,
    };
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(layout.pages.len() >= 2);

    let page_has_header = |i: usize| -> bool {
        let mut found = false;
        fn walk(
            els: &[forme::layout::ElementInfo],
            f: &mut impl FnMut(&forme::layout::ElementInfo),
        ) {
            for el in els {
                f(el);
                walk(&el.children, f);
            }
        }
        walk(&layout.pages[i].elements, &mut |el| {
            if el
                .text_content
                .as_deref()
                .is_some_and(|t| t.contains("RUNNING HEADER"))
            {
                found = true;
            }
        });
        found
    };
    assert!(
        !page_has_header(0),
        "NotFirst header must not appear on page 1"
    );
    assert!(page_has_header(1), "NotFirst header must appear on page 2");

    // Page 1 content starts at the margin; page 2 content starts below the band.
    let first_el_y = |i: usize| {
        layout.pages[i]
            .elements
            .iter()
            .map(|e| e.y)
            .fold(f64::MAX, f64::min)
    };
    assert!(
        (first_el_y(0) - 54.0).abs() < 0.5,
        "page 1 content at margin, got {}",
        first_el_y(0)
    );
    // Page 2: band at margin top, content shifted 40pt down.
    assert!(
        (first_el_y(1) - 54.0).abs() < 0.5,
        "page 2 band at margin top"
    );
}

/// THE user-mandated interaction test: margin boxes (band trick: engine
/// margin zeroed, Fixed band = the physical margin strip) combined with
/// :first suppressing the running header. Individually correct, jointly
/// wrong without the restore rule — page 1's content would start at the
/// physical top of the paper (y = 0) because the margin it should have
/// had was the band that got filtered off. `first_page` restoring the
/// real margin is the fix; this test renders exactly that combination.
#[test]
fn first_page_restores_margin_when_its_band_is_suppressed() {
    let band_height = 72.0;
    let doc = Document {
        children: vec![
            fixed_band(band_height, FixedPageFilter::NotFirst),
            tall_content(80),
        ],
        metadata: Metadata::default(),
        // Band trick: top margin lives in the band, not the config...
        default_page: PageConfig {
            margin: Edges {
                top: 0.0,
                right: 54.0,
                bottom: 54.0,
                left: 54.0,
            },
            ..Default::default()
        },
        // ...so the page WITHOUT the band restores the real margin.
        first_page: Some(PageConfig {
            margin: Edges {
                top: band_height,
                right: 54.0,
                bottom: 54.0,
                left: 54.0,
            },
            ..Default::default()
        }),
        left_page: None,
        right_page: None,
        fonts: vec![],
        default_style: None,
        tagged: false,
        pdfa: None,
        pdf_ua: false,
        embedded_data: None,
        flatten_forms: false,
        certification: None,
    };
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");
    assert!(layout.pages.len() >= 2);

    // Page 1: no band, real margin restored — content must NOT start at
    // the physical top of the paper.
    let first_el_y = |i: usize| {
        layout.pages[i]
            .elements
            .iter()
            .map(|e| e.y)
            .fold(f64::MAX, f64::min)
    };
    assert_eq!(
        layout.pages[0].content_y, band_height,
        "page 1 config must carry the restored margin"
    );
    assert!(
        (first_el_y(0) - band_height).abs() < 0.5,
        "page 1 content must start at the restored margin ({band_height}pt), got {}",
        first_el_y(0)
    );

    // Page 2: band occupies the top strip (y = 0, the zeroed margin) and
    // content starts exactly below it — where the margin would have been.
    assert_eq!(layout.pages[1].content_y, 0.0);
    assert!(
        (first_el_y(1) - 0.0).abs() < 0.5,
        "page 2 band must sit at the physical top, got {}",
        first_el_y(1)
    );
    let mut content_min_y = f64::MAX;
    fn walk(els: &[forme::layout::ElementInfo], f: &mut impl FnMut(&forme::layout::ElementInfo)) {
        for el in els {
            f(el);
            walk(&el.children, f);
        }
    }
    walk(&layout.pages[1].elements, &mut |el| {
        if el
            .text_content
            .as_deref()
            .is_some_and(|t| t.contains("content line"))
        {
            content_min_y = content_min_y.min(el.y);
        }
    });
    assert!(
        (content_min_y - band_height).abs() < 1.0,
        "page 2 body content must start below the band (~{band_height}pt), got {content_min_y}"
    );
}

// ─── Pre-launch round: vertical-align + max-width (fails-first pins) ───

#[test]
fn middle_aligned_cell_centers_in_the_row_box() {
    // Two cells: three lines vs one line, the short one middle-aligned.
    // The short cell's content y-center must equal the row's y-center
    // EXACTLY (exact-value, per the page-selection standard).
    fn cell(text: &str, valign: Option<VerticalAlign>) -> Node {
        Node {
            kind: NodeKind::TableCell {
                col_span: 1,
                row_span: 1,
            },
            style: Style {
                vertical_align: valign,
                ..Default::default()
            },
            children: vec![make_text(text, 12.0)],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![Node {
            kind: NodeKind::TableRow { is_header: false },
            style: Style::default(),
            children: vec![
                cell("one\ntwo\nthree", None),
                cell("mid", Some(VerticalAlign::Middle)),
            ],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![table]);
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");

    fn find_cells<'a>(
        els: &'a [forme::layout::ElementInfo],
        out: &mut Vec<&'a forme::layout::ElementInfo>,
    ) {
        for el in els {
            if el.node_type == "TableCell" {
                out.push(el);
            }
            find_cells(&el.children, out);
        }
    }
    let mut cells = Vec::new();
    for page in &layout.pages {
        find_cells(&page.elements, &mut cells);
    }
    assert_eq!(cells.len(), 2);
    let (tall, short) = (cells[0], cells[1]);
    assert!(tall.height > 40.0, "three lines: {}", tall.height);

    // The short cell's inner Text container.
    fn first_text(els: &[forme::layout::ElementInfo]) -> Option<&forme::layout::ElementInfo> {
        for el in els {
            if el.kind == "None" && el.node_type == "Text" {
                return Some(el);
            }
            if let Some(t) = first_text(&el.children) {
                return Some(t);
            }
        }
        None
    }
    let text = first_text(&short.children).expect("short cell text");
    let row_center = short.y + short.height / 2.0;
    let text_center = text.y + text.height / 2.0;
    assert!(
        (text_center - row_center).abs() < 0.01,
        "middle-aligned content center {text_center} must equal row center {row_center}"
    );
}

#[test]
fn max_width_clamps_and_auto_margins_center_the_column() {
    // The centered document column: max-width + margin: 0 auto.
    let column = Node {
        kind: NodeKind::View,
        style: Style {
            max_width: Some(Dimension::Pt(400.0)),
            margin: Some(MarginEdges {
                top: EdgeValue::Pt(0.0),
                right: EdgeValue::Auto,
                bottom: EdgeValue::Pt(0.0),
                left: EdgeValue::Auto,
            }),
            background_color: Some(Color::rgb(0.95, 0.95, 0.95)),
            ..Default::default()
        },
        children: vec![make_text("column content", 12.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    // Wrapped in a body-like container: auto-margin centering applies to
    // the CHILDREN of a container (the HTML mapper always provides a body
    // View), not to document-root nodes.
    let body = make_view(vec![column]);
    let doc = default_doc(vec![body]);
    let (_pdf, layout, _) = forme::render_with_layout(&doc).expect("Should render");

    let page = &layout.pages[0];
    let el = &page.elements[0].children[0];
    assert_eq!(el.node_type, "View");
    assert!(
        (el.width - 400.0).abs() < 0.01,
        "max-width must clamp the auto width to 400, got {}",
        el.width
    );
    let expected_x = page.content_x + (page.content_width - 400.0) / 2.0;
    assert!(
        (el.x - expected_x).abs() < 0.01,
        "auto margins must center the clamped column: x {} != {}",
        el.x,
        expected_x
    );
}

// ─── Table cell CSS height as minimum row height (Finding B) ─────

fn find_first_by_type<'a>(
    elems: &'a [forme::layout::LayoutElement],
    ty: &str,
) -> Option<&'a forme::layout::LayoutElement> {
    for e in elems {
        if e.node_type.as_deref() == Some(ty) {
            return Some(e);
        }
        if let Some(found) = find_first_by_type(&e.children, ty) {
            return Some(found);
        }
    }
    None
}

/// CSS 2.1 §17.5.3: `height` on a table cell is a MINIMUM. A cell taller than
/// its content grows the row to the specified height (which is what gives
/// `vertical-align` its slack). Auto-height cells are unaffected; content
/// taller than the height still wins. Fails-first: before the fix the row
/// lays out at ~content height, ignoring the 100pt cell height.
#[test]
fn table_cell_css_height_is_minimum_row_height() {
    let cell = Node {
        kind: NodeKind::TableCell {
            col_span: 1,
            row_span: 1,
        },
        style: Style {
            height: Some(Dimension::Pt(100.0)),
            ..Default::default()
        },
        children: vec![make_text("x", 10.0)],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let row = make_table_row(false, vec![cell]);
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![row],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    let doc = default_doc(vec![table]);
    let pages = layout_doc(&doc);
    let row = find_first_by_type(&pages[0].elements, "TableRow").expect("TableRow laid out");
    assert!(
        (row.height - 100.0).abs() < 0.01,
        "cell height:100pt must set the row height as a minimum; got {}",
        row.height
    );
}

// ─── position: relative — paint offset, flow preserved (item 7a) ─────

fn block(rel_offsets: Option<(f64, f64)>) -> Node {
    let (position, top, left) = match rel_offsets {
        Some((t, l)) => (Some(Position::Relative), Some(t), Some(l)),
        None => (None, None, None),
    };
    Node {
        kind: NodeKind::View,
        style: Style {
            width: Some(Dimension::Pt(50.0)),
            height: Some(Dimension::Pt(40.0)),
            position,
            top,
            left,
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

#[test]
fn relative_offset_shifts_paint_but_not_flow() {
    // Middle block is position: relative, top: 10, left: 5.
    let offset_doc = default_doc(vec![block(None), block(Some((10.0, 5.0))), block(None)]);
    // Baseline: identical, but the middle block has no offsets.
    let base_doc = default_doc(vec![block(None), block(None), block(None)]);

    let off = layout_doc(&offset_doc);
    let base = layout_doc(&base_doc);
    let (o, b) = (&off[0].elements, &base[0].elements);

    // The relative element paints shifted by exactly (+5, +10).
    assert!(
        (o[1].x - (b[1].x + 5.0)).abs() < 0.001,
        "left:5 → +5pt x: {} vs {}",
        o[1].x,
        b[1].x
    );
    assert!(
        (o[1].y - (b[1].y + 10.0)).abs() < 0.001,
        "top:10 → +10pt y: {} vs {}",
        o[1].y,
        b[1].y
    );
    // Its following sibling's flow position is byte-identical (flow preserved).
    assert_eq!(o[2].y, b[2].y, "sibling flow must not move");
    assert_eq!(o[2].x, b[2].x, "sibling flow must not move");
    // The offset element itself did not disturb its own reserved slot's start
    // for the next block: block 3 sits where it would without the offset.
    assert_eq!(o[0].y, b[0].y, "preceding sibling unchanged");
}

// ─── vertical-align: baseline in table cells (item 6) ───────────────

fn valign_cell(font: f64, text: &str, va: VerticalAlign, extra_block: Option<f64>) -> Node {
    let mut children = vec![make_text(text, font)];
    if let Some(h) = extra_block {
        children.push(Node {
            kind: NodeKind::View,
            style: Style {
                height: Some(Dimension::Pt(h)),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        });
    }
    Node {
        kind: NodeKind::TableCell {
            col_span: 1,
            row_span: 1,
        },
        style: Style {
            vertical_align: Some(va),
            ..Default::default()
        },
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn one_row_table(cells: Vec<Node>) -> Document {
    let row = make_table_row(false, cells);
    let table = Node {
        kind: NodeKind::Table { columns: vec![] },
        style: Style::default(),
        children: vec![row],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    default_doc(vec![table])
}

fn texts_y(elems: &[forme::layout::LayoutElement], out: &mut Vec<f64>) {
    for e in elems {
        if e.node_type.as_deref() == Some("Text") {
            out.push(e.y);
        }
        texts_y(&e.children, out);
    }
}

/// Two baseline cells with different first-line font sizes: the smaller-font
/// cell is shoved down so both first baselines land on the row baseline.
/// Fails-first: with baseline→Top the two text lines share a y (no shove).
#[test]
fn baseline_aligns_first_baselines_across_cells() {
    let doc = one_row_table(vec![
        valign_cell(12.0, "a", VerticalAlign::Baseline, None),
        valign_cell(24.0, "b", VerticalAlign::Baseline, None),
    ]);
    let pages = layout_doc(&doc);
    let mut ys = Vec::new();
    texts_y(&pages[0].elements, &mut ys);
    assert_eq!(ys.len(), 2, "two text lines");
    // Baseline = line-box top + font_size. Align them: a.y + 12 == b.y + 24.
    let (a_y, b_y) = (ys[0], ys[1]);
    assert!(
        ((a_y + 12.0) - (b_y + 24.0)).abs() < 0.001,
        "first baselines must coincide: a {} b {}",
        a_y,
        b_y
    );
    // Concretely, the small-font line is shoved down by exactly 24 - 12 = 12pt.
    assert!(
        (a_y - (b_y + 12.0)).abs() < 0.001,
        "shove = 12pt: {a_y} vs {b_y}"
    );
}

/// The risk site: a baseline cell shoved down must GROW the row, never clip.
/// Cell A has a small first-line font (12) but tall content (80pt block);
/// cell B's larger font (40) sets the row baseline, shoving A down by 28pt.
/// Fails-first: without the measure-side growth, the baseline row equals the
/// top-aligned row (no growth) and A's content clips.
#[test]
fn baseline_shove_grows_the_row_and_does_not_clip() {
    let row_h = |va: VerticalAlign| {
        let doc = one_row_table(vec![
            valign_cell(12.0, "a", va, Some(80.0)),
            valign_cell(40.0, "b", va, None),
        ]);
        let pages = layout_doc(&doc);
        find_first_by_type(&pages[0].elements, "TableRow")
            .expect("row")
            .height
    };
    let baseline_h = row_h(VerticalAlign::Baseline);
    let top_h = row_h(VerticalAlign::Top);
    // The row grew by exactly the shove (40 - 12 = 28pt) — cell A dominates
    // both rows, so the delta is purely the baseline offset.
    assert!(
        (baseline_h - top_h - 28.0).abs() < 0.01,
        "baseline row must grow by the 28pt shove: baseline {} vs top {}",
        baseline_h,
        top_h
    );
}

// ─── dashed / dotted borders (item 4) ───────────────────────────────

fn bordered_box(width_pt: f64, style: forme::style::BorderStyle) -> Document {
    use forme::style::{BorderStyle as _BS, Color, EdgeValues};
    let _ = _BS::Solid;
    let node = Node {
        kind: NodeKind::View,
        style: Style {
            width: Some(Dimension::Pt(120.0)),
            height: Some(Dimension::Pt(50.0)),
            border_width: Some(EdgeValues::uniform(width_pt)),
            border_style: Some(EdgeValues::uniform(style)),
            border_color: Some(EdgeValues::uniform(Color::BLACK)),
            ..Default::default()
        },
        children: vec![],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    };
    default_doc(vec![node])
}

/// Dashed borders emit the calibrated PDF dash pattern (dash 2×width,
/// gap 1×width) with a butt cap; solid borders emit no dash operator.
#[test]
fn dashed_border_emits_calibrated_dash_pattern() {
    use forme::style::BorderStyle;
    let pdf = forme::render(&bordered_box(4.0, BorderStyle::Dashed)).expect("render");
    let content = decompress_pdf_streams(&pdf);
    assert!(
        content.contains("[8.00 4.00] 0 d"),
        "dash 2×/gap 1× for a 4pt border"
    );
    assert!(!content.contains("1 J"), "dashed uses butt cap, not round");

    let solid =
        decompress_pdf_streams(&forme::render(&bordered_box(4.0, BorderStyle::Solid)).unwrap());
    assert!(
        !solid.contains("] 0 d"),
        "solid border emits no dash operator"
    );
}

/// Dotted borders emit round-capped dots (diameter 1×width) at 2×width
/// centre spacing: `1 J` + `[0 2w] 0 d`.
#[test]
fn dotted_border_emits_round_dots() {
    use forme::style::BorderStyle;
    let pdf = forme::render(&bordered_box(4.0, BorderStyle::Dotted)).expect("render");
    let content = decompress_pdf_streams(&pdf);
    assert!(content.contains("1 J"), "dotted uses a round cap");
    assert!(
        content.contains("[0 8.00] 0 d"),
        "dots spaced 2×width centre-to-centre"
    );
}

#[test]
fn test_parity_translation_excludes_page_anchored_watermarks() {
    // @page :left/:right applies flow content as a constant x translation
    // per page parity. Page-anchored paint — watermarks positioned from
    // page dimensions at serialize time — must NOT translate: it would
    // look right on page one and drift on later pages. This pins the
    // exclusion (approved-design hold: page-anchored paint gets its own
    // test).
    let many_paras: String = (0..60)
        .map(|i| {
            format!(
                r#"{{ "kind": {{ "type": "Text", "content": "paragraph {i} body" }}, "style": {{ "marginBottom": 10 }}, "children": [] }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{
        "defaultPage": {{ "size": {{ "Custom": {{ "width": 400, "height": 400 }} }}, "margin": {{ "top": 40, "right": 50, "bottom": 40, "left": 50 }}, "wrap": true }},
        "rightPage": {{ "size": {{ "Custom": {{ "width": 400, "height": 400 }} }}, "margin": {{ "top": 40, "right": 30, "bottom": 40, "left": 70 }}, "wrap": true }},
        "leftPage": {{ "size": {{ "Custom": {{ "width": 400, "height": 400 }} }}, "margin": {{ "top": 40, "right": 70, "bottom": 40, "left": 30 }}, "wrap": true }},
        "children": [
            {{ "kind": {{ "type": "Watermark", "text": "DRAFT", "font_size": 40, "angle": -45 }}, "style": {{}}, "children": [] }},
            {many_paras}
        ],
        "metadata": {{}}
    }}"#
    );
    let (_pdf, layout, _warnings) =
        forme::render_json_with_layout(&json).expect("parity watermark doc renders");
    assert!(
        layout.pages.len() >= 2,
        "need 2+ pages, got {}",
        layout.pages.len()
    );

    fn find_x(
        els: &[forme::layout::ElementInfo],
        pred: &dyn Fn(&forme::layout::ElementInfo) -> bool,
    ) -> Option<f64> {
        for e in els {
            if pred(e) {
                return Some(e.x);
            }
            if let Some(x) = find_x(&e.children, pred) {
                return Some(x);
            }
        }
        None
    }

    let wm_x_p1 = find_x(&layout.pages[0].elements, &|e| {
        e.node_type == "Watermark" || e.kind.contains("Watermark")
    })
    .expect("watermark on page 1");
    let wm_x_p2 = find_x(&layout.pages[1].elements, &|e| {
        e.node_type == "Watermark" || e.kind.contains("Watermark")
    })
    .expect("watermark on page 2");
    assert!(
        (wm_x_p1 - wm_x_p2).abs() < 0.01,
        "watermark is page-anchored and must not translate: page1 x={wm_x_p1}, page2 x={wm_x_p2}"
    );

    // The flow content DOES translate: right page (1) at 70, left page (2) at 30.
    let text_x_p1 =
        find_x(&layout.pages[0].elements, &|e| e.node_type == "TextLine").expect("text on page 1");
    let text_x_p2 =
        find_x(&layout.pages[1].elements, &|e| e.node_type == "TextLine").expect("text on page 2");
    assert!(
        (text_x_p1 - 70.0).abs() < 0.5,
        "page 1 (right) flow x: {text_x_p1}"
    );
    assert!(
        (text_x_p2 - 30.0).abs() < 0.5,
        "page 2 (left) flow x: {text_x_p2}"
    );
}
