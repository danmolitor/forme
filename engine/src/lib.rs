//! # Forme
//!
//! A page-native PDF rendering engine.
//!
//! Most PDF renderers treat a document as an infinite vertical canvas and then
//! slice it into pages after layout. This produces broken tables, orphaned
//! headers, collapsed flex layouts on page boundaries, and years of GitHub
//! issues begging for fixes.
//!
//! Forme does the opposite: **the page is the fundamental unit of layout.**
//! Every layout decision—every flex calculation, every line break, every table
//! row placement—is made with the page boundary as a hard constraint. Content
//! doesn't get "sliced" after the fact. It flows *into* pages.
//!
//! ## Architecture
//!
//! ```text
//! Input (JSON/API)
//!       ↓
//!   [model]    — Document tree: nodes, styles, content
//!       ↓
//!   [style]    — Resolve cascade, inheritance, defaults
//!       ↓
//!   [layout]   — Page-aware layout engine
//!       ↓
//!   [pdf]      — Serialize to PDF bytes
//! ```

pub mod model;
pub mod style;
pub mod layout;
pub mod text;
pub mod font;
pub mod image_loader;
pub mod pdf;

#[cfg(feature = "wasm")]
pub mod wasm;

use model::Document;
use font::FontContext;
use layout::LayoutEngine;
use pdf::PdfWriter;

/// Render a document to PDF bytes.
///
/// This is the primary entry point. Takes a document tree and returns
/// the raw bytes of a valid PDF file.
pub fn render(document: &Document) -> Vec<u8> {
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();
    let pages = engine.layout(document, &font_context);
    let writer = PdfWriter::new();
    writer.write(&pages, &document.metadata, &font_context)
}

/// Render a document described as JSON to PDF bytes.
pub fn render_json(json: &str) -> Result<Vec<u8>, serde_json::Error> {
    let document: Document = serde_json::from_str(json)?;
    Ok(render(&document))
}
