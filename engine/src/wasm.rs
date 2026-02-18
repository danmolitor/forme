use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_pdf(json: &str) -> Result<Vec<u8>, JsValue> {
    crate::render_json(json).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn render_pdf_with_layout(json: &str) -> Result<JsValue, JsValue> {
    let (pdf_bytes, layout_info) =
        crate::render_json_with_layout(json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let result = js_sys::Object::new();
    let pdf_array = js_sys::Uint8Array::from(pdf_bytes.as_slice());
    let layout = serde_wasm_bindgen::to_value(&layout_info)
        .map_err(|e| JsValue::from_str(&format!("Layout serialization error: {}", e)))?;

    js_sys::Reflect::set(&result, &JsValue::from_str("pdf"), &pdf_array)?;
    js_sys::Reflect::set(&result, &JsValue::from_str("layout"), &layout)?;

    Ok(result.into())
}
