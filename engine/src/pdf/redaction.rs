//! # PDF Redaction
//!
//! Public entry points: [`redact_pdf`] (regions), [`find_text_regions`]
//! (search) and [`redact_text`] (search + redact). The work is done by
//! [`super::redact_engine`]; see its documentation for what is removed and
//! when redaction is refused.
//!
//! Every successful redaction returns a complete rewrite of the document as a
//! single revision holding only reachable objects. The input file is never
//! carried into the output (an incremental update would keep the original
//! content readable in the earlier revision).

use std::collections::HashMap;

use crate::error::FormeError;
#[cfg(test)]
use crate::model::PatternType;
use crate::model::{RedactionPattern, RedactionRegion};

/// Redact the given regions (top-origin points, 0-indexed pages).
///
/// With no regions the input is returned unchanged.
pub fn redact_pdf(pdf_bytes: &[u8], regions: &[RedactionRegion]) -> Result<Vec<u8>, FormeError> {
    if regions.is_empty() {
        return Ok(pdf_bytes.to_vec());
    }
    super::redact_engine::redact(pdf_bytes, regions, &[]).map(|(out, _)| out)
}

/// Find text matching `patterns`. Literal patterns match case-insensitively.
/// Text is matched as it reads on the page, across text operators and blocks.
pub fn find_text_regions(
    pdf_bytes: &[u8],
    patterns: &[RedactionPattern],
) -> Result<Vec<RedactionRegion>, FormeError> {
    super::redact_engine::find_text_regions(pdf_bytes, patterns)
}

/// Find text matching `patterns` and redact it: on the pages, and in
/// annotations, bookmarks, structure alternate text and metadata.
///
/// When nothing matches anywhere, the input is returned unchanged.
pub fn redact_text(pdf_bytes: &[u8], patterns: &[RedactionPattern]) -> Result<Vec<u8>, FormeError> {
    let regions = find_text_regions(pdf_bytes, patterns)?;
    let (out, changed) = super::redact_engine::redact(pdf_bytes, &regions, patterns)?;
    if regions.is_empty() && !changed {
        return Ok(pdf_bytes.to_vec());
    }
    Ok(out)
}

/// Parse a ToUnicode CMap text into a CID → Unicode string table.
///
/// Supports the common `bfchar` and `bfrange` sections. The engine only emits
/// `bfchar` today but other PDF producers emit `bfrange` so we handle both so
/// this works on imported/mixed PDFs too.
pub(super) fn parse_tounicode_cmap(text: &str) -> HashMap<u16, String> {
    let mut map: HashMap<u16, String> = HashMap::new();

    // bfchar: repeating "<CID_HEX> <UNICODE_HEX>" pairs.
    let mut cursor = 0;
    while let Some(start) = text[cursor..].find("beginbfchar") {
        let abs_start = cursor + start + "beginbfchar".len();
        let Some(end_rel) = text[abs_start..].find("endbfchar") else {
            break;
        };
        let body = &text[abs_start..abs_start + end_rel];
        for (cid, uni) in extract_angle_hex_pairs(body) {
            let Some(key) = hex_to_u16(&cid) else {
                continue;
            };
            let Some(value) = hex_to_string(&uni) else {
                continue;
            };
            map.insert(key, value);
        }
        cursor = abs_start + end_rel + "endbfchar".len();
    }

    // bfrange: "<CID_START> <CID_END> <UNICODE_START>" or
    //         "<CID_START> <CID_END> [ <UNICODE_1> <UNICODE_2> ... ]"
    cursor = 0;
    while let Some(start) = text[cursor..].find("beginbfrange") {
        let abs_start = cursor + start + "beginbfrange".len();
        let Some(end_rel) = text[abs_start..].find("endbfrange") else {
            break;
        };
        let body = &text[abs_start..abs_start + end_rel];
        parse_bfrange_body(body, &mut map);
        cursor = abs_start + end_rel + "endbfrange".len();
    }

    map
}

