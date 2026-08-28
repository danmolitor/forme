//! # forme-pdf-html
//!
//! An HTML + print-CSS input path for the `forme-pdf` engine — "Satori for
//! paginated documents". Hand it the HTML you already have, get a correctly
//! paginated PDF back. No headless browser.
//!
//! Phase 0 spike scope: inline styles (`style=""` attributes) plus a
//! hardcoded UA stylesheet. Stylesheet CSS, selectors, and the cascade are
//! Phase 1; `@page` and the paged-media features are Phase 2.
//!
//! ```no_run
//! use forme_pdf_html::{render_html, HtmlOptions};
//!
//! let html = "<h1>Invoice #2024-001</h1><p>Due <strong>net 30</strong> days.</p>";
//! let out = render_html(html, &HtmlOptions::default()).unwrap();
//! std::fs::write("invoice.pdf", &out.pdf).unwrap();
//! ```

mod css;
mod dom;
mod map;
mod style;
mod ua;

pub use forme::model::PageSize;
use forme::model::{Edges, PageConfig};
pub use forme::{FormeError, LayoutInfo};

/// Options for HTML rendering.
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    /// Page size. Defaults to A4 (the engine default).
    pub page_size: PageSize,
    /// Uniform page margin in points. Defaults to the engine's 54pt
    /// (~0.75in). `@page` margin parsing is Phase 2.
    pub page_margin: Option<f64>,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            page_size: PageSize::A4,
            page_margin: None,
        }
    }
}

/// Rendered output: PDF bytes plus any warnings about unsupported CSS.
///
/// Warnings are the documented-subset contract in action: everything the
/// input asked for that the subset doesn't cover is listed, never silently
/// dropped.
#[derive(Debug)]
pub struct HtmlOutput {
    pub pdf: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Rendered output with layout metadata for every element on every page.
#[derive(Debug)]
pub struct HtmlLayoutOutput {
    pub pdf: Vec<u8>,
    pub layout: LayoutInfo,
    pub warnings: Vec<String>,
}

fn page_config(options: &HtmlOptions) -> PageConfig {
    let mut config = PageConfig {
        size: options.page_size,
        ..Default::default()
    };
    if let Some(m) = options.page_margin {
        config.margin = Edges::uniform(m);
    }
    config
}

/// Convert HTML to the engine's document tree without rendering. Exposed
/// for tests and tooling that want to inspect the mapping itself.
pub fn html_to_document(html: &str, options: &HtmlOptions) -> (forme::Document, Vec<String>) {
    let body = dom::parse_html(html);
    map::map_html(&body, page_config(options))
}

/// Render an HTML string to PDF bytes.
pub fn render_html(html: &str, options: &HtmlOptions) -> Result<HtmlOutput, FormeError> {
    let (doc, warnings) = html_to_document(html, options);
    let pdf = forme::render(&doc)?;
    Ok(HtmlOutput { pdf, warnings })
}

/// Render an HTML string to PDF bytes plus layout metadata.
pub fn render_html_with_layout(
    html: &str,
    options: &HtmlOptions,
) -> Result<HtmlLayoutOutput, FormeError> {
    let (doc, warnings) = html_to_document(html, options);
    let (pdf, layout) = forme::render_with_layout(&doc)?;
    Ok(HtmlLayoutOutput {
        pdf,
        layout,
        warnings,
    })
}
