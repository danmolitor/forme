//! # Visual Regression Tests
//!
//! Renders PDF documents to PNG via `pdftoppm` (from Poppler), then compares
//! pixel-by-pixel against stored reference images. Skips gracefully when
//! `pdftoppm` is not installed.
//!
//! Feature-gated behind `visual-tests`:
//! ```bash
//! cargo test --features visual-tests
//! ```
//!
//! To update reference images:
//! ```bash
//! FORME_UPDATE_REFERENCES=1 cargo test --features visual-tests
//! ```

#![cfg(feature = "visual-tests")]

use forme::model::*;
use forme::style::*;
use image::GenericImageView;
use std::path::PathBuf;
use std::process::Command;

// ── Helpers ────────────────────────────────────────────────────

/// Check if pdftoppm is available.
fn pdftoppm_available() -> bool {
    Command::new("pdftoppm")
        .arg("-v")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render PDF bytes to PNG images (one per page) using pdftoppm.
fn pdf_to_pngs(pdf_bytes: &[u8], dpi: u32) -> Vec<Vec<u8>> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_dir = std::env::temp_dir().join(format!("forme_visual_tests_{}", id));
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let pdf_path = tmp_dir.join("test.pdf");
    std::fs::write(&pdf_path, pdf_bytes).unwrap();

    let output_prefix = tmp_dir.join("page");

    let status = Command::new("pdftoppm")
        .args([
            "-r",
            &dpi.to_string(),
            "-png",
            pdf_path.to_str().unwrap(),
            output_prefix.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run pdftoppm");
    assert!(status.success(), "pdftoppm failed");

    // Collect output PNGs (named page-1.png, page-2.png, etc.)
    let mut pages = Vec::new();
    for i in 1..=100 {
        // pdftoppm can pad with different digit counts
        let candidates = [
            tmp_dir.join(format!("page-{}.png", i)),
            tmp_dir.join(format!("page-{:02}.png", i)),
            tmp_dir.join(format!("page-{:03}.png", i)),
        ];
        if let Some(path) = candidates.iter().find(|p| p.exists()) {
            pages.push(std::fs::read(path).unwrap());
        } else {
            break;
        }
    }

    // Cleanup temp files
    let _ = std::fs::remove_dir_all(&tmp_dir);

    pages
}

/// Compare two PNG images pixel-by-pixel. Returns the ratio of differing pixels.
fn compare_images(actual: &[u8], reference: &[u8]) -> f64 {
    let actual_img = image::load_from_memory(actual).expect("Failed to load actual PNG");
    let ref_img = image::load_from_memory(reference).expect("Failed to load reference PNG");

    let (w1, h1) = actual_img.dimensions();
    let (w2, h2) = ref_img.dimensions();

    if w1 != w2 || h1 != h2 {
        return 1.0; // Different dimensions = 100% different
    }

    let total_pixels = (w1 * h1) as f64;
    if total_pixels == 0.0 {
        return 0.0;
    }

    let actual_rgba = actual_img.to_rgba8();
    let ref_rgba = ref_img.to_rgba8();

    let mut diff_pixels = 0u64;
    for (a, b) in actual_rgba.pixels().zip(ref_rgba.pixels()) {
        // Allow small tolerance per channel (anti-aliasing can differ)
        let differs =
            a.0.iter()
                .zip(b.0.iter())
                .any(|(c1, c2)| (*c1 as i32 - *c2 as i32).unsigned_abs() > 2);
        if differs {
            diff_pixels += 1;
        }
    }

    diff_pixels as f64 / total_pixels
}

/// Get the references directory path for a test.
fn references_dir(test_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("references")
        .join(test_name)
}

/// Assert visual match against reference images, or save new references.
fn assert_visual_match(pdf_bytes: &[u8], test_name: &str, threshold: f64) {
    if !pdftoppm_available() {
        eprintln!(
            "SKIPPING visual test '{}': pdftoppm not installed (install poppler-utils)",
            test_name
        );
        return;
    }

    let actual_pages = pdf_to_pngs(pdf_bytes, 150);
    let ref_dir = references_dir(test_name);
    let update = std::env::var("FORME_UPDATE_REFERENCES").is_ok();

    if update {
        // Save/overwrite reference images
        std::fs::create_dir_all(&ref_dir).unwrap();
        for (i, page) in actual_pages.iter().enumerate() {
            let path = ref_dir.join(format!("page-{}.png", i + 1));
            std::fs::write(&path, page).unwrap();
            eprintln!("Updated reference: {}", path.display());
        }
        return;
    }

    // Compare against references
    for (i, actual) in actual_pages.iter().enumerate() {
        let ref_path = ref_dir.join(format!("page-{}.png", i + 1));
        if !ref_path.exists() {
            panic!(
                "No reference image for '{}' page {}. Run with FORME_UPDATE_REFERENCES=1 to create.",
                test_name,
                i + 1
            );
        }

        let reference = std::fs::read(&ref_path).unwrap();
        let diff_ratio = compare_images(actual, &reference);
        assert!(
            diff_ratio <= threshold,
            "Visual regression in '{}' page {}: {:.2}% pixels differ (threshold: {:.2}%)",
            test_name,
            i + 1,
            diff_ratio * 100.0,
            threshold * 100.0
        );
    }
}

// ── Helpers to build test documents ────────────────────────────

fn make_text_node(content: &str, font_size: f64) -> Node {
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

fn make_view_node(style: Style, children: Vec<Node>) -> Node {
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

// ── Visual regression test cases ──────────────────────────────

#[test]
fn visual_invoice() {
    // Simple invoice-like document
    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![
                make_text_node("INVOICE #12345", 24.0),
                make_text_node("Date: 2024-01-15", 12.0),
                make_text_node("", 12.0),
                make_text_node("Item 1: Widget A — $25.00", 12.0),
                make_text_node("Item 2: Widget B — $35.00", 12.0),
                make_text_node("Item 3: Widget C — $15.00", 12.0),
                make_text_node("", 12.0),
                make_text_node("Total: $75.00", 14.0),
            ],
        )],
        metadata: Metadata {
            title: Some("Invoice".to_string()),
            ..Default::default()
        },
        default_page: PageConfig::default(),
        fonts: vec![],
        tagged: false,
        pdfa: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_visual_match(&pdf, "visual_invoice", 0.01);
}

#[test]
fn visual_multi_page_text() {
    // Generate enough text to fill 3+ pages
    let mut children = Vec::new();
    for i in 0..80 {
        children.push(make_text_node(
            &format!(
                "Paragraph {}: Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                 Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
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
        fonts: vec![],
        tagged: false,
        pdfa: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_visual_match(&pdf, "visual_multi_page_text", 0.01);
}

#[test]
fn visual_table_header_repetition() {
    // Table with 80 rows — headers should repeat on each page
    let mut rows = Vec::new();

    // Header row
    rows.push(Node {
        kind: NodeKind::TableRow { is_header: true },
        style: Style {
            background_color: Some(Color::hex("#e0e0e0")),
            ..Default::default()
        },
        children: vec![
            Node {
                kind: NodeKind::TableCell {
                    col_span: 1,
                    row_span: 1,
                },
                style: Style::default(),
                children: vec![make_text_node("Name", 10.0)],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
            Node {
                kind: NodeKind::TableCell {
                    col_span: 1,
                    row_span: 1,
                },
                style: Style::default(),
                children: vec![make_text_node("Price", 10.0)],
                id: None,
                source_location: None,
                bookmark: None,
                href: None,
                alt: None,
            },
        ],
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    });

    // Body rows
    for i in 0..80 {
        rows.push(Node {
            kind: NodeKind::TableRow { is_header: false },
            style: Style::default(),
            children: vec![
                Node {
                    kind: NodeKind::TableCell {
                        col_span: 1,
                        row_span: 1,
                    },
                    style: Style::default(),
                    children: vec![make_text_node(&format!("Item {}", i + 1), 10.0)],
                    id: None,
                    source_location: None,
                    bookmark: None,
                    href: None,
                    alt: None,
                },
                Node {
                    kind: NodeKind::TableCell {
                        col_span: 1,
                        row_span: 1,
                    },
                    style: Style::default(),
                    children: vec![make_text_node(
                        &format!("${:.2}", (i + 1) as f64 * 9.99),
                        10.0,
                    )],
                    id: None,
                    source_location: None,
                    bookmark: None,
                    href: None,
                    alt: None,
                },
            ],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        });
    }

    let table = Node {
        kind: NodeKind::Table {
            columns: vec![
                ColumnDef {
                    width: ColumnWidth::Fraction(0.6),
                },
                ColumnDef {
                    width: ColumnWidth::Fraction(0.4),
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

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![table],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        fonts: vec![],
        tagged: false,
        pdfa: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_visual_match(&pdf, "visual_table_header_repetition", 0.01);
}

#[test]
fn visual_flex_layout() {
    // Flex container with grow, wrap, and justify-content
    let children: Vec<Node> = (0..6)
        .map(|i| {
            make_view_node(
                Style {
                    background_color: Some(Color::hex(
                        [
                            "#ff6b6b", "#4ecdc4", "#45b7d1", "#96ceb4", "#ffeaa7", "#dfe6e9",
                        ][i],
                    )),
                    width: Some(Dimension::Pt(150.0)),
                    height: Some(Dimension::Pt(80.0)),
                    margin: Some(Edges::uniform(8.0)),
                    ..Default::default()
                },
                vec![make_text_node(&format!("Box {}", i + 1), 14.0)],
            )
        })
        .collect();

    let flex_container = make_view_node(
        Style {
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            justify_content: Some(JustifyContent::SpaceBetween),
            ..Default::default()
        },
        children,
    );

    let doc = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            vec![flex_container],
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        fonts: vec![],
        tagged: false,
        pdfa: None,
    };

    let pdf = forme::render(&doc).unwrap();
    assert_visual_match(&pdf, "visual_flex_layout", 0.01);
}

#[test]
fn visual_tagged_no_visual_change() {
    // Proves that tagging is purely structural — rendering is identical
    let children = vec![
        make_text_node("Hello World", 16.0),
        make_text_node("This is a tagged document.", 12.0),
    ];

    let doc_untagged = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            children.clone(),
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        fonts: vec![],
        tagged: false,
        pdfa: None,
    };

    let doc_tagged = Document {
        children: vec![Node::page(
            PageConfig::default(),
            Style::default(),
            children,
        )],
        metadata: Default::default(),
        default_page: PageConfig::default(),
        fonts: vec![],
        tagged: true,
        pdfa: None,
    };

    let pdf_untagged = forme::render(&doc_untagged).unwrap();
    let pdf_tagged = forme::render(&doc_tagged).unwrap();

    if !pdftoppm_available() {
        eprintln!("SKIPPING visual_tagged_no_visual_change: pdftoppm not installed");
        return;
    }

    let pages_untagged = pdf_to_pngs(&pdf_untagged, 150);
    let pages_tagged = pdf_to_pngs(&pdf_tagged, 150);

    assert_eq!(pages_untagged.len(), pages_tagged.len());
    for (i, (u, t)) in pages_untagged.iter().zip(pages_tagged.iter()).enumerate() {
        let diff = compare_images(u, t);
        assert!(
            diff <= 0.001,
            "Tagged vs untagged differ on page {}: {:.4}% pixels",
            i + 1,
            diff * 100.0
        );
    }
}
