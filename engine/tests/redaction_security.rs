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

// ═════════════════════════ page geometry and vector graphics ═════════════════════════
//
// Region coordinates are in the page as displayed: points from the top-left
// corner of the visible area (CropBox ∩ MediaBox), after /Rotate. That is
// what a viewer (and the Dashboard editor, which draws on a pdf.js
// viewport) shows. The expected regions below are worked out by hand.

/// The SSN line of `page_content` in page space (user space): x 60..360,
/// y 676..696.
const SSN_USER: (f64, f64, f64, f64) = (60.0, 676.0, 360.0, 696.0);

fn region(x: f64, y: f64, width: f64, height: f64) -> RedactionRegion {
    RedactionRegion {
        page: 0,
        x,
        y,
        width,
        height,
        color: None,
    }
}

/// Independent content-stream geometry reader (not the engine's parser).
/// Returns every painted or clipping path as control polygons in page space,
/// with the painting operator.
struct PathRec {
    op: String,
    clip: bool,
    polys: Vec<Vec<(f64, f64)>>,
}

fn content_tokens(s: &[u8]) -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'%' {
            while i < s.len() && s[i] != b'\n' && s[i] != b'\r' {
                i += 1;
            }
        } else if c == b'(' {
            let mut depth = 0;
            while i < s.len() {
                match s[i] {
                    b'\\' => i += 1,
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            out.push("(str)".into());
        } else if c == b'<' && s.get(i + 1) != Some(&b'<') {
            while i < s.len() && s[i] != b'>' {
                i += 1;
            }
            i += 1;
            out.push("(str)".into());
        } else if b"[]{}".contains(&c) || (c == b'<' || c == b'>') {
            let n = if (c == b'<' || c == b'>') && s.get(i + 1) == Some(&c) {
                2
            } else {
                1
            };
            out.push(String::from_utf8_lossy(&s[i..i + n]).into());
            i += n;
        } else {
            let st = i;
            i += 1;
            while i < s.len() && !s[i].is_ascii_whitespace() && !b"()<>[]{}/%".contains(&s[i]) {
                i += 1;
            }
            let t = String::from_utf8_lossy(&s[st..i]).to_string();
            if t == "ID" {
                // Inline image data: skip to EI.
                while i + 2 < s.len() && !(s[i].is_ascii_whitespace() && &s[i + 1..i + 3] == b"EI")
                {
                    i += 1;
                }
                i += 3;
                out.push("EI".into());
                continue;
            }
            out.push(t);
        }
    }
    out
}

fn painted_paths(pdf: &[u8]) -> Vec<PathRec> {
    let mut out = vec![];
    for body in stream_bodies(pdf) {
        let data = decompress_to_vec_zlib(&body).unwrap_or(body);
        let toks = content_tokens(&data);
        let mut nums: Vec<f64> = vec![];
        let mut ctm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0f64];
        let mut stack = vec![];
        let mut polys: Vec<Vec<(f64, f64)>> = vec![];
        let mut cur: Vec<(f64, f64)> = vec![];
        let mut clip = false;
        let tf =
            |m: &[f64; 6], x: f64, y: f64| (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5]);
        for t in toks {
            if let Ok(v) = t.parse::<f64>() {
                nums.push(v);
                continue;
            }
            let n = nums.len();
            let a = |k: usize| nums[n - k];
            match t.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().unwrap_or(ctm),
                "cm" if n >= 6 => {
                    let m = [a(6), a(5), a(4), a(3), a(2), a(1)];
                    ctm = [
                        m[0] * ctm[0] + m[1] * ctm[2],
                        m[0] * ctm[1] + m[1] * ctm[3],
                        m[2] * ctm[0] + m[3] * ctm[2],
                        m[2] * ctm[1] + m[3] * ctm[3],
                        m[4] * ctm[0] + m[5] * ctm[2] + ctm[4],
                        m[4] * ctm[1] + m[5] * ctm[3] + ctm[5],
                    ];
                }
                "m" if n >= 2 => {
                    if cur.len() > 1 {
                        polys.push(std::mem::take(&mut cur));
                    }
                    cur = vec![tf(&ctm, a(2), a(1))];
                }
                "l" if n >= 2 => cur.push(tf(&ctm, a(2), a(1))),
                "c" if n >= 6 => {
                    cur.push(tf(&ctm, a(6), a(5)));
                    cur.push(tf(&ctm, a(4), a(3)));
                    cur.push(tf(&ctm, a(2), a(1)));
                }
                "v" | "y" if n >= 4 => {
                    cur.push(tf(&ctm, a(4), a(3)));
                    cur.push(tf(&ctm, a(2), a(1)));
                }
                "h" => {
                    if let Some(&f) = cur.first() {
                        cur.push(f);
                    }
                }
                "re" if n >= 4 => {
                    if cur.len() > 1 {
                        polys.push(std::mem::take(&mut cur));
                    }
                    let (x, y, w, h) = (a(4), a(3), a(2), a(1));
                    polys.push(vec![
                        tf(&ctm, x, y),
                        tf(&ctm, x + w, y),
                        tf(&ctm, x + w, y + h),
                        tf(&ctm, x, y + h),
                        tf(&ctm, x, y),
                    ]);
                    cur.clear();
                }
                "W" | "W*" => clip = true,
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                    if cur.len() > 1 {
                        polys.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    if t != "n" || clip {
                        out.push(PathRec {
                            op: t.clone(),
                            clip,
                            polys: std::mem::take(&mut polys),
                        });
                    }
                    polys.clear();
                    clip = false;
                }
                _ => {}
            }
            nums.clear();
        }
    }
    out
}

