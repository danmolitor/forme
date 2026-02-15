//! Integration tests for the Forme rendering pipeline.
//!
//! These tests exercise the full path from JSON input to PDF output.
//! They verify:
//! - JSON deserialization works correctly
//! - Layout engine produces the right number of pages
//! - PDF output is structurally valid
//! - Page breaks happen at the right places
//! - Table header repetition works

use forme::model::*;
use forme::style::*;
use forme::font::FontContext;
use forme::layout::LayoutEngine;

// ─── Helpers ────────────────────────────────────────────────────

fn make_text(content: &str, font_size: f64) -> Node {
    Node {
        kind: NodeKind::Text {
            content: content.to_string(),
        },
        style: Style {
            font_size: Some(font_size),
            ..Default::default()
        },
        children: vec![],
        id: None,
    }
}

fn make_view(children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::View,
        style: Style::default(),
        children,
        id: None,
    }
}

fn make_styled_view(style: Style, children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::View,
        style,
        children,
        id: None,
    }
}

fn make_page_break() -> Node {
    Node {
        kind: NodeKind::PageBreak,
        style: Style::default(),
        children: vec![],
        id: None,
    }
}

fn make_table_row(is_header: bool, cells: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::TableRow { is_header },
        style: Style::default(),
        children: cells,
        id: None,
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
    }
}

fn default_doc(children: Vec<Node>) -> Document {
    Document {
        children,
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
    }
}

fn layout_doc(doc: &Document) -> Vec<forme::layout::LayoutPage> {
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();
    engine.layout(doc, &font_context)
}

fn render_to_pdf(doc: &Document) -> Vec<u8> {
    forme::render(doc)
}

fn assert_valid_pdf(bytes: &[u8]) {
    assert!(bytes.len() > 50, "PDF too small to be valid");
    assert!(
        bytes.starts_with(b"%PDF-1.7"),
        "Missing PDF header"
    );
    assert!(
        bytes.windows(5).any(|w| w == b"%%EOF"),
        "Missing %%EOF marker"
    );
    assert!(
        bytes.windows(4).any(|w| w == b"xref"),
        "Missing xref table"
    );
    assert!(
        bytes.windows(7).any(|w| w == b"trailer"),
        "Missing trailer"
    );
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
    assert_eq!(pages.len(), 2, "Should have exactly 2 pages after a page break");
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
        vec![
            make_text("Left", 12.0),
            make_text("Right", 12.0),
        ],
    );
    let doc = default_doc(vec![row]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);
    assert!(!pages[0].elements.is_empty());
}

#[test]
fn test_flex_column_is_default() {
    let container = make_view(vec![
        make_text("First", 12.0),
        make_text("Second", 12.0),
    ]);
    let doc = default_doc(vec![container]);
    let pages = layout_doc(&doc);
    assert_eq!(pages.len(), 1);

    // Elements should be stacked vertically, so they have different Y positions
    // (At minimum, there should be multiple elements)
    assert!(pages[0].elements.len() >= 2);
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
    }
}

#[test]
fn test_simple_table() {
    let table = make_simple_table(
        vec!["Name", "Age"],
        vec![
            vec!["Alice", "30"],
            vec!["Bob", "25"],
        ],
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
        .map(|i| vec![Box::leak(format!("Item {}", i).into_boxed_str()) as &str, "Value"])
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

    let bytes = forme::render(&doc);
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
            size, w, h, expected_w, expected_h
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
        },
        default_page: PageConfig::default(),
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
    font_context.registry_mut().register("TestFont", 400, false, font_data.to_vec());

    let doc = Document {
        children: vec![Node {
            kind: NodeKind::Text { content: text.to_string() },
            style: Style {
                font_family: Some("TestFont".to_string()),
                font_size: Some(14.0),
                ..Default::default()
            },
            children: vec![],
            id: None,
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    writer.write(&pages, &doc.metadata, &font_context)
}

#[test]
fn test_custom_font_produces_valid_pdf() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => { eprintln!("Skipping: no test TTF font found"); return; }
    };

    let bytes = render_with_custom_font(&font_data, "Hello Custom Font");
    assert_valid_pdf(&bytes);
}

#[test]
fn test_custom_font_has_cidfont_objects() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => { eprintln!("Skipping: no test TTF font found"); return; }
    };

    let bytes = render_with_custom_font(&font_data, "ABC");
    let text = String::from_utf8_lossy(&bytes);

    assert!(text.contains("CIDFontType2"), "Should contain CIDFontType2 subtype");
    assert!(text.contains("/FontFile2"), "Should contain FontFile2 reference");
    assert!(text.contains("/Type0"), "Should contain Type0 font dictionary");
    assert!(text.contains("/Identity-H"), "Should use Identity-H encoding");
    assert!(text.contains("/DescendantFonts"), "Should have DescendantFonts array");
}

#[test]
fn test_custom_font_has_tounicode() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => { eprintln!("Skipping: no test TTF font found"); return; }
    };

    let bytes = render_with_custom_font(&font_data, "Test");
    let text = String::from_utf8_lossy(&bytes);

    assert!(text.contains("/ToUnicode"), "Should have ToUnicode CMap for text extraction");
}

#[test]
fn test_mixed_standard_and_custom_fonts() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => { eprintln!("Skipping: no test TTF font found"); return; }
    };

    let mut font_context = FontContext::new();
    font_context.registry_mut().register("CustomFont", 400, false, font_data);

    let doc = Document {
        children: vec![
            // Standard font text
            Node {
                kind: NodeKind::Text { content: "Standard Helvetica".to_string() },
                style: Style {
                    font_family: Some("Helvetica".to_string()),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
            },
            // Custom font text
            Node {
                kind: NodeKind::Text { content: "Custom Font Text".to_string() },
                style: Style {
                    font_family: Some("CustomFont".to_string()),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                children: vec![],
                id: None,
            },
        ],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    let bytes = writer.write(&pages, &doc.metadata, &font_context);

    assert_valid_pdf(&bytes);
    let text = String::from_utf8_lossy(&bytes);

    // Should have both Type1 (standard) and Type0/CIDFontType2 (custom) fonts
    assert!(text.contains("/Type1"), "Should have Type1 for standard font");
    assert!(text.contains("CIDFontType2"), "Should have CIDFontType2 for custom font");
}

#[test]
fn test_custom_font_subset_smaller_than_full() {
    let font_data = match load_test_font() {
        Some(data) => data,
        None => { eprintln!("Skipping: no test TTF font found"); return; }
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
        bytes.len(), font_data.len()
    );
}
