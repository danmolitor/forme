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
        source_location: None,
        bookmark: None,
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
        }],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    writer.write(&pages, &doc.metadata, &font_context).unwrap()
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
            },
        ],
        metadata: Metadata::default(),
        default_page: PageConfig::default(),
    };

    let engine = LayoutEngine::new();
    let pages = engine.layout(&doc, &font_context);
    let writer = PdfWriter::new();
    let bytes = writer.write(&pages, &doc.metadata, &font_context).unwrap();

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
    }
}

fn make_fixed_footer(text: &str) -> Node {
    Node {
        kind: NodeKind::Fixed {
            position: FixedPosition::Footer,
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
        },
        style: Style {
            padding: Some(Edges::symmetric(40.0, 8.0)), // tall footer
            ..Default::default()
        },
        children: vec![make_text("Big Footer", 14.0)],
        id: None,
        source_location: None,
        bookmark: None,
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
                },
                style: Style::default(),
                children: vec![make_text("Page {{pageNumber}} of {{totalPages}}", 12.0)],
                id: None,
                source_location: None,
                bookmark: None,
            },
            make_text("Hello", 12.0),
        ],
        id: None,
        source_location: None,
        bookmark: None,
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
        },
        style: Style {
            font_size: Some(10.0),
            ..Style::default()
        },
        children: vec![make_text("{{pageNumber}}/{{totalPages}}", 10.0)],
        id: None,
        source_location: None,
        bookmark: None,
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
        },
        make_text("Content for chapter 1", 12.0),
        Node {
            kind: NodeKind::View,
            style: Style::default(),
            children: vec![make_text("Chapter 2", 18.0)],
            id: None,
            source_location: None,
            bookmark: Some("Chapter 2".to_string()),
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
    }]);
    let bytes = render_to_pdf(&doc);
    assert_valid_pdf(&bytes);
}
