//! # forme-pdf-html
//!
//! An HTML + print-CSS input path for the `forme-pdf` engine — "Satori for
//! paginated documents". Hand it the HTML you already have, get a correctly
//! paginated PDF back. No headless browser.
//!
//! Styling comes from three cascading origins: a built-in UA stylesheet,
//! the document's `<style>` blocks (plus `HtmlOptions::css`), and inline
//! `style=""` attributes. The supported CSS subset is documented
//! property-by-property in the README; everything outside it is reported
//! in the output's `warnings` — never silently dropped.
//!
//! `@page` and the paged-media features (margin boxes, `break-*`,
//! counters) are the next phase.
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
mod sheet;
mod style;
mod ua;

pub use forme::model::PageSize;
use forme::model::{Edges, PageConfig};
pub use forme::{FormeError, LayoutInfo};

/// Options for HTML rendering.
///
/// Page geometry precedence: an explicit option here overrides the
/// document's `@page` rule, which overrides the defaults (A4, 54pt
/// margins) — mirroring how a print dialog overrides a stylesheet.
#[derive(Debug, Clone, Default)]
pub struct HtmlOptions {
    /// Page size override. `None` uses `@page size` if present, else A4.
    pub page_size: Option<PageSize>,
    /// Uniform page margin override in points. `None` uses `@page margin`
    /// if present, else the engine's 54pt (~0.75in).
    pub page_margin: Option<f64>,
    /// Additional CSS applied AFTER the document's own `<style>` blocks —
    /// equal-specificity rules here win ties, mirroring a stylesheet
    /// appended at the end of the document.
    pub css: Option<String>,
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

fn page_config(
    options: &HtmlOptions,
    page_rule: Option<&sheet::PageRule>,
    warnings: &mut Vec<String>,
) -> PageConfig {
    let mut config = PageConfig::default();

    // @page size, then the explicit option on top.
    if let Some((w, h)) = page_rule.and_then(|r| r.size) {
        config.size = PageSize::Custom {
            width: w,
            height: h,
        };
    }
    if let Some(size) = options.page_size {
        config.size = size;
    }

    // @page margins, then the explicit uniform-margin option on top.
    if let Some(rule) = page_rule {
        let resolve = |l: css::Length, warnings: &mut Vec<String>| -> Option<f64> {
            match l {
                css::Length::Pt(v) => Some(v),
                css::Length::Em(e) => Some(e * style::ROOT_FONT_SIZE),
                css::Length::Rem(r) => Some(r * style::ROOT_FONT_SIZE),
                css::Length::Auto => None,
                css::Length::Percent(_) => {
                    warnings.push("percentage @page margins are unsupported".to_string());
                    None
                }
            }
        };
        if let Some(v) = rule.margin[0].and_then(|l| resolve(l, warnings)) {
            config.margin.top = v;
        }
        if let Some(v) = rule.margin[1].and_then(|l| resolve(l, warnings)) {
            config.margin.right = v;
        }
        if let Some(v) = rule.margin[2].and_then(|l| resolve(l, warnings)) {
            config.margin.bottom = v;
        }
        if let Some(v) = rule.margin[3].and_then(|l| resolve(l, warnings)) {
            config.margin.left = v;
        }
    }
    if let Some(m) = options.page_margin {
        config.margin = Edges::uniform(m);
    }
    config
}

/// Convert HTML to the engine's document tree without rendering. Exposed
/// for tests and tooling that want to inspect the mapping itself.
pub fn html_to_document(html: &str, options: &HtmlOptions) -> (forme::Document, Vec<String>) {
    let mut warnings = Vec::new();
    let (body, style_texts) = dom::parse_html_with_styles(html);

    let mut stylesheet = sheet::Stylesheet::default();
    for text in &style_texts {
        stylesheet.append(sheet::parse_stylesheet(text, &mut warnings));
    }
    if let Some(css) = &options.css {
        stylesheet.append(sheet::parse_stylesheet(css, &mut warnings));
    }

    let config = page_config(options, stylesheet.page.as_ref(), &mut warnings);
    let (doc, map_warnings) = map::map_html(&body, stylesheet, config);
    warnings.extend(map_warnings);
    (doc, warnings)
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
