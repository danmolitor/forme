//! Security regression tests for redaction.
//!
//! A redacted PDF must not let anyone recover the redacted content: not from
//! the current page, a previous revision, an unreferenced object, a compressed
//! stream or object stream, metadata, annotations, attachments, alternate
//! text, or bookmarks. Non-redacted content must survive.
//!
//! The checks deliberately do NOT use the engine's own PDF parser (that is the
//! code under test). `Evidence` scans the raw bytes, inflates every stream it
//! can find (including object streams), and decodes every literal, hex and
//! UTF-16 string, then searches all of it.

use forme::pdf::redaction::{find_text_regions, redact_pdf, redact_text};
use forme::{PatternType, RedactionPattern, RedactionRegion};
use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib;

// ───────────────────────── independent evidence scanner ─────────────────────────

/// Everything an attacker could read out of a file without special tools.
struct Evidence {
    corpus: Vec<String>,
}

impl Evidence {
    fn of(pdf: &[u8]) -> Self {
        let mut blobs: Vec<Vec<u8>> = vec![pdf.to_vec()];
        let mut i = 0;
        // Inflate every stream body in the raw bytes; then the same inside
        // anything inflated (object streams hold further objects).
        while i < blobs.len() {
            let blob = blobs[i].clone();
            for body in stream_bodies(&blob) {
                if let Ok(inflated) = decompress_to_vec_zlib(&body) {
                    blobs.push(inflated);
                }
                if let Some(hex) = ascii_hex_decode(&body) {
                    blobs.push(hex);
                }
            }
            i += 1;
        }
        let mut corpus = Vec::new();
        for b in &blobs {
            corpus.push(latin1(b));
            for s in pdf_strings(b) {
                corpus.push(latin1(&s));
                if let Some(u) = utf16be(&s) {
                    corpus.push(u);
                }
            }
        }
        Evidence { corpus }
    }

    fn contains(&self, needle: &str) -> bool {
        self.corpus.iter().any(|c| c.contains(needle))
    }

    /// Where the needle was found (for failure messages).
    fn locate(&self, needle: &str) -> String {
        for c in &self.corpus {
            if let Some(i) = c.find(needle) {
                let s = c[..i]
                    .char_indices()
                    .rev()
                    .nth(80)
                    .map(|(p, _)| p)
                    .unwrap_or(0);
                let e = (i + needle.len() + 80).min(c.len());
                let e = (e..=c.len())
                    .find(|&k| c.is_char_boundary(k))
                    .unwrap_or(c.len());
                return format!("…{}…", &c[s..e]);
            }
        }
        String::new()
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn utf16be(b: &[u8]) -> Option<String> {
    if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
        let units: Vec<u16> = b[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    None
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from >= h.len() || n.len() > h.len() {
        return None;
    }
    h[from..]
        .windows(n.len())
        .position(|w| w == n)
        .map(|p| p + from)
}

/// Bodies between `stream<EOL>` and `endstream`, without trusting /Length.
fn stream_bodies(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(s) = find(b, b"stream", pos) {
        // skip the "endstream" keyword itself
        if s >= 3 && &b[s - 3..s] == b"end" {
            pos = s + 6;
            continue;
        }
        let mut start = s + 6;
        if b.get(start) == Some(&b'\r') {
            start += 1;
        }
        if b.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(e) = find(b, b"endstream", start) else {
            break;
        };
        let mut end = e;
        while end > start && (b[end - 1] == b'\n' || b[end - 1] == b'\r') {
            end -= 1;
        }
        out.push(b[start..end].to_vec());
        pos = e + 9;
    }
    out
}

fn ascii_hex_decode(b: &[u8]) -> Option<Vec<u8>> {
    let digits: Vec<u8> = b
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if digits.len() < 4 || !digits.iter().all(|c| c.is_ascii_hexdigit() || *c == b'>') {
        return None;
    }
    let mut h: Vec<u8> = digits.into_iter().filter(|c| *c != b'>').collect();
    if h.len() % 2 == 1 {
        h.push(b'0');
    }
    Some(
        h.chunks(2)
            .map(|p| u8::from_str_radix(&latin1(p), 16).unwrap_or(0))
            .collect(),
    )
}

/// Decode every `( … )` literal and `< … >` hex string in a blob.
fn pdf_strings(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                let mut depth = 1;
                let mut s = Vec::new();
                i += 1;
                while i < b.len() && depth > 0 {
                    let c = b[i];
                    if c == b'\\' && i + 1 < b.len() {
                        let n = b[i + 1];
                        i += 2;
                        match n {
                            b'n' => s.push(b'\n'),
                            b'r' => s.push(b'\r'),
                            b't' => s.push(b'\t'),
                            b'b' => s.push(8),
                            b'f' => s.push(12),
                            b'0'..=b'7' => {
                                let mut v = (n - b'0') as u32;
                                let mut k = 0;
                                while k < 2 && i < b.len() && (b'0'..=b'7').contains(&b[i]) {
                                    v = v * 8 + (b[i] - b'0') as u32;
                                    i += 1;
                                    k += 1;
                                }
                                s.push(v as u8);
                            }
                            b'\r' | b'\n' => {}
                            other => s.push(other),
                        }
                        continue;
                    }
                    if c == b'(' {
                        depth += 1;
                    } else if c == b')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    s.push(c);
                    i += 1;
                }
                out.push(s);
                i += 1;
            }
            b'<' if b.get(i + 1) != Some(&b'<') => {
                let start = i + 1;
                let Some(e) = find(b, b">", start) else { break };
                let inner: Vec<u8> = b[start..e]
                    .iter()
                    .copied()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect();
                if !inner.is_empty() && inner.iter().all(|c| c.is_ascii_hexdigit()) {
                    let mut h = inner.clone();
                    if h.len() % 2 == 1 {
                        h.push(b'0');
                    }
                    out.push(
                        h.chunks(2)
                            .map(|p| u8::from_str_radix(&latin1(p), 16).unwrap())
                            .collect(),
                    );
                }
                i = e + 1;
            }
            b'<' => i += 2,
            _ => i += 1,
        }
    }
    out
}

