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

#[cfg(feature = "wasm")]
mod wasm;

mod css;
mod dom;
mod map;
mod sheet;
mod style;
mod ua;

pub use forme::model::PageSize;
use forme::model::{Edges, PageConfig};
pub use forme::{FormeError, LayoutInfo};

/// A font provided by the caller — the offline half of the web-font
/// migration recipe (download the TTF, hand it over here or via
/// `--font Family=path` on the CLI).
#[derive(Debug, Clone)]
pub struct FontSpec {
    /// The family name templates reference in `font-family`.
    pub family: String,
    /// Raw TTF bytes.
    pub data: Vec<u8>,
    /// CSS weight (400 regular, 700 bold, ...). Defaults to 400.
    pub weight: u32,
    pub italic: bool,
}

impl FontSpec {
    pub fn new(family: impl Into<String>, data: Vec<u8>) -> Self {
        FontSpec {
            family: family.into(),
            data,
            weight: 400,
            italic: false,
        }
    }
}

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
    /// Fonts registered with the engine (TTF bytes keyed by family name).
    pub fonts: Vec<FontSpec>,
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
    let (body, style_texts, stylesheet_links) = dom::parse_html_with_styles(html);
    for href in &stylesheet_links {
        warnings.push(format!(
            "stylesheet link '{href}' is not fetched — inline the CSS in a <style> block or pass the file via --css / options.css"
        ));
    }

    // Pass 1 (probe): resolve @page geometry so `@media` feature queries have a
    // viewport to evaluate against. Warnings from this pass are discarded — the
    // real pass below re-emits them.
    let mut probe_warn = Vec::new();
    let mut probe = sheet::Stylesheet::default();
    for text in &style_texts {
        probe.append(sheet::parse_stylesheet(text, &mut probe_warn));
    }
    if let Some(css) = &options.css {
        probe.append(sheet::parse_stylesheet(css, &mut probe_warn));
    }
    let probe_config = page_config(options, probe.page.as_ref(), &mut probe_warn);
    let (pw, ph) = probe_config.size.dimensions();
    let viewport = sheet::Viewport {
        width: pw - probe_config.margin.left - probe_config.margin.right,
        height: ph - probe_config.margin.top - probe_config.margin.bottom,
    };

    // Pass 2 (real): parse with the viewport bound so feature queries evaluate.
    let mut stylesheet = sheet::Stylesheet::default();
    for text in &style_texts {
        stylesheet.append(sheet::parse_stylesheet_with_viewport(text, viewport, &mut warnings));
    }
    if let Some(css) = &options.css {
        stylesheet.append(sheet::parse_stylesheet_with_viewport(css, viewport, &mut warnings));
    }

    let mut config = page_config(options, stylesheet.page.as_ref(), &mut warnings);

    // Margin boxes: the band trick. Each occupied edge's page margin is
    // zeroed and a Fixed band of exactly that height takes its place, so
    // the band occupies precisely the strip CSS calls the margin and
    // content starts exactly where @page declared.
    //
    // The :first interaction (designed, not discovered): when :first
    // suppresses a band via `content: none`, that page's config RESTORES
    // the real margin — otherwise page one's content would start at the
    // physical top of the paper, the two features individually correct
    // and jointly wrong.
    let mut bands: Vec<forme::Node> = Vec::new();
    let mut first_page: Option<forme::model::PageConfig> = None;
    if let Some(rule) = stylesheet.page.as_ref() {
        use forme::model::FixedPageFilter;

        let resolve_len = |l: css::Length, warnings: &mut Vec<String>| -> Option<f64> {
            match l {
                css::Length::Pt(v) => Some(v),
                css::Length::Em(e) => Some(e * style::ROOT_FONT_SIZE),
                css::Length::Rem(r) => Some(r * style::ROOT_FONT_SIZE),
                css::Length::Auto => None,
                css::Length::Percent(_) => {
                    warnings.push("percentage @page :first margins are unsupported".to_string());
                    None
                }
            }
        };
        let first_rule = rule.first.as_ref();
        let first_margin = |idx: usize, warnings: &mut Vec<String>| -> Option<f64> {
            first_rule
                .and_then(|f| f.margin[idx])
                .and_then(|l| resolve_len(l, warnings))
        };
        let orig_top = config.margin.top;
        let orig_bottom = config.margin.bottom;
        let mut first_cfg = config.clone();
        first_cfg.margin.top = first_margin(0, &mut warnings).unwrap_or(orig_top);
        first_cfg.margin.right = first_margin(1, &mut warnings).unwrap_or(config.margin.right);
        first_cfg.margin.bottom = first_margin(2, &mut warnings).unwrap_or(orig_bottom);
        first_cfg.margin.left = first_margin(3, &mut warnings).unwrap_or(config.margin.left);

        let mut edge = |top: bool| {
            let boxes: Vec<&sheet::MarginBox> = rule
                .margin_boxes
                .iter()
                .filter(|b| b.position.is_top() == top)
                .collect();
            let band_height = if top { orig_top } else { orig_bottom };
            if boxes.is_empty() || band_height <= 0.0 {
                return;
            }
            let suppressed: Vec<bool> = boxes
                .iter()
                .map(|b| first_rule.is_some_and(|f| f.suppress.contains(&b.position)))
                .collect();
            let any_suppressed = suppressed.iter().any(|s| *s);
            if any_suppressed && !suppressed.iter().all(|s| *s) {
                warnings.push(format!(
                    "@page :first suppresses only some {} margin boxes; the whole band is suppressed on the first page",
                    if top { "top" } else { "bottom" }
                ));
            }
            let filter = if any_suppressed {
                FixedPageFilter::NotFirst
            } else {
                FixedPageFilter::All
            };
            bands.push(map::build_margin_band(
                &boxes,
                band_height,
                top,
                filter,
                &mut warnings,
            ));
            // The band replaces the margin on pages it appears on.
            if top {
                config.margin.top = 0.0;
                if any_suppressed {
                    // Band absent on page one: the restore rule. An
                    // explicit :first margin override wins over the plain
                    // restored value (first_cfg already holds it).
                } else {
                    if first_cfg.margin.top != orig_top {
                        warnings.push(
                            "@page :first margin-top with margin boxes on the first page is unsupported (suppress the boxes with content: none, or match the margins)".to_string(),
                        );
                    }
                    first_cfg.margin.top = 0.0;
                }
            } else {
                config.margin.bottom = 0.0;
                if !any_suppressed {
                    if first_cfg.margin.bottom != orig_bottom {
                        warnings.push(
                            "@page :first margin-bottom with margin boxes on the first page is unsupported (suppress the boxes with content: none, or match the margins)".to_string(),
                        );
                    }
                    first_cfg.margin.bottom = 0.0;
                }
            }
        };
        edge(true);
        edge(false);

        let margins_differ = |a: &forme::model::Edges, b: &forme::model::Edges| {
            a.top != b.top || a.right != b.right || a.bottom != b.bottom || a.left != b.left
        };
        if margins_differ(&first_cfg.margin, &config.margin) {
            first_page = Some(first_cfg);
        }
    }

    let (mut doc, map_warnings) = map::map_html(&body, stylesheet, config);
    warnings.extend(map_warnings);
    doc.first_page = first_page;
    for font in &options.fonts {
        use base64::Engine as _;
        doc.fonts.push(forme::model::FontEntry {
            family: font.family.clone(),
            src: base64::engine::general_purpose::STANDARD.encode(&font.data),
            weight: font.weight,
            italic: font.italic,
        });
    }
    if !bands.is_empty() {
        if let Some(body_view) = doc.children.first_mut() {
            // Bands go first so the engine registers them before any
            // content lands on page one.
            for band in bands.into_iter().rev() {
                body_view.children.insert(0, band);
            }
        }
    }
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
