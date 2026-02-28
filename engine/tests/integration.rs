//! Integration tests for the Forme rendering pipeline.
//!
//! These tests exercise the full path from JSON input to PDF output.
//! They verify:
//! - JSON deserialization works correctly
//! - Layout engine produces the right number of pages
//! - PDF output is structurally valid
//! - Page breaks happen at the right places
//! - Table header repetition works

use base64::Engine as _;
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
        fonts: vec![],
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
        fonts: vec![],
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
        fonts: vec![],
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
        fonts: vec![],
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
        },
        fonts: vec![],
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
        },
        fonts: vec![],
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
        },
        fonts: vec![],
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
        fonts: vec![],
    };

    let pages = layout_doc(&doc);
    assert!(
        pages.len() >= 2,
        "Expected at least 2 pages, got {}",
        pages.len()
    );

    // On continuation pages (page 2+), the wrapper Rect element should exist
    // and the first child inside it should be offset by padding.top from the Rect's top edge.
    for page_idx in 1..pages.len() {
        let page = &pages[page_idx];
        // Find the wrapper Rect element (the breakable view's background)
        let wrapper = page
            .elements
            .iter()
            .find(|el| matches!(el.draw, forme::layout::DrawCommand::Rect { .. }))
            .expect(&format!(
                "Page {} should have a wrapper Rect element",
                page_idx + 1
            ));

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
        fonts: vec![],
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
        },
        fonts: vec![],
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
        },
        fonts: vec![],
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
        },
        fonts: vec![],
    };

    let bytes = forme::render(&doc).expect("Should render with per-node lang override");
    assert!(bytes.starts_with(b"%PDF"));
}