/// Does the segment p→q pass through the interior of `r` (shrunk by `e`)?
fn segment_enters(p: (f64, f64), q: (f64, f64), r: (f64, f64, f64, f64), e: f64) -> bool {
    let (x0, y0, x1, y1) = (r.0 + e, r.1 + e, r.2 - e, r.3 - e);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    for (pp, qq) in [
        (-dx, p.0 - x0),
        (dx, x1 - p.0),
        (-dy, p.1 - y0),
        (dy, y1 - p.1),
    ] {
        if pp.abs() < 1e-12 {
            if qq < 0.0 {
                return false;
            }
        } else {
            let t = qq / pp;
            if pp < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    t0 < t1
}

/// No path geometry (control polygons, page space) inside `r`, except the
/// opaque redaction box itself, whose edges lie on the boundary.
fn assert_no_geometry_inside(pdf: &[u8], r: (f64, f64, f64, f64)) {
    for p in painted_paths(pdf) {
        for poly in &p.polys {
            for w in poly.windows(2) {
                assert!(
                    !segment_enters(w[0], w[1], r, 0.05),
                    "path geometry inside the redaction ({} {}): {:?} -> {:?}",
                    p.op,
                    if p.clip { "clip" } else { "" },
                    w[0],
                    w[1]
                );
            }
        }
    }
}

fn has_geometry_near(pdf: &[u8], x: f64, y: f64, tol: f64) -> bool {
    painted_paths(pdf).iter().any(|p| {
        p.polys.iter().any(|poly| {
            poly.windows(2)
                .any(|w| segment_enters(w[0], w[1], (x - tol, y - tol, x + tol, y + tol), 0.0))
        })
    })
}

/// The opaque box is drawn at `r` in page space.
fn assert_box_at(pdf: &[u8], r: (f64, f64, f64, f64)) {
    let found = painted_paths(pdf).iter().any(|p| {
        p.op == "f"
            && p.polys.iter().any(|poly| {
                let xs = poly.iter().map(|q| q.0);
                let ys = poly.iter().map(|q| q.1);
                let (a, b) = (
                    xs.clone().fold(f64::MAX, f64::min),
                    xs.fold(f64::MIN, f64::max),
                );
                let (c, d) = (
                    ys.clone().fold(f64::MAX, f64::min),
                    ys.fold(f64::MIN, f64::max),
                );
                (a - r.0).abs() < 0.01
                    && (c - r.1).abs() < 0.01
                    && (b - r.2).abs() < 0.01
                    && (d - r.3).abs() < 0.01
            })
    });
    assert!(found, "no redaction box at {r:?}");
}

fn page_with(page_extra: &str, pages_extra: &str) -> Vec<u8> {
    let mut objs = base_objs(page_content(""), page_extra);
    objs[1] = obj(
        2,
        &format!("<< /Type /Pages /Kids [3 0 R] /Count 1{pages_extra} >>"),
    );
    build(&objs, 1, None, true)
}

fn assert_ssn_line_redacted(out: &[u8]) {
    assert_ssn_gone_name_kept(out);
    assert_box_at(out, SSN_USER);
}

#[test]
fn rotated_pages_use_displayed_coordinates() {
    // MediaBox 612x792. Displayed size is 792x612 at 90/270.
    for (rot, r) in [
        ("0", region(60.0, 96.0, 300.0, 20.0)),
        ("90", region(676.0, 60.0, 20.0, 300.0)),
        ("180", region(252.0, 676.0, 300.0, 20.0)),
        ("270", region(96.0, 252.0, 20.0, 300.0)),
        ("-90", region(96.0, 252.0, 20.0, 300.0)),
        ("450", region(676.0, 60.0, 20.0, 300.0)),
    ] {
        let input = page_with(&format!(" /Rotate {rot}"), "");
        let out = redact_pdf(&input, &[r]).unwrap_or_else(|e| panic!("rotate {rot}: {e}"));
        assert_ssn_line_redacted(&out);
    }
}

#[test]
fn crop_box_and_non_zero_origins_use_the_visible_area() {
    // CropBox (50,100)-(562,742): the visible page starts at x=50, top 742.
    let out = redact_pdf(
        &page_with(" /CropBox [50 100 562 742]", ""),
        &[region(10.0, 46.0, 300.0, 20.0)],
    )
    .unwrap();
    assert_ssn_line_redacted(&out);
    // MediaBox with a negative origin.
    let mut objs = base_objs(page_content(""), "");
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [-100 -50 512 742] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>");
    let out = redact_pdf(
        &build(&objs, 1, None, true),
        &[region(160.0, 46.0, 300.0, 20.0)],
    )
    .unwrap();
    assert_ssn_line_redacted(&out);
    // A CropBox larger than the MediaBox is clipped to it.
    let out = redact_pdf(
        &page_with(" /CropBox [-500 -500 2000 2000]", ""),
        &[region(60.0, 96.0, 300.0, 20.0)],
    )
    .unwrap();
    assert_ssn_line_redacted(&out);
}

#[test]
fn inherited_rotation_and_crop_box_combine() {
    // Rotate 90 and CropBox set on the page-tree node, inherited by the page.
    let input = page_with("", " /Rotate 90 /CropBox [50 100 562 742]");
    let out = redact_pdf(&input, &[region(576.0, 10.0, 20.0, 300.0)]).unwrap();
    assert_ssn_line_redacted(&out);
    // Same with 270 on the page, overriding the inherited 90.
    let input = page_with(" /Rotate 270", " /Rotate 90 /CropBox [50 100 562 742]");
    // 270: u = V.y1 - y = 742-696..742-676 = 46..66; v = V.x1 - x = 562-360..562-60 = 202..502
    let out = redact_pdf(&input, &[region(46.0, 202.0, 20.0, 300.0)]).unwrap();
    assert_ssn_line_redacted(&out);
}

#[test]
fn found_regions_are_in_displayed_coordinates_and_round_trip() {
    let input = page_with("", " /Rotate 90 /CropBox [50 100 562 742]");
    let found = find_text_regions(&input, &[lit(SECRET)]).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    let f = &found[0];
    // The SSN glyphs sit at user x≈98..170, y≈677..692 → displayed
    // x = y-100 ≈ 577..592, y = x-50 ≈ 48..120.
    assert!(f.x > 570.0 && f.x < 580.0 && f.width < 25.0, "{f:?}");
    assert!(f.y > 40.0 && f.y < 60.0 && f.height > 60.0, "{f:?}");
    let out = redact_pdf(&input, &found).unwrap();
    assert!(!Evidence::of(&out).contains(SECRET));
    assert!(has_text(&out, "Jane Example"));
}

#[test]
fn unsupported_page_geometry_is_refused() {
    for (page_extra, why) in [
        (" /Rotate 45", "rotation that is not a multiple of 90"),
        (" /UserUnit 2", "UserUnit"),
    ] {
        let err = redact_pdf(&page_with(page_extra, ""), &[ssn_line()])
            .err()
            .unwrap_or_else(|| panic!("{why}: must be refused"));
        assert!(
            err.to_string().contains("Redaction refused"),
            "{why}: {err}"
        );
    }
    let mut bad = ssn_line();
    bad.width = f64::NAN;
    assert!(
        redact_pdf(&simple_pdf(true), &[bad]).is_err(),
        "NaN region must be refused"
    );
    let mut bad = ssn_line();
    bad.x = f64::INFINITY;
    assert!(
        redact_pdf(&simple_pdf(true), &[bad]).is_err(),
        "infinite region must be refused"
    );
}

/// Text outlined as paths (no text operators): the letters "SSN" drawn with
/// curves inside the SSN line, plus a logo path elsewhere.
const OUTLINED: &str = "0 0 0 rg\n\
    72 680 m 80 690 l 74 694 80 696 84 692 c 76 684 l 84 678 78 676 72 680 c f\n\
    90 680 m 98 690 l 92 694 98 696 102 692 c 94 684 l 102 678 96 676 90 680 c f\n\
    108 678 m 108 694 l 120 680 l 120 694 l 116 694 l 116 686 l 110 694 l h f\n\
    0 0 1 rg 400 500 m 450 500 l 450 540 420 560 400 540 c h f\n";

#[test]
fn outlined_text_paths_under_the_region_are_removed() {
    let input = build(&base_objs(page_content(OUTLINED), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert_ssn_line_redacted(&out);
    assert_no_geometry_inside(&out, SSN_USER);
    // The logo outside the region is untouched.
    assert!(
        has_geometry_near(&out, 450.0, 520.0, 1.0),
        "logo path must remain"
    );
    assert!(
        has_geometry_near(&out, 420.0, 500.0, 1.0),
        "logo path must remain"
    );
}

#[test]
fn lines_and_rectangles_crossing_the_region_keep_their_outside_parts() {
    // A full-width table rule through the SSN line, a background band behind
    // it, and a bar of a chart that reaches into the region from below.
    let extra =
        "0.9 g 40 670 540 30 re f\n0 G 1 w 40 686 m 580 686 l S\n0.5 g 200 600 30 85 re f\n";
    let input = build(&base_objs(page_content(extra), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert_ssn_line_redacted(&out);
    assert_no_geometry_inside(&out, SSN_USER);
    // Outside parts survive: the rule on both sides, the band edges, the bar.
    assert!(has_geometry_near(&out, 50.0, 686.0, 0.5), "rule, left part");
    assert!(
        has_geometry_near(&out, 500.0, 686.0, 0.5),
        "rule, right part"
    );
    assert!(
        has_geometry_near(&out, 300.0, 670.0, 0.5),
        "band, bottom edge"
    );
    assert!(has_geometry_near(&out, 300.0, 700.0, 0.5), "band, top edge");
    assert!(has_geometry_near(&out, 215.0, 600.0, 0.5), "bar, base");
    // The bar's height inside the region (its top at 685) is gone.
    assert!(
        !has_geometry_near(&out, 215.0, 685.0, 0.5),
        "bar top inside the region"
    );
}

#[test]
fn stroked_curves_like_a_signature_are_cut_at_the_region() {
    // A signature stroke that starts left of the region, loops inside it,
    // and leaves on the right.
    let extra = "0 0 1 RG 2 w 30 686 m 80 700 120 670 160 690 c 200 705 260 672 300 688 c 340 700 380 690 420 686 c S\n";
    let input = build(&base_objs(page_content(extra), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert_ssn_line_redacted(&out);
    assert_no_geometry_inside(&out, SSN_USER);
    assert!(
        has_geometry_near(&out, 40.0, 689.0, 3.0),
        "stroke start outside the region"
    );
    assert!(
        has_geometry_near(&out, 400.0, 688.0, 3.0),
        "stroke end outside the region"
    );
}

#[test]
fn clipping_paths_do_not_keep_the_shape_of_redacted_content() {
    // Text-shaped clip (outlined "S") inside the region, then a fill through
    // it; and a rounded clip around a card that crosses the region.
    let extra = "q 72 680 m 80 690 l 74 694 80 696 84 692 c 76 684 l 84 678 78 676 72 680 c W n 0 0 1 rg 60 670 40 30 re f Q\n\
                 q 30 650 m 560 650 l 580 650 590 660 590 680 c 590 720 l 30 720 l h W n 0.95 g 0 0 612 792 re f Q\n";
    let input = build(&base_objs(page_content(extra), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert_ssn_line_redacted(&out);
    assert_no_geometry_inside(&out, SSN_USER);
    // The rounded corner of the card (outside the region) is kept.
    assert!(
        has_geometry_near(&out, 586.0, 664.0, 3.0),
        "card corner clip must remain"
    );
}

#[test]
fn vector_graphics_under_nested_transformations_are_removed() {
    // Outlined glyph drawn in a scaled, rotated local space inside nested q/cm.
    // Local square (0,0)-(20,10) under: translate(100,680) · rotate 90 ·
    // scale 0.5 → page x 95..100, y 680..690.
    let extra = "q 1 0 0 1 100 680 cm q 0 1 -1 0 0 0 cm q 0.5 0 0 0.5 0 0 cm \
                 0 0 m 20 0 l 20 10 10 12 0 10 c h f Q Q Q\n";
    let input = build(&base_objs(page_content(extra), ""), 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert_ssn_line_redacted(&out);
    assert_no_geometry_inside(&out, SSN_USER);
}

#[test]
fn vector_graphics_inside_a_form_xobject_under_the_region_are_refused() {
    let with_forms = |fx_dict: &str, fx: &[u8]| {
        let mut objs = base_objs(
            "BT /F1 12 Tf 72 700 Td (Name: Jane Example) Tj ET\nq 1 0 0 1 72 676 cm /Fx Do Q\nq 1 0 0 1 72 400 cm /Fy Do Q\n"
                .to_string()
                .into_bytes(),
            "",
        );
        objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /Fx 6 0 R /Fy 7 0 R >> >> >>");
        objs.push(sobj(6, fx_dict, fx));
        // Fy: an outlined shape far from the region.
        objs.push(sobj(
            7,
            "/Type /XObject /Subtype /Form /BBox [0 0 300 20]",
            b"0 0 m 10 18 l 20 0 l h f",
        ));
        build(&objs, 1, None, true)
    };
    // Fx: an outlined shape under the region.
    let input = with_forms(
        "/Type /XObject /Subtype /Form /BBox [0 0 300 20]",
        b"0 0 m 10 18 l 20 0 l h f",
    );
    let err = redact_pdf(&input, &[ssn_line()]).expect_err("must refuse");
    assert!(err.to_string().contains("Redaction refused"), "{err}");
    // A form whose bounding box touches the region but whose drawing does
    // not is kept, not refused.
    let input = with_forms(
        "/Type /XObject /Subtype /Form /BBox [0 -300 300 20]",
        b"0 -200 m 10 -182 l 20 -200 l h f",
    );
    let out = redact_pdf(&input, &[ssn_line()]).expect("form outside the region is kept");
    assert_single_self_contained_revision(&out);
}

#[test]
fn tiling_patterns_painted_under_the_region_are_refused() {
    // A tiling pattern's cell is its own content stream (here: text). It is
    // not rewritten, so painting it under the region must be refused.
    let mut objs = base_objs(page_content("/Pattern cs /P1 scn 60 670 300 30 re f\n"), "");
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /Pattern << /P1 6 0 R >> >> >>");
    objs.push(sobj(
        6,
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 100 20] /XStep 100 /YStep 20 /Resources << /Font << /F1 5 0 R >> >>",
        format!("BT /F1 8 Tf 0 5 Td (P-{SECRET}) Tj ET").as_bytes(),
    ));
    let input = build(&objs, 1, None, true);
    match redact_pdf(&input, &[ssn_line()]) {
        Err(e) => assert!(e.to_string().contains("Redaction refused"), "{e}"),
        Ok(out) => panic!(
            "tiling pattern under the region must be refused; output still has it: {}",
            Evidence::of(&out).contains(SECRET)
        ),
    }
}

#[test]
fn tightly_spaced_lines_next_to_the_redaction_are_kept() {
    // One run per line, 12 pt type on 12 pt leading (as macOS Quartz writes
    // plain text). Redacting the SSN line must not take its neighbours.
    let content = format!(
        "BT /F1 12 Tf 72 712 Td (Name: Jane Example) Tj ET\n\
         BT /F1 12 Tf 72 700 Td (SSN: {SECRET}) Tj ET\n\
         BT /F1 12 Tf 72 688 Td (Department: Accounts) Tj ET\n"
    );
    let input = build(&base_objs(content.into_bytes(), ""), 1, None, true);
    for out in [
        redact_text(&input, &[ssn_regex()]).unwrap(),
        // A box drawn around the line, touching its neighbours' outer bounds.
        redact_pdf(&input, &[region(60.0, 80.0, 300.0, 14.5)]).unwrap(),
    ] {
        assert!(!Evidence::of(&out).contains(SECRET));
        assert!(has_text(&out, "Jane Example"), "line above must stay");
        assert!(
            has_text(&out, "Department: Accounts"),
            "line below must stay"
        );
    }
}

#[test]
fn zero_width_glyphs_inside_the_region_are_removed() {
    // A font whose widths are all zero: every glyph box is a vertical line.
    let content = format!("BT /F2 12 Tf 72 680 Td (ZW-{SECRET}) Tj ET\n");
    let mut objs = base_objs(page_content(&content), "");
    objs[2] = obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>");
    objs.push(obj(6, "<< /Type /Font /Subtype /Type1 /BaseFont /Zero /FirstChar 32 /LastChar 126 /Widths [95*0] >>".replace("95*0", &vec!["0"; 95].join(" ")).as_str()));
    let input = build(&objs, 1, None, true);
    let out = redact_pdf(&input, &[ssn_line()]).unwrap();
    assert!(
        !Evidence::of(&out).contains(SECRET),
        "{}",
        Evidence::of(&out).locate(SECRET)
    );
}
