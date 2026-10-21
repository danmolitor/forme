//! Shared options wire for the render bindings.
//!
//! Both the JS (`wasm`, wasm-bindgen) and Python (`wasm-raw`, C-ABI) paths
//! deserialize the SAME camelCase JSON into engine `HtmlOptions` through this
//! one function — so the option surface, and therefore the output bytes, can't
//! drift between languages. Mirror any new option in exactly one place.

use crate::{HtmlOptions, PageSize};
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmOptions<'a> {
    page_size: Option<String>,
    page_margin: Option<f64>,
    css: Option<String>,
    #[serde(default, borrow)]
    fonts: Vec<WasmFont<'a>>,
    #[serde(default)]
    tagged: bool,
    #[serde(default)]
    pdf_ua: bool,
    #[serde(default)]
    pdf_ua2: bool,
    lang: Option<String>,
    #[serde(default, rename = "pdfA")]
    pdf_a: Option<String>,
    #[serde(default)]
    embed_standard_fonts: bool,
    #[serde(default)]
    audit_content: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmFont<'a> {
    family: String,
    /// Base64-encoded TTF bytes, borrowed from the options JSON: base64 needs
    /// no unescaping, so serde hands back a slice instead of copying ~470 KB
    /// a face into a String that lived beside the decoded bytes (#197).
    #[serde(borrow)]
    data: Cow<'a, str>,
    weight: Option<u32>,
    italic: Option<bool>,
}

/// Parse the JSON options blob into engine `HtmlOptions`. Empty or `"{}"` gives
/// defaults. Errors are plain strings; each C-ABI maps them to its own error
/// type.
pub(crate) fn parse_options(options_json: &str) -> Result<HtmlOptions, String> {
    let raw: WasmOptions = if options_json.trim().is_empty() {
        WasmOptions::default()
    } else {
        serde_json::from_str(options_json).map_err(|e| format!("invalid options: {e}"))?
    };

    let mut options = HtmlOptions {
        page_margin: raw.page_margin,
        css: raw.css,
        tagged: raw.tagged,
        pdf_ua: raw.pdf_ua,
        pdf_ua2: raw.pdf_ua2,
        lang: raw.lang,
        pdf_a: raw.pdf_a,
        embed_standard_fonts: raw.embed_standard_fonts,
        audit_content: raw.audit_content,
        ..Default::default()
    };
    for f in raw.fonts {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD
            .decode(f.data.as_bytes())
            .map_err(|e| format!("invalid font data for '{}': {e}", f.family))?;
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
            other => return Err(format!("unknown page size '{other}'")),
        });
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::parse_options;

    #[test]
    fn embed_standard_fonts_reaches_the_options() {
        assert!(
            parse_options(r#"{"embedStandardFonts": true}"#)
                .unwrap()
                .embed_standard_fonts
        );
        assert!(!parse_options("{}").unwrap().embed_standard_fonts);
    }
}
