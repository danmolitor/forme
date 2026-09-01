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

pub mod barcode;
pub mod chart;
pub mod error;
pub mod font;
pub mod image_loader;
pub mod layout;
pub mod model;
pub mod pdf;
pub mod qrcode;
pub mod style;
pub mod svg;
pub mod template;
pub mod text;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "wasm-raw")]
pub mod wasm_raw;

pub use error::FormeError;
pub use layout::LayoutInfo;
pub use model::{
    CertificationConfig, ColumnDef, ColumnWidth, FontEntry, PatternType, RedactionPattern,
    RedactionRegion, TextRun,
};
pub use model::{ChartDataPoint, ChartSeries, DotPlotGroup};
pub use model::{Document, Metadata, Node, NodeKind, PageConfig, PageSize};
pub use style::Style;

use font::FontContext;
use layout::LayoutEngine;
use pdf::PdfWriter;

/// Certify PDF bytes with an X.509 certificate.
///
/// Takes arbitrary PDF bytes and a certification configuration, and returns
/// new PDF bytes with a valid digital signature. Uses incremental update
/// to preserve the original PDF content.
pub fn certify_pdf(
    pdf_bytes: &[u8],
    config: &model::CertificationConfig,
) -> Result<Vec<u8>, FormeError> {
    pdf::certify::certify_pdf(pdf_bytes, config)
}

/// Redact regions of a PDF by overlaying opaque rectangles.
///
/// Takes arbitrary PDF bytes and a list of redaction regions (page, x, y,
/// width, height in top-origin coordinates). Returns new PDF bytes with
/// the redaction rectangles drawn on top via incremental update.
pub fn redact_pdf(
    pdf_bytes: &[u8],
    regions: &[model::RedactionRegion],
) -> Result<Vec<u8>, FormeError> {
    pdf::redaction::redact_pdf(pdf_bytes, regions)
}

/// Find text regions matching patterns in a PDF.
///
/// Searches PDF content streams for literal or regex patterns and returns
/// redaction regions (in web top-origin coordinates) for each match.
pub fn find_text_regions(
    pdf_bytes: &[u8],
    patterns: &[model::RedactionPattern],
) -> Result<Vec<RedactionRegion>, FormeError> {
    pdf::redaction::find_text_regions(pdf_bytes, patterns)
}

/// Redact text matching patterns from a PDF.
///
/// Convenience wrapper: finds text regions matching the patterns, then
/// applies coordinate-based redaction to all matches.
pub fn redact_text(
    pdf_bytes: &[u8],
    patterns: &[model::RedactionPattern],
) -> Result<Vec<u8>, FormeError> {
    pdf::redaction::redact_text(pdf_bytes, patterns)
}

/// Merge multiple PDFs into a single document.
///
/// Takes a slice of PDF byte slices and returns merged PDF bytes containing
/// all pages in order. Requires at least 2 input PDFs.
pub fn merge_pdfs(pdfs: &[&[u8]]) -> Result<Vec<u8>, FormeError> {
    pdf::merge::merge_pdfs(pdfs)
}

/// Render a document to PDF bytes.
///
/// This is the primary entry point. Takes a document tree and returns
/// the raw bytes of a valid PDF file. If the document has a `certification`
/// configuration, the output PDF is digitally signed.
pub fn render(document: &Document) -> Result<Vec<u8>, FormeError> {
    render_with_warnings(document).map(|(pdf, _warnings)| pdf)
}

/// Render a document to PDF bytes plus any non-fatal warnings (e.g. pdfUa
/// requested without an embeddable font registered). Same output as `render`;
/// the warnings surface through the WASM bindings and the HTML wrapper.
pub fn render_with_warnings(document: &Document) -> Result<(Vec<u8>, Vec<String>), FormeError> {
    let mut font_context = FontContext::new();
    register_document_fonts(&mut font_context, &document.fonts);
    let engine = LayoutEngine::new();
    let mut pages = engine.layout(document, &font_context);

    // Re-layout if sentinel digit count was wrong (up to 3 total passes)
    for _ in 0..2 {
        let needed = digits_for_count(pages.len());
        if needed == font_context.sentinel_digit_count() {
            break;
        }
        font_context.set_sentinel_digit_count(needed);
        pages = engine.layout(document, &font_context);
    }

    let writer = PdfWriter::new();
    let tagged = document.tagged
        || document.pdf_ua
        || matches!(document.pdfa, Some(model::PdfAConformance::A2a));
    let (pdf, warnings) = writer.write(
        &pages,
        &document.metadata,
        &font_context,
        tagged,
        document.pdfa.as_ref(),
        document.pdf_ua,
        document.embedded_data.as_deref(),
        document.flatten_forms,
    )?;
    let pdf = if let Some(ref sig_config) = document.certification {
        pdf::certify::certify_pdf(&pdf, sig_config)?
    } else {
        pdf
    };
    Ok((pdf, warnings))
}

/// Render a document to PDF bytes along with layout metadata.
///
/// Same as `render()` but also returns `LayoutInfo` describing the
/// position and dimensions of every element on every page.
/// If the document has a `certification` configuration, the output PDF
/// is digitally signed.
pub fn render_with_layout(
    document: &Document,
) -> Result<(Vec<u8>, LayoutInfo, Vec<String>), FormeError> {
    let mut font_context = FontContext::new();
    register_document_fonts(&mut font_context, &document.fonts);
    let engine = LayoutEngine::new();
    let mut pages = engine.layout(document, &font_context);

    // Re-layout if sentinel digit count was wrong (up to 3 total passes)
    for _ in 0..2 {
        let needed = digits_for_count(pages.len());
        if needed == font_context.sentinel_digit_count() {
            break;
        }
        font_context.set_sentinel_digit_count(needed);
        pages = engine.layout(document, &font_context);
    }

    let layout_info = LayoutInfo::from_pages(&pages);
    let writer = PdfWriter::new();
    let tagged = document.tagged
        || document.pdf_ua
        || matches!(document.pdfa, Some(model::PdfAConformance::A2a));
    let (pdf, warnings) = writer.write(
        &pages,
        &document.metadata,
        &font_context,
        tagged,
        document.pdfa.as_ref(),
        document.pdf_ua,
        document.embedded_data.as_deref(),
        document.flatten_forms,
    )?;
    let pdf = if let Some(ref sig_config) = document.certification {
        pdf::certify::certify_pdf(&pdf, sig_config)?
    } else {
        pdf
    };
    Ok((pdf, layout_info, warnings))
}

/// Return the number of digits needed to display `n` as a decimal string.
fn digits_for_count(n: usize) -> u32 {
    if n < 10 {
        1
    } else if n < 100 {
        2
    } else if n < 1000 {
        3
    } else {
        4
    }
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
pub fn render_json_with_layout(
    json: &str,
) -> Result<(Vec<u8>, LayoutInfo, Vec<String>), FormeError> {
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
    render_with_layout(&document).map(|(pdf, layout, _warnings)| (pdf, layout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digits_for_count() {
        assert_eq!(digits_for_count(0), 1);
        assert_eq!(digits_for_count(1), 1);
        assert_eq!(digits_for_count(9), 1);
        assert_eq!(digits_for_count(10), 2);
        assert_eq!(digits_for_count(99), 2);
        assert_eq!(digits_for_count(100), 3);
        assert_eq!(digits_for_count(999), 3);
        assert_eq!(digits_for_count(1000), 4);
    }
}
