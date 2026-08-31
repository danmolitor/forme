use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn render_pdf(json: &str) -> Result<Vec<u8>, JsValue> {
    crate::render_json(json).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Certify PDF bytes with an X.509 certificate (PKCS#7 detached signature).
#[wasm_bindgen]
pub fn certify_pdf(pdf_bytes: &[u8], config_json: &str) -> Result<Vec<u8>, JsValue> {
    let config: crate::model::CertificationConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    crate::certify_pdf(pdf_bytes, &config).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Merge multiple PDFs into a single document.
/// Accepts a JSON array of base64-encoded PDF strings.
#[wasm_bindgen]
pub fn merge_pdfs(pdfs_json: &str) -> Result<Vec<u8>, JsValue> {
    let b64_pdfs: Vec<String> =
        serde_json::from_str(pdfs_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let decoded: Vec<Vec<u8>> = b64_pdfs
        .iter()
        .map(|s| {
            b64.decode(s)
                .map_err(|e| JsValue::from_str(&format!("Invalid base64: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let refs: Vec<&[u8]> = decoded.iter().map(|v| v.as_slice()).collect();
    crate::merge_pdfs(&refs).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Redact regions of a PDF by overlaying opaque rectangles.
#[wasm_bindgen]
pub fn redact_pdf(pdf_bytes: &[u8], redactions_json: &str) -> Result<Vec<u8>, JsValue> {
    let regions: Vec<crate::model::RedactionRegion> =
        serde_json::from_str(redactions_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    crate::redact_pdf(pdf_bytes, &regions).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Find text regions matching patterns in a PDF.
/// Returns JSON string of Vec<RedactionRegion>.
#[wasm_bindgen]
pub fn find_text_regions(pdf_bytes: &[u8], patterns_json: &str) -> Result<String, JsValue> {
    let patterns: Vec<crate::model::RedactionPattern> =
        serde_json::from_str(patterns_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let regions = crate::find_text_regions(pdf_bytes, &patterns)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&regions).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Redact text matching patterns from a PDF.
#[wasm_bindgen]
pub fn redact_text(pdf_bytes: &[u8], patterns_json: &str) -> Result<Vec<u8>, JsValue> {
    let patterns: Vec<crate::model::RedactionPattern> =
        serde_json::from_str(patterns_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    crate::redact_text(pdf_bytes, &patterns).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn render_pdf_with_layout(json: &str) -> Result<JsValue, JsValue> {
    let (pdf_bytes, layout_info, warnings) =
        crate::render_json_with_layout(json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let result = js_sys::Object::new();
    let pdf_array = js_sys::Uint8Array::from(pdf_bytes.as_slice());
    let layout = serde_wasm_bindgen::to_value(&layout_info)
        .map_err(|e| JsValue::from_str(&format!("Layout serialization error: {}", e)))?;
    let warnings_arr = serde_wasm_bindgen::to_value(&warnings)
        .map_err(|e| JsValue::from_str(&format!("Warnings serialization error: {}", e)))?;

    js_sys::Reflect::set(&result, &JsValue::from_str("pdf"), &pdf_array)?;
    js_sys::Reflect::set(&result, &JsValue::from_str("layout"), &layout)?;
    js_sys::Reflect::set(&result, &JsValue::from_str("warnings"), &warnings_arr)?;

    Ok(result.into())
}

#[wasm_bindgen]
pub fn render_template_pdf(template_json: &str, data_json: &str) -> Result<Vec<u8>, JsValue> {
    crate::render_template(template_json, data_json).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn render_template_pdf_with_layout(
    template_json: &str,
    data_json: &str,
) -> Result<JsValue, JsValue> {
    let (pdf_bytes, layout_info) = crate::render_template_with_layout(template_json, data_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let result = js_sys::Object::new();
    let pdf_array = js_sys::Uint8Array::from(pdf_bytes.as_slice());
    let layout = serde_wasm_bindgen::to_value(&layout_info)
        .map_err(|e| JsValue::from_str(&format!("Layout serialization error: {}", e)))?;

    js_sys::Reflect::set(&result, &JsValue::from_str("pdf"), &pdf_array)?;
    js_sys::Reflect::set(&result, &JsValue::from_str("layout"), &layout)?;

    Ok(result.into())
}
