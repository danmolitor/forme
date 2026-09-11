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

use crate::{render_html, render_html_with_layout, HtmlOptions};
use wasm_bindgen::prelude::*;

/// The result of a render: PDF bytes plus subset warnings.
#[wasm_bindgen]
pub struct HtmlRenderResult {
    pdf: Vec<u8>,
    warnings: Vec<String>,
    passes: u32,
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

    /// Number of layout passes the render took (benchmark evidence — see
    /// `benchmarks/`). 1 for the common case; 2–3 only when a page-number
    /// sentinel's reserved width needed correction.
    #[wasm_bindgen(getter)]
    pub fn passes(&self) -> u32 {
        self.passes
    }
}

/// The result of a layout-bearing render: PDF bytes, the `LayoutInfo` as a
/// JSON string, and subset warnings.
#[wasm_bindgen]
pub struct HtmlLayoutRenderResult {
    pdf: Vec<u8>,
    layout_json: String,
    warnings: Vec<String>,
    passes: u32,
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

    /// Number of layout passes the render took — identical semantics to
    /// [`HtmlRenderResult::passes`].
    #[wasm_bindgen(getter)]
    pub fn passes(&self) -> u32 {
        self.passes
    }
}

/// Parse the JSON options blob into engine `HtmlOptions`, mapping the shared
/// parser's error into a `JsValue`. The actual option surface lives in
/// `crate::options_wire` so it stays identical to the Python C-ABI.
fn parse_options(options_json: &str) -> Result<HtmlOptions, JsValue> {
    crate::options_wire::parse_options(options_json).map_err(|e| JsValue::from_str(&e))
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
        passes: out.passes,
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
        passes: out.passes,
    })
}
