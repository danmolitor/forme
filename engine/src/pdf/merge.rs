//! # PDF Merging
//!
//! Combines multiple PDF files into a single document by:
//! 1. Parsing each input PDF to extract page tree and object structure
//! 2. Copying all objects from each PDF into a new output with renumbered IDs
//! 3. Building a new /Pages tree and /Catalog referencing all collected pages
//!
//! Object ID renumbering is the critical operation: each input PDF has its own
//! ID space, so objects from PDF 1+ are offset to avoid collisions. All indirect
//! references (`N 0 R`) within those objects are rewritten with the same offset.
//!
//! v1 limitations: no font/resource deduplication, no AcroForm/bookmark merging,
//! no encrypted PDF support.

use crate::error::FormeError;

/// Merge multiple PDFs into a single document.
///
/// Takes a slice of PDF byte slices and returns merged PDF bytes.
/// All pages are included in order. Requires at least 2 input PDFs.
pub fn merge_pdfs(pdfs: &[&[u8]]) -> Result<Vec<u8>, FormeError> {
    if pdfs.len() < 2 {
        return Err(FormeError::RenderError(
            "merge_pdfs requires at least 2 PDFs".to_string(),
        ));
    }

    // 1. Scan each PDF for structural metadata.
    let scans: Vec<MergeScanResult> = pdfs
        .iter()
        .enumerate()
        .map(|(i, pdf)| {
            scan_pdf(pdf)
                .map_err(|e| FormeError::RenderError(format!("Failed to scan PDF {i}: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // 2. Compute ID offsets: PDF 0 keeps its IDs, PDF 1 starts at scans[0].size, etc.
    let mut offsets = vec![0usize; pdfs.len()];
    for i in 1..pdfs.len() {
        offsets[i] = offsets[i - 1] + scans[i - 1].size;
    }
    let total_original_objects: usize = scans.iter().map(|s| s.size).sum();

    // 3. Collect page object IDs from each PDF (with offsets applied).
    let mut all_page_ids: Vec<usize> = Vec::new();
    for (i, pdf) in pdfs.iter().enumerate() {
        let page_ids = collect_page_ids(pdf, &scans[i])?;
        for id in page_ids {
            all_page_ids.push(id + offsets[i]);
        }
    }

    // 4. Locate all objects in each PDF.
    let mut all_objects: Vec<Vec<ObjectSpan>> = Vec::new();
    for (i, pdf) in pdfs.iter().enumerate() {
        let objects = locate_objects(pdf, scans[i].size)?;
        all_objects.push(objects);
    }

    // 5. Build output PDF.
    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n");

    // Track byte offset for each new object ID.
    let new_pages_id = total_original_objects;
    let new_catalog_id = total_original_objects + 1;
    let total_objects = total_original_objects + 2; // +pages +catalog
    let mut xref_offsets: Vec<(usize, usize)> = Vec::new(); // (obj_id, byte_offset)

    // Write objects from each PDF, applying ID offsets.
    for (pdf_idx, pdf) in pdfs.iter().enumerate() {
        let offset = offsets[pdf_idx];
        for span in &all_objects[pdf_idx] {
            if span.obj_id == 0 {
                continue; // Skip object 0 (free entry)
            }
            let new_id = span.obj_id + offset;
            let obj_bytes = &pdf[span.start..span.end];

            xref_offsets.push((new_id, buf.len()));

            // Write object header with new ID.
            let header = format!("{new_id} 0 obj\n");
            buf.extend_from_slice(header.as_bytes());

            // Extract content between "N 0 obj\n" and "endobj" from original.
            let orig_header = format!("{} 0 obj", span.obj_id);
            let content_start = find_bytes(obj_bytes, orig_header.as_bytes())
                .map(|p| p + orig_header.len())
                .unwrap_or(0);
            let content_end = rfind_bytes(obj_bytes, b"endobj").unwrap_or(obj_bytes.len());
            let content = &obj_bytes[content_start..content_end];

            if offset == 0 {
                // First PDF: no rewriting needed.
                buf.extend_from_slice(content);
            } else {
                // Rewrite indirect references with offset.
                let rewritten = rewrite_references(content, offset);
                buf.extend_from_slice(&rewritten);
            }

            buf.extend_from_slice(b"endobj\n\n");
        }
    }

    // 6. Write new /Pages object.
    let kids: String = all_page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let page_count = all_page_ids.len();

    xref_offsets.push((new_pages_id, buf.len()));
    let pages_obj = format!(
        "{new_pages_id} 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {page_count} >>\nendobj\n\n"
    );
    buf.extend_from_slice(pages_obj.as_bytes());

    // Rewrite each page's /Parent to point to the new /Pages.
    // We already wrote the page objects above — now patch their /Parent references.
    // Instead of patching in place (complex), we handle /Parent during rewrite:
    // The rewrite_references function already offsets the old /Parent ref, but we
    // need all pages to point to new_pages_id. We'll do a post-pass to fix this.
    patch_parent_refs(&mut buf, &all_page_ids, new_pages_id);

    // 7. Write new /Catalog object.
    xref_offsets.push((new_catalog_id, buf.len()));
    let catalog_obj = format!(
        "{new_catalog_id} 0 obj\n<< /Type /Catalog /Pages {new_pages_id} 0 R >>\nendobj\n\n"
    );
    buf.extend_from_slice(catalog_obj.as_bytes());

    // 8. Write xref table.
    let xref_offset = buf.len();
    buf.extend_from_slice(b"xref\n");

    // Object 0 (free entry) + all objects.
    xref_offsets.sort_by_key(|(id, _)| *id);

    // Write object 0 free entry.
    buf.extend_from_slice(b"0 1\n0000000000 65535 f \n");

    // Write entries in subsections for consecutive IDs.
    let mut i = 0;
    while i < xref_offsets.len() {
        let start_id = xref_offsets[i].0;
        let mut count = 1;
        while i + count < xref_offsets.len() && xref_offsets[i + count].0 == start_id + count {
            count += 1;
        }
        buf.extend_from_slice(format!("{start_id} {count}\n").as_bytes());
        for j in 0..count {
            let offset = xref_offsets[i + j].1;
            buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        i += count;
    }

    // 9. Write trailer.
    buf.extend_from_slice(
        format!(
            "trailer\n<< /Size {total_objects} /Root {new_catalog_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
        )
        .as_bytes(),
    );

    Ok(buf)
}

// ── PDF scanning ────────────────────────────────────────────────────

struct MergeScanResult {
    size: usize,
    _root_obj: usize,
    pages_obj: usize,
}

fn scan_pdf(pdf: &[u8]) -> Result<MergeScanResult, FormeError> {
    let startxref_pos = rfind_bytes(pdf, b"startxref")
        .ok_or_else(|| FormeError::RenderError("No startxref found".to_string()))?;

    let trailer_pos = rfind_bytes(pdf, b"trailer")
        .ok_or_else(|| FormeError::RenderError("No trailer found".to_string()))?;
    let trailer_section = &pdf[trailer_pos..startxref_pos];

    let size = find_value_in_bytes(trailer_section, b"/Size")
        .ok_or_else(|| FormeError::RenderError("No /Size in trailer".to_string()))?;

    let root_obj = find_ref_in_bytes(trailer_section, b"/Root")
        .ok_or_else(|| FormeError::RenderError("No /Root in trailer".to_string()))?;

    // Find the catalog object using byte-level scanning to avoid
    // position corruption from String::from_utf8_lossy on binary streams.
    let pages_obj = find_catalog_ref_bytes(pdf, root_obj, b"/Pages")
        .ok_or_else(|| FormeError::RenderError("No /Pages in catalog".to_string()))?;

    Ok(MergeScanResult {
        size,
        _root_obj: root_obj,
        pages_obj,
    })
}

/// Collect leaf page object IDs in document order.
fn collect_page_ids(pdf: &[u8], scan: &MergeScanResult) -> Result<Vec<usize>, FormeError> {
    let kids = extract_kids_refs_bytes(pdf, scan.pages_obj)?;
    let mut page_ids = Vec::new();
    for kid_id in &kids {
        collect_page_ids_recursive_bytes(pdf, *kid_id, &mut page_ids)?;
    }
    Ok(page_ids)
}

fn collect_page_ids_recursive_bytes(
    pdf: &[u8],
    obj_id: usize,
    page_ids: &mut Vec<usize>,
) -> Result<(), FormeError> {
    let content = find_object_content_bytes(pdf, obj_id)
        .ok_or_else(|| FormeError::RenderError(format!("Cannot find object {obj_id}")))?;

    if find_bytes(content, b"/Type /Pages").is_some() {
        let kids = extract_kids_from_content_bytes(content)?;
        for kid_id in &kids {
            collect_page_ids_recursive_bytes(pdf, *kid_id, page_ids)?;
        }
    } else if find_bytes(content, b"/Type /Page").is_some() {
        page_ids.push(obj_id);
    }

    Ok(())
}

// ── Object location ─────────────────────────────────────────────────

/// Byte span of one object in the original PDF.
struct ObjectSpan {
    obj_id: usize,
    start: usize, // Start of "N 0 obj"
    end: usize,   // End of "endobj" (exclusive)
}

/// Locate all objects in a PDF by reading byte offsets from the xref table.
/// This is more reliable than scanning for "N 0 obj" patterns, which can
/// produce false matches inside compressed stream data.
fn locate_objects(pdf: &[u8], size: usize) -> Result<Vec<ObjectSpan>, FormeError> {
    let xref_offsets = parse_xref_offsets(pdf, size)?;
    let mut spans = Vec::new();

    for (obj_id, offset) in &xref_offsets {
        let obj_start = *offset;
        if obj_start >= pdf.len() {
            continue;
        }
        // Find "endobj" after the object header. Skip over any stream data
        // to avoid matching a phantom "endobj" inside compressed content.
        let rest = &pdf[obj_start..];
        let obj_end = find_endobj_skipping_stream(rest)
            .map(|e| obj_start + e)
            .unwrap_or(pdf.len());
        spans.push(ObjectSpan {
            obj_id: *obj_id,
            start: obj_start,
            end: obj_end,
        });
    }

    Ok(spans)
}

/// Parse the xref table to get byte offsets for each object.
fn parse_xref_offsets(pdf: &[u8], size: usize) -> Result<Vec<(usize, usize)>, FormeError> {
    let startxref_pos = rfind_bytes(pdf, b"startxref")
        .ok_or_else(|| FormeError::RenderError("No startxref".to_string()))?;
    let after = &pdf[startxref_pos + 9..];
    let xref_offset = parse_number_from_bytes(after)
        .ok_or_else(|| FormeError::RenderError("Cannot parse startxref value".to_string()))?;

    if xref_offset >= pdf.len() {
        return Err(FormeError::RenderError("Invalid xref offset".to_string()));
    }

    let mut offsets = Vec::with_capacity(size);
    let xref_data = &pdf[xref_offset..];

    // Skip "xref\n"
    if xref_data.len() <= 4 || &xref_data[..4] != b"xref" {
        return Err(FormeError::RenderError(
            "xref table not found at offset".to_string(),
        ));
    }
    let mut pos = 4;
    while pos < xref_data.len() && (xref_data[pos] == b'\n' || xref_data[pos] == b'\r') {
        pos += 1;
    }

    // Parse subsections: "start_id count\n" followed by count entries
    while pos < xref_data.len() && xref_data[pos].is_ascii_digit() {
        // Parse start_id
        let num_start = pos;
        while pos < xref_data.len() && xref_data[pos].is_ascii_digit() {
            pos += 1;
        }
        let start_id: usize = std::str::from_utf8(&xref_data[num_start..pos])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Skip space
        while pos < xref_data.len() && xref_data[pos] == b' ' {
            pos += 1;
        }

        // Parse count
        let num_start = pos;
        while pos < xref_data.len() && xref_data[pos].is_ascii_digit() {
            pos += 1;
        }
        let count: usize = std::str::from_utf8(&xref_data[num_start..pos])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Skip newline
        while pos < xref_data.len() && (xref_data[pos] == b'\n' || xref_data[pos] == b'\r') {
            pos += 1;
        }

        // Read count entries, each is "OOOOOOOOOO GGGGG n \n" (20 bytes)
        for i in 0..count {
            if pos + 18 > xref_data.len() {
                break;
            }
            let entry = &xref_data[pos..pos + 20.min(xref_data.len() - pos)];
            // Parse the offset (first 10 digits)
            if let Some(byte_offset) = std::str::from_utf8(&entry[..10])
                .ok()
                .and_then(|s| s.trim().parse::<usize>().ok())
            {
                // Check if it's an 'n' (in-use) entry, not 'f' (free)
                if entry.len() > 17 && entry[17] == b'n' {
                    offsets.push((start_id + i, byte_offset));
                }
            }
            // Advance past the entry (20 bytes including trailing whitespace)
            pos += 20;
        }
    }

    Ok(offsets)
}

/// Find "endobj" after an object header, skipping over stream..endstream
/// to avoid matching phantom "endobj" inside compressed data.
fn find_endobj_skipping_stream(data: &[u8]) -> Option<usize> {
    let endobj_pos = find_bytes(data, b"endobj")?;
    let stream_pos = find_bytes(data, b"stream");

    // If "stream" appears BEFORE "endobj", this object has a stream block.
    // We need to skip past endstream to find the real endobj.
    if let Some(sp) = stream_pos {
        if sp < endobj_pos
            && sp + 6 < data.len()
            && (data[sp + 6] == b'\n' || data[sp + 6] == b'\r')
        {
            // Find "endstream" after the stream start
            if let Some(es) = find_bytes(&data[sp + 6..], b"endstream") {
                let after_endstream = sp + 6 + es + 9;
                // Find "endobj" after endstream
                if let Some(eo) = find_bytes(&data[after_endstream..], b"endobj") {
                    return Some(after_endstream + eo + 6);
                }
            }
        }
    }

    // No stream before endobj — this is a simple dictionary object.
    Some(endobj_pos + 6)
}

// ── Reference rewriting ─────────────────────────────────────────────

/// Rewrite all indirect references (N 0 R) in object content, adding `offset`
/// to each object number. Skips stream data and string literals to avoid
/// corrupting binary content.
fn rewrite_references(content: &[u8], offset: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 256);
    let mut i = 0;
    let len = content.len();

    // Track whether we're inside a stream (skip rewriting).
    let mut in_stream = false;

    while i < len {
        // Check for stream start.
        if !in_stream && i + 6 <= len && &content[i..i + 6] == b"stream" {
            // Verify it's followed by \n or \r\n.
            let after = i + 6;
            if after < len && (content[after] == b'\n' || content[after] == b'\r') {
                in_stream = true;
                out.extend_from_slice(&content[i..after]);
                i = after;
                continue;
            }
        }

        // Check for stream end.
        if in_stream && i + 9 <= len && &content[i..i + 9] == b"endstream" {
            in_stream = false;
            out.extend_from_slice(b"endstream");
            i += 9;
            continue;
        }

        // Inside a stream: copy verbatim.
        if in_stream {
            out.push(content[i]);
            i += 1;
            continue;
        }

        // Skip string literals (...) — they can contain arbitrary bytes.
        if content[i] == b'(' {
            out.push(b'(');
            i += 1;
            let mut depth = 1;
            while i < len && depth > 0 {
                if content[i] == b'\\' && i + 1 < len {
                    // Escaped character — copy both bytes.
                    out.push(content[i]);
                    out.push(content[i + 1]);
                    i += 2;
                    continue;
                }
                if content[i] == b'(' {
                    depth += 1;
                } else if content[i] == b')' {
                    depth -= 1;
                }
                out.push(content[i]);
                i += 1;
            }
            continue;
        }

        // Skip comments (% to end of line).
        if content[i] == b'%' {
            while i < len && content[i] != b'\n' && content[i] != b'\r' {
                out.push(content[i]);
                i += 1;
            }
            continue;
        }

        // Look for digit sequences that might be indirect references (N 0 R)
        // or object definitions (N 0 obj).
        if content[i].is_ascii_digit() {
            // Verify this is the START of a number (not a suffix of a larger number).
            let is_number_start = i == 0 || !content[i - 1].is_ascii_digit();

            if is_number_start {
                // Collect the full number.
                let num_start = i;
                while i < len && content[i].is_ascii_digit() {
                    i += 1;
                }
                let num_end = i;

                // Check for " 0 R" after the number (4 bytes: space, '0', space, 'R').
                if i + 4 <= len && &content[i..i + 4] == b" 0 R" {
                    // Verify it's a complete reference (next byte is not a letter).
                    let after_r = i + 4;
                    let is_complete = after_r >= len || !content[after_r].is_ascii_alphabetic();

                    if is_complete {
                        if let Ok(obj_id) = std::str::from_utf8(&content[num_start..num_end])
                            .unwrap_or("")
                            .parse::<usize>()
                        {
                            let new_id = obj_id + offset;
                            out.extend_from_slice(format!("{new_id} 0 R").as_bytes());
                            i += 4; // skip " 0 R"
                            continue;
                        }
                    }
                }

                // Not a reference — emit the number as-is.
                out.extend_from_slice(&content[num_start..num_end]);
                continue;
            }
        }

        out.push(content[i]);
        i += 1;
    }

    out
}

/// After writing all objects, patch /Parent references in page objects
/// to point to the new unified /Pages object.
fn patch_parent_refs(buf: &mut Vec<u8>, page_ids: &[usize], new_pages_id: usize) {
    let new_ref = format!("{new_pages_id} 0 R");
    let new_ref_bytes = new_ref.as_bytes();

    // Collect (start, end) byte ranges of old "/Parent N 0 R" values to replace.
    // Uses raw byte scanning to avoid position corruption from lossy UTF-8.
    let mut patches: Vec<(usize, usize)> = Vec::new();
    {
        let data: &[u8] = buf;
        for &page_id in page_ids {
            let header = format!("{page_id} 0 obj");
            if let Some(obj_start) = find_bytes(data, header.as_bytes()) {
                let rest = &data[obj_start..];
                if let Some(endobj) = find_bytes(rest, b"endobj") {
                    let obj_section = &rest[..endobj];
                    if let Some(parent_pos) = find_bytes(obj_section, b"/Parent ") {
                        let abs_parent = obj_start + parent_pos + 8; // skip "/Parent "
                        let remaining = &data[abs_parent..obj_start + endobj];
                        if let Some(r_pos) = find_bytes(remaining, b" 0 R") {
                            patches.push((abs_parent, abs_parent + r_pos + 4));
                        }
                    }
                }
            }
        }
    }

    // Apply patches in reverse order so byte offsets remain valid.
    patches.sort_by_key(|b| std::cmp::Reverse(b.0));
    for (start, end) in patches {
        let mut new_buf = Vec::with_capacity(buf.len());
        new_buf.extend_from_slice(&buf[..start]);
        new_buf.extend_from_slice(new_ref_bytes);
        new_buf.extend_from_slice(&buf[end..]);
        *buf = new_buf;
    }
}

// ── Shared helpers (same patterns as redaction.rs) ──────────────────

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

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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

fn find_value_in_bytes(section: &[u8], key: &[u8]) -> Option<usize> {
    let pos = find_bytes(section, key)?;
    parse_number_from_bytes(&section[pos + key.len()..])
}

fn find_ref_in_bytes(section: &[u8], key: &[u8]) -> Option<usize> {
    let pos = find_bytes(section, key)?;
    parse_number_from_bytes(&section[pos + key.len()..])
}

/// Find an object's content bytes (between "N 0 obj" header and "endobj").
/// Works on raw bytes to avoid position corruption from lossy UTF-8 conversion.
fn find_object_content_bytes(pdf: &[u8], obj_id: usize) -> Option<&[u8]> {
    let header = format!("{obj_id} 0 obj");
    let header_bytes = header.as_bytes();
    let mut from = 0;
    while let Some(rel) = find_bytes(&pdf[from..], header_bytes) {
        let start = from + rel;
        // The id must be a standalone number: a search for object 3 must not
        // match the "3 0 obj" inside "13 0 obj" (#206).
        if start == 0 || !pdf[start - 1].is_ascii_digit() {
            let after_header = start + header_bytes.len();
            let rest = &pdf[after_header..];
            let end = find_bytes(rest, b"endobj")?;
            return Some(&rest[..end]);
        }
        from = start + 1;
    }
    None
}

/// Find a reference value (N 0 R) for a key inside an object.
fn find_catalog_ref_bytes(pdf: &[u8], obj_id: usize, key: &[u8]) -> Option<usize> {
    let content = find_object_content_bytes(pdf, obj_id)?;
    let key_pos = find_bytes(content, key)?;
    let after_key = &content[key_pos + key.len()..];
    // Skip whitespace, parse number.
    let mut i = 0;
    while i < after_key.len() && after_key[i].is_ascii_whitespace() {
        i += 1;
    }
    let num_start = i;
    while i < after_key.len() && after_key[i].is_ascii_digit() {
        i += 1;
    }
    if i == num_start {
        return None;
    }
    std::str::from_utf8(&after_key[num_start..i])
        .ok()?
        .parse()
        .ok()
}

fn extract_kids_refs_bytes(pdf: &[u8], pages_obj: usize) -> Result<Vec<usize>, FormeError> {
    let content = find_object_content_bytes(pdf, pages_obj)
        .ok_or_else(|| FormeError::RenderError(format!("Cannot find /Pages object {pages_obj}")))?;
    extract_kids_from_content_bytes(content)
}

fn extract_kids_from_content_bytes(content: &[u8]) -> Result<Vec<usize>, FormeError> {
    let kids_pos = find_bytes(content, b"/Kids")
        .ok_or_else(|| FormeError::RenderError("No /Kids in /Pages object".to_string()))?;
    let after = &content[kids_pos + 5..];
    let bracket_start = find_bytes(after, b"[")
        .ok_or_else(|| FormeError::RenderError("No [ after /Kids".to_string()))?;
    let bracket_end = find_bytes(after, b"]")
        .ok_or_else(|| FormeError::RenderError("No ] after /Kids".to_string()))?;
    let inner = &after[bracket_start + 1..bracket_end];

    let mut refs = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        // Skip non-digit bytes.
        if !inner[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // Collect digit sequence.
        let num_start = i;
        while i < inner.len() && inner[i].is_ascii_digit() {
            i += 1;
        }
        if let Some(id) = std::str::from_utf8(&inner[num_start..i])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            refs.push(id);
        }
        // Skip " 0 R" if present.
        if i + 3 <= inner.len() && &inner[i..i + 3] == b" 0 " {
            i += 3;
            if i < inner.len() && inner[i] == b'R' {
                i += 1;
            }
        }
    }

    Ok(refs)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_pdf(text: &str) -> Vec<u8> {
        let json = format!(
            r#"{{"children":[{{"kind":{{"type":"Page"}},"children":[{{"kind":{{"type":"Text","content":"{text}"}}}}]}}]}}"#
        );
        crate::render_json(&json).unwrap()
    }

    /// Build a minimal valid PDF with the objects emitted in `order`.
    /// Object 13 comes before object 3 in the reordered variant, which used to
    /// make the `"{id} 0 obj"` substring lookup find 13 when asked for 3.
    fn build_reordered_pdf(order: &[usize]) -> Vec<u8> {
        let stream = "BT /F1 24 Tf 72 700 Td (Hello) Tj ET";
        let body = |id: usize| -> String {
            match id {
                1 => "<< /Type /Catalog /Pages 3 0 R >>".to_string(),
                2 => "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
                3 => "<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_string(),
                4 => "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 2 0 R >> >> /Contents 5 0 R >>".to_string(),
                5 => format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
                13 => "<< /Producer (repro) >>".to_string(),
                _ => unreachable!(),
            }
        };

        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = std::collections::HashMap::new();
        for &id in order {
            offsets.insert(id, pdf.len());
            pdf.push_str(&format!("{id} 0 obj\n{}\nendobj\n", body(id)));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for id in 1..=5 {
            pdf.push_str(&format!("{:010} 00000 n \n", offsets[&id]));
        }
        pdf.push_str("13 1\n");
        pdf.push_str(&format!("{:010} 00000 n \n", offsets[&13]));
        pdf.push_str(&format!(
            "trailer\n<< /Size 14 /Root 1 0 R /Info 13 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        pdf.into_bytes()
    }

    #[test]
    fn test_merge_pdf_with_out_of_order_objects() {
        let ordered = build_reordered_pdf(&[1, 2, 3, 4, 5, 13]);
        let reordered = build_reordered_pdf(&[13, 1, 2, 3, 4, 5]);
        let merged =
            merge_pdfs(&[&ordered, &reordered]).expect("merge must accept out-of-order objects");
        assert!(merged.starts_with(b"%PDF"));
    }

    #[test]
    fn test_merge_two_pdfs() {
        let pdf1 = make_simple_pdf("Page One");
        let pdf2 = make_simple_pdf("Page Two");
        let merged = merge_pdfs(&[&pdf1, &pdf2]).unwrap();

        // Valid PDF header.
        assert!(merged.starts_with(b"%PDF"));

        // Count leaf /Page objects (not /Pages).
        let text = String::from_utf8_lossy(&merged);
        let page_count = text.matches("/Type /Page\n").count()
            + text.matches("/Type /Page ").count()
            + text.matches("/Type /Page/").count()
            + text.matches("/Type /Page>").count();
        // Should have at least 2 page objects.
        assert!(page_count >= 2, "Expected >= 2 pages, found {page_count}");
    }

    #[test]
    fn test_merge_three_pdfs() {
        let pdf1 = make_simple_pdf("First");
        let pdf2 = make_simple_pdf("Second");
        let pdf3 = make_simple_pdf("Third");
        let merged = merge_pdfs(&[&pdf1, &pdf2, &pdf3]).unwrap();

        assert!(merged.starts_with(b"%PDF"));

        let text = String::from_utf8_lossy(&merged);
        let page_count = text.matches("/Type /Page\n").count()
            + text.matches("/Type /Page ").count()
            + text.matches("/Type /Page/").count()
            + text.matches("/Type /Page>").count();
        assert!(page_count >= 3, "Expected >= 3 pages, found {page_count}");
    }

    #[test]
    fn test_merge_preserves_content() {
        let pdf1 = make_simple_pdf("Hello");
        let pdf2 = make_simple_pdf("World");
        let merged = merge_pdfs(&[&pdf1, &pdf2]).unwrap();

        // Content streams are FlateDecode-compressed, so "Hello" won't appear
        // as plain text. Instead verify structural integrity: the merged PDF
        // should contain the content stream objects from both inputs.
        // Each input has one /Contents reference, so merged should have two.
        let contents_count = merged.windows(9).filter(|w| *w == b"/Contents").count();
        assert!(
            contents_count >= 2,
            "Merged PDF should have >= 2 /Contents refs, found {contents_count}"
        );

        // Verify valid PDF structure.
        assert!(merged.starts_with(b"%PDF"));
        assert!(merged.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn test_merge_with_embedded_data() {
        // PDFs with embedded data have extra objects (EmbeddedFile, FileSpec, Names)
        // that previously caused phantom object matches inside compressed streams.
        let json1 = r#"{"embeddedData":"{\"invoice\":1}","children":[{"kind":{"type":"Page"},"children":[{"kind":{"type":"Text","content":"Invoice 1"}}]}]}"#;
        let json2 = r#"{"embeddedData":"{\"invoice\":2}","children":[{"kind":{"type":"Page"},"children":[{"kind":{"type":"Text","content":"Invoice 2"}}]}]}"#;
        let pdf1 = crate::render_json(json1).unwrap();
        let pdf2 = crate::render_json(json2).unwrap();
        let merged = merge_pdfs(&[&pdf1, &pdf2]).unwrap();

        assert!(merged.starts_with(b"%PDF"));

        // Verify we get 2 page objects with correct references.
        let page_count = merged.windows(11).filter(|w| *w == b"/Type /Page").count()
            - merged.windows(12).filter(|w| *w == b"/Type /Pages").count();
        assert!(page_count >= 2, "Expected >= 2 pages, found {page_count}");

        // Verify references in PDF 2's pages were rewritten (offset applied).
        // PDF 1 has /Size 9, so PDF 2's refs should be offset by 9.
        // Original page in PDF 2 had /Contents 5 0 R — after offset should be 14 0 R.
        // If we still see "/Contents 5 0 R" in a page with 612x792 MediaBox
        // (PDF 2's page size), the rewriting failed.
        let merged_text = String::from_utf8_lossy(&merged);
        for line in merged_text.lines() {
            if line.contains("/Type /Page") && line.contains("612.00") {
                assert!(
                    !line.contains("/Contents 5 0 R"),
                    "PDF 2 page refs were not rewritten: {line}"
                );
            }
        }
    }

    #[test]
    fn test_rewrite_references_directly() {
        let content = b"\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Contents 4 0 R /Resources << /Font << /F0 3 0 R >> >> >>\n";
        let rewritten = rewrite_references(content, 9);
        let text = String::from_utf8_lossy(&rewritten);
        eprintln!("Rewritten: {text}");
        assert!(text.contains("11 0 R"), "Parent should be 11, got: {text}");
        assert!(
            text.contains("13 0 R"),
            "Contents should be 13, got: {text}"
        );
        assert!(text.contains("12 0 R"), "Font should be 12, got: {text}");
    }

    #[test]
    fn test_merge_refs_are_rewritten() {
        // Verify that merging two PDFs with embedded data produces
        // correctly rewritten references (not original object IDs).
        let json1 = r#"{"embeddedData":"{\"x\":1}","children":[{"kind":{"type":"Page"},"children":[{"kind":{"type":"Text","content":"One"}}]}]}"#;
        let json2 = r#"{"embeddedData":"{\"y\":2}","children":[{"kind":{"type":"Page"},"children":[{"kind":{"type":"Text","content":"Two"}}]}]}"#;
        let pdf1 = crate::render_json(json1).unwrap();
        let pdf2 = crate::render_json(json2).unwrap();

        let scan1 = scan_pdf(&pdf1).unwrap();
        let scan2 = scan_pdf(&pdf2).unwrap();
        eprintln!("PDF1 /Size={}, PDF2 /Size={}", scan1.size, scan2.size);

        let merged = merge_pdfs(&[&pdf1, &pdf2]).unwrap();

        // PDF 2's page originally has /Contents 4 0 R.
        // With offset = scan1.size (9), it should become /Contents 13 0 R.
        // Find the second page (from PDF 2) and verify its Contents ref.
        let offset = scan1.size;
        eprintln!("Expected offset: {offset}");

        // The merged PDF's second page should NOT have /Contents 4 0 R.
        // Find all page objects and check.
        let mut found_pdf2_page = false;
        let objs = locate_objects(&merged, 0).unwrap();
        for span in &objs {
            let content = find_object_content_bytes(&merged, span.obj_id);
            if let Some(c) = content {
                if find_bytes(c, b"/Type /Page").is_some()
                    && find_bytes(c, b"/Type /Pages").is_none()
                    && span.obj_id > scan1.size
                {
                    found_pdf2_page = true;
                    let text = String::from_utf8_lossy(c);
                    eprintln!("PDF2 page obj {}: {}", span.obj_id, text.trim());
                    // Contents ref should be 4 + offset = 13
                    let expected = format!("/Contents {} 0 R", 4 + offset);
                    assert!(
                        text.contains(&expected),
                        "Expected '{}' in page {}, got: {}",
                        expected,
                        span.obj_id,
                        text.trim()
                    );
                }
            }
        }
        assert!(found_pdf2_page, "Should have found PDF 2's page");
    }

    #[test]
    fn test_merge_requires_two_pdfs() {
        let pdf1 = make_simple_pdf("Only one");
        let result = merge_pdfs(&[&pdf1]);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_empty_input() {
        let result = merge_pdfs(&[]);
        assert!(result.is_err());
    }
}
