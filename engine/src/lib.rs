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
pub mod svg;
pub mod template;
pub mod text;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use error::FormeError;

use font::FontContext;
use layout::{LayoutEngine, LayoutInfo};
use model::{Document, FontEntry};
use pdf::PdfWriter;

/// Render a document to PDF bytes.
///
/// This is the primary entry point. Takes a document tree and returns
/// the raw bytes of a valid PDF file.
pub fn render(document: &Document) -> Result<Vec<u8>, FormeError> {
    let mut font_context = FontContext::new();
    register_document_fonts(&mut font_context, &document.fonts);
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
    let mut font_context = FontContext::new();
    register_document_fonts(&mut font_context, &document.fonts);
    let engine = LayoutEngine::new();
    let pages = engine.layout(document, &font_context);
    let layout_info = LayoutInfo::from_pages(&pages);
    let writer = PdfWriter::new();
    let pdf = writer.write(&pages, &document.metadata, &font_context)?;
    Ok((pdf, layout_info))
}

/// Register custom fonts from the document's `fonts` array.
fn register_document_fonts(font_context: &mut FontContext, fonts: &[FontEntry]) {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    for entry in fonts {
        let bytes = if let Some(comma_pos) = entry.src.find(',') {
            // data URI: "data:font/ttf;base64,AAAA..."
            b64.decode(&entry.src[comma_pos + 1..]).ok()
        } else {
            // raw base64 string
            b64.decode(&entry.src).ok()
        };

        if let Some(data) = bytes {
            font_context
                .registry_mut()
                .register(&entry.family, entry.weight, entry.italic, data);
        }
    }
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

/// Render a template with data to PDF bytes.
///
/// Takes a template JSON tree (with `$ref`, `$each`, `$if`, operators) and
/// a data JSON object. Evaluates all expressions, then renders the resulting
/// document to PDF.
pub fn render_template(template_json: &str, data_json: &str) -> Result<Vec<u8>, FormeError> {
    let template: serde_json::Value = serde_json::from_str(template_json)?;
    let data: serde_json::Value = serde_json::from_str(data_json)?;
    let resolved = template::evaluate_template(&template, &data)?;
    let document: Document = serde_json::from_value(resolved)?;
    render(&document)
}

/// Render a template with data to PDF bytes along with layout metadata.
pub fn render_template_with_layout(
    template_json: &str,
    data_json: &str,
) -> Result<(Vec<u8>, LayoutInfo), FormeError> {
    let template: serde_json::Value = serde_json::from_str(template_json)?;
    let data: serde_json::Value = serde_json::from_str(data_json)?;
    let resolved = template::evaluate_template(&template, &data)?;
    let document: Document = serde_json::from_value(resolved)?;
    render_with_layout(&document)
}