/// One revision, one xref, no `/Prev`, and every `N G R` reference resolves to
/// an object defined exactly once.
fn assert_single_self_contained_revision(pdf: &[u8]) {
    let text = latin1(pdf);
    assert_eq!(
        text.matches("%%EOF").count(),
        1,
        "output must have exactly one revision (one %%EOF)"
    );
    assert_eq!(
        text.matches("startxref").count(),
        1,
        "output must have exactly one startxref"
    );
    let trailer = &text[text.rfind("trailer").expect("classic trailer")..];
    assert!(
        !trailer.contains("/Prev"),
        "output must not chain to a previous revision (/Prev)"
    );
    // Object headers outside stream bodies.
    let outside = strip_stream_bodies(pdf);
    let o = latin1(&outside);
    let mut defined = std::collections::HashMap::new();
    for cap in o.split('\n') {
        let t = cap.trim();
        if let Some(rest) = t.strip_suffix(" obj") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
                *defined.entry(parts[0].to_string()).or_insert(0) += 1;
            }
        }
    }
    for (id, n) in &defined {
        assert_eq!(*n, 1, "object {id} defined {n} times");
    }
    let toks: Vec<&str> = o
        .split(|c: char| c.is_whitespace() || "[]<>/()".contains(c))
        .filter(|s| !s.is_empty())
        .collect();
    for w in toks.windows(3) {
        if w[2] == "R" && w[0].parse::<u32>().is_ok() && w[1].parse::<u32>().is_ok() {
            assert!(
                defined.contains_key(w[0]),
                "dangling reference {} {} R",
                w[0],
                w[1]
            );
        }
    }
}

fn strip_stream_bodies(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(s) = find(b, b"stream", pos) {
        if s >= 3 && &b[s - 3..s] == b"end" {
            out.extend_from_slice(&b[pos..s + 6]);
            pos = s + 6;
            continue;
        }
        out.extend_from_slice(&b[pos..s + 6]);
        let Some(e) = find(b, b"endstream", s + 6) else {
            return out;
        };
        out.extend_from_slice(b"\n");
        pos = e;
    }
    out.extend_from_slice(&b[pos..]);
    out
}

fn has_text(pdf: &[u8], s: &str) -> bool {
    !find_text_regions(pdf, &[lit(s)])
        .expect("output must parse")
        .is_empty()
}

fn lit(s: &str) -> RedactionPattern {
    RedactionPattern {
        pattern: s.to_string(),
        pattern_type: PatternType::Literal,
        page: None,
        color: None,
    }
}

fn ssn_regex() -> RedactionPattern {
    RedactionPattern {
        pattern: r"\b\d{3}-\d{2}-\d{4}\b".to_string(),
        pattern_type: PatternType::Regex,
        page: None,
        color: None,
    }
}