/// Extract pairs of `<HEX>` tokens from a CMap bfchar body.
fn extract_angle_hex_pairs(body: &str) -> Vec<(String, String)> {
    let tokens = extract_angle_hex_tokens(body);
    let mut pairs = Vec::new();
    let mut i = 0;
    while i + 1 < tokens.len() {
        pairs.push((tokens[i].clone(), tokens[i + 1].clone()));
        i += 2;
    }
    pairs
}

/// Extract every `<...>` token from a CMap section body, preserving order.
fn extract_angle_hex_tokens(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'>' {
                j += 1;
            }
            if j < bytes.len() {
                let token: String = bytes[start..j]
                    .iter()
                    .filter(|b| !b.is_ascii_whitespace())
                    .map(|&b| b as char)
                    .collect();
                out.push(token);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Parse the body of a `beginbfrange ... endbfrange` section into CID→Unicode
/// entries. Each entry is three tokens; the third can be either a single
/// `<HEX>` (incrementing range) or a `[<HEX> <HEX> ...]` array (explicit).
fn parse_bfrange_body(body: &str, map: &mut HashMap<u16, String>) {
    // Walk the body looking for triples. This is a tokenizer for:
    //   <HEX> <HEX> <HEX>               (incrementing)
    //   <HEX> <HEX> [ <HEX> ... ]       (explicit list)
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut group: Vec<String> = Vec::new();
    let mut array_values: Option<Vec<String>> = None;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'<' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'>' {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            let token: String = bytes[start..j]
                .iter()
                .filter(|b| !b.is_ascii_whitespace())
                .map(|&b| b as char)
                .collect();
            if let Some(arr) = array_values.as_mut() {
                arr.push(token);
            } else {
                group.push(token);
                // An incrementing triple is complete: apply it now. (Applying
                // only a trailing triple lost every range but the last one
                // when a section had several, as Chrome writes them.)
                if group.len() == 3 {
                    apply_incrementing_range(&group, map);
                    group.clear();
                }
            }
            i = j + 1;
            continue;
        }
        if b == b'[' {
            array_values = Some(Vec::new());
            i += 1;
            continue;
        }
        if b == b']' {
            if let Some(values) = array_values.take() {
                if group.len() == 2 {
                    let start_cid = hex_to_u16(&group[0]);
                    let end_cid = hex_to_u16(&group[1]);
                    if let (Some(start_cid), Some(end_cid)) = (start_cid, end_cid) {
                        let count = (end_cid as usize).saturating_sub(start_cid as usize) + 1;
                        for (k, uni_hex) in values.iter().take(count).enumerate() {
                            if let Some(s) = hex_to_string(uni_hex) {
                                map.insert(start_cid + k as u16, s);
                            }
                        }
                    }
                }
                group.clear();
            }
            i += 1;
            continue;
        }
        i += 1;
    }
}

/// `<start_cid> <end_cid> <first_unicode>`: consecutive codes map to
/// consecutive values; the last character of the destination increments
/// (so a multi-character destination such as a ligature works too).
fn apply_incrementing_range(group: &[String], map: &mut HashMap<u16, String>) {
    let (Some(start_cid), Some(end_cid), Some(first)) = (
        hex_to_u16(&group[0]),
        hex_to_u16(&group[1]),
        hex_to_string(&group[2]),
    ) else {
        return;
    };
    let count = (end_cid as usize).saturating_sub(start_cid as usize) + 1;
    let mut chars: Vec<char> = first.chars().collect();
    let Some(&last) = chars.last() else { return };
    for k in 0..count.min(65536) {
        if let Some(ch) = char::from_u32(last as u32 + k as u32) {
            *chars.last_mut().unwrap() = ch;
            map.insert(start_cid.wrapping_add(k as u16), chars.iter().collect());
        }
    }
}

/// Parse a hex string into a u16 (truncates if longer, pads if shorter).
fn hex_to_u16(hex: &str) -> Option<u16> {
    if hex.is_empty() {
        return None;
    }
    u32::from_str_radix(hex, 16).ok().map(|v| v as u16)
}

/// Parse a hex string representing a sequence of UTF-16BE code units into a
/// Unicode string. Each 4-hex-char pair is a 16-bit code unit; surrogate pairs
/// combine to form characters above U+FFFF.
fn hex_to_string(hex: &str) -> Option<String> {
    if hex.is_empty() || !hex.len().is_multiple_of(4) {
        // Short/odd hex: interpret as a single code point (best-effort).
        let code = u32::from_str_radix(hex, 16).ok()?;
        return char::from_u32(code).map(|c| c.to_string());
    }
    let mut units: Vec<u16> = Vec::new();
    let chars: Vec<char> = hex.chars().collect();
    let mut i = 0;
    while i + 3 < chars.len() {
        let chunk: String = chars[i..i + 4].iter().collect();
        let unit = u16::from_str_radix(&chunk, 16).ok()?;
        units.push(unit);
        i += 4;
    }
    String::from_utf16(&units).ok()
}

// ── Color parsing ───────────────────────────────────────────────────

/// Parse a hex color string to (r, g, b) in 0-1 range for PDF operators.
pub(super) fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
    } else {
        (0.0, 0.0, 0.0) // Default to black
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniz_oxide::inflate::decompress_to_vec_zlib;

    #[test]
    fn test_parse_hex_color() {
        let (r, g, b) = parse_hex_color("#000000");
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));

        let (r, g, b) = parse_hex_color("#ffffff");
        assert_eq!((r, g, b), (1.0, 1.0, 1.0));

        let (r, g, b) = parse_hex_color("#ff0000");
        assert_eq!((r, g, b), (1.0, 0.0, 0.0));
    }

    #[test]
    fn test_redact_empty_regions() {
        let pdf = b"%PDF-1.7\nsome content";
        let result = redact_pdf(pdf, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), pdf.to_vec());
    }

    #[test]
    fn test_redact_integration() {
        // Render a simple document to get valid PDF bytes
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Hello, world!",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let regions = vec![RedactionRegion {
            page: 0,
            x: 50.0,
            y: 50.0,
            width: 200.0,
            height: 30.0,
            color: None,
        }];

        let result = redact_pdf(&pdf_bytes, &regions).expect("redact should succeed");

        // Verify the output is larger than the input (incremental update was appended)
        assert!(result.len() > pdf_bytes.len());

        // Verify it starts with %PDF
        assert!(result.starts_with(b"%PDF"));

        // The page content draws the opaque box.
        assert!(any_stream_contains(&result, "re f Q"));

        // A complete rewrite: one revision, no chain to the input file.
        let text = String::from_utf8_lossy(&result);
        assert_eq!(text.matches("%%EOF").count(), 1, "single revision");
        assert!(
            !text[text.rfind("trailer").unwrap()..].contains("/Prev"),
            "no /Prev"
        );
        assert!(
            !result.starts_with(&pdf_bytes),
            "the input file is not carried over"
        );
    }

    #[test]
    fn test_redact_invalid_page() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Test",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let regions = vec![RedactionRegion {
            page: 5, // Invalid — only 1 page
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 20.0,
            color: None,
        }];

        let result = redact_pdf(&pdf_bytes, &regions);
        assert!(result.is_err());
    }

    #[test]
    fn test_redact_removes_text_from_content_stream() {
        // Render a document with known text
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Hello, world!",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        // Verify text exists in the original PDF
        let original_has_text = pdf_contains_text_showing_ops(&pdf_bytes, "Hello");
        assert!(
            original_has_text,
            "Original PDF should contain text-showing operators"
        );

        // Redact a large region covering the entire page content area
        // Default page is 595.28 x 841.89 (A4), text starts near top-left
        let regions = vec![RedactionRegion {
            page: 0,
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 100.0,
            color: None,
        }];

        let result = redact_pdf(&pdf_bytes, &regions).expect("redact should succeed");

        // The NEW replacement content stream (in the incremental update portion)
        // should NOT contain text-showing operators.
        // The original stream is still in the file but no longer referenced.
        // Nowhere in the file — not only in new objects. (Before the fix,
        // the original stream stayed in the file, unreferenced.)
        assert!(
            !pdf_contains_text_showing_ops(&result, "Hello"),
            "No stream in the output may still show 'Hello'"
        );

        // The visual overlay is still drawn
        assert!(
            any_stream_contains(&result, "re f Q"),
            "Overlay rectangle should be present"
        );
    }

    #[test]
    fn test_redact_preserves_text_outside_region() {
        // Render a document with text at known position
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style {
                    font_size: Some(12.0),
                    ..crate::style::Style::default()
                },
                vec![
                    crate::model::Node::text("Keep this text", crate::style::Style::default()),
                    // Add a spacer view to push second text down
                    crate::model::Node::view(
                        crate::style::Style {
                            height: Some(crate::style::Dimension::Pt(200.0)),
                            ..crate::style::Style::default()
                        },
                        vec![],
                    ),
                    crate::model::Node::text("Remove this text", crate::style::Style::default()),
                ],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        // Redact only the lower region (where "Remove this text" is)
        // y=220 in top-origin coords (past the spacer)
        let regions = vec![RedactionRegion {
            page: 0,
            x: 0.0,
            y: 220.0,
            width: 600.0,
            height: 50.0,
            color: None,
        }];

        let result = redact_pdf(&pdf_bytes, &regions).expect("redact should succeed");

        // The result should still be valid PDF
        assert!(result.starts_with(b"%PDF"));

        // A complete rewrite (no incremental update chained to the input)
        let text = String::from_utf8_lossy(&result);
        assert_eq!(text.matches("%%EOF").count(), 1);
    }

    #[test]
    fn test_redact_custom_color() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Color test",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let regions = vec![RedactionRegion {
            page: 0,
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 50.0,
            color: Some("#ff0000".to_string()),
        }];

        let result = redact_pdf(&pdf_bytes, &regions).expect("redact should succeed");
        // The box is drawn in red (1 0 0 rg) in the page content
        assert!(any_stream_contains(&result, "1 0 0 rg"));
    }

    #[test]
    fn test_redact_strips_metadata() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Secret doc",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata {
                title: Some("Confidential Report".to_string()),
                author: Some("John Doe".to_string()),
                creator: Some("SecretApp".to_string()),
                ..Default::default()
            },
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        // Verify original has metadata
        let original_text = String::from_utf8_lossy(&pdf_bytes);
        assert!(
            original_text.contains("Confidential Report"),
            "Original PDF should contain title"
        );
        assert!(
            original_text.contains("John Doe"),
            "Original PDF should contain author"
        );

        // Redact any region
        let regions = vec![RedactionRegion {
            page: 0,
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 50.0,
            color: None,
        }];
        let result = redact_pdf(&pdf_bytes, &regions).expect("redact should succeed");

        // The whole output: /Info is replaced by a minimal one, and the
        // original values appear nowhere (not only in new objects).
        let whole = String::from_utf8_lossy(&result);
        assert!(
            whole.contains("/Producer (Forme)"),
            "Replacement /Info should have /Producer (Forme)"
        );
        assert!(
            !whole.contains("Confidential Report"),
            "Original title must not appear anywhere"
        );
        assert!(
            !whole.contains("John Doe"),
            "Original author must not appear anywhere"
        );
        assert!(
            !whole.contains("SecretApp"),
            "Original creator must not appear anywhere"
        );
        assert!(
            !any_stream_contains(&result, "Confidential Report"),
            "nor in XMP or any stream"
        );
    }

    // ── Text-search redaction tests ──────────────────────────────────────

    #[test]
    fn test_find_text_literal() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "John Smith works at Acme Corp",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let patterns = vec![RedactionPattern {
            pattern: "john smith".to_string(), // lowercase — should match case-insensitive
            pattern_type: PatternType::Literal,
            page: None,
            color: None,
        }];

        let regions = find_text_regions(&pdf_bytes, &patterns).expect("should succeed");
        assert!(
            !regions.is_empty(),
            "Should find 'John Smith' via case-insensitive literal search"
        );
        assert_eq!(regions[0].page, 0);
        assert!(regions[0].width > 0.0);
        assert!(regions[0].height > 0.0);
    }

    #[test]
    fn test_find_text_no_match() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Hello World",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let patterns = vec![RedactionPattern {
            pattern: "xyznotfound".to_string(),
            pattern_type: PatternType::Literal,
            page: None,
            color: None,
        }];

        let regions = find_text_regions(&pdf_bytes, &patterns).expect("should succeed");
        assert!(regions.is_empty(), "Should return empty vec, not error");
    }

    #[cfg(feature = "regex")]
    #[test]
    fn test_find_text_regex() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "SSN: 123-45-6789 and phone 555-0100",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let patterns = vec![RedactionPattern {
            pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
            pattern_type: PatternType::Regex,
            page: None,
            color: None,
        }];

        let regions = find_text_regions(&pdf_bytes, &patterns).expect("should succeed");
        assert_eq!(
            regions.len(),
            1,
            "Should find exactly one SSN-pattern match"
        );
    }

    #[test]
    fn test_redact_text_end_to_end() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Confidential: John Smith SSN 123-45-6789",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let patterns = vec![RedactionPattern {
            pattern: "John Smith".to_string(),
            pattern_type: PatternType::Literal,
            page: None,
            color: Some("#ff0000".to_string()),
        }];

        let result = redact_text(&pdf_bytes, &patterns).expect("redact_text should succeed");
        assert!(
            result.len() > pdf_bytes.len(),
            "Should produce incremental update"
        );
        assert!(result.starts_with(b"%PDF"));
    }

    #[test]
    fn test_redact_text_no_match_returns_original() {
        let doc = crate::model::Document {
            children: vec![crate::model::Node::page(
                crate::model::PageConfig::default(),
                crate::style::Style::default(),
                vec![crate::model::Node::text(
                    "Hello World",
                    crate::style::Style::default(),
                )],
            )],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        let patterns = vec![RedactionPattern {
            pattern: "notfound".to_string(),
            pattern_type: PatternType::Literal,
            page: None,
            color: None,
        }];

        let result = redact_text(&pdf_bytes, &patterns).expect("should succeed");
        assert_eq!(
            result, pdf_bytes,
            "No match → return original bytes unchanged"
        );
    }

    #[test]
    fn test_parse_tounicode_cmap_bfchar() {
        let cmap = r#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo
<< /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Inter-UTF16 def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
3 beginbfchar
<0042> <0048>
<0043> <0065>
<0044> <006C>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;
        let map = parse_tounicode_cmap(cmap);
        assert_eq!(map.get(&0x0042).map(String::as_str), Some("H"));
        assert_eq!(map.get(&0x0043).map(String::as_str), Some("e"));
        assert_eq!(map.get(&0x0044).map(String::as_str), Some("l"));
    }

    #[test]
    fn test_parse_tounicode_cmap_several_incrementing_ranges() {
        // Chrome writes several incrementing ranges in one section; every
        // range must apply, not only the last.
        let cmap = "4 beginbfrange\n<0014> <001D> <0031>\n<0027> <0028> <0044>\n\
                    <004E> <0053> <006B>\n<0055> <0058> <0072>\nendbfrange\n";
        let m = parse_tounicode_cmap(cmap);
        assert_eq!(m.get(&0x0014).map(String::as_str), Some("1"));
        assert_eq!(m.get(&0x001D).map(String::as_str), Some(":"));
        assert_eq!(m.get(&0x0028).map(String::as_str), Some("E"));
        assert_eq!(m.get(&0x0051).map(String::as_str), Some("n"));
        assert_eq!(m.get(&0x0058).map(String::as_str), Some("u"));
        assert_eq!(m.len(), 10 + 2 + 6 + 4);
    }

    #[test]
    fn test_parse_tounicode_cmap_bfrange() {
        // Range form: <start> <end> <unicode_start> — CIDs 0001..=0003 → 'A'..='C'
        let cmap = r#"
begincmap
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 beginbfrange
<0001> <0003> <0041>
endbfrange
endcmap
"#;
        let map = parse_tounicode_cmap(cmap);
        assert_eq!(map.get(&0x0001).map(String::as_str), Some("A"));
        assert_eq!(map.get(&0x0002).map(String::as_str), Some("B"));
        assert_eq!(map.get(&0x0003).map(String::as_str), Some("C"));
    }

    #[test]
    fn test_find_text_page_filter() {
        // Two-page document
        let doc = crate::model::Document {
            children: vec![
                crate::model::Node::page(
                    crate::model::PageConfig::default(),
                    crate::style::Style::default(),
                    vec![crate::model::Node::text(
                        "Page one content",
                        crate::style::Style::default(),
                    )],
                ),
                crate::model::Node::page(
                    crate::model::PageConfig::default(),
                    crate::style::Style::default(),
                    vec![crate::model::Node::text(
                        "Page two content",
                        crate::style::Style::default(),
                    )],
                ),
            ],
            metadata: crate::model::Metadata::default(),
            default_page: crate::model::PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            attachments: vec![],
            zugferd: None,
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
            pdf_version: Default::default(),
            pdf_ua2: false,
            embed_standard_fonts: false,
        };

        let pdf_bytes = crate::render(&doc).expect("render should succeed");

        // Search only page 1 for "content" — should find it
        let patterns = vec![RedactionPattern {
            pattern: "content".to_string(),
            pattern_type: PatternType::Literal,
            page: Some(1),
            color: None,
        }];

        let regions = find_text_regions(&pdf_bytes, &patterns).expect("should succeed");
        for r in &regions {
            assert_eq!(r.page, 1, "All matches should be on page 1");
        }
    }

    /// Helper: check if a PDF's content streams contain text-showing operators
    /// (Tj/TJ) with the given needle string.
    ///
    /// When `after_offset` is provided, only checks streams that start after
    /// that byte offset (useful for checking only incremental update streams).
    fn pdf_contains_text_showing_ops_after(pdf: &[u8], needle: &str, after_offset: usize) -> bool {
        let mut pos = after_offset;
        while pos < pdf.len() {
            if let Some(stream_pos) = find_bytes(&pdf[pos..], b"stream\n") {
                let abs_pos = pos + stream_pos + 7;
                if let Some(end_pos) = find_bytes(&pdf[abs_pos..], b"endstream") {
                    let stream_data = &pdf[abs_pos..abs_pos + end_pos];

                    let decompressed = decompress_to_vec_zlib(stream_data)
                        .unwrap_or_else(|_| stream_data.to_vec());

                    let stream_text = String::from_utf8_lossy(&decompressed);
                    if (stream_text.contains("Tj") || stream_text.contains("TJ"))
                        && stream_text.contains(needle)
                    {
                        return true;
                    }

                    pos = abs_pos + end_pos;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        false
    }

    fn pdf_contains_text_showing_ops(pdf: &[u8], needle: &str) -> bool {
        pdf_contains_text_showing_ops_after(pdf, needle, 0)
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    /// True when any (inflated) stream in the file contains `needle`.
    fn any_stream_contains(pdf: &[u8], needle: &str) -> bool {
        let mut pos = 0;
        while let Some(s) = find_bytes(&pdf[pos..], b"stream\n") {
            let start = pos + s + 7;
            let Some(e) = find_bytes(&pdf[start..], b"endstream") else {
                break;
            };
            let data = &pdf[start..start + e];
            let inflated = decompress_to_vec_zlib(data).unwrap_or_else(|_| data.to_vec());
            if String::from_utf8_lossy(&inflated).contains(needle) {
                return true;
            }
            pos = start + e;
        }
        false
    }
}
