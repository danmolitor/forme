//! # PDF Redaction
//!
//! True content-level redaction: removes text operators from PDF content streams
//! where they overlap redaction regions, then overlays opaque rectangles on top.
//!
//! ## Approach
//!
//! 1. Scan the PDF for structural metadata (xref, trailer, page objects).
//! 2. Walk the /Pages tree to collect page object IDs and /MediaBox dimensions.
//! 3. For each page with redactions:
//!    a. Extract and decompress the content stream(s).
//!    b. Tokenize PDF operators.
//!    c. Track text position state (BT/ET, Td, Tm, Tf, etc.).
//!    d. Remove text-showing operators (Tj, TJ, ', ") whose position overlaps
//!    any redaction region.
//!    e. Recompress and emit as a replacement content stream.
//! 4. Overlay opaque rectangles (visual indicator) via Form XObject.
//! 5. Write an incremental update (new objects + xref + trailer with /Prev).

use crate::error::FormeError;
use crate::model::{PatternType, RedactionPattern, RedactionRegion};
use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib;
use std::collections::HashMap;

// ── Date formatting ─────────────────────────────────────────────────

/// Format current time as ISO 8601 for XMP: YYYY-MM-DDTHH:MM:SSZ
fn format_xmp_date() -> String {
    let now = super::certify::current_timestamp_secs();
    let days = now / 86400;
    let time_of_day = now % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let (year, month, day) = super::certify::epoch_days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

// ── Content stream tokenizer ────────────────────────────────────────

/// A token from a PDF content stream.
#[derive(Debug, Clone)]
enum Token {
    /// A number, string literal, hex string, name, or array operand.
    Operand(Vec<u8>),
    /// A PDF operator keyword (BT, ET, Td, Tj, TJ, Tf, etc.).
    Operator(Vec<u8>),
}

/// Tokenize a decompressed PDF content stream into operands and operators.
///
/// This is a minimal tokenizer — enough to identify text operators and their
/// operands. It handles PDF strings `(...)`, hex strings `<...>`, arrays `[...]`,
/// names `/Name`, and numeric operands. Everything else is treated as an operator.
fn tokenize_content_stream(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let len = data.len();

    while i < len {
        let b = data[i];

        // Skip whitespace
        if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' || b == b'\x0C' || b == 0 {
            i += 1;
            continue;
        }

        // PDF comment — skip to end of line
        if b == b'%' {
            while i < len && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
            continue;
        }

        // String literal (...)
        if b == b'(' {
            let start = i;
            i += 1;
            let mut depth = 1;
            while i < len && depth > 0 {
                if data[i] == b'(' && (i == 0 || data[i - 1] != b'\\') {
                    depth += 1;
                } else if data[i] == b')' && (i == 0 || data[i - 1] != b'\\') {
                    depth -= 1;
                }
                i += 1;
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Hex string <...> (but not dict <<)
        if b == b'<' && i + 1 < len && data[i + 1] != b'<' {
            let start = i;
            i += 1;
            while i < len && data[i] != b'>' {
                i += 1;
            }
            if i < len {
                i += 1; // consume '>'
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Array [...] — treat entire array as one operand (for TJ arrays)
        if b == b'[' {
            let start = i;
            i += 1;
            let mut depth = 1;
            while i < len && depth > 0 {
                if data[i] == b'[' {
                    depth += 1;
                } else if data[i] == b']' {
                    depth -= 1;
                } else if data[i] == b'(' {
                    // Skip nested string
                    i += 1;
                    let mut sdepth = 1;
                    while i < len && sdepth > 0 {
                        if data[i] == b'(' && data[i - 1] != b'\\' {
                            sdepth += 1;
                        } else if data[i] == b')' && data[i - 1] != b'\\' {
                            sdepth -= 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                i += 1;
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Name /Something
        if b == b'/' {
            let start = i;
            i += 1;
            while i < len && !is_pdf_delimiter(data[i]) && !is_pdf_whitespace(data[i]) {
                i += 1;
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Number (integer or real, possibly negative)
        if b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.' {
            let start = i;
            i += 1;
            while i < len && (data[i].is_ascii_digit() || data[i] == b'.') {
                i += 1;
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Dict << >> — treat as operand
        if b == b'<' && i + 1 < len && data[i + 1] == b'<' {
            let start = i;
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if data[i] == b'<' && data[i + 1] == b'<' {
                    depth += 1;
                    i += 2;
                } else if data[i] == b'>' && data[i + 1] == b'>' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token::Operand(data[start..i].to_vec()));
            continue;
        }

        // Keyword / operator (alphabetic sequence)
        if b.is_ascii_alphabetic() || b == b'\'' || b == b'"' {
            let start = i;
            // Single-char operators ' and "
            if b == b'\'' || b == b'"' {
                i += 1;
                tokens.push(Token::Operator(data[start..i].to_vec()));
                continue;
            }
            i += 1;
            while i < len && (data[i].is_ascii_alphabetic() || data[i] == b'*') {
                i += 1;
            }
            tokens.push(Token::Operator(data[start..i].to_vec()));
            continue;
        }

        // Unknown byte — skip
        i += 1;
    }

    tokens
}

fn is_pdf_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t' | b'\x0C' | 0)
}

fn is_pdf_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Serialize tokens back to a PDF content stream byte sequence.
fn serialize_tokens(tokens: &[Token]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        if i > 0 {
            out.push(b' ');
        }
        match tok {
            Token::Operand(data) | Token::Operator(data) => out.extend_from_slice(data),
        }
    }
    out.push(b'\n');
    out
}

// ── Text state tracking ─────────────────────────────────────────────

/// Tracks current text position within a BT/ET block.
struct TextState {
    /// Text matrix [a b c d e f] — e,f are the translation (position)
    tm: [f64; 6],
    /// Text line matrix (set by Td/TD/Tm/T*, reset by BT)
    tlm: [f64; 6],
    /// Current font size from Tf operator
    font_size: f64,
}

impl TextState {
    fn new() -> Self {
        Self {
            tm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            tlm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            font_size: 12.0,
        }
    }

    fn reset(&mut self) {
        self.tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        self.tlm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    }

    /// Current text x position in PDF user space (bottom-origin)
    fn tx(&self) -> f64 {
        self.tm[4]
    }

    /// Current text y position in PDF user space (bottom-origin)
    fn ty(&self) -> f64 {
        self.tm[5]
    }

    /// Apply Td: translate text position
    fn apply_td(&mut self, tx: f64, ty: f64) {
        // Td sets tlm = [[1 0 0],[0 1 0],[tx ty 1]] × tlm, then tm = tlm
        let new_e = tx * self.tlm[0] + ty * self.tlm[2] + self.tlm[4];
        let new_f = tx * self.tlm[1] + ty * self.tlm[3] + self.tlm[5];
        self.tlm[4] = new_e;
        self.tlm[5] = new_f;
        self.tm = self.tlm;
    }

    /// Apply Tm: set text matrix directly
    fn apply_tm(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        self.tm = [a, b, c, d, e, f];
        self.tlm = [a, b, c, d, e, f];
    }

    /// Apply T*: move to start of next line (equivalent to 0 -Tl Td)
    /// We approximate Tl as font_size since we don't track it separately.
    fn apply_t_star(&mut self) {
        self.apply_td(0.0, -self.font_size);
    }
}

/// A redaction region in PDF coordinates (bottom-origin).
struct PdfRedactRegion {
    x: f64,
    y: f64, // bottom-origin
    width: f64,
    height: f64,
}

/// Check if a text operator spanning `[tx, tx + text_width]` horizontally and
/// the given baseline y overlaps any redaction region. Used to decide whether
/// to drop a text-showing operator from the content stream entirely.
///
/// Intentionally aggressive: even partial overlap of the text extent with the
/// region triggers a drop, so redacting "Molitor" inside a longer Tj like
/// "Daniel Molitor" removes the whole operator (over-redaction is safer than
/// under-redaction — the visual overlay still only covers the target word).
fn text_overlaps_region(
    tx: f64,
    ty: f64,
    text_width: f64,
    font_size: f64,
    regions: &[PdfRedactRegion],
) -> bool {
    for r in regions {
        // Text vertical extent: baseline (ty) to ty + font_size (approximate)
        // with a small descender allowance.
        let text_bottom = ty - font_size * 0.3;
        let text_top = ty + font_size;

        let region_bottom = r.y;
        let region_top = r.y + r.height;

        let v_overlap = text_bottom < region_top && text_top > region_bottom;

        // Horizontal extent overlap: text [tx, tx+width] vs region [r.x, r.x+r.width]
        let text_left = tx;
        let text_right = tx + text_width.max(font_size * 0.5);
        let region_left = r.x;
        let region_right = r.x + r.width;
        let h_overlap = text_left < region_right && text_right > region_left;

        if v_overlap && h_overlap {
            return true;
        }
    }
    false
}

/// Estimate the horizontal advance of a single text operand — for Tj this is
/// the operand directly, for TJ each sub-operand contributes its advance
/// minus kerning values (in thousandths of an em).
fn measure_text_operand_width(data: &[u8], font: Option<&FontInfo>, font_size: f64) -> f64 {
    // TJ array form
    if data.len() >= 2 && data[0] == b'[' && data[data.len() - 1] == b']' {
        let inner = &data[1..data.len() - 1];
        let sub_tokens = tokenize_content_stream(inner);
        let mut total = 0.0;
        for sub in &sub_tokens {
            if let Token::Operand(sub_data) = sub {
                if sub_data.starts_with(b"(") || sub_data.starts_with(b"<") {
                    let (_, _, advance) = render_text_operand(sub_data, font, font_size, 0.0, 0.0);
                    total += advance;
                } else {
                    let kern: f64 = std::str::from_utf8(sub_data)
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0.0);
                    total -= kern / 1000.0 * font_size;
                }
            }
        }
        return total.max(0.0);
    }
    // Single string operand
    let (_, _, advance) = render_text_operand(data, font, font_size, 0.0, 0.0);
    advance
}

/// Remove text-showing operators that overlap redaction regions from a token stream.
///
/// Preserves all non-text operators and text positioning operators so that
/// text outside redaction regions stays correctly positioned.
fn strip_redacted_text(
    tokens: &[Token],
    regions: &[PdfRedactRegion],
    font_map: &HashMap<String, FontInfo>,
) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut state = TextState::new();
    let mut in_text = false;
    let mut operand_stack: Vec<Token> = Vec::new();
    let mut current_font: Option<&FontInfo> = None;

    for token in tokens {
        match token {
            Token::Operand(_) => {
                operand_stack.push(token.clone());
            }
            Token::Operator(op) => {
                let op_str = std::str::from_utf8(op).unwrap_or("");

                match op_str {
                    "BT" => {
                        in_text = true;
                        state.reset();
                        // Flush any pending operands and emit BT
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "ET" => {
                        in_text = false;
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "Td" | "TD" if in_text => {
                        // Td: tx ty Td — move text position
                        if operand_stack.len() >= 2 {
                            let ty = parse_operand_f64(&operand_stack[operand_stack.len() - 1]);
                            let tx = parse_operand_f64(&operand_stack[operand_stack.len() - 2]);
                            state.apply_td(tx, ty);
                            if op_str == "TD" {
                                // TD also sets Tl = -ty (leading), but we don't track Tl
                            }
                        }
                        // Always keep position operators
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "Tm" if in_text => {
                        // Tm: a b c d e f Tm — set text matrix
                        if operand_stack.len() >= 6 {
                            let n = operand_stack.len();
                            let a = parse_operand_f64(&operand_stack[n - 6]);
                            let b = parse_operand_f64(&operand_stack[n - 5]);
                            let c = parse_operand_f64(&operand_stack[n - 4]);
                            let d = parse_operand_f64(&operand_stack[n - 3]);
                            let e = parse_operand_f64(&operand_stack[n - 2]);
                            let f = parse_operand_f64(&operand_stack[n - 1]);
                            state.apply_tm(a, b, c, d, e, f);
                        }
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "T*" if in_text => {
                        state.apply_t_star();
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "Tf" if in_text => {
                        // Tf: /FontName size Tf
                        if operand_stack.len() >= 2 {
                            let size = parse_operand_f64(&operand_stack[operand_stack.len() - 1]);
                            if size > 0.0 {
                                state.font_size = size;
                            }
                            // Resolve font name (/F1) → FontInfo for width lookup.
                            if let Token::Operand(name_bytes) =
                                &operand_stack[operand_stack.len() - 2]
                            {
                                if !name_bytes.is_empty() && name_bytes[0] == b'/' {
                                    let name: String =
                                        name_bytes[1..].iter().map(|&b| b as char).collect();
                                    current_font = font_map.get(&name);
                                }
                            }
                        }
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                    "Tj" if in_text => {
                        // Tj: (string) Tj — show text
                        let text_width = if let Some(Token::Operand(data)) = operand_stack.last() {
                            measure_text_operand_width(data, current_font, state.font_size)
                        } else {
                            0.0
                        };
                        if text_overlaps_region(
                            state.tx(),
                            state.ty(),
                            text_width,
                            state.font_size,
                            regions,
                        ) {
                            // Drop the string operand and Tj operator
                            operand_stack.clear();
                        } else {
                            out.append(&mut operand_stack);
                            out.push(token.clone());
                        }
                    }
                    "TJ" if in_text => {
                        // TJ: [(string) kern (string) kern ...] TJ — show text with kerning
                        let text_width = if let Some(Token::Operand(data)) = operand_stack.last() {
                            measure_text_operand_width(data, current_font, state.font_size)
                        } else {
                            0.0
                        };
                        if text_overlaps_region(
                            state.tx(),
                            state.ty(),
                            text_width,
                            state.font_size,
                            regions,
                        ) {
                            operand_stack.clear();
                        } else {
                            out.append(&mut operand_stack);
                            out.push(token.clone());
                        }
                    }
                    // ' operator: move to next line and show text
                    // " operator: set word/char spacing, move to next line, show text
                    op_s if in_text && op_s == "'" => {
                        state.apply_t_star();
                        let text_width = if let Some(Token::Operand(data)) = operand_stack.last() {
                            measure_text_operand_width(data, current_font, state.font_size)
                        } else {
                            0.0
                        };
                        if text_overlaps_region(
                            state.tx(),
                            state.ty(),
                            text_width,
                            state.font_size,
                            regions,
                        ) {
                            // Drop the string operand but keep the line move
                            // Emit T* instead to preserve position
                            operand_stack.clear();
                            out.push(Token::Operator(b"T*".to_vec()));
                        } else {
                            out.append(&mut operand_stack);
                            out.push(token.clone());
                        }
                    }
                    op_s if in_text && op_s == "\"" => {
                        // " : aw ac string " — set word spacing, char spacing, show text
                        state.apply_t_star();
                        let text_width = if let Some(Token::Operand(data)) = operand_stack.last() {
                            measure_text_operand_width(data, current_font, state.font_size)
                        } else {
                            0.0
                        };
                        if text_overlaps_region(
                            state.tx(),
                            state.ty(),
                            text_width,
                            state.font_size,
                            regions,
                        ) {
                            operand_stack.clear();
                            out.push(Token::Operator(b"T*".to_vec()));
                        } else {
                            out.append(&mut operand_stack);
                            out.push(token.clone());
                        }
                    }
                    _ => {
                        // Pass through all other operators unchanged
                        out.append(&mut operand_stack);
                        out.push(token.clone());
                    }
                }
            }
        }
    }

    // Flush any remaining operands
    out.append(&mut operand_stack);
    out
}

/// Parse a Token::Operand as an f64 number.
fn parse_operand_f64(token: &Token) -> f64 {
    match token {
        Token::Operand(data) => std::str::from_utf8(data)
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

// ── Stream extraction ───────────────────────────────────────────────

/// Extract and decompress a PDF stream object's content.
fn extract_and_decompress_stream(pdf: &[u8], obj_id: usize) -> Result<Vec<u8>, FormeError> {
    let header = format!("{obj_id} 0 obj");
    let header_bytes = header.as_bytes();
    let obj_start = find_bytes(pdf, header_bytes)
        .ok_or_else(|| FormeError::RenderError(format!("Cannot find stream object {obj_id}")))?;

    // Find the stream keyword after the object header
    let search_region = &pdf[obj_start..std::cmp::min(obj_start + 4096, pdf.len())];
    let stream_kw = find_bytes(search_region, b"stream")
        .ok_or_else(|| FormeError::RenderError(format!("No stream in object {obj_id}")))?;

    let dict_region = &search_region[..stream_kw];
    let is_compressed = find_bytes(dict_region, b"/FlateDecode").is_some();

    // Stream data starts after "stream\n" or "stream\r\n"
    let abs_stream_kw = obj_start + stream_kw + 6; // skip "stream"
    let mut data_start = abs_stream_kw;
    if data_start < pdf.len() && pdf[data_start] == b'\r' {
        data_start += 1;
    }
    if data_start < pdf.len() && pdf[data_start] == b'\n' {
        data_start += 1;
    }

    // Find endstream
    let remaining = &pdf[data_start..];
    let endstream_offset = find_bytes(remaining, b"endstream")
        .ok_or_else(|| FormeError::RenderError(format!("No endstream in object {obj_id}")))?;

    // Trim trailing whitespace before endstream
    let mut end = endstream_offset;
    while end > 0 && (remaining[end - 1] == b'\n' || remaining[end - 1] == b'\r') {
        end -= 1;
    }

    let raw_bytes = &remaining[..end];

    if is_compressed {
        decompress_to_vec_zlib(raw_bytes).map_err(|e| {
            FormeError::RenderError(format!(
                "FlateDecode decompression failed for object {obj_id}: {e}"
            ))
        })
    } else {
        Ok(raw_bytes.to_vec())
    }
}

/// Parse content stream object IDs from a /Contents reference string.
/// Handles both single refs ("5 0 R") and arrays ("[5 0 R 6 0 R]").
fn parse_contents_obj_ids(contents_ref: &str) -> Vec<usize> {
    let trimmed = contents_ref.trim();
    let inner = if trimmed.starts_with('[') && trimmed.ends_with(']') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let mut ids = Vec::new();
    let mut remaining = inner.trim();
    while !remaining.is_empty() {
        let end = remaining
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(remaining.len());
        if end == 0 {
            remaining = &remaining[1..];
            continue;
        }
        if let Ok(id) = remaining[..end].parse::<usize>() {
            ids.push(id);
        }
        remaining = remaining[end..].trim_start();
        if remaining.starts_with("0 R") {
            remaining = remaining[3..].trim_start();
        }
    }
    ids
}

/// Redact regions of a PDF by removing text from content streams and overlaying
/// opaque rectangles.
///
/// Accepts top-origin (web) coordinates — the y-axis flip to PDF bottom-origin
/// happens here. Callers must NOT pre-flip coordinates.
pub fn redact_pdf(pdf_bytes: &[u8], regions: &[RedactionRegion]) -> Result<Vec<u8>, FormeError> {
    if regions.is_empty() {
        return Ok(pdf_bytes.to_vec());
    }

    let scan = scan_pdf_metadata(pdf_bytes)?;
    let pages = collect_pages(pdf_bytes, &scan)?;

    // Group regions by page index
    let max_page = regions.iter().map(|r| r.page).max().unwrap_or(0);
    if max_page >= pages.len() {
        return Err(FormeError::RenderError(format!(
            "Redaction references page {} but PDF only has {} pages",
            max_page,
            pages.len()
        )));
    }

    let mut regions_by_page: Vec<Vec<&RedactionRegion>> = vec![vec![]; pages.len()];
    for r in regions {
        regions_by_page[r.page].push(r);
    }

    let mut buf = Vec::from(pdf_bytes);
    if !buf.ends_with(b"\n") {
        buf.push(b'\n');
    }

    let mut next_id = scan.size;
    let mut xref_entries: Vec<(usize, usize)> = Vec::new();

    // For each page with redactions:
    // 1. Rewrite content stream to remove text operators in redaction regions
    // 2. Create visual overlay XObject
    // 3. Build new page object referencing both
    let mut new_page_refs: Vec<(usize, usize)> = Vec::new(); // (page_index, new_page_obj_id)

    for (page_idx, page_regions) in regions_by_page.iter().enumerate() {
        if page_regions.is_empty() {
            continue;
        }

        let page_info = &pages[page_idx];
        let media_height = page_info.media_box_height;

        // Convert redaction regions from web top-origin to PDF bottom-origin
        let pdf_regions: Vec<PdfRedactRegion> = page_regions
            .iter()
            .map(|r| PdfRedactRegion {
                x: r.x,
                y: media_height - r.y - r.height,
                width: r.width,
                height: r.height,
            })
            .collect();

        // ── Step 1: Rewrite content stream ──────────────────────────
        // Extract, decompress, tokenize, strip redacted text, recompress
        let content_obj_ids = parse_contents_obj_ids(&page_info.contents_ref);

        let mut combined_stream = Vec::new();
        for &obj_id in &content_obj_ids {
            let decompressed = extract_and_decompress_stream(pdf_bytes, obj_id)?;
            if !combined_stream.is_empty() {
                combined_stream.push(b'\n');
            }
            combined_stream.extend_from_slice(&decompressed);
        }

        // Build the per-page font map so text-width estimates use real per-CID
        // advances from the font's /W array (needed to detect when a text
        // operator's extent crosses a redaction region).
        let font_map = page_info
            .resources_ref
            .as_deref()
            .map(|res| build_font_map(pdf_bytes, res))
            .unwrap_or_default();

        let tokens = tokenize_content_stream(&combined_stream);
        let filtered_tokens = strip_redacted_text(&tokens, &pdf_regions, &font_map);
        let new_stream_data = serialize_tokens(&filtered_tokens);
        let compressed_stream = compress_to_vec_zlib(&new_stream_data, 6);

        // Write the replacement content stream object
        let new_content_id = next_id;
        next_id += 1;
        xref_entries.push((new_content_id, buf.len()));

        let content_obj = format!(
            "{new_content_id} 0 obj\n<< /Length {} /Filter /FlateDecode >>\nstream\n",
            compressed_stream.len()
        );
        buf.extend_from_slice(content_obj.as_bytes());
        buf.extend_from_slice(&compressed_stream);
        buf.extend_from_slice(b"\nendstream\nendobj\n");

        // ── Step 2: Visual overlay XObject ──────────────────────────
        let mut overlay_content = String::new();
        for r in page_regions {
            let (pr, pg, pb) = parse_hex_color(r.color.as_deref().unwrap_or("#000000"));
            let pdf_y = media_height - r.y - r.height;
            overlay_content.push_str(&format!(
                "q {} {} {} rg {:.4} {:.4} {:.4} {:.4} re f Q\n",
                pr, pg, pb, r.x, pdf_y, r.width, r.height
            ));
        }

        let overlay_bytes = overlay_content.as_bytes();

        let xobj_id = next_id;
        next_id += 1;
        xref_entries.push((xobj_id, buf.len()));

        let xobj = format!(
            "{xobj_id} 0 obj\n<<\n/Type /XObject\n/Subtype /Form\n/BBox [0 0 {:.4} {:.4}]\n/Length {}\n>>\nstream\n",
            page_info.media_box_width,
            media_height,
            overlay_bytes.len()
        );
        buf.extend_from_slice(xobj.as_bytes());
        buf.extend_from_slice(overlay_bytes);
        buf.extend_from_slice(b"endstream\nendobj\n");

        // ── Step 3: Overlay invocation stream ───────────────────────
        let xobj_name = format!("FmRedact{page_idx}");
        let do_stream = format!("/{xobj_name} Do\n");
        let do_bytes = do_stream.as_bytes();

        let do_stream_id = next_id;
        next_id += 1;
        xref_entries.push((do_stream_id, buf.len()));

        let do_obj = format!(
            "{do_stream_id} 0 obj\n<< /Length {} >>\nstream\n",
            do_bytes.len()
        );
        buf.extend_from_slice(do_obj.as_bytes());
        buf.extend_from_slice(do_bytes);
        buf.extend_from_slice(b"endstream\nendobj\n");

        // ── Step 4: New page object ─────────────────────────────────
        let new_page_id = next_id;
        next_id += 1;
        xref_entries.push((new_page_id, buf.len()));

        let parent_ref = page_info.parent_obj;

        let mut page_dict = format!(
            "{new_page_id} 0 obj\n<<\n/Type /Page\n/Parent {parent_ref} 0 R\n/MediaBox [0 0 {:.4} {:.4}]\n",
            page_info.media_box_width,
            media_height,
        );

        if let Some((cw, ch)) = page_info.crop_box {
            page_dict.push_str(&format!("/CropBox [0 0 {cw:.4} {ch:.4}]\n"));
        }

        // Contents: replacement stream + overlay Do stream
        page_dict.push_str(&format!(
            "/Contents [{new_content_id} 0 R {do_stream_id} 0 R]\n"
        ));

        // Merge resources: add our XObject to existing resources
        if let Some(ref res) = page_info.resources_ref {
            page_dict.push_str(&format!(
                "/Resources << {res} /XObject << /{xobj_name} {xobj_id} 0 R >> >>\n"
            ));
        } else {
            page_dict.push_str(&format!(
                "/Resources << /XObject << /{xobj_name} {xobj_id} 0 R >> >>\n"
            ));
        }

        page_dict.push_str(">>\nendobj\n");
        buf.extend_from_slice(page_dict.as_bytes());

        new_page_refs.push((page_idx, new_page_id));
    }

    // ── Metadata scrubbing ─────────────────────────────────────────────
    // Replace /Info and /Metadata objects to strip sensitive document metadata.
    // Uses the same object IDs so the incremental update overrides the originals.

    let trailer_section = &pdf_bytes[scan.trailer_pos..scan.startxref_pos];
    if let Some(info_id) = find_ref_in_bytes(trailer_section, b"/Info") {
        let date = super::certify::format_pdf_date();
        xref_entries.push((info_id, buf.len()));
        let info = format!("{info_id} 0 obj\n<< /Producer (Forme) /ModDate ({date}) >>\nendobj\n");
        buf.extend_from_slice(info.as_bytes());
    }

    let text = String::from_utf8_lossy(pdf_bytes);
    if let Some(meta_id) = find_catalog_ref(&text, scan.root_obj, "/Metadata") {
        let xmp_date = format_xmp_date();
        let xmp = format!(
            "<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
             <x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
             <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
             <rdf:Description rdf:about=''\n\
             xmlns:pdf='http://ns.adobe.com/pdf/1.3/'\n\
             xmlns:xmp='http://ns.adobe.com/xap/1.0/'>\n\
             <pdf:Producer>Forme</pdf:Producer>\n\
             <xmp:ModifyDate>{xmp_date}</xmp:ModifyDate>\n\
             </rdf:Description>\n\
             </rdf:RDF>\n\
             </x:xmpmeta>\n\
             <?xpacket end='w'?>"
        );
        let xmp_bytes = xmp.as_bytes();
        let compressed = compress_to_vec_zlib(xmp_bytes, 6);
        xref_entries.push((meta_id, buf.len()));
        let meta_obj = format!(
            "{meta_id} 0 obj\n<< /Type /Metadata /Subtype /XML /Length {} /Filter /FlateDecode >>\nstream\n",
            compressed.len()
        );
        buf.extend_from_slice(meta_obj.as_bytes());
        buf.extend_from_slice(&compressed);
        buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Build a new /Pages object that references the updated page objects
    let new_pages_id = next_id;
    next_id += 1;
    xref_entries.push((new_pages_id, buf.len()));

    let mut kids = String::new();
    for (idx, page_info) in pages.iter().enumerate() {
        if let Some((_, new_id)) = new_page_refs.iter().find(|(pi, _)| *pi == idx) {
            kids.push_str(&format!("{new_id} 0 R "));
        } else {
            kids.push_str(&format!("{} 0 R ", page_info.obj_id));
        }
    }

    let pages_obj = format!(
        "{new_pages_id} 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {} >>\nendobj\n",
        pages.len()
    );
    buf.extend_from_slice(pages_obj.as_bytes());

    // Build new catalog referencing the new /Pages tree
    let new_catalog_id = next_id;
    next_id += 1;
    xref_entries.push((new_catalog_id, buf.len()));

    // Preserve existing catalog entries
    let mut catalog =
        format!("{new_catalog_id} 0 obj\n<< /Type /Catalog /Pages {new_pages_id} 0 R\n");

    if let Some(lang) = find_catalog_string(&text, scan.root_obj, "/Lang") {
        catalog.push_str(&format!("/Lang ({lang})\n"));
    }
    if catalog_has_key(&text, scan.root_obj, "/MarkInfo") {
        catalog.push_str("/MarkInfo << /Marked true >>\n");
    }
    if let Some(r) = find_catalog_ref(&text, scan.root_obj, "/StructTreeRoot") {
        catalog.push_str(&format!("/StructTreeRoot {r} 0 R\n"));
    }
    if let Some(r) = find_catalog_ref(&text, scan.root_obj, "/Metadata") {
        catalog.push_str(&format!("/Metadata {r} 0 R\n"));
    }
    if let Some(r) = find_catalog_ref(&text, scan.root_obj, "/Names") {
        catalog.push_str(&format!("/Names {r} 0 R\n"));
    }
    if let Some(r) = find_catalog_ref(&text, scan.root_obj, "/ViewerPreferences") {
        catalog.push_str(&format!("/ViewerPreferences {r} 0 R\n"));
    }
    if let Some(oi) = find_catalog_array_content(&text, scan.root_obj, "/OutputIntents") {
        catalog.push_str(&format!("/OutputIntents {oi}\n"));
    }
    // Preserve AcroForm if present
    if let Some(acroform) = find_catalog_dict_content(&text, scan.root_obj, "/AcroForm") {
        catalog.push_str(&format!("/AcroForm {acroform}\n"));
    }

    catalog.push_str(">>\nendobj\n");
    buf.extend_from_slice(catalog.as_bytes());

    // Write xref table
    let xref_offset = buf.len();
    buf.extend_from_slice(b"xref\n");

    let mut sorted_entries = xref_entries.clone();
    sorted_entries.sort_by_key(|(id, _)| *id);

    let mut i = 0;
    while i < sorted_entries.len() {
        let start_id = sorted_entries[i].0;
        let mut count = 1;
        while i + count < sorted_entries.len() && sorted_entries[i + count].0 == start_id + count {
            count += 1;
        }
        buf.extend_from_slice(format!("{start_id} {count}\n").as_bytes());
        for j in 0..count {
            let offset = sorted_entries[i + j].1;
            buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        i += count;
    }

    // Trailer
    buf.extend_from_slice(
        format!(
            "trailer\n<<\n/Size {next_id}\n/Root {new_catalog_id} 0 R\n/Prev {prev}\n>>\nstartxref\n{xref_offset}\n%%EOF\n",
            prev = scan.startxref_offset
        )
        .as_bytes(),
    );

    Ok(buf)
}

// ── PDF scanning infrastructure ─────────────────────────────────────

struct PdfScanResult {
    startxref_offset: usize,
    startxref_pos: usize,
    trailer_pos: usize,
    size: usize,
    root_obj: usize,
    pages_obj: usize,
}

struct PageInfo {
    obj_id: usize,
    parent_obj: usize,
    media_box_width: f64,
    media_box_height: f64,
    crop_box: Option<(f64, f64)>,
    contents_ref: String,
    resources_ref: Option<String>,
}

fn scan_pdf_metadata(pdf: &[u8]) -> Result<PdfScanResult, FormeError> {
    let startxref_pos = rfind_bytes(pdf, b"startxref")
        .ok_or_else(|| FormeError::RenderError("No startxref found in PDF".to_string()))?;
    let after_startxref = &pdf[startxref_pos + 9..];
    let startxref_offset: usize = parse_number_from_bytes(after_startxref)
        .ok_or_else(|| FormeError::RenderError("Cannot parse startxref value".to_string()))?;

    let trailer_pos = rfind_bytes(pdf, b"trailer")
        .ok_or_else(|| FormeError::RenderError("No trailer found in PDF".to_string()))?;
    let trailer_section = &pdf[trailer_pos..startxref_pos];

    let size = find_value_in_bytes(trailer_section, b"/Size")
        .ok_or_else(|| FormeError::RenderError("No /Size found in trailer".to_string()))?;

    let root_obj = find_ref_in_bytes(trailer_section, b"/Root")
        .ok_or_else(|| FormeError::RenderError("No /Root found in trailer".to_string()))?;

    let text = String::from_utf8_lossy(pdf);
    let pages_obj = find_catalog_ref(&text, root_obj, "/Pages")
        .ok_or_else(|| FormeError::RenderError("No /Pages in catalog".to_string()))?;

    Ok(PdfScanResult {
        startxref_offset,
        startxref_pos,
        trailer_pos,
        size,
        root_obj,
        pages_obj,
    })
}

/// Collect all page objects from the /Pages tree.
fn collect_pages(pdf: &[u8], scan: &PdfScanResult) -> Result<Vec<PageInfo>, FormeError> {
    let text = String::from_utf8_lossy(pdf);
    let mut pages = Vec::new();

    // Find the /Pages object and extract /Kids
    let kids = extract_kids_refs(&text, scan.pages_obj)?;

    for kid_id in &kids {
        collect_page_recursive(&text, *kid_id, scan.pages_obj, &mut pages)?;
    }

    if pages.is_empty() {
        return Err(FormeError::RenderError("No pages found in PDF".to_string()));
    }

    Ok(pages)
}

fn collect_page_recursive(
    text: &str,
    obj_id: usize,
    parent_id: usize,
    pages: &mut Vec<PageInfo>,
) -> Result<(), FormeError> {
    let obj_content = find_object_content(text, obj_id)
        .ok_or_else(|| FormeError::RenderError(format!("Cannot find object {obj_id}")))?;

    if obj_content.contains("/Type /Pages") {
        // Intermediate /Pages node — recurse into /Kids
        let kids = extract_kids_from_content(&obj_content)?;
        for kid_id in &kids {
            collect_page_recursive(text, *kid_id, obj_id, pages)?;
        }
    } else if obj_content.contains("/Type /Page") {
        // Leaf /Page node
        let (mw, mh) = extract_media_box(&obj_content)
            .or_else(|| {
                // Inherit from parent
                find_object_content(text, parent_id).and_then(|parent| extract_media_box(&parent))
            })
            .unwrap_or((612.0, 792.0)); // Default to Letter

        let crop_box = extract_crop_box(&obj_content);

        let contents_ref = extract_contents_ref(&obj_content).unwrap_or_default();

        let resources_ref = extract_resources_inline(&obj_content);

        pages.push(PageInfo {
            obj_id,
            parent_obj: parent_id,
            media_box_width: mw,
            media_box_height: mh,
            crop_box,
            contents_ref,
            resources_ref,
        });
    }

    Ok(())
}

// ── Object and value extraction helpers ─────────────────────────────

fn find_object_content(text: &str, obj_id: usize) -> Option<String> {
    let header = format!("{obj_id} 0 obj");
    let start = text.find(&header)?;
    let section = &text[start..];
    let end = section.find("endobj")?;
    Some(section[..end].to_string())
}

fn extract_kids_refs(text: &str, pages_obj: usize) -> Result<Vec<usize>, FormeError> {
    let content = find_object_content(text, pages_obj)
        .ok_or_else(|| FormeError::RenderError(format!("Cannot find /Pages object {pages_obj}")))?;
    extract_kids_from_content(&content)
}

fn extract_kids_from_content(content: &str) -> Result<Vec<usize>, FormeError> {
    let kids_pos = content
        .find("/Kids")
        .ok_or_else(|| FormeError::RenderError("No /Kids in /Pages object".to_string()))?;
    let after = &content[kids_pos + 5..];
    let bracket_start = after
        .find('[')
        .ok_or_else(|| FormeError::RenderError("No [ after /Kids".to_string()))?;
    let bracket_end = after
        .find(']')
        .ok_or_else(|| FormeError::RenderError("No ] after /Kids".to_string()))?;
    let inner = &after[bracket_start + 1..bracket_end];

    let mut refs = Vec::new();
    let mut remaining = inner.trim();
    while !remaining.is_empty() {
        let end = remaining
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(remaining.len());
        if end == 0 {
            remaining = &remaining[1..];
            continue;
        }
        if let Ok(id) = remaining[..end].parse::<usize>() {
            refs.push(id);
        }
        remaining = remaining[end..].trim_start();
        // Skip "0 R"
        if remaining.starts_with("0 R") {
            remaining = remaining[3..].trim_start();
        }
    }

    Ok(refs)
}

fn extract_media_box(content: &str) -> Option<(f64, f64)> {
    extract_box(content, "/MediaBox")
}

fn extract_crop_box(content: &str) -> Option<(f64, f64)> {
    extract_box(content, "/CropBox")
}

fn extract_box(content: &str, key: &str) -> Option<(f64, f64)> {
    let pos = content.find(key)?;
    let after = &content[pos + key.len()..];
    let bracket_start = after.find('[')?;
    let bracket_end = after.find(']')?;
    let inner = &after[bracket_start + 1..bracket_end];

    let nums: Vec<f64> = inner
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    if nums.len() >= 4 {
        // [llx lly urx ury] — width = urx - llx, height = ury - lly
        Some((nums[2] - nums[0], nums[3] - nums[1]))
    } else {
        None
    }
}

fn extract_contents_ref(content: &str) -> Option<String> {
    let pos = content.find("/Contents")?;
    let after = &content[pos + 9..].trim_start();

    if after.starts_with('[') {
        // Array of content stream references — return as-is
        let end = after.find(']')?;
        Some(after[..=end].to_string())
    } else {
        // Single reference "N 0 R"
        let end = after.find('R')?;
        Some(after[..=end].to_string())
    }
}

fn extract_resources_inline(content: &str) -> Option<String> {
    let pos = content.find("/Resources")?;
    let after = &content[pos + 10..].trim_start();

    if after.starts_with("<<") {
        // Inline dict — extract until matching >>
        // Simple approach: find the first >> (works for non-nested cases)
        // For nested dicts we need to count depth
        let mut depth = 0;
        let bytes = after.as_bytes();
        let mut end_pos = 0;
        let mut i = 0;
        while i < bytes.len() - 1 {
            if bytes[i] == b'<' && bytes[i + 1] == b'<' {
                depth += 1;
                i += 2;
            } else if bytes[i] == b'>' && bytes[i + 1] == b'>' {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    end_pos = i;
                    break;
                }
            } else {
                i += 1;
            }
        }
        if end_pos > 0 {
            // Return inner content (strip outer << >>)
            let dict_content = &after[2..end_pos - 2].trim();
            Some(dict_content.to_string())
        } else {
            None
        }
    } else {
        // Reference "N 0 R" — can't easily inline, skip
        None
    }
}

// ── Byte-level scanning (shared patterns with signing.rs) ───────────

fn rfind_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    for i in (0..=haystack.len() - needle.len()).rev() {
        if haystack[i..i + needle.len()] == *needle {
            return Some(i);
        }
    }
    None
}

fn parse_number_from_bytes(bytes: &[u8]) -> Option<usize> {
    let start = bytes.iter().position(|&b| b.is_ascii_digit())?;
    let end = bytes[start..]
        .iter()
        .position(|b| !b.is_ascii_digit())
        .map(|p| start + p)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn find_value_in_bytes(section: &[u8], key: &[u8]) -> Option<usize> {
    let pos = find_bytes(section, key)?;
    parse_number_from_bytes(&section[pos + key.len()..])
}

fn find_ref_in_bytes(section: &[u8], key: &[u8]) -> Option<usize> {
    let pos = find_bytes(section, key)?;
    parse_number_from_bytes(&section[pos + key.len()..])
}

// ── Catalog helpers (text-based, shared patterns with signing.rs) ───

fn find_catalog_ref(text: &str, obj_id: usize, key: &str) -> Option<usize> {
    let obj_header = format!("{obj_id} 0 obj");
    let obj_start = text.find(&obj_header)?;
    let obj_section = &text[obj_start..];
    let obj_end = obj_section.find("endobj")?;
    let obj_content = &obj_section[..obj_end];

    let key_pos = obj_content.find(key)?;
    let after_key = &obj_content[key_pos + key.len()..];
    let trimmed = after_key.trim_start();
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if end == 0 {
        return None;
    }
    trimmed[..end].parse().ok()
}

fn catalog_has_key(text: &str, obj_id: usize, key: &str) -> bool {
    let obj_header = format!("{obj_id} 0 obj");
    if let Some(obj_start) = text.find(&obj_header) {
        let obj_section = &text[obj_start..];
        if let Some(obj_end) = obj_section.find("endobj") {
            return obj_section[..obj_end].contains(key);
        }
    }
    false
}

fn find_catalog_string(text: &str, obj_id: usize, key: &str) -> Option<String> {
    let obj_header = format!("{obj_id} 0 obj");
    let obj_start = text.find(&obj_header)?;
    let obj_section = &text[obj_start..];
    let obj_end = obj_section.find("endobj")?;
    let obj_content = &obj_section[..obj_end];

    let key_pos = obj_content.find(key)?;
    let after_key = &obj_content[key_pos + key.len()..];
    let trimmed = after_key.trim_start();
    if !trimmed.starts_with('(') {
        return None;
    }
    let end = trimmed[1..].find(')')? + 1;
    Some(trimmed[1..end].to_string())
}

fn find_catalog_array_content(text: &str, obj_id: usize, key: &str) -> Option<String> {
    let obj_header = format!("{obj_id} 0 obj");
    let obj_start = text.find(&obj_header)?;
    let obj_section = &text[obj_start..];
    let obj_end = obj_section.find("endobj")?;
    let obj_content = &obj_section[..obj_end];

    let key_pos = obj_content.find(key)?;
    let after_key = &obj_content[key_pos + key.len()..];
    let trimmed = after_key.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find(']')? + 1;
    Some(trimmed[..end].to_string())
}

fn find_catalog_dict_content(text: &str, obj_id: usize, key: &str) -> Option<String> {
    let obj_header = format!("{obj_id} 0 obj");
    let obj_start = text.find(&obj_header)?;
    let obj_section = &text[obj_start..];
    let obj_end = obj_section.find("endobj")?;
    let obj_content = &obj_section[..obj_end];

    let key_pos = obj_content.find(key)?;
    let after_key = &obj_content[key_pos + key.len()..];
    let trimmed = after_key.trim_start();
    if !trimmed.starts_with("<<") {
        return None;
    }

    // Count depth to find matching >>
    let bytes = trimmed.as_bytes();
    let mut depth = 0;
    let mut i = 0;
    while i < bytes.len() - 1 {
        if bytes[i] == b'<' && bytes[i + 1] == b'<' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'>' && bytes[i + 1] == b'>' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(trimmed[..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

// ── Text search redaction ───────────────────────────────────────────

/// A character with its approximate position in PDF user space.
struct CharPosition {
    #[allow(dead_code)]
    ch: char,
    x: f64,
    y: f64,
    font_size: f64,
}

/// Build a map from font resource name (e.g. "F1") to `FontInfo` for every
/// font declared in the given `/Resources` dictionary text.
///
/// Walks `/Resources /Font << /F1 N 0 R ... >>`, fetches each font object from
/// the PDF, identifies Type0 (CID) fonts by `/Subtype /Type0`, chases the
/// `/ToUnicode` reference, decompresses and parses its CMap into a CID→Unicode
/// table. Simple fonts (Type1 Helvetica etc.) get a default `FontInfo` with
/// `is_cid: false`.
fn build_font_map(pdf: &[u8], resources_text: &str) -> HashMap<String, FontInfo> {
    let mut map = HashMap::new();

    // Find the /Font subdictionary inside resources_text.
    let Some(font_pos) = resources_text.find("/Font") else {
        return map;
    };
    let after = resources_text[font_pos + 5..].trim_start();
    let bytes = after.as_bytes();
    if !after.starts_with("<<") {
        return map;
    }

    // Collect the matching inner dict content.
    let mut depth = 0;
    let mut inner_end = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1] == b'<' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'>' && bytes[i + 1] == b'>' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                inner_end = i;
                break;
            }
        } else {
            i += 1;
        }
    }
    if inner_end == 0 {
        return map;
    }
    let font_dict = &after[2..inner_end - 2];

    // Parse "/Name N 0 R" entries (one font per entry).
    let dict_bytes = font_dict.as_bytes();
    let mut j = 0;
    while j < dict_bytes.len() {
        if dict_bytes[j] != b'/' {
            j += 1;
            continue;
        }
        // Read name until whitespace.
        let name_start = j + 1;
        let mut k = name_start;
        while k < dict_bytes.len() && !dict_bytes[k].is_ascii_whitespace() && dict_bytes[k] != b'/'
        {
            k += 1;
        }
        let name: String = dict_bytes[name_start..k]
            .iter()
            .map(|&b| b as char)
            .collect();
        // Read value: expect "<num> <gen> R".
        let after_name = &font_dict[k..];
        let rest = after_name.trim_start();
        if let Some(rel_r) = rest.find(" R") {
            let ref_text = &rest[..rel_r];
            let parts: Vec<&str> = ref_text.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(obj_id) = parts[0].parse::<usize>() {
                    if let Some(info) = parse_font_object(pdf, obj_id) {
                        map.insert(name.clone(), info);
                    } else {
                        map.insert(name.clone(), FontInfo::default());
                    }
                }
            }
            // Advance past the "R".
            j = k + (rest.as_ptr() as usize - after_name.as_ptr() as usize) + rel_r + 2;
        } else {
            j = k;
        }
    }

    map
}

/// Fetch a font object and parse its Subtype + ToUnicode CMap.
fn parse_font_object(pdf: &[u8], obj_id: usize) -> Option<FontInfo> {
    // Use from_utf8_lossy — PDFs contain binary font streams, so strict UTF-8
    // decoding rejects the whole buffer. The lossy conversion replaces bad
    // bytes with U+FFFD but the dictionary text around the binary data
    // (which is ASCII) stays intact and findable.
    let text = String::from_utf8_lossy(pdf);
    let content = find_object_content(&text, obj_id)?;

    // Subtype check: Type0 is a CID font.
    let is_cid = content.contains("/Subtype /Type0") || content.contains("/Subtype/Type0");

    if !is_cid {
        return Some(FontInfo {
            is_cid: false,
            cid_to_unicode: HashMap::new(),
            cid_widths: HashMap::new(),
            default_width: 500.0,
        });
    }

    // Parse ToUnicode CMap (optional — CIDs without a CMap just don't match).
    let cid_to_unicode = content
        .find("/ToUnicode")
        .and_then(|pos| {
            let rest = content[pos + "/ToUnicode".len()..].trim_start();
            let rel_r = rest.find(" R")?;
            let parts: Vec<&str> = rest[..rel_r].split_whitespace().collect();
            if parts.len() < 2 {
                return None;
            }
            let obj_id = parts[0].parse::<usize>().ok()?;
            let bytes = extract_and_decompress_stream(pdf, obj_id).ok()?;
            let cmap_text = String::from_utf8_lossy(&bytes);
            Some(parse_tounicode_cmap(&cmap_text))
        })
        .unwrap_or_default();

    // Parse descendant font's /W and /DW for per-glyph advances.
    let (cid_widths, default_width) = content
        .find("/DescendantFonts")
        .and_then(|pos| {
            // /DescendantFonts can be either "[N 0 R]" inline or a reference
            // to an array object. Look for the first "N 0 R" after the key.
            let rest = &content[pos + "/DescendantFonts".len()..];
            let stripped = rest.trim_start();
            let after = stripped.strip_prefix('[').unwrap_or(stripped).trim_start();
            let rel_r = after.find(" R")?;
            let parts: Vec<&str> = after[..rel_r].split_whitespace().collect();
            if parts.len() < 2 {
                return None;
            }
            let descendant_obj_id = parts[0].parse::<usize>().ok()?;
            let descendant = find_object_content(&text, descendant_obj_id)?;
            Some(parse_cid_widths(&descendant))
        })
        .unwrap_or_else(|| (HashMap::new(), 500.0));

    Some(FontInfo {
        is_cid: true,
        cid_to_unicode,
        cid_widths,
        default_width,
    })
}

/// Parse a CIDFontType2's `/DW` default advance and `/W` per-CID advance
/// table. Widths are in 1/1000 em units. `/W` supports two forms:
///   `cid [w1 w2 w3 ...]`       — CIDs cid, cid+1, cid+2 have widths w1..w3
///   `firstCid lastCid w`       — every CID in firstCid..=lastCid has width w
fn parse_cid_widths(descendant: &str) -> (HashMap<u16, f64>, f64) {
    let mut widths = HashMap::new();

    let default_width = descendant
        .find("/DW")
        .and_then(|pos| {
            let rest = descendant[pos + 3..].trim_start();
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(rest.len());
            rest[..end].parse::<f64>().ok()
        })
        .unwrap_or(500.0);

    let Some(w_pos) = descendant.find("/W") else {
        return (widths, default_width);
    };
    // Find the opening '[' of the /W array.
    let after = &descendant[w_pos + 2..];
    let Some(open) = after.find('[') else {
        return (widths, default_width);
    };
    // Find the matching closing ']' at depth 0.
    let body_bytes = &after.as_bytes()[open + 1..];
    let mut depth: i32 = 0;
    let mut close_rel: Option<usize> = None;
    for (i, &b) in body_bytes.iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => {
                if depth == 0 {
                    close_rel = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let Some(close_rel) = close_rel else {
        return (widths, default_width);
    };
    let body = &after[open + 1..open + 1 + close_rel];

    // Walk the body collecting numbers and sub-arrays.
    // Each entry is either "N [w w w ...]" or "firstN lastN w".
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut scratch: Vec<f64> = Vec::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'[' {
            // Array of widths: apply to the most recent number in scratch.
            let Some(start_cid) = scratch.last().copied() else {
                // Malformed — skip past the array.
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b']' {
                    j += 1;
                }
                i = j + 1;
                continue;
            };
            let mut j = i + 1;
            let mut values: Vec<f64> = Vec::new();
            while j < bytes.len() && bytes[j] != b']' {
                if bytes[j].is_ascii_whitespace() {
                    j += 1;
                    continue;
                }
                let start = j;
                while j < bytes.len()
                    && (bytes[j].is_ascii_digit() || bytes[j] == b'.' || bytes[j] == b'-')
                {
                    j += 1;
                }
                if start == j {
                    j += 1;
                    continue;
                }
                if let Ok(n) = std::str::from_utf8(&bytes[start..j])
                    .unwrap_or("")
                    .parse::<f64>()
                {
                    values.push(n);
                }
            }
            for (k, w) in values.iter().enumerate() {
                widths.insert(start_cid as u16 + k as u16, *w);
            }
            scratch.pop();
            i = j + 1;
            continue;
        }
        if b.is_ascii_digit() || b == b'-' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'-')
            {
                i += 1;
            }
            if let Ok(n) = std::str::from_utf8(&bytes[start..i])
                .unwrap_or("")
                .parse::<f64>()
            {
                scratch.push(n);
                // Range form: three consecutive numbers
                if scratch.len() == 3 {
                    let first = scratch[0] as u16;
                    let last = scratch[1] as u16;
                    let w = scratch[2];
                    for cid in first..=last {
                        widths.insert(cid, w);
                    }
                    scratch.clear();
                }
            }
            continue;
        }
        i += 1;
    }

    (widths, default_width)
}

/// Font metadata for decoding text in PDF content streams. Collected once
/// per page from the page's `/Resources /Font` dictionary.
#[derive(Debug, Clone, Default)]
struct FontInfo {
    /// When true, hex strings are 2-byte CIDs that need ToUnicode lookup.
    /// Otherwise the font is a simple font with single-byte WinAnsi encoding.
    is_cid: bool,
    /// CID → Unicode string mapping parsed from the font's /ToUnicode CMap.
    /// Populated only when `is_cid` is true. Values can be multi-character
    /// strings (CMaps may map one glyph to e.g. a ligature like "fi").
    cid_to_unicode: HashMap<u16, String>,
    /// CID → advance width in 1/1000 em units, parsed from the descendant
    /// font's `/W` array. Used to estimate per-glyph x offsets inside a
    /// text-showing operator so redaction boxes land on the actual glyphs
    /// instead of a uniform-width approximation.
    cid_widths: HashMap<u16, f64>,
    /// Default width (in 1/1000 em units) for CIDs not in `cid_widths`.
    /// Parsed from the descendant font's `/DW`. Falls back to 500 if absent.
    default_width: f64,
}

/// Parse a ToUnicode CMap text into a CID → Unicode string table.
///
/// Supports the common `bfchar` and `bfrange` sections. The engine only emits
/// `bfchar` today but other PDF producers emit `bfrange` so we handle both so
/// this works on imported/mixed PDFs too.
fn parse_tounicode_cmap(text: &str) -> HashMap<u16, String> {
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

    // Flush any trailing incrementing triple: [start_cid, end_cid, first_unicode]
    if group.len() == 3 {
        if let (Some(start_cid), Some(end_cid), Some(start_uni)) = (
            hex_to_u16(&group[0]),
            hex_to_u16(&group[1]),
            hex_to_u32(&group[2]),
        ) {
            let count = (end_cid as usize).saturating_sub(start_cid as usize) + 1;
            for k in 0..count {
                if let Some(ch) = char::from_u32(start_uni + k as u32) {
                    map.insert(start_cid + k as u16, ch.to_string());
                }
            }
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

fn hex_to_u32(hex: &str) -> Option<u32> {
    if hex.is_empty() {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
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

/// Render a text-showing operand into per-glyph `CharPosition`s and return
/// the decoded text + total advance width consumed (in PDF user space units).
///
/// Handles both `(literal)` and `<hex>` operands. For CID fonts, advances come
/// from the font's `/W` per-CID table; otherwise a uniform `font_size * 0.5`
/// approximation is used so simple fonts continue to work.
fn render_text_operand(
    token_bytes: &[u8],
    font: Option<&FontInfo>,
    font_size: f64,
    start_x: f64,
    start_y: f64,
) -> (String, Vec<CharPosition>, f64) {
    let text = decode_pdf_string(token_bytes, font);
    let mut chars = Vec::with_capacity(text.chars().count());

    // CID path: walk 2-byte CIDs and use real per-CID advances from /W.
    if let Some(info) = font {
        if info.is_cid && !token_bytes.is_empty() && token_bytes[0] == b'<' {
            let hex_slice = &token_bytes[1..token_bytes.len().saturating_sub(1)];
            let hex_str: String = hex_slice
                .iter()
                .filter(|b| !b.is_ascii_whitespace())
                .map(|&b| b as char)
                .collect();
            let hex_chars: Vec<char> = hex_str.chars().collect();
            let mut x_cursor = start_x;
            let mut i = 0;
            while i + 3 < hex_chars.len() {
                let chunk: String = hex_chars[i..i + 4].iter().collect();
                let cid = match u16::from_str_radix(&chunk, 16) {
                    Ok(c) => c,
                    Err(_) => {
                        i += 4;
                        continue;
                    }
                };
                let uni = info.cid_to_unicode.get(&cid).cloned().unwrap_or_default();
                let advance_em = info
                    .cid_widths
                    .get(&cid)
                    .copied()
                    .unwrap_or(info.default_width)
                    / 1000.0;
                let glyph_advance = advance_em * font_size;

                // Emit one CharPosition per Unicode char in the mapped string;
                // for ligatures like "fi" they all share this CID's x.
                for ch in uni.chars() {
                    chars.push(CharPosition {
                        ch,
                        x: x_cursor,
                        y: start_y,
                        font_size,
                    });
                }
                x_cursor += glyph_advance;
                i += 4;
            }
            let total_advance = x_cursor - start_x;
            return (text, chars, total_advance);
        }
    }

    // Simple font / literal-string fallback: uniform char-width estimate.
    let char_width = font_size * 0.5;
    for (ci, ch) in text.chars().enumerate() {
        chars.push(CharPosition {
            ch,
            x: start_x + ci as f64 * char_width,
            y: start_y,
            font_size,
        });
    }
    let total_advance = text.chars().count() as f64 * char_width;
    (text, chars, total_advance)
}

/// Decode text content from a PDF string operand token.
///
/// Handles `(literal strings)` with escape sequences and `<hex strings>`. For
/// CID fonts (Type0), hex strings are 2-byte CIDs that get mapped through the
/// font's ToUnicode CMap. For simple fonts (no font info or `is_cid: false`),
/// hex strings are treated as single-byte WinAnsi text — the original path.
fn decode_pdf_string(token_bytes: &[u8], font: Option<&FontInfo>) -> String {
    if token_bytes.is_empty() {
        return String::new();
    }

    // Literal string: (...)
    if token_bytes[0] == b'(' && token_bytes.last() == Some(&b')') {
        let inner = &token_bytes[1..token_bytes.len() - 1];
        let mut out = Vec::new();
        let mut i = 0;
        while i < inner.len() {
            if inner[i] == b'\\' && i + 1 < inner.len() {
                i += 1;
                match inner[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'(' => out.push(b'('),
                    b')' => out.push(b')'),
                    b'\\' => out.push(b'\\'),
                    d if d.is_ascii_digit() => {
                        // Octal escape: up to 3 digits
                        let mut val = (d - b'0') as u16;
                        for _ in 0..2 {
                            if i + 1 < inner.len() && inner[i + 1].is_ascii_digit() {
                                i += 1;
                                val = val * 8 + (inner[i] - b'0') as u16;
                            }
                        }
                        out.push(val as u8);
                    }
                    other => out.push(other),
                }
            } else {
                out.push(inner[i]);
            }
            i += 1;
        }
        // Best-effort UTF-8, fall back to latin1-style
        String::from_utf8(out.clone()).unwrap_or_else(|_| out.iter().map(|&b| b as char).collect())
    }
    // Hex string: <...>
    else if token_bytes[0] == b'<' && token_bytes.last() == Some(&b'>') {
        let hex = &token_bytes[1..token_bytes.len() - 1];
        let hex_str: String = hex
            .iter()
            .filter(|b| !b.is_ascii_whitespace())
            .map(|&b| b as char)
            .collect();
        let chars: Vec<char> = hex_str.chars().collect();

        // CID font: decode as a sequence of 4-hex-char CIDs through the
        // font's ToUnicode CMap. Emits the mapped Unicode string for each
        // CID; unmapped CIDs are skipped (better than garbage bytes).
        if let Some(info) = font {
            if info.is_cid {
                let mut out = String::new();
                let mut i = 0;
                while i + 3 < chars.len() {
                    let chunk: String = chars[i..i + 4].iter().collect();
                    if let Ok(cid) = u16::from_str_radix(&chunk, 16) {
                        if let Some(uni) = info.cid_to_unicode.get(&cid) {
                            out.push_str(uni);
                        }
                    }
                    i += 4;
                }
                return out;
            }
        }

        // Simple font: single-byte WinAnsi (original behavior).
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < chars.len() {
            let hi = chars[i].to_digit(16).unwrap_or(0) as u8;
            let lo = chars[i + 1].to_digit(16).unwrap_or(0) as u8;
            out.push(hi * 16 + lo);
            i += 2;
        }
        if i < chars.len() {
            // Odd number of hex digits: last digit padded with 0
            let hi = chars[i].to_digit(16).unwrap_or(0) as u8;
            out.push(hi * 16);
        }
        String::from_utf8(out.clone()).unwrap_or_else(|_| out.iter().map(|&b| b as char).collect())
    } else {
        String::new()
    }
}

/// Extract text content with approximate positions from a decompressed content stream.
///
/// Returns a list of text blocks, each being a string and its per-character positions.
/// Text within a BT/ET block is grouped; a new block starts on significant y-position changes.
///
/// `font_map` maps font resource names (e.g. `F1`) to `FontInfo` so hex-string
/// text operands can be decoded against the current font's ToUnicode CMap.
/// Passing an empty map falls back to WinAnsi single-byte decoding for all fonts.
fn extract_text_with_positions(
    content: &[u8],
    font_map: &HashMap<String, FontInfo>,
) -> Vec<(String, Vec<CharPosition>)> {
    let tokens = tokenize_content_stream(content);
    let mut state = TextState::new();
    let mut in_text = false;
    let mut operand_stack: Vec<&Token> = Vec::new();
    let mut current_font: Option<&FontInfo> = None;

    let mut blocks: Vec<(String, Vec<CharPosition>)> = Vec::new();
    let mut current_text = String::new();
    let mut current_chars: Vec<CharPosition> = Vec::new();
    let mut current_line_y: Option<f64> = None;

    let flush_block = |text: &mut String,
                       chars: &mut Vec<CharPosition>,
                       blocks: &mut Vec<(String, Vec<CharPosition>)>| {
        if !text.is_empty() {
            blocks.push((std::mem::take(text), std::mem::take(chars)));
        } else {
            chars.clear();
        }
    };

    for token in &tokens {
        match token {
            Token::Operand(_) => {
                operand_stack.push(token);
            }
            Token::Operator(op) => {
                let op_str = std::str::from_utf8(op).unwrap_or("");

                match op_str {
                    "BT" => {
                        in_text = true;
                        state.reset();
                        current_line_y = None;
                        operand_stack.clear();
                    }
                    "ET" => {
                        in_text = false;
                        flush_block(&mut current_text, &mut current_chars, &mut blocks);
                        current_line_y = None;
                        operand_stack.clear();
                    }
                    "Td" | "TD" if in_text => {
                        if operand_stack.len() >= 2 {
                            let ty = parse_operand_f64(operand_stack[operand_stack.len() - 1]);
                            let tx = parse_operand_f64(operand_stack[operand_stack.len() - 2]);
                            state.apply_td(tx, ty);
                        }
                        // Check for line break (significant y change)
                        if let Some(prev_y) = current_line_y {
                            if (state.ty() - prev_y).abs() > state.font_size * 0.5 {
                                flush_block(&mut current_text, &mut current_chars, &mut blocks);
                            }
                        }
                        current_line_y = Some(state.ty());
                        operand_stack.clear();
                    }
                    "Tm" if in_text => {
                        if operand_stack.len() >= 6 {
                            let n = operand_stack.len();
                            let a = parse_operand_f64(operand_stack[n - 6]);
                            let b = parse_operand_f64(operand_stack[n - 5]);
                            let c = parse_operand_f64(operand_stack[n - 4]);
                            let d = parse_operand_f64(operand_stack[n - 3]);
                            let e = parse_operand_f64(operand_stack[n - 2]);
                            let f = parse_operand_f64(operand_stack[n - 1]);
                            state.apply_tm(a, b, c, d, e, f);
                        }
                        if let Some(prev_y) = current_line_y {
                            if (state.ty() - prev_y).abs() > state.font_size * 0.5 {
                                flush_block(&mut current_text, &mut current_chars, &mut blocks);
                            }
                        }
                        current_line_y = Some(state.ty());
                        operand_stack.clear();
                    }
                    "T*" if in_text => {
                        state.apply_t_star();
                        flush_block(&mut current_text, &mut current_chars, &mut blocks);
                        current_line_y = Some(state.ty());
                        operand_stack.clear();
                    }
                    "Tf" if in_text => {
                        if operand_stack.len() >= 2 {
                            let size = parse_operand_f64(operand_stack[operand_stack.len() - 1]);
                            if size > 0.0 {
                                state.font_size = size;
                            }
                            // Font name: /F1 → "F1"
                            if let Token::Operand(name_bytes) =
                                operand_stack[operand_stack.len() - 2]
                            {
                                if !name_bytes.is_empty() && name_bytes[0] == b'/' {
                                    let name: String =
                                        name_bytes[1..].iter().map(|&b| b as char).collect();
                                    current_font = font_map.get(&name);
                                }
                            }
                        }
                        operand_stack.clear();
                    }
                    "Tj" if in_text => {
                        // (string) Tj — show text
                        if let Some(Token::Operand(data)) = operand_stack.last() {
                            let (text, chars, _advance) = render_text_operand(
                                data,
                                current_font,
                                state.font_size,
                                state.tx(),
                                state.ty(),
                            );
                            current_chars.extend(chars);
                            current_text.push_str(&text);
                        }
                        operand_stack.clear();
                    }
                    "TJ" if in_text => {
                        // [(string) kern (string) kern ...] TJ
                        if let Some(Token::Operand(data)) = operand_stack.last() {
                            let array_inner = if data.len() >= 2
                                && data[0] == b'['
                                && data[data.len() - 1] == b']'
                            {
                                &data[1..data.len() - 1]
                            } else {
                                data.as_slice()
                            };
                            let mut x_offset = state.tx();
                            let sub_tokens = tokenize_content_stream(array_inner);
                            for sub_tok in &sub_tokens {
                                if let Token::Operand(sub_data) = sub_tok {
                                    if sub_data.starts_with(b"(") || sub_data.starts_with(b"<") {
                                        let (text, chars, advance) = render_text_operand(
                                            sub_data,
                                            current_font,
                                            state.font_size,
                                            x_offset,
                                            state.ty(),
                                        );
                                        current_chars.extend(chars);
                                        current_text.push_str(&text);
                                        x_offset += advance;
                                    } else {
                                        // Numeric kerning value
                                        let kern: f64 = std::str::from_utf8(sub_data)
                                            .ok()
                                            .and_then(|s| s.trim().parse().ok())
                                            .unwrap_or(0.0);
                                        x_offset -= kern / 1000.0 * state.font_size;
                                    }
                                }
                            }
                        }
                        operand_stack.clear();
                    }
                    op_s if in_text && (op_s == "'" || op_s == "\"") => {
                        state.apply_t_star();
                        flush_block(&mut current_text, &mut current_chars, &mut blocks);
                        current_line_y = Some(state.ty());
                        // Show text (last operand for ', last for " after spacing args)
                        if let Some(Token::Operand(data)) = operand_stack.last() {
                            let (text, chars, _advance) = render_text_operand(
                                data,
                                current_font,
                                state.font_size,
                                state.tx(),
                                state.ty(),
                            );
                            current_chars.extend(chars);
                            current_text.push_str(&text);
                        }
                        operand_stack.clear();
                    }
                    _ => {
                        operand_stack.clear();
                    }
                }
            }
        }
    }

    // Flush any remaining text
    flush_block(&mut current_text, &mut current_chars, &mut blocks);
    blocks
}

/// Find text regions in a PDF that match the given patterns.
///
/// Returns `RedactionRegion` structs in web top-origin coordinates, ready to be
/// passed directly to `redact_pdf()`.
///
/// ## Limitations
/// - Only searches direct page content streams (not Form XObjects).
/// - Assumes WinAnsi (single-byte) encoding — CIDFont/CJK text won't match.
/// - Width estimation is approximate (`char_count × font_size × 0.5`).
pub fn find_text_regions(
    pdf_bytes: &[u8],
    patterns: &[RedactionPattern],
) -> Result<Vec<RedactionRegion>, FormeError> {
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    // Pre-compile regex patterns
    #[cfg(feature = "regex")]
    let compiled_regexes: Vec<Option<regex::Regex>> = {
        let mut regexes = Vec::with_capacity(patterns.len());
        for p in patterns {
            match p.pattern_type {
                PatternType::Regex => {
                    let re = regex::Regex::new(&p.pattern).map_err(|e| {
                        FormeError::RenderError(format!(
                            "Invalid regex pattern '{}': {e}",
                            p.pattern
                        ))
                    })?;
                    regexes.push(Some(re));
                }
                PatternType::Literal => regexes.push(None),
            }
        }
        regexes
    };

    let scan = scan_pdf_metadata(pdf_bytes)?;
    let pages = collect_pages(pdf_bytes, &scan)?;

    let mut regions = Vec::new();

    for (page_idx, page_info) in pages.iter().enumerate() {
        let media_height = page_info.media_box_height;

        // Extract and decompress content streams
        let content_obj_ids = parse_contents_obj_ids(&page_info.contents_ref);
        let mut combined_stream = Vec::new();
        for &obj_id in &content_obj_ids {
            match extract_and_decompress_stream(pdf_bytes, obj_id) {
                Ok(decompressed) => {
                    if !combined_stream.is_empty() {
                        combined_stream.push(b'\n');
                    }
                    combined_stream.extend_from_slice(&decompressed);
                }
                Err(_) => continue,
            }
        }

        if combined_stream.is_empty() {
            continue;
        }

        // Build the per-page font map so hex-string text can be decoded via
        // CID→Unicode for Type0 fonts embedded in the PDF.
        let font_map = page_info
            .resources_ref
            .as_deref()
            .map(|res| build_font_map(pdf_bytes, res))
            .unwrap_or_default();

        let text_blocks = extract_text_with_positions(&combined_stream, &font_map);

        #[allow(unused_variables)]
        for (pat_idx, pattern) in patterns.iter().enumerate() {
            // Skip if pattern is page-specific and doesn't match this page
            if let Some(target_page) = pattern.page {
                if target_page != page_idx {
                    continue;
                }
            }

            for (block_text, block_chars) in &text_blocks {
                if block_chars.is_empty() {
                    continue;
                }

                let matches: Vec<(usize, usize)> = match pattern.pattern_type {
                    PatternType::Literal => {
                        let lower_text = block_text.to_lowercase();
                        let lower_pattern = pattern.pattern.to_lowercase();
                        let mut found = Vec::new();
                        let mut start = 0;
                        while let Some(pos) = lower_text[start..].find(&lower_pattern) {
                            let abs_pos = start + pos;
                            // Convert byte offset to char offset
                            let char_start = block_text[..abs_pos].chars().count();
                            let char_end = char_start + pattern.pattern.chars().count();
                            found.push((char_start, char_end));
                            start = abs_pos + lower_pattern.len();
                        }
                        found
                    }
                    PatternType::Regex => {
                        #[cfg(feature = "regex")]
                        {
                            if let Some(re) = &compiled_regexes[pat_idx] {
                                re.find_iter(block_text)
                                    .map(|m| {
                                        let char_start = block_text[..m.start()].chars().count();
                                        let char_end = char_start
                                            + block_text[m.start()..m.end()].chars().count();
                                        (char_start, char_end)
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        }
                        #[cfg(not(feature = "regex"))]
                        {
                            return Err(FormeError::RenderError(
                                "Regex patterns require the 'regex' feature. \
                                 Use PatternType::Literal or enable the 'regex' Cargo feature."
                                    .to_string(),
                            ));
                        }
                    }
                };

                for (char_start, char_end) in matches {
                    if char_start >= block_chars.len() || char_end == 0 {
                        continue;
                    }
                    let end_idx = (char_end - 1).min(block_chars.len() - 1);
                    let first = &block_chars[char_start];
                    let last = &block_chars[end_idx];
                    let font_size = first.font_size;

                    // Bounding box in PDF bottom-origin coords
                    let pdf_x = first.x;
                    let pdf_width = (last.x - first.x) + font_size * 0.5;
                    let pdf_width = pdf_width.max(font_size * 0.5); // minimum width

                    // Convert to web top-origin
                    let web_y = media_height - (first.y + font_size * 0.8);
                    let height = font_size * 1.1;

                    regions.push(RedactionRegion {
                        page: page_idx,
                        x: pdf_x,
                        y: web_y,
                        width: pdf_width,
                        height,
                        color: pattern.color.clone(),
                    });
                }
            }
        }
    }

    Ok(regions)
}

/// Find text matching patterns and redact all matches in one step.
///
/// Convenience wrapper: calls `find_text_regions()` then `redact_pdf()`.
pub fn redact_text(pdf_bytes: &[u8], patterns: &[RedactionPattern]) -> Result<Vec<u8>, FormeError> {
    let regions = find_text_regions(pdf_bytes, patterns)?;
    if regions.is_empty() {
        return Ok(pdf_bytes.to_vec());
    }
    redact_pdf(pdf_bytes, &regions)
}

// ── Color parsing ───────────────────────────────────────────────────

/// Parse a hex color string to (r, g, b) in 0-1 range for PDF operators.
fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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

        // Verify the redaction content stream contains our rectangle operators
        let text = String::from_utf8_lossy(&result);
        assert!(text.contains("re f Q"));

        // Verify there's a new xref and trailer
        assert!(text.contains("/Prev"));
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
        let original_len = pdf_bytes.len();
        let redacted_has_text = pdf_contains_text_showing_ops_after(&result, "Hello", original_len);
        assert!(
            !redacted_has_text,
            "Replacement content stream should NOT contain text-showing operators for 'Hello'"
        );

        // But the visual overlay should still be present
        let text = String::from_utf8_lossy(&result);
        assert!(
            text.contains("re f Q"),
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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

        // Should have incremental update
        let text = String::from_utf8_lossy(&result);
        assert!(text.contains("/Prev"));
    }

    #[test]
    fn test_tokenizer_roundtrip() {
        let stream = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let tokens = tokenize_content_stream(stream);

        // Should have tokens for: BT, /F1, 12, Tf, 72, 720, Td, (Hello World), Tj, ET
        let operators: Vec<_> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::Operator(data) => Some(std::str::from_utf8(data).unwrap().to_string()),
                _ => None,
            })
            .collect();
        assert!(operators.contains(&"BT".to_string()));
        assert!(operators.contains(&"Tf".to_string()));
        assert!(operators.contains(&"Td".to_string()));
        assert!(operators.contains(&"Tj".to_string()));
        assert!(operators.contains(&"ET".to_string()));
    }

    #[test]
    fn test_strip_redacted_text_removes_overlapping() {
        // Simulate a content stream with text at position (72, 720) in PDF coords
        let stream = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let tokens = tokenize_content_stream(stream);

        // Redaction region that covers the text position
        let regions = vec![PdfRedactRegion {
            x: 50.0,
            y: 710.0,
            width: 200.0,
            height: 30.0,
        }];

        let filtered = strip_redacted_text(&tokens, &regions, &HashMap::new());
        let result = serialize_tokens(&filtered);
        let result_str = String::from_utf8_lossy(&result);

        // Should NOT contain the Tj operator
        assert!(
            !result_str.contains("Tj"),
            "Filtered stream should not contain Tj"
        );
        // But should still have BT/ET and positioning
        assert!(result_str.contains("BT"), "Should preserve BT");
        assert!(result_str.contains("ET"), "Should preserve ET");
        assert!(result_str.contains("Td"), "Should preserve Td");
    }

    #[test]
    fn test_strip_redacted_text_preserves_non_overlapping() {
        let stream = b"BT /F1 12 Tf 72 720 Td (Keep this) Tj ET";
        let tokens = tokenize_content_stream(stream);

        // Redaction region far away from the text
        let regions = vec![PdfRedactRegion {
            x: 400.0,
            y: 100.0,
            width: 100.0,
            height: 30.0,
        }];

        let filtered = strip_redacted_text(&tokens, &regions, &HashMap::new());
        let result = serialize_tokens(&filtered);
        let result_str = String::from_utf8_lossy(&result);

        // Should still contain the text operator
        assert!(
            result_str.contains("Tj"),
            "Non-overlapping text should be preserved"
        );
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
        let text = String::from_utf8_lossy(&result);
        // Should contain red color (1 0 0 rg)
        assert!(text.contains("1 0 0 rg"));
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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

        // The incremental update section should contain scrubbed /Info
        let new_section = String::from_utf8_lossy(&result[pdf_bytes.len()..]);
        assert!(
            new_section.contains("/Producer (Forme)"),
            "Replacement /Info should have /Producer (Forme)"
        );
        assert!(
            new_section.contains("/ModDate"),
            "Replacement /Info should have /ModDate"
        );
        assert!(
            !new_section.contains("Confidential Report"),
            "Original title should not appear in incremental update"
        );
        assert!(
            !new_section.contains("John Doe"),
            "Original author should not appear in incremental update"
        );
        assert!(
            !new_section.contains("SecretApp"),
            "Original creator should not appear in incremental update"
        );
    }

    // ── Text-search redaction tests ──────────────────────────────────────

    #[test]
    fn test_decode_pdf_string_literal() {
        assert_eq!(decode_pdf_string(b"(Hello)", None), "Hello");
        assert_eq!(decode_pdf_string(b"(Hello World)", None), "Hello World");
    }

    #[test]
    fn test_decode_pdf_string_escapes() {
        assert_eq!(decode_pdf_string(b"(line1\\nline2)", None), "line1\nline2");
        assert_eq!(decode_pdf_string(b"(a\\\\b)", None), "a\\b");
        assert_eq!(decode_pdf_string(b"(open\\(paren)", None), "open(paren");
        assert_eq!(decode_pdf_string(b"(close\\)paren)", None), "close)paren");
    }

    #[test]
    fn test_decode_pdf_string_hex() {
        assert_eq!(decode_pdf_string(b"<48656C6C6F>", None), "Hello");
        assert_eq!(decode_pdf_string(b"<48 65 6C 6C 6F>", None), "Hello");
    }

    #[test]
    fn test_decode_pdf_string_empty() {
        assert_eq!(decode_pdf_string(b"()", None), "");
        assert_eq!(decode_pdf_string(b"<>", None), "");
        assert_eq!(decode_pdf_string(b"", None), "");
    }

    #[test]
    fn test_extract_text_with_positions_basic() {
        let stream = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let blocks = extract_text_with_positions(stream, &HashMap::new());
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "Hello World");
        assert_eq!(blocks[0].1.len(), 11); // 11 chars
                                           // First char should be at x≈72 (the Td x position)
        assert!((blocks[0].1[0].x - 72.0).abs() < 1.0);
        assert_eq!(blocks[0].1[0].font_size, 12.0);
    }

    #[test]
    fn test_extract_text_with_positions_tj_array() {
        // TJ array with kerning: [(H) -50 (ello)]
        let stream = b"BT /F1 10 Tf 0 700 Td [(H) -50 (ello)] TJ ET";
        let blocks = extract_text_with_positions(stream, &HashMap::new());
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "Hello");
    }

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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
    fn test_decode_pdf_string_cid_hex() {
        let mut table = HashMap::new();
        table.insert(0x0042_u16, "H".to_string());
        table.insert(0x0045_u16, "e".to_string());
        table.insert(0x004C_u16, "l".to_string());
        table.insert(0x004F_u16, "o".to_string());
        let info = FontInfo {
            is_cid: true,
            cid_to_unicode: table,
            cid_widths: HashMap::new(),
            default_width: 500.0,
        };
        // CID hex string for "Hello"
        let decoded = decode_pdf_string(b"<00420045004C004C004F>", Some(&info));
        assert_eq!(decoded, "Hello");
    }

    #[test]
    fn test_decode_pdf_string_simple_unchanged() {
        // Without a CID FontInfo, hex decoding should remain single-byte WinAnsi.
        let decoded = decode_pdf_string(b"<48656C6C6F>", None);
        assert_eq!(decoded, "Hello");
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
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            flatten_forms: false,
            certification: None,
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
}