// ───────────────────────── fixture builder ─────────────────────────

struct Obj {
    id: usize,
    /// Dictionary entries (without << >>) for streams, or the full object body.
    body: String,
    stream: Option<Vec<u8>>,
}

fn obj(id: usize, body: &str) -> Obj {
    Obj {
        id,
        body: body.to_string(),
        stream: None,
    }
}

fn sobj(id: usize, dict_entries: &str, data: &[u8]) -> Obj {
    Obj {
        id,
        body: dict_entries.to_string(),
        stream: Some(data.to_vec()),
    }
}

fn write_obj(buf: &mut Vec<u8>, o: &Obj, compress: bool) {
    buf.extend_from_slice(format!("{} 0 obj\n", o.id).as_bytes());
    match &o.stream {
        None => buf.extend_from_slice(o.body.as_bytes()),
        Some(data) => {
            let (bytes, filter) = if compress {
                (compress_to_vec_zlib(data, 6), " /Filter /FlateDecode")
            } else {
                (data.clone(), "")
            };
            buf.extend_from_slice(
                format!(
                    "<< {}{} /Length {} >>\nstream\n",
                    o.body,
                    filter,
                    bytes.len()
                )
                .as_bytes(),
            );
            buf.extend_from_slice(&bytes);
            buf.extend_from_slice(b"\nendstream");
        }
    }
    buf.extend_from_slice(b"\nendobj\n");
}

fn trailer_extra(info: Option<usize>) -> String {
    info.map(|i| format!(" /Info {i} 0 R")).unwrap_or_default()
}

