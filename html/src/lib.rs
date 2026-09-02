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
    /// Emit a tagged PDF (structure tree). Implied by `pdf_ua`.
    pub tagged: bool,
    /// Emit a PDF/UA-1 conforming file: tags, metadata, embedded fonts. Requires
    /// a metric-compatible font (register `@formepdf/fonts-standard` via `fonts`),
    /// a document language (`lang`), and alt text on informational images.
    pub pdf_ua: bool,
    /// Document language for PDF/UA (`/Lang`). If `pdf_ua` is set and this is
    /// `None`, the `<html lang>` attribute is used, else it defaults to "en"
    /// with a warning.
    pub lang: Option<String>,
    /// PDF/A conformance level: `"2b"`, `"2u"`, or `"2a"`. Like `pdf_ua`, this
    /// needs an embeddable font registered via `fonts`
    /// (`@formepdf/fonts-standard`) — the base-14 families are not embeddable.
    /// Composes with `pdf_ua`: a file can be both PDF/A and PDF/UA-1.
    pub pdf_a: Option<String>,
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
        stylesheet.append(sheet::parse_stylesheet_with_viewport(
            text,
            viewport,
            &mut warnings,
        ));
    }
    if let Some(css) = &options.css {
        stylesheet.append(sheet::parse_stylesheet_with_viewport(
            css,
            viewport,
            &mut warnings,
        ));
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
    let mut left_page: Option<forme::model::PageConfig> = None;
    let mut right_page: Option<forme::model::PageConfig> = None;
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

        // ── @page :left / :right ───────────────────────────────────
        //
        // Approved design: flow layout keeps the base horizontal geometry;
        // mirrored margins preserve content width by construction, so a
        // parity page is the base layout plus a constant x translation
        // (applied by the engine at finalize). Anything a translation
        // cannot express — unequal horizontal sums — warns by name and
        // normalizes to the base. Vertical geometry stays the base's
        // (the band trick's margin accounting is per-document).
        let base_h_sum = config.margin.left + config.margin.right;
        let build_side = |side_rule: Option<&sheet::SidePageRule>,
                              label: &str,
                              warnings: &mut Vec<String>|
         -> Option<forme::model::PageConfig> {
            let side = side_rule?;
            if side.margin[0].is_some() || side.margin[2].is_some() {
                warnings.push(format!(
                        "top/bottom margins on @page :{label} are not supported (normalized to the base @page)"
                    ));
            }
            let l = side.margin[3].and_then(|l| resolve_len(l, warnings));
            let r = side.margin[1].and_then(|l| resolve_len(l, warnings));
            let (left, right) = match (l, r) {
                (Some(l), Some(r)) => (l, r),
                (Some(l), None) => (l, config.margin.right),
                (None, Some(r)) => (config.margin.left, r),
                (None, None) => (config.margin.left, config.margin.right),
            };
            if (left + right - base_h_sum).abs() > 0.01 {
                warnings.push(format!(
                        "@page :{label} left and right margins must sum equally with the base @page (mirrored margins); content width is normalized to the base"
                    ));
                return None;
            }
            let mut cfg = config.clone();
            cfg.margin.left = left;
            cfg.margin.right = right;
            if margins_differ(&cfg.margin, &config.margin) {
                Some(cfg)
            } else {
                None
            }
        };
        left_page = build_side(rule.left.as_ref(), "left", &mut warnings);
        right_page = build_side(rule.right.as_ref(), "right", &mut warnings);

        // Per-side margin boxes: slot-level overrides of the base band,
        // supported on edges where the base @page also defines boxes (the
        // band trick's margin zeroing is shared; a side-only band on an
        // otherwise boxless edge would double-count the margin).
        let first_suppresses_edge = |top: bool| {
            rule.first.as_ref().is_some_and(|f| {
                rule.margin_boxes
                    .iter()
                    .filter(|b| b.position.is_top() == top)
                    .any(|b| f.suppress.contains(&b.position))
            })
        };
        for top in [true, false] {
            let base_boxes: Vec<&sheet::MarginBox> = rule
                .margin_boxes
                .iter()
                .filter(|b| b.position.is_top() == top)
                .collect();
            let band_height = if top { orig_top } else { orig_bottom };
            let mut side_overrides = [false, false]; // [left, right]
            for (i, (side_rule, side_filter, label)) in [
                (rule.left.as_ref(), FixedPageFilter::Left, "left"),
                (rule.right.as_ref(), FixedPageFilter::Right, "right"),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(side) = side_rule else { continue };
                let touches_edge = side.margin_boxes.iter().any(|b| b.position.is_top() == top)
                    || side.suppress.iter().any(|p| p.is_top() == top);
                if !touches_edge {
                    continue;
                }
                if base_boxes.is_empty() {
                    warnings.push(format!(
                        "@page :{label} margin boxes on the {} edge need base @page boxes on that edge (band accounting); skipped",
                        if top { "top" } else { "bottom" }
                    ));
                    continue;
                }
                side_overrides[i] = true;
                // Merged view for this side: base boxes minus suppressed,
                // overridden per slot by the side's own boxes.
                let mut merged: Vec<&sheet::MarginBox> = base_boxes
                    .iter()
                    .copied()
                    .filter(|b| !side.suppress.contains(&b.position))
                    .filter(|b| !side.margin_boxes.iter().any(|sb| sb.position == b.position))
                    .collect();
                merged.extend(
                    side.margin_boxes
                        .iter()
                        .filter(|b| b.position.is_top() == top),
                );
                if merged.is_empty() {
                    continue; // side suppresses the whole band: no side band
                }
                let filter = if matches!(side_filter, FixedPageFilter::Right)
                    && first_suppresses_edge(top)
                {
                    FixedPageFilter::RightNotFirst
                } else {
                    side_filter
                };
                bands.push(map::build_margin_band(
                    &merged,
                    band_height,
                    top,
                    filter,
                    &mut warnings,
                ));
            }
            // Restrict the base band to the sides it still owns.
            if side_overrides[0] || side_overrides[1] {
                for band in bands.iter_mut() {
                    if let forme::NodeKind::Fixed { position, pages } = &mut band.kind {
                        let is_top_band = matches!(position, forme::model::FixedPosition::Header);
                        if is_top_band != top {
                            continue;
                        }
                        let adjusted = match (*pages, side_overrides) {
                            // Base bands only (side bands already carry
                            // parity filters — leave them).
                            (FixedPageFilter::All, [true, true])
                            | (FixedPageFilter::NotFirst, [true, true]) => None,
                            (FixedPageFilter::All, [true, false]) => Some(FixedPageFilter::Right),
                            (FixedPageFilter::NotFirst, [true, false]) => {
                                Some(FixedPageFilter::RightNotFirst)
                            }
                            (FixedPageFilter::All, [false, true])
                            | (FixedPageFilter::NotFirst, [false, true]) => {
                                Some(FixedPageFilter::Left)
                            }
                            _ => continue,
                        };
                        match adjusted {
                            Some(f) => *pages = f,
                            None => {
                                // Both sides override: the side bands cover
                                // every page; drop the base band entirely.
                                band.children.clear();
                            }
                        }
                        break; // only the first (base) band for this edge
                    }
                }
            }
        }
        bands.retain(|b| !b.children.is_empty());
    }

    let (mut doc, map_warnings) = map::map_html(&body, stylesheet, config);
    warnings.extend(map_warnings);
    doc.first_page = first_page;
    doc.left_page = left_page;
    doc.right_page = right_page;
    if (doc.left_page.is_some() || doc.right_page.is_some())
        && body
            .attr("dir")
            .is_some_and(|d| d.eq_ignore_ascii_case("rtl"))
    {
        warnings.push(
            "page progression follows the inline base direction (CSS Paged Media); dir=\"rtl\" \
             page parity is not modeled — @page :left/:right are applied with left-to-right \
             page progression (page 1 = :right)"
                .to_string(),
        );
    }
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

    // Tagging / PDF-UA: the mapper already emits Heading/Table/List/Lbl/Figure
    // nodes, so the engine's tag builder produces the structure tree; here we
    // just flip the flags and settle the PDF/UA prerequisites.
    doc.tagged = options.tagged || options.pdf_ua;
    doc.pdf_ua = options.pdf_ua;
    if options.pdf_ua {
        if doc.metadata.lang.is_none() {
            doc.metadata.lang = Some(options.lang.clone().unwrap_or_else(|| {
                warnings.push(
                    "pdf_ua: no document language set — defaulting /Lang to \"en\". Set options.lang or an <html lang> attribute."
                        .to_string(),
                );
                "en".to_string()
            }));
        }
        // Informational images must carry alt text; decorative ones should be
        // marked decorative (not yet expressible in HTML input — see README).
        fn warn_missing_alt(node: &forme::Node, warnings: &mut Vec<String>) {
            if let forme::model::NodeKind::Image { src, .. } = &node.kind {
                if node.alt.as_deref().unwrap_or("").is_empty() {
                    warnings.push(format!(
                        "pdf_ua: image without alt text: {src} — add an alt attribute (or mark it decorative)."
                    ));
                }
            }
            for child in &node.children {
                warn_missing_alt(child, warnings);
            }
        }
        for child in &doc.children {
            warn_missing_alt(child, &mut warnings);
        }
    }

    // PDF/A conformance level. Composes with pdf_ua; 2a additionally needs the
    // structure tree, so it implies tagging.
    if let Some(level) = options.pdf_a.as_deref() {
        use forme::model::PdfAConformance;
        match level {
            "2b" => doc.pdfa = Some(PdfAConformance::A2b),
            "2u" => doc.pdfa = Some(PdfAConformance::A2u),
            "2a" => {
                doc.pdfa = Some(PdfAConformance::A2a);
                doc.tagged = true;
            }
            other => warnings.push(format!(
                "pdf_a: unknown conformance level {other:?} — expected \"2b\", \"2u\", or \"2a\". Ignoring."
            )),
        }
        // PDF/A needs embedded fonts too; if the doc has a language for pdf_ua
        // it's set above. Nothing else to settle here — the engine embeds fonts
        // via the fonts-standard substitution and errors by name if none is
        // registered.
        if doc.pdfa.is_some() && doc.metadata.lang.is_none() {
            if let Some(l) = options.lang.clone() {
                doc.metadata.lang = Some(l);
            }
        }
    }

    (doc, warnings)
}

/// Render an HTML string to PDF bytes.
pub fn render_html(html: &str, options: &HtmlOptions) -> Result<HtmlOutput, FormeError> {
    let (doc, mut warnings) = html_to_document(html, options);
    let (pdf, engine_warnings) = forme::render_with_warnings(&doc)?;
    warnings.extend(engine_warnings);
    Ok(HtmlOutput { pdf, warnings })
}

/// Render an HTML string to PDF bytes plus layout metadata.
pub fn render_html_with_layout(
    html: &str,
    options: &HtmlOptions,
) -> Result<HtmlLayoutOutput, FormeError> {
    let (doc, mut warnings) = html_to_document(html, options);
    let (pdf, layout, engine_warnings) = forme::render_with_layout(&doc)?;
    warnings.extend(engine_warnings);
    Ok(HtmlLayoutOutput {
        pdf,
        layout,
        warnings,
    })
}
