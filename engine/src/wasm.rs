use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_pdf(json: &str) -> Result<Vec<u8>, JsValue> {
    crate::render_json(json)
        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))
}