/// Classic xref, single revision.
fn build(objs: &[Obj], root: usize, info: Option<usize>, compress: bool) -> Vec<u8> {
    let mut buf = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let size = objs.iter().map(|o| o.id).max().unwrap() + 1;
    let mut offsets = vec![0usize; size];
    for o in objs {
        offsets[o.id] = buf.len();
        write_obj(&mut buf, o, compress);
    }
    let xref = buf.len();
    buf.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for &off in offsets.iter().skip(1) {
        if off == 0 {
            buf.extend_from_slice(b"0000000000 65535 f \n");
        } else {
            buf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
    }
    buf.extend_from_slice(
        format!(
            "trailer\n<< /Size {size} /Root {root} 0 R{} >>\nstartxref\n{xref}\n%%EOF\n",
            trailer_extra(info)
        )
        .as_bytes(),
    );
    buf
}

/// Append an incremental update revision with a classic xref and /Prev.
fn append_revision(mut buf: Vec<u8>, objs: &[Obj], root: usize, info: Option<usize>) -> Vec<u8> {
    let t = latin1(&buf);
    let sx = t.rfind("startxref").unwrap();
    let prev: usize = t[sx + 9..]
        .trim()
        .lines()
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let size = objs.iter().map(|o| o.id).max().unwrap().max(
        t.rfind("/Size ")
            .map(|p| {
                t[p + 6..]
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
                    - 1
            })
            .unwrap(),
    ) + 1;
    let mut entries = Vec::new();
    for o in objs {
        entries.push((o.id, buf.len()));
        write_obj(&mut buf, o, true);
    }
    let xref = buf.len();
    buf.extend_from_slice(b"xref\n0 1\n0000000000 65535 f \n");
    entries.sort();
    for (id, off) in entries {
        buf.extend_from_slice(format!("{id} 1\n{off:010} 00000 n \n").as_bytes());
    }
    buf.extend_from_slice(format!("trailer\n<< /Size {size} /Root {root} 0 R{} /Prev {prev} >>\nstartxref\n{xref}\n%%EOF\n", trailer_extra(info)).as_bytes());
    buf
}

/// Non-stream objects packed into an object stream; cross-reference stream
/// (optionally with a PNG Up predictor). This is how most modern producers
/// (Chrome, Word, macOS) write PDFs.
fn build_objstm(objs: &[Obj], root: usize, info: Option<usize>, predictor: bool) -> Vec<u8> {
    let packed: Vec<&Obj> = objs.iter().filter(|o| o.stream.is_none()).collect();
    let plain: Vec<&Obj> = objs.iter().filter(|o| o.stream.is_some()).collect();
    let max = objs.iter().map(|o| o.id).max().unwrap();
    let objstm_id = max + 1;
    let xref_id = max + 2;
    let size = max + 3;
    // Object stream body: "id off id off ..." header then objects.
    let mut header = String::new();
    let mut body = String::new();
    for o in &packed {
        header.push_str(&format!("{} {} ", o.id, body.len()));
        body.push_str(&o.body);
        body.push('\n');
    }
    let first = header.len();
    let objstm_data = format!("{header}{body}");
    let mut buf = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut entry: Vec<(u8, u32, u16)> = vec![(0, 0, 0xFFFF); size];
    for o in &plain {
        entry[o.id] = (1, buf.len() as u32, 0);
        write_obj(&mut buf, o, true);
    }
    entry[objstm_id] = (1, buf.len() as u32, 0);
    write_obj(
        &mut buf,
        &sobj(
            objstm_id,
            &format!("/Type /ObjStm /N {} /First {first}", packed.len()),
            objstm_data.as_bytes(),
        ),
        true,
    );
    for (idx, o) in packed.iter().enumerate() {
        entry[o.id] = (2, objstm_id as u32, idx as u16);
    }
    let xref_off = buf.len();
    entry[xref_id] = (1, xref_off as u32, 0);
    let mut rows: Vec<u8> = Vec::new();
    let mut prev_row = vec![0u8; 7];
    for (t, f2, f3) in &entry {
        let row = [
            vec![*t],
            f2.to_be_bytes().to_vec(),
            f3.to_be_bytes().to_vec(),
        ]
        .concat();
        if predictor {
            rows.push(2); // PNG Up
            for k in 0..7 {
                rows.push(row[k].wrapping_sub(prev_row[k]));
            }
            prev_row = row;
        } else {
            rows.extend_from_slice(&row);
        }
    }
    let parms = if predictor {
        " /DecodeParms << /Predictor 12 /Columns 7 >>"
    } else {
        ""
    };
    let comp = compress_to_vec_zlib(&rows, 6);
    buf.extend_from_slice(format!(
        "{xref_id} 0 obj\n<< /Type /XRef /Size {size} /W [1 4 2] /Root {root} 0 R{} /Filter /FlateDecode{parms} /Length {} >>\nstream\n",
        trailer_extra(info), comp.len()
    ).as_bytes());
    buf.extend_from_slice(&comp);
    buf.extend_from_slice(
        format!("\nendstream\nendobj\nstartxref\n{xref_off}\n%%EOF\n").as_bytes(),
    );
    buf
}

const SECRET: &str = "123-45-6789";

/// Letter page (612x792), Helvetica. Line 1 "Name: Jane Example" at baseline
/// 700; line 2 "SSN: 123-45-6789" at baseline 680. Region `ssn_line()`
/// covers line 2 only (top-origin coordinates).
fn page_content(extra: &str) -> Vec<u8> {
    format!("BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nBT /F1 12 Tf 72 680 Td (SSN: {SECRET}) Tj ET\n{extra}").into_bytes()
}

fn ssn_line() -> RedactionRegion {
    RedactionRegion {
        page: 0,
        x: 60.0,
        y: 96.0,
        width: 300.0,
        height: 20.0,
        color: None,
    }
}

fn base_objs(content: Vec<u8>, page_extra: &str) -> Vec<Obj> {
    vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(3, &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >>{page_extra} >>")),
        sobj(4, "", &content),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"),
    ]
}

fn simple_pdf(compress: bool) -> Vec<u8> {
    build(&base_objs(page_content(""), ""), 1, None, compress)
}

/// Shared assertions for "the SSN line is gone, the name line stays".
fn assert_ssn_gone_name_kept(out: &[u8]) {
    assert_single_self_contained_revision(out);
    let ev = Evidence::of(out);
    assert!(
        !ev.contains(SECRET),
        "redacted SSN is recoverable from the output: {}",
        ev.locate(SECRET)
    );
    assert!(
        has_text(out, "Jane Example"),
        "non-redacted text must remain"
    );
}

// ───────────────────────── tests ─────────────────────────

#[test]
fn region_redaction_of_simple_pdf_leaves_nothing_recoverable() {
    for compress in [false, true] {
        let input = simple_pdf(compress);
        assert!(has_text(&input, SECRET));
        let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
        assert_ssn_gone_name_kept(&out);
    }
}

#[test]
fn pattern_redaction_of_simple_pdf_leaves_nothing_recoverable() {
    let out = redact_text(&simple_pdf(true), &[ssn_regex()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
}

#[test]
fn output_does_not_contain_the_original_file() {
    let input = simple_pdf(true);
    let out = redact_text(&input, &[ssn_regex()]).unwrap();
    assert!(
        !out.starts_with(&input),
        "the original bytes must not be carried into the output"
    );
}

#[test]
fn earlier_revisions_are_not_carried_into_the_output() {
    // Revision 1 had a different secret on the page and in /Info; revision 2
    // replaced the page content and /Info. Only revision 2 is the document.
    let mut r1 = base_objs(b"BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nBT /F1 12 Tf 72 640 Td (OLD-SECRET-4471) Tj ET\n".to_vec(), "");
    r1.push(obj(
        6,
        "<< /Title (Draft OLD-SECRET-4471) /Author (Old Author) >>",
    ));
    let v1 = build(&r1, 1, Some(6), true);
    let v2 = append_revision(
        v1,
        &[
            sobj(4, "", &page_content("")),
            obj(6, "<< /Title (Payroll) >>"),
        ],
        1,
        Some(6),
    );
    assert!(
        has_text(&v2, SECRET),
        "the resolver must read the current revision"
    );

    let out = redact_text(&v2, &[ssn_regex()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
    let ev = Evidence::of(&out);
    assert!(
        !ev.contains("OLD-SECRET-4471"),
        "content from a previous revision survived"
    );
    assert!(
        !ev.contains("Old Author"),
        "metadata from a previous revision survived"
    );
}

#[test]
fn object_numbers_that_share_a_suffix_resolve_correctly() {
    // Object 15 (a decoy holding the secret, unreferenced) is written before
    // object 5 (the real content). A substring search for "5 0 obj" finds 15.
    let mut objs = vec![sobj(15, "", b"BT /F1 12 Tf 72 640 Td (DECOY-15) Tj ET\n")];
    objs.extend(vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R /Resources << /Font << /F1 7 0 R >> >> >>"),
        sobj(5, "", &page_content("")),
        obj(7, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"),
    ]);
    let input = build(&objs, 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
    assert!(
        !Evidence::of(&out).contains("DECOY-15"),
        "unreferenced objects must be dropped"
    );
}

#[test]
fn object_streams_and_xref_streams_are_handled() {
    for predictor in [false, true] {
        let input = build_objstm(&base_objs(page_content(""), ""), 1, None, predictor);
        assert!(
            has_text(&input, SECRET),
            "the resolver must read object streams"
        );
        let out = redact_text(&input, &[ssn_regex()]).expect("redaction succeeds");
        assert_ssn_gone_name_kept(&out);
    }
}

#[test]
fn text_positioned_by_transforms_and_implicit_advance_is_removed() {
    // (a) the whole text block moved by `cm`; (b) the secret follows another
    // Tj with no Td between them (position comes from the text advance);
    // (c) the font size comes from the text matrix (1 Tf, Tm scaled by 12).
    let content = format!(
        "q 1 0 0 1 72 680 cm BT /F1 12 Tf 0 0 Td (CM-{SECRET}) Tj ET Q\n\
         BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\n\
         BT /F1 12 Tf 300 680 Td (Label: ) Tj (ADV-{SECRET}) Tj ET\n\
         BT /F1 1 Tf 12 0 0 12 200 684 Tm (TM-{SECRET}) Tj ET\n"
    );
    let input = build(&base_objs(content.into_bytes(), ""), 1, None, true);
    // User space 676..697: all three secret lines; the name line starts at
    // 697.36 (baseline 700 minus the 0.22 em descent) and is not touched.
    let region = RedactionRegion {
        page: 0,
        x: 0.0,
        y: 95.0,
        width: 612.0,
        height: 21.0,
        color: None,
    };
    let out = redact_pdf(&input, &[region]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
}

#[test]
fn invisible_text_in_the_region_is_removed() {
    let content = format!("BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nBT 3 Tr /F1 12 Tf 72 680 Td (HIDDEN-{SECRET}) Tj ET\n");
    let input = build(&base_objs(content.into_bytes(), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
}

#[test]
fn alternate_text_in_marked_content_is_removed() {
    let content = format!("BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\n/Span << /ActualText (ALT-{SECRET}) >> BDC BT /F1 12 Tf 72 680 Td (xxx) Tj ET EMC\n");
    let input = build(&base_objs(content.into_bytes(), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
}

#[test]
fn metadata_info_and_xmp_are_scrubbed() {
    let mut objs = base_objs(page_content(""), " ");
    objs[0] = obj(1, "<< /Type /Catalog /Pages 2 0 R /Metadata 7 0 R >>");
    objs.push(obj(6, &format!("<< /Title (Payroll {SECRET}) /Author (HR) /Subject (SSN {SECRET}) /Keywords ({SECRET}) >>")));
    objs.push(sobj(
        7,
        "/Type /Metadata /Subtype /XML",
        format!("<x:xmpmeta xmlns:x='adobe:ns:meta/'><dc:title>{SECRET}</dc:title></x:xmpmeta>")
            .as_bytes(),
    ));
    for compress in [false, true] {
        let input = build(&objs, 1, Some(6), compress);
        for out in [
            redact_pdf(&input, &[ssn_line()]).unwrap(),
            redact_text(&input, &[ssn_regex()]).unwrap(),
        ] {
            assert_ssn_gone_name_kept(&out);
        }
    }
}

#[test]
fn annotations_over_or_matching_the_redaction_are_removed() {
    // Text annot in the region; FreeText annot whose appearance stream draws
    // the secret in the region; a Text annot elsewhere whose /Contents holds
    // the secret (pattern mode); and an unrelated annot that must stay.
    let mut objs = base_objs(page_content(""), " /Annots [8 0 R 9 0 R 11 0 R 12 0 R]");
    objs.push(obj(
        8,
        &format!(
            "<< /Type /Annot /Subtype /Text /Rect [80 682 100 702] /Contents (NOTE {SECRET}) >>"
        ),
    ));
    objs.push(obj(9, "<< /Type /Annot /Subtype /FreeText /Rect [200 676 400 694] /DA (/F1 12 Tf) /AP << /N 10 0 R >> >>"));
    objs.push(sobj(
        10,
        "/Type /XObject /Subtype /Form /BBox [0 0 200 18] /Resources << /Font << /F1 5 0 R >> >>",
        format!("BT /F1 12 Tf 2 4 Td (AP-{SECRET}) Tj ET").as_bytes(),
    ));
    objs.push(obj(11, &format!("<< /Type /Annot /Subtype /Text /Rect [500 100 520 120] /Contents (Elsewhere {SECRET}) >>")));
    objs.push(obj(
        12,
        "<< /Type /Annot /Subtype /Text /Rect [500 300 520 320] /Contents (KEEP-THIS-NOTE) >>",
    ));
    let input = build(&objs, 1, None, true);

    let out = redact_text(&input, &[ssn_regex()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
    assert!(
        Evidence::of(&out).contains("KEEP-THIS-NOTE"),
        "unrelated annotations must stay"
    );

    // Region mode removes what lies over the region (8, 9); the stray
    // /Contents of 11 is outside the region and not text on the page, so it
    // is only caught in pattern mode. Here we check the region part.
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    let ev = Evidence::of(&out);
    assert!(!ev.contains(&format!("NOTE {SECRET}")) && !ev.contains(&format!("AP-{SECRET}")));
    assert!(ev.contains("KEEP-THIS-NOTE"));
}

#[test]
fn embedded_files_and_attachments_are_removed() {
    let mut objs = base_objs(page_content(""), " /Annots [9 0 R]");
    objs[0] = obj(1, "<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles << /Names [(data.json) 6 0 R] >> >> >>");
    objs.push(obj(
        6,
        "<< /Type /Filespec /F (data.json) /EF << /F 7 0 R >> >>",
    ));
    objs.push(sobj(
        7,
        "/Type /EmbeddedFile",
        format!("{{\"ssn\":\"{SECRET}\"}}").as_bytes(),
    ));
    objs.push(obj(
        9,
        "<< /Type /Annot /Subtype /FileAttachment /Rect [500 100 520 120] /FS 10 0 R >>",
    ));
    objs.push(obj(
        10,
        "<< /Type /Filespec /F (a.txt) /EF << /F 11 0 R >> >>",
    ));
    objs.push(sobj(
        11,
        "/Type /EmbeddedFile",
        format!("attached {SECRET}").as_bytes(),
    ));
    let input = build(&objs, 1, None, true);
    for out in [
        redact_pdf(&input, &[ssn_line()]).unwrap(),
        redact_text(&input, &[ssn_regex()]).unwrap(),
    ] {
        assert_ssn_gone_name_kept(&out);
    }
}

#[test]
fn bookmarks_naming_the_secret_are_scrubbed() {
    let mut objs = base_objs(page_content(""), "");
    objs[0] = obj(1, "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>");
    objs.push(obj(
        6,
        "<< /Type /Outlines /First 7 0 R /Last 8 0 R /Count 2 >>",
    ));
    objs.push(obj(
        7,
        &format!("<< /Title (SSN {SECRET}) /Parent 6 0 R /Next 8 0 R /Dest [3 0 R /Fit] >>"),
    ));
    objs.push(obj(
        8,
        "<< /Title (Summary) /Parent 6 0 R /Prev 7 0 R /Dest [3 0 R /Fit] >>",
    ));
    let input = build(&objs, 1, None, true);
    let out = redact_text(&input, &[ssn_regex()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
    assert!(
        Evidence::of(&out).contains("Summary"),
        "unrelated bookmarks stay"
    );
}

#[test]
fn text_inside_a_form_xobject_under_the_region_is_refused_not_kept() {
    // This version does not rewrite shared form XObjects. It must refuse
    // rather than return a PDF that still carries the text.
    let mut objs = base_objs(
        "BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nq 1 0 0 1 72 676 cm /Fx Do Q\n"
            .to_string()
            .into_bytes(),
        "",
    );
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /Fx 6 0 R >> >> >>");
    objs.push(sobj(
        6,
        "/Type /XObject /Subtype /Form /BBox [0 0 300 20] /Resources << /Font << /F1 5 0 R >> >>",
        format!("BT /F1 12 Tf 0 4 Td (FX-{SECRET}) Tj ET").as_bytes(),
    ));
    let input = build(&objs, 1, None, true);
    match redact_pdf(&input, &[ssn_line()]) {
        Err(_) => {}
        Ok(out) => assert!(
            !Evidence::of(&out).contains(SECRET),
            "form XObject text survived redaction"
        ),
    }
}

#[test]
fn images_under_the_region_are_removed() {
    let mut objs = base_objs(
        "BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nq 200 0 0 20 72 676 cm /Im1 Do Q\n"
            .to_string()
            .into_bytes(),
        "",
    );
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> >>");
    // A 1x1 gray image whose bytes spell a marker we can search for.
    objs.push(sobj(6, "/Type /XObject /Subtype /Image /Width 16 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", b"IMG-123-45-6789!"));
    let input = build(&objs, 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    assert_single_self_contained_revision(&out);
    assert!(
        !Evidence::of(&out).contains("IMG-123-45-6789"),
        "image data under the region survived"
    );
    assert!(has_text(&out, "Jane Example"));
}

#[test]
fn forme_rendered_document_with_metadata_and_embedded_data() {
    let json = format!(
        r#"{{
        "metadata": {{ "title": "Payroll {SECRET}", "author": "HR Department", "subject": "SSN {SECRET}" }},
        "embeddedData": "{{\"employee\":\"Jane Example\",\"ssn\":\"{SECRET}\"}}",
        "tagged": true,
        "children": [
            {{ "kind": {{ "type": "Text", "content": "Name: Jane Example" }}, "style": {{ "fontSize": 12 }} }},
            {{ "kind": {{ "type": "Text", "content": "SSN: {SECRET}" }}, "style": {{ "fontSize": 12 }} }},
            {{ "kind": {{ "type": "Text", "content": "Department: Accounts" }}, "style": {{ "fontSize": 12 }} }}
        ]
    }}"#
    );
    let input = forme::render_json(&json).expect("render");
    assert!(
        Evidence::of(&input).contains(SECRET),
        "fixture sanity: the secret is in the input"
    );
    let out = redact_text(&input, &[ssn_regex()]).expect("redaction succeeds");
    assert_single_self_contained_revision(&out);
    let ev = Evidence::of(&out);
    assert!(
        !ev.contains(SECRET),
        "secret survives in a redacted Forme document: {}",
        ev.locate(SECRET)
    );
    assert!(has_text(&out, "Jane Example"));
    assert!(has_text(&out, "Accounts"));
}

#[test]
fn redacting_twice_stays_valid() {
    let once = redact_text(&simple_pdf(true), &[ssn_regex()]).unwrap();
    let twice = redact_text(&once, &[lit("Jane")]).expect("second redaction succeeds");
    assert_single_self_contained_revision(&twice);
    let ev = Evidence::of(&twice);
    assert!(!ev.contains(SECRET) && !ev.contains("Jane"));
}

#[test]
fn encrypted_input_is_refused() {
    let mut objs = base_objs(page_content(""), "");
    objs.push(obj(6, "<< /Filter /Standard /V 1 /R 2 /O (xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx) /U (xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx) /P -4 >>"));
    let mut input = build(&objs, 1, None, true);
    let needle = b"/Root 1 0 R";
    let at = (0..input.len() - needle.len())
        .rev()
        .find(|&i| &input[i..i + needle.len()] == needle)
        .unwrap();
    input.splice(at..at, b"/Encrypt 6 0 R ".iter().copied());
    assert!(
        redact_pdf(&input, &[ssn_line()]).is_err(),
        "encrypted PDFs must be refused, not half-redacted"
    );
}

#[test]
fn cid_font_text_decoded_through_bfrange_is_found_and_removed() {
    // Like Chrome output: a Type0 font, 2-byte glyph codes, ToUnicode with
    // several incrementing bfrange entries. The secret is not readable as
    // text in the file; only glyph codes are. Codes: S=0036 N=0031 :=001D
    // space=0003, digits 1-9 = 0014-001C, '-'=0010.
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
                4 beginbfchar\n<0003> <0020>\n<0010> <002D>\n<0031> <004E>\n<0036> <0053>\nendbfchar\n\
                1 beginbfrange\n<0014> <001D> <0031>\nendbfrange\nendcmap end end\n";
    let codes = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                'S' => "0036".to_string(),
                'N' => "0031".to_string(),
                ':' => "001D".to_string(),
                ' ' => "0003".to_string(),
                '-' => "0010".to_string(),
                d @ '1'..='9' => format!("{:04X}", 0x14 + (d as u32 - '1' as u32)),
                _ => "0003".to_string(),
            })
            .collect()
    };
    let secret_codes = codes(SECRET);
    let content = format!(
        "BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nBT /F2 12 Tf 72 680 Td <{}> Tj ET\n",
        codes(&format!("SSN: {SECRET}"))
    );
    let mut objs = base_objs(content.into_bytes(), "");
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>");
    objs.push(obj(6, "<< /Type /Font /Subtype /Type0 /BaseFont /ABCDEF+Arial /Encoding /Identity-H /DescendantFonts [7 0 R] /ToUnicode 8 0 R >>"));
    objs.push(obj(7, "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /ABCDEF+Arial /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /DW 556 >>"));
    objs.push(sobj(8, "", cmap.as_bytes()));
    let input = build(&objs, 1, None, true);
    assert!(
        has_text(&input, SECRET),
        "the CID text must decode through bfrange"
    );
    let out = redact_text(&input, &[ssn_regex()]).expect("redaction succeeds");
    assert_single_self_contained_revision(&out);
    assert!(!has_text(&out, SECRET));
    let ev = Evidence::of(&out);
    assert!(
        !ev.contains(&secret_codes),
        "the secret's glyph codes are still in the file: {}",
        ev.locate(&secret_codes)
    );
    assert!(has_text(&out, "Jane Example"));
}

#[test]
fn region_mode_removes_hidden_copies_of_the_removed_text() {
    // The user boxes the SSN line. A link elsewhere on the page carries the
    // same number in its URL, and a bookmark names it. Neither is visible
    // text, so the user cannot box them; they must go with the redaction.
    let mut objs = base_objs(page_content(""), " /Annots [8 0 R 9 0 R]");
    objs[0] = obj(1, "<< /Type /Catalog /Pages 2 0 R /Outlines 10 0 R >>");
    objs.push(obj(8, &format!("<< /Type /Annot /Subtype /Link /Rect [400 400 450 420] /A << /S /URI /URI (https://example.com/ssn/{SECRET}) >> >>")));
    objs.push(obj(9, "<< /Type /Annot /Subtype /Link /Rect [400 300 450 320] /A << /S /URI /URI (https://example.com/help) >> >>"));
    objs.push(obj(
        10,
        "<< /Type /Outlines /First 11 0 R /Last 11 0 R /Count 1 >>",
    ));
    objs.push(obj(
        11,
        &format!("<< /Title (Record {SECRET}) /Parent 10 0 R /Dest [3 0 R /Fit] >>"),
    ));
    let input = build(&objs, 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).expect("redaction succeeds");
    assert_ssn_gone_name_kept(&out);
    assert!(
        Evidence::of(&out).contains("https://example.com/help"),
        "unrelated links stay"
    );
}
