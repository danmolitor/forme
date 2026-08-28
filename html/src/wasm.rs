//! WASM bindings for `@formepdf/html`.
//!
//! One entry point: `render_html_wasm(html, options_json)` → a result
//! object carrying PDF bytes and the warnings list. Options arrive as
//! JSON (`{"pageSize": "Letter", "pageMargin": 36, "css": "..."}`) so the
//! JS wrapper stays a thin shim.

use crate::{render_html, HtmlOptions, PageSize};
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

/// Render an HTML string to PDF. `options_json` may be empty or `"{}"`.
#[wasm_bindgen]
pub fn render_html_wasm(html: &str, options_json: &str) -> Result<HtmlRenderResult, JsValue> {
    let raw: WasmOptions = if options_json.trim().is_empty() {
        WasmOptions::default()
    } else {
        serde_json::from_str(options_json)
            .map_err(|e| JsValue::from_str(&format!("invalid options: {e}")))?
    };

    let mut options = HtmlOptions {
        page_margin: raw.page_margin,
        css: raw.css,
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

    let out = render_html(html, &options)
        .map_err(|e| JsValue::from_str(&format!("render failed: {e}")))?;
    Ok(HtmlRenderResult {
        pdf: out.pdf,
        warnings: out.warnings,
    })
}
