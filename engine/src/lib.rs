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

pub mod error;
pub mod font;
pub mod image_loader;
pub mod layout;
pub mod model;
pub mod pdf;
pub mod style;
pub mod text;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use error::FormeError;

use font::FontContext;
use layout::{LayoutEngine, LayoutInfo};
use model::Document;
use pdf::PdfWriter;

/// Render a document to PDF bytes.
///
/// This is the primary entry point. Takes a document tree and returns
/// the raw bytes of a valid PDF file.
pub fn render(document: &Document) -> Result<Vec<u8>, FormeError> {
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();
    let pages = engine.layout(document, &font_context);
    let writer = PdfWriter::new();
    writer.write(&pages, &document.metadata, &font_context)
}

/// Render a document to PDF bytes along with layout metadata.
///
/// Same as `render()` but also returns `LayoutInfo` describing the
/// position and dimensions of every element on every page.
pub fn render_with_layout(document: &Document) -> Result<(Vec<u8>, LayoutInfo), FormeError> {
    let font_context = FontContext::new();
    let engine = LayoutEngine::new();
    let pages = engine.layout(document, &font_context);
    let layout_info = LayoutInfo::from_pages(&pages);
    let writer = PdfWriter::new();
    let pdf = writer.write(&pages, &document.metadata, &font_context)?;
    Ok((pdf, layout_info))
}

/// Render a document described as JSON to PDF bytes.
pub fn render_json(json: &str) -> Result<Vec<u8>, FormeError> {
    let document: Document = serde_json::from_str(json)?;
    render(&document)
}

/// Render a document described as JSON to PDF bytes along with layout metadata.
pub fn render_json_with_layout(json: &str) -> Result<(Vec<u8>, LayoutInfo), FormeError> {
    let document: Document = serde_json::from_str(json)?;
    render_with_layout(&document)
}
