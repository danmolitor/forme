//! WASM bindings for `@formepdf/html`.
//!
//! Two entry points, sharing one options parser:
//! - `render_html_wasm(html, options_json)` → PDF bytes + warnings. The
//!   lean path for the `npx`/server PDF use case.
//! - `render_html_wasm_with_layout(html, options_json)` → PDF bytes +
//!   `LayoutInfo` (as a JSON string) + warnings. The layout is what powers
//!   the VS Code extension's tree/inspector/overlays — the same
//!   `forme::LayoutInfo` the core WASM emits, so downstream consumers can't
//!   tell HTML output from JSX output.
//!
//! Options arrive as JSON (`{"pageSize": "Letter", "pageMargin": 36,
//! "css": "..."}`) so the JS wrapper stays a thin shim. Layout is returned
//! as a serde_json string rather than a native JS object on purpose: the
//! crate's `wasm` feature intentionally omits `serde-wasm-bindgen`/`js-sys`
//! (core's dependency, not ours), and the JS wrapper `JSON.parse`s it back
//! into the identical object shape.

use crate::{render_html, render_html_with_layout, HtmlOptions, PageSize};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmOptions {
    page_size: Option<String>,
    page_margin: Option<f64>,
    css: Option<String>,
    #[serde(default)]
    fonts: Vec<WasmFont>,
    #[serde(default)]
    tagged: bool,
    #[serde(default)]
    pdf_ua: bool,
    lang: Option<String>,
    #[serde(default, rename = "pdfA")]
    pdf_a: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmFont {
    family: String,
    /// Base64-encoded TTF bytes.
    data: String,
    weight: Option<u32>,
    italic: Option<bool>,
}

/// The result of a render: PDF bytes plus subset warnings.
#[wasm_bindgen]
pub struct HtmlRenderResult {
    pdf: Vec<u8>,
    warnings: Vec<String>,
}

#[wasm_bindgen]
impl HtmlRenderResult {
    #[wasm_bindgen(getter)]
    pub fn pdf(&self) -> Vec<u8> {
        self.pdf.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }
}

/// The result of a layout-bearing render: PDF bytes, the `LayoutInfo` as a
/// JSON string, and subset warnings.
#[wasm_bindgen]
pub struct HtmlLayoutRenderResult {
    pdf: Vec<u8>,
    layout_json: String,
    warnings: Vec<String>,
}

#[wasm_bindgen]
impl HtmlLayoutRenderResult {
    #[wasm_bindgen(getter)]
    pub fn pdf(&self) -> Vec<u8> {
        self.pdf.clone()
    }

    /// The `LayoutInfo` serialized as JSON — `JSON.parse` on the JS side.
    #[wasm_bindgen(getter)]
    pub fn layout_json(&self) -> String {
        self.layout_json.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }
}

/// Parse the JSON options blob into engine `HtmlOptions`. Shared by both
/// entry points so page-size / font / margin handling can't drift.
fn parse_options(options_json: &str) -> Result<HtmlOptions, JsValue> {
    let raw: WasmOptions = if options_json.trim().is_empty() {
        WasmOptions::default()
    } else {
        serde_json::from_str(options_json)
            .map_err(|e| JsValue::from_str(&format!("invalid options: {e}")))?
    };

    let mut options = HtmlOptions {
        page_margin: raw.page_margin,
        css: raw.css,
        tagged: raw.tagged,
        pdf_ua: raw.pdf_ua,
        lang: raw.lang,
        pdf_a: raw.pdf_a,
        ..Default::default()
    };
    for f in raw.fonts {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD
            .decode(&f.data)
            .map_err(|e| {
                JsValue::from_str(&format!("invalid font data for '{}': {e}", f.family))
            })?;
        options.fonts.push(crate::FontSpec {
            family: f.family,
            data,
            weight: f.weight.unwrap_or(400),
            italic: f.italic.unwrap_or(false),
        });
    }
    if let Some(size) = raw.page_size {
        options.page_size = Some(match size.to_ascii_lowercase().as_str() {
            "a4" => PageSize::A4,
            "a3" => PageSize::A3,
            "a5" => PageSize::A5,
            "letter" => PageSize::Letter,
            "legal" => PageSize::Legal,
            "tabloid" => PageSize::Tabloid,
            other => {
                return Err(JsValue::from_str(&format!("unknown page size '{other}'")));
            }
        });
    }
    Ok(options)
}

/// Render an HTML string to PDF. `options_json` may be empty or `"{}"`.
#[wasm_bindgen]
pub fn render_html_wasm(html: &str, options_json: &str) -> Result<HtmlRenderResult, JsValue> {
    let options = parse_options(options_json)?;
    let out = render_html(html, &options)
        .map_err(|e| JsValue::from_str(&format!("render failed: {e}")))?;
    Ok(HtmlRenderResult {
        pdf: out.pdf,
        warnings: out.warnings,
    })
}

/// Render an HTML string to PDF *and* its `LayoutInfo`. The layout drives
/// the VS Code extension's tree, inspector, and overlays. `options_json`
/// may be empty or `"{}"`.
#[wasm_bindgen]
pub fn render_html_wasm_with_layout(
    html: &str,
    options_json: &str,
) -> Result<HtmlLayoutRenderResult, JsValue> {
    let options = parse_options(options_json)?;
    let out = render_html_with_layout(html, &options)
        .map_err(|e| JsValue::from_str(&format!("render failed: {e}")))?;
    let layout_json = serde_json::to_string(&out.layout)
        .map_err(|e| JsValue::from_str(&format!("layout serialization failed: {e}")))?;
    Ok(HtmlLayoutRenderResult {
        pdf: out.pdf,
        layout_json,
        warnings: out.warnings,
    })
}
