//! # Redaction engine
//!
//! Produces a new, self-contained PDF in which redacted content is gone, not
//! hidden:
//!
//! - The input is read with [`pdfobj::Document`], so every object is at its
//!   latest definition (incremental updates, object streams, xref streams).
//! - Page content is interpreted with real geometry (CTM, text matrix, font
//!   size, spacing, scaling, rise, per-glyph advances). A text-showing
//!   operator whose extent touches a redaction region is removed; its advance
//!   is kept so the rest of the line does not move. Images and inline images
//!   touching a region are removed. Alternate text (`/ActualText`, `/Alt`,
//!   `/E`) on affected marked content and structure elements is removed.
//! - Annotations over a region, or whose text matches, are removed; file
//!   attachments, embedded files, XMP metadata, document info, page
//!   thumbnails and private application data are removed.
//! - The output is written with [`pdfobj::write_reachable`]: one revision,
//!   only reachable objects. Nothing from the input file is carried over
//!   byte-for-byte except objects that are still part of the document.
//!
//! Where this version cannot remove content safely it returns an error
//! instead of a PDF: text or images inside a form XObject under a region,
//! form fields under a region or holding matching text, encrypted input, and
//! content streams with filters it cannot decode.

use std::collections::{HashMap, HashSet};

use super::pdfobj::{
    decode_stream, dict_get, dict_remove, dict_set, flate, write_reachable, Dict, Document, Obj,
    Parser,
};
use crate::error::FormeError;
use crate::font::metrics::winansi_to_char;
use crate::font::StandardFont;
use crate::model::{PatternType, RedactionPattern, RedactionRegion};

fn refuse(msg: impl Into<String>) -> FormeError {
    FormeError::RenderError(format!("Redaction refused: {}", msg.into()))
}

// ─────────────────────────────── geometry ───────────────────────────────

type M = [f64; 6];
const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mul(a: &M, b: &M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn apply(m: &M, x: f64, y: f64) -> (f64, f64) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn of_points(pts: &[(f64, f64)]) -> Rect {
        let mut r = Rect {
            x0: f64::MAX,
            y0: f64::MAX,
            x1: f64::MIN,
            y1: f64::MIN,
        };
        for &(x, y) in pts {
            r.x0 = r.x0.min(x);
            r.y0 = r.y0.min(y);
            r.x1 = r.x1.max(x);
            r.y1 = r.y1.max(y);
        }
        r
    }
    fn transformed(m: &M, x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::of_points(&[
            apply(m, x0, y0),
            apply(m, x1, y0),
            apply(m, x0, y1),
            apply(m, x1, y1),
        ])
    }
    fn intersects(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && self.x1 > o.x0 && self.y0 < o.y1 && self.y1 > o.y0
    }
    fn union(&self, o: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

fn any_hit(r: &Rect, regions: &[Rect]) -> bool {
    regions.iter().any(|g| r.intersects(g))
}

/// Is a glyph under a region? Its box is an upper bound of the glyph (full
/// advance, ascent and descent), so boxes of neighbouring lines overlap. A
/// glyph counts when at least a fifth of its box, in each direction, lies
/// under the region, or when the centre of its box does; a glyph 80% outside
/// a region along an axis, with its centre outside, is not covered by it. A
/// degenerate box (zero width or height) counts when it lies inside the
/// region.
fn glyph_hit(g: &Rect, regions: &[Rect]) -> bool {
    const SHARE: f64 = 0.2;
    let axis = |a0: f64, a1: f64, r0: f64, r1: f64| {
        let len = a1 - a0;
        if len < 1e-6 {
            return a0 > r0 && a0 < r1;
        }
        let o = a1.min(r1) - a0.max(r0);
        o > 0.0 && o >= SHARE * len
    };
    let (cx, cy) = ((g.x0 + g.x1) / 2.0, (g.y0 + g.y1) / 2.0);
    regions.iter().any(|r| {
        (axis(g.x0, g.x1, r.x0, r.x1) && axis(g.y0, g.y1, r.y0, r.y1))
            // A box over the middle of a glyph covers it, however small.
            || (cx > r.x0 && cx < r.x1 && cy > r.y0 && cy < r.y1)
    })
}

// ───────────────────────────────── fonts ─────────────────────────────────

#[derive(Default, Clone)]
struct Font {
    /// Type0: 2-byte codes.
    two_byte: bool,
    to_unicode: HashMap<u32, String>,
    /// code → width in 1/1000 em, when known.
    widths: HashMap<u32, f64>,
    default_width: Option<f64>,
    /// All widths come from the font program's metrics (not estimated).
    exact: bool,
    /// Type3: glyph bounds per unit of font size (FontBBox through
    /// FontMatrix), or `Some(None)` when they are unknown.
    type3: Option<Option<Rect>>,
}

const STANDARD: [StandardFont; 14] = [
    StandardFont::Helvetica,
    StandardFont::HelveticaBold,
    StandardFont::HelveticaOblique,
    StandardFont::HelveticaBoldOblique,
    StandardFont::TimesRoman,
    StandardFont::TimesBold,
    StandardFont::TimesItalic,
    StandardFont::TimesBoldItalic,
    StandardFont::Courier,
    StandardFont::CourierBold,
    StandardFont::CourierOblique,
    StandardFont::CourierBoldOblique,
    StandardFont::Symbol,
    StandardFont::ZapfDingbats,
];

fn standard_font(base: &[u8]) -> Option<StandardFont> {
    let name = String::from_utf8_lossy(base);
    let name = name.split('+').next_back().unwrap_or(&name).to_string();
    let alias = match name.as_str() {
        "Arial" | "ArialMT" => "Helvetica",
        "Arial,Bold" | "Arial-BoldMT" => "Helvetica-Bold",
        "TimesNewRoman" | "TimesNewRomanPSMT" => "Times-Roman",
        "CourierNew" | "CourierNewPSMT" => "Courier",
        other => other,
    };
    STANDARD.iter().copied().find(|f| f.pdf_name() == alias)
}

fn load_font(doc: &Document, font: &Obj) -> Font {
    let Some(d) = doc.resolve_dict(font) else {
        return Font::default();
    };
    let subtype = dict_get(d, b"Subtype")
        .and_then(|o| o.as_name())
        .unwrap_or(b"");
    let mut f = Font::default();
    if let Some(tu) = dict_get(d, b"ToUnicode") {
        if let Obj::Stream(sd, raw) = doc.resolve(tu) {
            if let Ok(bytes) = decode_stream(sd, raw) {
                for (k, v) in
                    super::redaction::parse_tounicode_cmap(&String::from_utf8_lossy(&bytes))
                {
                    f.to_unicode.insert(k as u32, v);
                }
            }
        }
    }
    if subtype == b"Type0" {
        f.two_byte = true;
        if let Some(Obj::Array(desc)) = dict_get(d, b"DescendantFonts").map(|o| doc.resolve(o)) {
            if let Some(dd) = desc.first().and_then(|o| doc.resolve_dict(o)) {
                f.default_width = Some(
                    dict_get(dd, b"DW")
                        .and_then(|o| doc.resolve(o).as_f64())
                        .unwrap_or(1000.0),
                );
                if let Obj::Array(w) = dict_get(dd, b"W")
                    .map(|o| doc.resolve(o))
                    .unwrap_or(&Obj::Null)
                {
                    let mut i = 0;
                    while i < w.len() {
                        let first = doc.resolve(&w[i]).as_f64().unwrap_or(0.0) as u32;
                        match w.get(i + 1).map(|o| doc.resolve(o)) {
                            Some(Obj::Array(list)) => {
                                for (k, v) in list.iter().enumerate() {
                                    if let Some(x) = doc.resolve(v).as_f64() {
                                        f.widths.insert(first + k as u32, x);
                                    }
                                }
                                i += 2;
                            }
                            Some(o) => {
                                let last = o.as_f64().unwrap_or(0.0) as u32;
                                let x = w
                                    .get(i + 2)
                                    .and_then(|o| doc.resolve(o).as_f64())
                                    .unwrap_or(0.0);
                                for c in first..=last.min(first + 65535) {
                                    f.widths.insert(c, x);
                                }
                                i += 3;
                            }
                            None => break,
                        }
                    }
                }
                f.exact = true;
            }
        }
        return f;
    }
    if subtype == b"Type3" {
        // Widths are in glyph space; FontMatrix maps glyph space to text
        // space (1/1000 em in other fonts).
        let num_arr = |key: &[u8], n: usize| -> Option<Vec<f64>> {
            match dict_get(d, key).map(|o| doc.resolve(o)) {
                Some(Obj::Array(a)) if a.len() == n => {
                    let v: Vec<f64> = a.iter().filter_map(|o| doc.resolve(o).as_f64()).collect();
                    (v.len() == n && v.iter().all(|x| x.is_finite())).then_some(v)
                }
                _ => None,
            }
        };
        let fm = num_arr(b"FontMatrix", 6).unwrap_or(vec![0.001, 0.0, 0.0, 0.001, 0.0, 0.0]);
        let m: M = [fm[0], fm[1], fm[2], fm[3], fm[4], fm[5]];
        let axis_aligned = m[1] == 0.0 && m[2] == 0.0 && m[0] > 0.0;
        if let (Some(fc), Some(Obj::Array(ws))) = (
            dict_get(d, b"FirstChar").and_then(|o| doc.resolve(o).as_f64()),
            dict_get(d, b"Widths").map(|o| doc.resolve(o)),
        ) {
            for (k, v) in ws.iter().enumerate() {
                if let Some(x) = doc.resolve(v).as_f64() {
                    f.widths.insert(fc as u32 + k as u32, x * m[0] * 1000.0);
                }
            }
        }
        f.exact = axis_aligned;
        let bbox = num_arr(b"FontBBox", 4)
            .filter(|b| axis_aligned && b[2] > b[0] && b[3] > b[1])
            .map(|b| Rect::transformed(&m, b[0], b[1], b[2], b[3]));
        f.type3 = Some(bbox);
        return f;
    }
    // Simple font.
    if let (Some(fc), Some(Obj::Array(ws))) = (
        dict_get(d, b"FirstChar").and_then(|o| doc.resolve(o).as_f64()),
        dict_get(d, b"Widths").map(|o| doc.resolve(o)),
    ) {
        for (k, v) in ws.iter().enumerate() {
            if let Some(x) = doc.resolve(v).as_f64() {
                f.widths.insert(fc as u32 + k as u32, x);
            }
        }
        f.exact = true;
        return f;
    }
    if let Some(std) = dict_get(d, b"BaseFont")
        .and_then(|o| o.as_name())
        .and_then(standard_font)
    {
        let m = std.metrics();
        for code in 32u32..=255 {
            f.widths.insert(code, m.widths[(code - 32) as usize] as f64);
        }
        f.default_width = Some(m.default_width as f64);
        f.exact = true;
    }
    f
}

impl Font {
    fn codes(&self, s: &[u8]) -> Vec<u32> {
        if self.two_byte {
            s.chunks(2)
                .map(|c| {
                    if c.len() == 2 {
                        u16::from_be_bytes([c[0], c[1]]) as u32
                    } else {
                        c[0] as u32
                    }
                })
                .collect()
        } else {
            s.iter().map(|&b| b as u32).collect()
        }
    }
    fn unicode(&self, code: u32) -> String {
        if let Some(u) = self.to_unicode.get(&code) {
            return u.clone();
        }
        if self.two_byte {
            return "\u{FFFD}".to_string();
        }
        winansi_to_char(code as u8)
            .map(|c| c.to_string())
            .unwrap_or_else(|| (code as u8 as char).to_string())
    }
    /// (best-estimate width, upper-bound width) in 1/1000 em.
    fn width(&self, code: u32) -> (f64, f64) {
        match self.widths.get(&code).copied().or(self.default_width) {
            Some(w) if self.exact => (w, w.max(0.0)),
            Some(w) => (w, w.max(1000.0)),
            None => (600.0, 1000.0),
        }
    }
}

// ─────────────────────────────── content lexer ───────────────────────────────

#[derive(Clone, Debug)]
enum Tok {
    Operand(Obj, Vec<u8>),
    Op(Vec<u8>),
    /// Whole `BI … ID … EI` sequence, raw.
    InlineImage(Vec<u8>),
}

fn lex(data: &[u8]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut p = Parser::new(data, 0);
    loop {
        p.skip_ws();
        if p.pos >= data.len() {
            break;
        }
        let start = p.pos;
        let b = data[p.pos];
        let regular = !matches!(
            b,
            b'(' | b'<' | b'[' | b'/' | b')' | b'>' | b']' | b'{' | b'}'
        );
        if regular && !(b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.') {
            // Keyword: operator, or true/false/null operand.
            let mut e = p.pos;
            while e < data.len()
                && !matches!(
                    data[e],
                    b' ' | b'\t'
                        | b'\r'
                        | b'\n'
                        | b'\x0c'
                        | b'\0'
                        | b'('
                        | b')'
                        | b'<'
                        | b'>'
                        | b'['
                        | b']'
                        | b'{'
                        | b'}'
                        | b'/'
                        | b'%'
                )
            {
                e += 1;
            }
            let kw = data[start..e].to_vec();
            p.pos = e.max(start + 1);
            match kw.as_slice() {
                b"true" => out.push(Tok::Operand(Obj::Bool(true), kw)),
                b"false" => out.push(Tok::Operand(Obj::Bool(false), kw)),
                b"null" => out.push(Tok::Operand(Obj::Null, kw)),
                b"BI" => {
                    // Inline image: dictionary until ID, then binary until EI.
                    let id = find_kw(data, b"ID", p.pos);
                    let Some(id) = id else {
                        out.push(Tok::InlineImage(data[start..].to_vec()));
                        break;
                    };
                    let mut q = id + 3;
                    let mut end = data.len();
                    while q + 2 <= data.len() {
                        if &data[q..q + 2] == b"EI"
                            && q > 0
                            && is_ws(data[q - 1])
                            && (q + 2 == data.len() || is_ws(data[q + 2]))
                        {
                            end = q + 2;
                            break;
                        }
                        q += 1;
                    }
                    out.push(Tok::InlineImage(data[start..end].to_vec()));
                    p.pos = end;
                }
                _ => out.push(Tok::Op(kw)),
            }
            continue;
        }
        match p.parse() {
            Ok(o) => {
                let raw = data[start..p.pos.max(start + 1)].to_vec();
                out.push(Tok::Operand(o, raw));
            }
            Err(_) => {
                p.pos = start + 1;
            }
        }
        if p.pos <= start {
            p.pos = start + 1;
        }
    }
    out
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

fn find_kw(data: &[u8], kw: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + kw.len() <= data.len() {
        if &data[i..i + kw.len()] == kw
            && (i == 0 || is_ws(data[i - 1]))
            && (i + kw.len() == data.len() || is_ws(data[i + kw.len()]))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn serialize(toks: &[Tok]) -> Vec<u8> {
    let mut out = Vec::new();
    for t in toks {
        match t {
            Tok::Operand(_, raw) => {
                out.extend_from_slice(raw);
                out.push(b' ');
            }
            Tok::Op(op) => {
                out.extend_from_slice(op);
                out.push(b'\n');
            }
            Tok::InlineImage(raw) => {
                out.extend_from_slice(raw);
                out.push(b'\n');
            }
        }
    }
    out
}

/// Number in plain PDF form (no trailing zeros).
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn obj_bytes(o: &Obj) -> Vec<u8> {
    let mut v = Vec::new();
    super::pdfobj::write_obj(&mut v, o, &HashMap::new());
    v
}

// ─────────────────────────────── interpreter ───────────────────────────────

/// Vertical glyph extent as a fraction of the font size, below and above the
/// baseline. Covers the descenders and ascenders of common Latin fonts
/// (Helvetica descender 0.207, Times 0.217; accented capitals ≈ 0.93).
const DESCENT: f64 = 0.22;
const ASCENT: f64 = 0.95;

#[derive(Clone)]
struct Gs {
    ctm: M,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    ts: f64,
    font: Option<Vec<u8>>,
    fs: f64,
    /// Text rendering mode (`Tr`).
    tr: i64,
    /// Pattern names of the current fill / stroke paint, if a pattern.
    fill_pat: Option<Vec<u8>>,
    stroke_pat: Option<Vec<u8>>,
}

/// A path segment in the coordinates it was written in (before the CTM).
#[derive(Clone, Copy)]
enum Seg {
    M(f64, f64),
    L(f64, f64),
    C(f64, f64, f64, f64, f64, f64),
}

#[derive(Clone, Default)]
struct SubPath {
    segs: Vec<Seg>,
    closed: bool,
}

/// A path and the operator that paints (or clips with) it.
struct PathPaint {
    /// Token ranges (first operand ..= operator) of construction and clip
    /// operators; the painting operator is `paint_index`.
    spans: Vec<(usize, usize)>,
    paint_index: usize,
    paint_op: Vec<u8>,
    /// `Some(even_odd)` when the path also sets the clip.
    clip: Option<bool>,
    ctm: M,
    subpaths: Vec<SubPath>,
    fill_pat: Option<Vec<u8>>,
    stroke_pat: Option<Vec<u8>>,
}

/// One glyph, in user space of the page.
#[derive(Clone)]
struct Glyph {
    text: String,
    rect: Rect,
    /// Inside a form XObject or appearance stream (cannot be rewritten).
    nested: bool,
}

struct ShowOp {
    /// Token index of the operator.
    op_index: usize,
    /// Number of operand tokens consumed.
    operands: usize,
    /// Total advance in text space (for the replacement).
    advance: f64,
    fs: f64,
    th: f64,
    text: String,
    /// The operator's glyphs: `Interp::glyphs[glyphs.0..glyphs.1]`.
    glyphs: (usize, usize),
    /// Drawn with a Type3 font (glyphs are content streams).
    type3: bool,
    /// Type3 font whose glyph bounds are unknown: may draw anywhere.
    unbounded: bool,
    /// Text rendering mode adds the glyphs to the clip (Tr 4–7).
    clip_text: bool,
    /// Indices into the BDC frame stack open at this operator.
    open_frames: Vec<usize>,
}

struct Frame {
    operand_index: Option<usize>,
    props_name: Option<Vec<u8>>,
    mcid: Option<i64>,
    tainted: bool,
}

struct XUse {
    op_index: usize,
    rect: Rect,
    image: bool,
}

struct Interp<'a> {
    doc: &'a Document,
    fonts: HashMap<Vec<u8>, Font>,
    glyphs: Vec<Glyph>,
    shows: Vec<ShowOp>,
    xuses: Vec<XUse>,
    inline_images: Vec<(usize, Rect)>,
    frames: Vec<Frame>,
    paths: Vec<PathPaint>,
    /// Page-space extents of everything drawn inside forms (which are read,
    /// never rewritten): images, painted paths, unreadable forms.
    nested_marks: Vec<Rect>,
    depth: usize,
}

fn resources_get<'a>(doc: &'a Document, res: &'a Dict, cat: &[u8], name: &[u8]) -> Option<&'a Obj> {
    let sub = doc.resolve_dict(dict_get(res, cat)?)?;
    dict_get(sub, name)
}

impl<'a> Interp<'a> {
    fn new(doc: &'a Document) -> Self {
        Interp {
            doc,
            fonts: HashMap::new(),
            glyphs: vec![],
            shows: vec![],
            xuses: vec![],
            inline_images: vec![],
            frames: vec![],
            paths: vec![],
            nested_marks: vec![],
            depth: 0,
        }
    }

    fn font(&mut self, res: &Dict, name: &[u8], scope: &[u8]) -> Font {
        let key = [scope, b"/", name].concat();
        if let Some(f) = self.fonts.get(&key) {
            return f.clone();
        }
        let f = resources_get(self.doc, res, b"Font", name)
            .map(|o| load_font(self.doc, o))
            .unwrap_or_default();
        self.fonts.insert(key, f.clone());
        f
    }

    /// Interpret `toks` with resources `res` under `base` CTM. When
    /// `nested`, glyphs are recorded as nested and nothing is collected for
    /// rewriting (forms and appearance streams are read, never edited).
    fn run(&mut self, toks: &[Tok], res: &Dict, base: M, nested: bool, scope: &[u8]) -> bool {
        // Returns whether any text or image was drawn (used for forms).
        let mut drew = false;
        let mut gs = Gs {
            ctm: base,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            ts: 0.0,
            font: None,
            fs: 0.0,
            tr: 0,
            fill_pat: None,
            stroke_pat: None,
        };
        let mut stack: Vec<Gs> = vec![];
        // Path under construction.
        let mut path: Vec<SubPath> = vec![];
        let mut spans: Vec<(usize, usize)> = vec![];
        let mut pclip: Option<bool> = None;
        let mut cur = (0.0f64, 0.0f64);
        let mut start = (0.0f64, 0.0f64);
        let mut tm = ID;
        let mut tlm = ID;
        let mut ops: Vec<usize> = vec![];
        let mut open: Vec<usize> = vec![]; // indices into self.frames
        for (i, t) in toks.iter().enumerate() {
            match t {
                Tok::Operand(..) => ops.push(i),
                Tok::InlineImage(_) => {
                    let r = Rect::transformed(&gs.ctm, 0.0, 0.0, 1.0, 1.0);
                    drew = true;
                    if !nested {
                        self.inline_images.push((i, r));
                    } else {
                        self.nested_marks.push(r);
                    }
                    ops.clear();
                }
                Tok::Op(op) => {
                    let num = |k: usize| -> f64 {
                        ops.get(ops.len().wrapping_sub(k))
                            .and_then(|&j| match &toks[j] {
                                Tok::Operand(o, _) => o.as_f64(),
                                _ => None,
                            })
                            .unwrap_or(0.0)
                    };
                    let span = (ops.first().copied().unwrap_or(i), i);
                    match op.as_slice() {
                        b"m" if ops.len() >= 2 => {
                            cur = (num(2), num(1));
                            start = cur;
                            path.push(SubPath {
                                segs: vec![Seg::M(cur.0, cur.1)],
                                closed: false,
                            });
                            spans.push(span);
                        }
                        b"l" | b"c" | b"v" | b"y" => {
                            let seg = match op.as_slice() {
                                b"l" if ops.len() >= 2 => Some(Seg::L(num(2), num(1))),
                                b"c" if ops.len() >= 6 => {
                                    Some(Seg::C(num(6), num(5), num(4), num(3), num(2), num(1)))
                                }
                                b"v" if ops.len() >= 4 => {
                                    Some(Seg::C(cur.0, cur.1, num(4), num(3), num(2), num(1)))
                                }
                                b"y" if ops.len() >= 4 => {
                                    Some(Seg::C(num(4), num(3), num(2), num(1), num(2), num(1)))
                                }
                                _ => None,
                            };
                            if let Some(seg) = seg {
                                if path.last().is_none_or(|s| s.closed) {
                                    path.push(SubPath {
                                        segs: vec![Seg::M(cur.0, cur.1)],
                                        closed: false,
                                    });
                                }
                                cur = match seg {
                                    Seg::L(x, y) | Seg::C(_, _, _, _, x, y) | Seg::M(x, y) => {
                                        (x, y)
                                    }
                                };
                                if let Some(sp) = path.last_mut() {
                                    sp.segs.push(seg);
                                }
                            }
                            spans.push(span);
                        }
                        b"h" => {
                            if let Some(sp) = path.last_mut() {
                                sp.closed = true;
                            }
                            cur = start;
                            spans.push(span);
                        }
                        b"re" if ops.len() >= 4 => {
                            let (x, y, w, h) = (num(4), num(3), num(2), num(1));
                            path.push(SubPath {
                                segs: vec![
                                    Seg::M(x, y),
                                    Seg::L(x + w, y),
                                    Seg::L(x + w, y + h),
                                    Seg::L(x, y + h),
                                ],
                                closed: true,
                            });
                            cur = (x, y);
                            start = cur;
                            spans.push(span);
                        }
                        b"W" | b"W*" => {
                            pclip = Some(op == b"W*");
                            spans.push(span);
                        }
                        b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                            let subpaths = std::mem::take(&mut path);
                            let clip = pclip.take();
                            let sp = std::mem::take(&mut spans);
                            if !subpaths.is_empty() {
                                if op != b"n" {
                                    drew = true;
                                }
                                if nested {
                                    if op != b"n" {
                                        let b = path_bbox(&subpaths, &gs.ctm);
                                        self.nested_marks.push(b);
                                    }
                                } else {
                                    self.paths.push(PathPaint {
                                        spans: sp,
                                        paint_index: i,
                                        paint_op: op.clone(),
                                        clip,
                                        ctm: gs.ctm,
                                        subpaths,
                                        fill_pat: gs.fill_pat.clone(),
                                        stroke_pat: gs.stroke_pat.clone(),
                                    });
                                }
                            }
                        }
                        b"scn" | b"SCN" | b"sc" | b"SC" | b"g" | b"G" | b"rg" | b"RG" | b"k"
                        | b"K" | b"cs" | b"CS" => {
                            let pat = match (op.as_slice(), ops.last().map(|&j| &toks[j])) {
                                (b"scn" | b"SCN", Some(Tok::Operand(Obj::Name(n), _))) => {
                                    Some(n.clone())
                                }
                                _ => None,
                            };
                            if op.iter().all(|c| c.is_ascii_lowercase()) {
                                gs.fill_pat = pat;
                            } else {
                                gs.stroke_pat = pat;
                            }
                        }
                        b"q" => stack.push(gs.clone()),
                        b"Q" => {
                            if let Some(g) = stack.pop() {
                                gs = g;
                            }
                        }
                        b"cm" if ops.len() >= 6 => {
                            let m = [num(6), num(5), num(4), num(3), num(2), num(1)];
                            gs.ctm = mul(&m, &gs.ctm);
                        }
                        b"BT" => {
                            tm = ID;
                            tlm = ID;
                        }
                        b"Tc" => gs.tc = num(1),
                        b"Tw" => gs.tw = num(1),
                        b"Tz" => gs.th = num(1) / 100.0,
                        b"TL" => gs.tl = num(1),
                        b"Ts" => gs.ts = num(1),
                        b"Tr" => gs.tr = num(1) as i64,
                        b"Tf" if ops.len() >= 2 => {
                            gs.fs = num(1);
                            if let Some(Tok::Operand(Obj::Name(n), _)) =
                                ops.get(ops.len() - 2).map(|&j| &toks[j])
                            {
                                gs.font = Some(n.clone());
                            }
                        }
                        b"Td" | b"TD" => {
                            let (tx, ty) = (num(2), num(1));
                            if op == b"TD" {
                                gs.tl = -ty;
                            }
                            tlm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], &tlm);
                            tm = tlm;
                        }
                        b"Tm" if ops.len() >= 6 => {
                            tlm = [num(6), num(5), num(4), num(3), num(2), num(1)];
                            tm = tlm;
                        }
                        b"T*" => {
                            tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.tl], &tlm);
                            tm = tlm;
                        }
                        b"Tj" | b"TJ" | b"'" | b"\"" => {
                            if op == b"'" || op == b"\"" {
                                if op == b"\"" {
                                    gs.tw = num(3);
                                    gs.tc = num(2);
                                }
                                tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.tl], &tlm);
                                tm = tlm;
                            }
                            let operand = ops.last().map(|&j| &toks[j]);
                            let parts: Vec<Obj> = match operand {
                                Some(Tok::Operand(Obj::Array(a), _)) => a.clone(),
                                Some(Tok::Operand(o @ Obj::Str(_), _)) => vec![o.clone()],
                                _ => vec![],
                            };
                            let font = match &gs.font {
                                Some(n) => self.font(res, n, scope),
                                None => Font::default(),
                            };
                            let first_glyph = self.glyphs.len();
                            let mut adv_total = 0.0;
                            let mut text = String::new();
                            for part in &parts {
                                match part {
                                    Obj::Str(s) => {
                                        for code in font.codes(s) {
                                            let (w, wu) = font.width(code);
                                            let space = !font.two_byte && code == 32;
                                            let adv = ((w / 1000.0) * gs.fs
                                                + gs.tc
                                                + if space { gs.tw } else { 0.0 })
                                                * gs.th;
                                            let upper = ((wu / 1000.0) * gs.fs
                                                + gs.tc.max(0.0)
                                                + if space { gs.tw.max(0.0) } else { 0.0 })
                                                * gs.th;
                                            let trm = mul(&tm, &gs.ctm);
                                            let lo = gs.ts - DESCENT * gs.fs.abs();
                                            let hi = gs.ts + ASCENT * gs.fs.abs();
                                            let gr = match font.type3 {
                                                // Type3: the glyph procedure may draw
                                                // anywhere in FontBBox.
                                                Some(Some(b)) => Rect::transformed(
                                                    &trm,
                                                    (b.x0 * gs.fs).min(0.0),
                                                    gs.ts + b.y0 * gs.fs,
                                                    (b.x1 * gs.fs).max(adv.max(upper)),
                                                    gs.ts + b.y1 * gs.fs,
                                                ),
                                                _ => Rect::transformed(
                                                    &trm,
                                                    0.0,
                                                    lo,
                                                    adv.max(upper),
                                                    hi,
                                                ),
                                            };
                                            let u = font.unicode(code);
                                            text.push_str(&u);
                                            self.glyphs.push(Glyph {
                                                text: u,
                                                rect: gr,
                                                nested,
                                            });
                                            tm = mul(&[1.0, 0.0, 0.0, 1.0, adv, 0.0], &tm);
                                            adv_total += adv;
                                        }
                                    }
                                    o => {
                                        if let Some(n) = o.as_f64() {
                                            let adj = -n / 1000.0 * gs.fs * gs.th;
                                            tm = mul(&[1.0, 0.0, 0.0, 1.0, adj, 0.0], &tm);
                                            adv_total += adj;
                                        }
                                    }
                                }
                            }
                            if !parts.is_empty() {
                                drew = true;
                                // Inside a form, Type3 text with unknown glyph
                                // bounds (or clip-mode text) may reach any region.
                                if nested
                                    && (matches!(font.type3, Some(None))
                                        || (4..=7).contains(&gs.tr))
                                {
                                    self.nested_marks.push(match font.type3 {
                                        Some(None) => Rect {
                                            x0: f64::MIN,
                                            y0: f64::MIN,
                                            x1: f64::MAX,
                                            y1: f64::MAX,
                                        },
                                        _ => Rect::of_points(
                                            &self.glyphs[first_glyph..]
                                                .iter()
                                                .flat_map(|g| {
                                                    [(g.rect.x0, g.rect.y0), (g.rect.x1, g.rect.y1)]
                                                })
                                                .collect::<Vec<_>>(),
                                        ),
                                    });
                                }
                            }
                            if !nested {
                                let n_operands = match op.as_slice() {
                                    b"\"" => 3,
                                    _ => 1,
                                };
                                self.shows.push(ShowOp {
                                    op_index: i,
                                    operands: n_operands.min(ops.len()),
                                    advance: adv_total,
                                    fs: gs.fs,
                                    th: gs.th,
                                    text,
                                    glyphs: (first_glyph, self.glyphs.len()),
                                    type3: font.type3.is_some(),
                                    unbounded: matches!(font.type3, Some(None)),
                                    clip_text: (4..=7).contains(&gs.tr),
                                    open_frames: open.clone(),
                                });
                            }
                        }
                        b"Do" => {
                            if let Some(Tok::Operand(Obj::Name(n), _)) =
                                ops.last().map(|&j| &toks[j])
                            {
                                if let Some(xo) = resources_get(self.doc, res, b"XObject", n) {
                                    let target = self.doc.resolve(xo);
                                    if let Obj::Stream(xd, raw) = target {
                                        let sub = dict_get(xd, b"Subtype")
                                            .and_then(|o| o.as_name())
                                            .unwrap_or(b"");
                                        if sub == b"Image" {
                                            drew = true;
                                            let rect =
                                                Rect::transformed(&gs.ctm, 0.0, 0.0, 1.0, 1.0);
                                            if !nested {
                                                self.xuses.push(XUse {
                                                    op_index: i,
                                                    rect,
                                                    image: true,
                                                });
                                            } else {
                                                self.nested_marks.push(rect);
                                            }
                                        } else if sub == b"Form" && self.depth < 12 {
                                            let fm: M = match dict_get(xd, b"Matrix") {
                                                Some(Obj::Array(a)) if a.len() == 6 => {
                                                    let v: Vec<f64> = a
                                                        .iter()
                                                        .map(|o| o.as_f64().unwrap_or(0.0))
                                                        .collect();
                                                    [v[0], v[1], v[2], v[3], v[4], v[5]]
                                                }
                                                _ => ID,
                                            };
                                            let ctm = mul(&fm, &gs.ctm);
                                            let bbox = match dict_get(xd, b"BBox") {
                                                Some(Obj::Array(a)) if a.len() == 4 => {
                                                    let v: Vec<f64> = a
                                                        .iter()
                                                        .map(|o| {
                                                            self.doc
                                                                .resolve(o)
                                                                .as_f64()
                                                                .unwrap_or(0.0)
                                                        })
                                                        .collect();
                                                    Rect::transformed(&ctm, v[0], v[1], v[2], v[3])
                                                }
                                                _ => Rect {
                                                    x0: f64::MIN,
                                                    y0: f64::MIN,
                                                    x1: f64::MAX,
                                                    y1: f64::MAX,
                                                },
                                            };
                                            let fres = dict_get(xd, b"Resources")
                                                .and_then(|o| self.doc.resolve_dict(o))
                                                .cloned()
                                                .unwrap_or_else(|| res.clone());
                                            let has = match decode_stream(xd, raw) {
                                                Ok(data) => {
                                                    self.depth += 1;
                                                    let ftoks = lex(&data);
                                                    let scope2 = [scope, b"/", n].concat();
                                                    let r =
                                                        self.run(&ftoks, &fres, ctm, true, &scope2);
                                                    self.depth -= 1;
                                                    r
                                                }
                                                // Unreadable form: assume it draws
                                                // anywhere in its bounding box.
                                                Err(_) => {
                                                    self.nested_marks.push(bbox);
                                                    true
                                                }
                                            };
                                            drew |= has;
                                            if !nested {
                                                self.xuses.push(XUse {
                                                    op_index: i,
                                                    rect: bbox,
                                                    image: false,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        b"BDC" | b"BMC" => {
                            let mut frame = Frame {
                                operand_index: None,
                                props_name: None,
                                mcid: None,
                                tainted: false,
                            };
                            if op == b"BDC" {
                                if let Some(&j) = ops.last() {
                                    match &toks[j] {
                                        Tok::Operand(Obj::Dict(d), _) => {
                                            frame.operand_index = Some(j);
                                            frame.mcid = dict_get(d, b"MCID")
                                                .and_then(|o| o.as_f64())
                                                .map(|f| f as i64);
                                        }
                                        Tok::Operand(Obj::Name(n), _) => {
                                            frame.props_name = Some(n.clone());
                                            frame.mcid =
                                                resources_get(self.doc, res, b"Properties", n)
                                                    .and_then(|o| self.doc.resolve_dict(o))
                                                    .and_then(|d| dict_get(d, b"MCID"))
                                                    .and_then(|o| o.as_f64())
                                                    .map(|f| f as i64);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if !nested {
                                self.frames.push(frame);
                                open.push(self.frames.len() - 1);
                            }
                        }
                        b"EMC" if !nested => {
                            open.pop();
                        }
                        _ => {}
                    }
                    ops.clear();
                }
            }
        }
        drew
    }
}

// ─────────────────────────────── vector paths ───────────────────────────────

type P = (f64, f64);

fn invert(m: &M) -> Option<M> {
    let det = m[0] * m[3] - m[1] * m[2];
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

/// Bounding box of the control points (a superset of the drawn geometry).
fn subpath_bbox(sp: &SubPath, ctm: &M) -> Rect {
    let mut pts = vec![];
    for s in &sp.segs {
        match *s {
            Seg::M(x, y) | Seg::L(x, y) => pts.push(apply(ctm, x, y)),
            Seg::C(a, b, c, d, e, f) => {
                pts.push(apply(ctm, a, b));
                pts.push(apply(ctm, c, d));
                pts.push(apply(ctm, e, f));
            }
        }
    }
    Rect::of_points(&pts)
}

fn path_bbox(sps: &[SubPath], ctm: &M) -> Rect {
    sps.iter()
        .map(|s| subpath_bbox(s, ctm))
        .reduce(|a, b| a.union(&b))
        .unwrap_or(Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        })
}

/// The subpath as a polyline in page space (curves flattened to 0.05 pt).
fn flatten(sp: &SubPath, ctm: &M) -> Vec<P> {
    fn bez(p0: P, p1: P, p2: P, p3: P, depth: u32, out: &mut Vec<P>) {
        let d = |p: P| {
            let (dx, dy) = (p3.0 - p0.0, p3.1 - p0.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-9 {
                ((p.0 - p0.0).powi(2) + (p.1 - p0.1).powi(2)).sqrt()
            } else {
                ((p.0 - p0.0) * dy - (p.1 - p0.1) * dx).abs() / len
            }
        };
        if depth >= 12 || (d(p1) <= 0.05 && d(p2) <= 0.05) {
            out.push(p3);
            return;
        }
        let mid = |a: P, b: P| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (a, b, c) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
        let (e, f) = (mid(a, b), mid(b, c));
        let g = mid(e, f);
        bez(p0, a, e, g, depth + 1, out);
        bez(g, f, c, p3, depth + 1, out);
    }
    let mut out: Vec<P> = vec![];
    for s in &sp.segs {
        match *s {
            Seg::M(x, y) | Seg::L(x, y) => out.push(apply(ctm, x, y)),
            Seg::C(a, b, c, d, e, f) => {
                let p0 = *out.last().unwrap_or(&apply(ctm, a, b));
                bez(
                    p0,
                    apply(ctm, a, b),
                    apply(ctm, c, d),
                    apply(ctm, e, f),
                    0,
                    &mut out,
                );
            }
        }
    }
    out
}

/// Sutherland–Hodgman against one half-plane: keep `coord(axis) >= v`
/// (`ge`) or `<= v`. Preserves the winding number inside the half-plane.
fn clip_half(poly: &[P], axis_y: bool, v: f64, ge: bool) -> Vec<P> {
    let inside = |p: &P| {
        let c = if axis_y { p.1 } else { p.0 };
        if ge {
            c >= v
        } else {
            c <= v
        }
    };
    let cross = |a: &P, b: &P| {
        let (ca, cb) = if axis_y { (a.1, b.1) } else { (a.0, b.0) };
        let t = (v - ca) / (cb - ca);
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    };
    let mut out = vec![];
    for k in 0..poly.len() {
        let a = &poly[k];
        let b = &poly[(k + 1) % poly.len()];
        match (inside(a), inside(b)) {
            (true, true) => out.push(*b),
            (true, false) => out.push(cross(a, b)),
            (false, true) => {
                out.push(cross(a, b));
                out.push(*b);
            }
            (false, false) => {}
        }
    }
    out
}

fn area(poly: &[P]) -> f64 {
    let mut s = 0.0;
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    s / 2.0
}

/// `poly` minus the regions, as pieces whose union is exactly the part of
/// the polygon outside every region (each region's outside split into four
/// convex bands). Fill semantics (winding, even-odd) are preserved outside
/// the regions; nothing is left inside them.
fn polygon_minus(poly: Vec<P>, regions: &[Rect]) -> Vec<Vec<P>> {
    let mut polys = vec![poly];
    for r in regions {
        let mut next = vec![];
        for poly in polys {
            if !Rect::of_points(&poly).intersects(r) {
                next.push(poly);
                continue;
            }
            let bands: [&[(bool, f64, bool)]; 4] = [
                &[(true, r.y1, true)],
                &[(true, r.y0, false)],
                &[
                    (false, r.x0, false),
                    (true, r.y0, true),
                    (true, r.y1, false),
                ],
                &[(false, r.x1, true), (true, r.y0, true), (true, r.y1, false)],
            ];
            for band in bands {
                let mut q = poly.clone();
                for &(ay, v, ge) in band {
                    if q.len() < 3 {
                        break;
                    }
                    q = clip_half(&q, ay, v, ge);
                }
                if q.len() >= 3 && area(&q).abs() > 1e-9 {
                    next.push(q);
                }
            }
        }
        polys = next;
    }
    polys
}

/// Parameter interval of segment a→b inside `r`, if it crosses the interior.
fn seg_inside(a: P, b: P, r: &Rect) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [
        (-dx, a.0 - r.x0),
        (dx, r.x1 - a.0),
        (-dy, a.1 - r.y0),
        (dy, r.y1 - a.1),
    ] {
        if p.abs() < 1e-12 {
            if q <= 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t1 - t0 > 1e-9).then_some((t0, t1))
}

/// A polyline minus the regions: the pieces outside every region.
fn polyline_minus(line: Vec<P>, regions: &[Rect]) -> Vec<Vec<P>> {
    let mut lines = vec![line];
    for r in regions {
        let mut next = vec![];
        for line in lines {
            let mut cur: Vec<P> = vec![];
            let at = |a: P, b: P, t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            for w in line.windows(2) {
                let (a, b) = (w[0], w[1]);
                let outside: Vec<(f64, f64)> = match seg_inside(a, b, r) {
                    None => vec![(0.0, 1.0)],
                    Some((t0, t1)) => [(0.0, t0), (t1, 1.0)]
                        .into_iter()
                        .filter(|(s, e)| e - s > 1e-9)
                        .collect(),
                };
                for (s, e) in outside {
                    let (pa, pb) = (at(a, b, s), at(a, b, e));
                    let joins = cur
                        .last()
                        .is_some_and(|l| (l.0 - pa.0).abs() < 1e-9 && (l.1 - pa.1).abs() < 1e-9);
                    if !joins && cur.len() >= 2 {
                        next.push(std::mem::take(&mut cur));
                    }
                    if !joins {
                        cur = vec![pa];
                    }
                    cur.push(pb);
                }
            }
            if cur.len() >= 2 {
                next.push(cur);
            }
        }
        lines = next;
    }
    lines
}

fn path_num(v: f64) -> Tok {
    let s = {
        let s = format!("{v:.5}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s == "-0" {
            "0".to_string()
        } else {
            s
        }
    };
    Tok::Operand(Obj::Real(v), s.into_bytes())
}

fn push_op(out: &mut Vec<Tok>, nums: &[f64], op: &[u8]) {
    out.extend(nums.iter().map(|v| path_num(*v)));
    out.push(Tok::Op(op.to_vec()));
}

fn emit_original(out: &mut Vec<Tok>, sp: &SubPath, close: bool) {
    for s in &sp.segs {
        match *s {
            Seg::M(x, y) => push_op(out, &[x, y], b"m"),
            Seg::L(x, y) => push_op(out, &[x, y], b"l"),
            Seg::C(a, b, c, d, e, f) => push_op(out, &[a, b, c, d, e, f], b"c"),
        }
    }
    if close {
        out.push(Tok::Op(b"h".to_vec()));
    }
}

/// Page-space points, written back in the path's own coordinates.
fn emit_points(out: &mut Vec<Tok>, pts: &[P], inv: &M, close: bool) {
    for (k, p) in pts.iter().enumerate() {
        let (x, y) = apply(inv, p.0, p.1);
        push_op(out, &[x, y], if k == 0 { b"m" } else { b"l" });
    }
    if close {
        out.push(Tok::Op(b"h".to_vec()));
    }
}

fn pattern_is_tiling(doc: &Document, res: &Dict, name: &[u8]) -> bool {
    let Some(o) = resources_get(doc, res, b"Pattern", name) else {
        // Unknown pattern: cannot tell what it draws.
        return true;
    };
    let d = match doc.resolve(o) {
        Obj::Stream(d, _) | Obj::Dict(d) => d,
        _ => return true,
    };
    dict_get(d, b"PatternType").and_then(|o| o.as_f64()) != Some(2.0)
}

/// Replacement tokens for a path that touches a region, or `None` when it
/// does not. Fills keep exactly their parts outside the regions; strokes are
/// cut where they enter a region; clip paths keep their shape outside the
/// regions and are opened to the full region inside (where the opaque box
/// covers everything anyway), so no shape inside a region survives.
fn redact_path(
    doc: &Document,
    res: &Dict,
    pp: &PathPaint,
    regions: &[Rect],
) -> Result<Option<Vec<Tok>>, FormeError> {
    let hit: Vec<bool> = pp
        .subpaths
        .iter()
        .map(|s| any_hit(&subpath_bbox(s, &pp.ctm), regions))
        .collect();
    if !hit.iter().any(|h| *h) {
        return Ok(None);
    }
    let op = pp.paint_op.as_slice();
    let fill = matches!(op, b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*");
    let stroke = matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
    let close_stroke = matches!(op, b"s" | b"b" | b"b*");
    let fill_op: &[u8] = if matches!(op, b"f*" | b"B*" | b"b*") {
        b"f*"
    } else {
        b"f"
    };
    if (fill
        && pp
            .fill_pat
            .as_ref()
            .is_some_and(|n| pattern_is_tiling(doc, res, n)))
        || (stroke
            && pp
                .stroke_pat
                .as_ref()
                .is_some_and(|n| pattern_is_tiling(doc, res, n)))
    {
        return Err(refuse("a redaction region covers an area painted with a tiling pattern, which this version cannot rewrite"));
    }
    let mut out = vec![];
    let Some(inv) = invert(&pp.ctm) else {
        // A degenerate transform draws nothing; an equivalent clip is empty.
        if let Some(eo) = pp.clip {
            push_op(&mut out, &[0.0, 0.0, 0.0, 0.0], b"re");
            out.push(Tok::Op(if eo { b"W*".to_vec() } else { b"W".to_vec() }));
            out.push(Tok::Op(b"n".to_vec()));
        }
        return Ok(Some(out));
    };
    let area_path = |out: &mut Vec<Tok>| -> bool {
        let start = out.len();
        for (sp, h) in pp.subpaths.iter().zip(&hit) {
            if !h {
                emit_original(out, sp, sp.closed);
                continue;
            }
            for piece in polygon_minus(flatten(sp, &pp.ctm), regions) {
                emit_points(out, &piece, &inv, true);
            }
        }
        out.len() > start
    };
    if fill && area_path(&mut out) {
        out.push(Tok::Op(fill_op.to_vec()));
    }
    if stroke {
        let start = out.len();
        for (sp, h) in pp.subpaths.iter().zip(&hit) {
            let closed = sp.closed || close_stroke;
            if !h {
                emit_original(&mut out, sp, closed);
                continue;
            }
            let mut line = flatten(sp, &pp.ctm);
            if closed {
                if let Some(&f) = line.first() {
                    line.push(f);
                }
            }
            for piece in polyline_minus(line, regions) {
                emit_points(&mut out, &piece, &inv, false);
            }
        }
        if out.len() > start {
            out.push(Tok::Op(b"S".to_vec()));
        }
    }
    if let Some(eo) = pp.clip {
        area_path(&mut out);
        let bb = path_bbox(&pp.subpaths, &pp.ctm);
        for r in regions.iter().filter(|r| r.intersects(&bb)) {
            let corners = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1)];
            emit_points(&mut out, &corners, &inv, true);
        }
        out.push(Tok::Op(if eo { b"W*".to_vec() } else { b"W".to_vec() }));
        out.push(Tok::Op(b"n".to_vec()));
    }
    Ok(Some(out))
}

// ─────────────────────────────── page model ───────────────────────────────

struct PageRef {
    id: u32,
    /// The visible area in page space: CropBox ∩ MediaBox (or the MediaBox).
    view: Rect,
    /// `/Rotate`, normalised to 0, 90, 180 or 270.
    rotate: i32,
    /// Why this page's coordinates cannot be mapped safely, if they cannot.
    geometry_error: Option<String>,
    resources: Dict,
}

fn parse_box(doc: &Document, o: Option<&Obj>) -> Option<Rect> {
    let Some(Obj::Array(a)) = o.map(|o| doc.resolve(o)) else {
        return None;
    };
    if a.len() != 4 {
        return None;
    }
    let v: Vec<f64> = a.iter().filter_map(|o| doc.resolve(o).as_f64()).collect();
    if v.len() != 4 || v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let r = Rect {
        x0: v[0].min(v[2]),
        y0: v[1].min(v[3]),
        x1: v[0].max(v[2]),
        y1: v[1].max(v[3]),
    };
    (r.x1 > r.x0 && r.y1 > r.y0).then_some(r)
}

fn collect_pages(doc: &Document) -> Result<Vec<PageRef>, FormeError> {
    let root = doc
        .resolve_dict(dict_get(&doc.trailer, b"Root").ok_or_else(|| refuse("no catalog"))?)
        .ok_or_else(|| refuse("no catalog"))?;
    let pages = dict_get(root, b"Pages").ok_or_else(|| refuse("no page tree"))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    #[derive(Clone, Default)]
    struct Inherited {
        res: Option<Dict>,
        media: Option<Rect>,
        crop: Option<Rect>,
        rotate: Option<f64>,
    }
    fn walk(
        doc: &Document,
        node: &Obj,
        inh: Inherited,
        out: &mut Vec<PageRef>,
        seen: &mut HashSet<u32>,
        depth: usize,
    ) {
        if depth > 64 {
            return;
        }
        let Obj::Ref(id, _) = node else { return };
        if !seen.insert(*id) {
            return;
        }
        let Some(d) = doc.resolve_dict(node) else {
            return;
        };
        let here = Inherited {
            res: dict_get(d, b"Resources")
                .and_then(|o| doc.resolve_dict(o))
                .cloned()
                .or(inh.res),
            media: parse_box(doc, dict_get(d, b"MediaBox")).or(inh.media),
            crop: parse_box(doc, dict_get(d, b"CropBox")).or(inh.crop),
            rotate: dict_get(d, b"Rotate")
                .and_then(|o| doc.resolve(o).as_f64())
                .or(inh.rotate),
        };
        match dict_get(d, b"Type").and_then(|o| o.as_name()) {
            Some(b"Pages") => {
                if let Some(Obj::Array(kids)) = dict_get(d, b"Kids").map(|o| doc.resolve(o)) {
                    for k in kids {
                        walk(doc, k, here.clone(), out, seen, depth + 1);
                    }
                }
            }
            _ => {
                let media = here.media.unwrap_or(Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 612.0,
                    y1: 792.0,
                });
                // The visible area, as viewers compute it.
                let view = match here.crop {
                    Some(c) => {
                        let v = Rect {
                            x0: c.x0.max(media.x0),
                            y0: c.y0.max(media.y0),
                            x1: c.x1.min(media.x1),
                            y1: c.y1.min(media.y1),
                        };
                        if v.x1 > v.x0 && v.y1 > v.y0 {
                            v
                        } else {
                            media
                        }
                    }
                    None => media,
                };
                let mut geometry_error = None;
                let r = here.rotate.unwrap_or(0.0);
                let rotate = if r.is_finite() && r.fract() == 0.0 && (r as i64) % 90 == 0 {
                    (((r as i64) % 360 + 360) % 360) as i32
                } else {
                    geometry_error =
                        Some(format!("the page rotation ({r}) is not a multiple of 90"));
                    0
                };
                if let Some(u) = dict_get(d, b"UserUnit").and_then(|o| doc.resolve(o).as_f64()) {
                    if (u - 1.0).abs() > 1e-9 {
                        geometry_error = Some(format!("the page uses UserUnit {u}"));
                    }
                }
                out.push(PageRef {
                    id: *id,
                    view,
                    rotate,
                    geometry_error,
                    resources: here.res.unwrap_or_default(),
                })
            }
        }
    }
    walk(doc, pages, Inherited::default(), &mut out, &mut seen, 0);
    Ok(out)
}

impl PageRef {
    /// Displayed point (top-left origin, after /Rotate) → page space.
    fn to_user(&self, u: f64, v: f64) -> (f64, f64) {
        let b = &self.view;
        match self.rotate {
            90 => (b.x0 + v, b.y0 + u),
            180 => (b.x1 - u, b.y0 + v),
            270 => (b.x1 - v, b.y1 - u),
            _ => (b.x0 + u, b.y1 - v),
        }
    }
    /// Page space → displayed point.
    fn to_display(&self, x: f64, y: f64) -> (f64, f64) {
        let b = &self.view;
        match self.rotate {
            90 => (y - b.y0, x - b.x0),
            180 => (b.x1 - x, y - b.y0),
            270 => (b.y1 - y, b.x1 - x),
            _ => (x - b.x0, b.y1 - y),
        }
    }
    fn check_geometry(&self) -> Result<(), FormeError> {
        match &self.geometry_error {
            Some(e) => Err(refuse(format!(
                "{e}; region coordinates on this page cannot be mapped safely"
            ))),
            None => Ok(()),
        }
    }
}

fn page_content(doc: &Document, page_id: u32) -> Result<Vec<u8>, FormeError> {
    let d = doc
        .get(page_id)
        .and_then(|o| o.as_dict())
        .ok_or_else(|| refuse("bad page"))?;
    let mut parts: Vec<&Obj> = vec![];
    match dict_get(d, b"Contents").map(|o| (o, doc.resolve(o))) {
        Some((_, Obj::Array(a))) => parts.extend(a.iter()),
        Some((o, _)) => parts.push(o),
        None => {}
    }
    let mut out = Vec::new();
    for p in parts {
        if let Obj::Stream(sd, raw) = doc.resolve(p) {
            let data = decode_stream(sd, raw)
                .map_err(|e| refuse(format!("page content cannot be decoded ({e})")))?;
            out.extend_from_slice(&data);
            out.push(b'\n');
        }
    }
    Ok(out)
}

/// Regions on a page, in page space. Region coordinates are points from
/// the top-left corner of the page as displayed (visible area, after
/// /Rotate).
fn user_rects(
    regions: &[RedactionRegion],
    page_idx: usize,
    page: &PageRef,
) -> Result<Vec<(Rect, Option<String>)>, FormeError> {
    let mine: Vec<&RedactionRegion> = regions.iter().filter(|r| r.page == page_idx).collect();
    if mine.is_empty() {
        return Ok(vec![]);
    }
    page.check_geometry()?;
    mine.into_iter()
        .map(|r| {
            if ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()) {
                return Err(refuse(
                    "a redaction region has a coordinate that is not a finite number",
                ));
            }
            let (u0, u1) = (r.x.min(r.x + r.width), r.x.max(r.x + r.width));
            let (v0, v1) = (r.y.min(r.y + r.height), r.y.max(r.y + r.height));
            let a = page.to_user(u0, v0);
            let b = page.to_user(u1, v1);
            Ok((Rect::of_points(&[a, b]), r.color.clone()))
        })
        .collect()
}

// ─────────────────────────────── text search ───────────────────────────────

struct Matcher {
    /// (lowercased pattern, pattern as given, page, color)
    literals: Vec<(String, String, Option<usize>, Option<String>)>,
    #[cfg(feature = "regex")]
    regexes: Vec<(regex::Regex, Option<usize>, Option<String>)>,
}

impl Matcher {
    fn new(patterns: &[RedactionPattern]) -> Result<Matcher, FormeError> {
        let mut m = Matcher {
            literals: vec![],
            #[cfg(feature = "regex")]
            regexes: vec![],
        };
        for p in patterns {
            match p.pattern_type {
                PatternType::Literal => {
                    if !p.pattern.is_empty() {
                        m.literals.push((
                            p.pattern.to_lowercase(),
                            p.pattern.clone(),
                            p.page,
                            p.color.clone(),
                        ));
                    }
                }
                PatternType::Regex => {
                    #[cfg(feature = "regex")]
                    {
                        let re = regex::Regex::new(&p.pattern).map_err(|e| {
                            FormeError::RenderError(format!(
                                "Invalid regex pattern '{}': {e}",
                                p.pattern
                            ))
                        })?;
                        m.regexes.push((re, p.page, p.color.clone()));
                    }
                    #[cfg(not(feature = "regex"))]
                    {
                        return Err(FormeError::RenderError(
                            "Regex patterns require the 'regex' feature. Use PatternType::Literal or enable the 'regex' Cargo feature."
                                .to_string(),
                        ));
                    }
                }
            }
        }
        Ok(m)
    }

    fn is_empty(&self) -> bool {
        #[cfg(feature = "regex")]
        {
            self.literals.is_empty() && self.regexes.is_empty()
        }
        #[cfg(not(feature = "regex"))]
        {
            self.literals.is_empty()
        }
    }

    /// Byte ranges (start, end, color) matching on `page` (None = any page).
    fn find(&self, text: &str, page: Option<usize>) -> Vec<(usize, usize, Option<String>)> {
        let mut out = vec![];
        let lower = text.to_lowercase();
        let same_len = lower.len() == text.len();
        for (lit, given, pg, color) in &self.literals {
            if pg.is_some() && page.is_some() && *pg != page {
                continue;
            }
            // Case-insensitive when lowercasing keeps byte offsets aligned;
            // otherwise search the text as written (offsets must index `text`).
            let (hay, needle) = if same_len && lit.len() == given.len() {
                (lower.as_str(), lit.clone())
            } else {
                (text, given.clone())
            };
            let mut s = 0;
            while let Some(p) = hay[s..].find(&needle) {
                out.push((s + p, s + p + needle.len(), color.clone()));
                s += p + needle.len().max(1);
            }
        }
        #[cfg(feature = "regex")]
        for (re, pg, color) in &self.regexes {
            if pg.is_some() && page.is_some() && *pg != page {
                continue;
            }
            for m in re.find_iter(text) {
                if m.end() > m.start() {
                    out.push((m.start(), m.end(), color.clone()));
                }
            }
        }
        out
    }

    fn matches(&self, text: &str) -> bool {
        !self.find(text, None).is_empty()
    }
}

/// Lay glyphs out as text: spaces for horizontal gaps, newlines for baseline
/// changes. Returns the text and, per byte, the glyph index it came from.
fn page_text(glyphs: &[Glyph]) -> (String, Vec<Option<usize>>) {
    let mut s = String::new();
    let mut owner: Vec<Option<usize>> = vec![];
    let mut prev: Option<&Glyph> = None;
    for (i, g) in glyphs.iter().enumerate() {
        if let Some(p) = prev {
            let h = (p.rect.y1 - p.rect.y0).abs().max(1.0);
            let same_line = (g.rect.y0 - p.rect.y0).abs() < h * 0.4;
            let gap = g.rect.x0 - p.rect.x1;
            let sep = if !same_line {
                Some('\n')
            } else if gap > h * 0.25 || gap < -h * 2.0 {
                Some(' ')
            } else {
                None
            };
            if let Some(c) = sep {
                if !s.ends_with(c) && !g.text.starts_with(' ') && !p.text.ends_with(' ') {
                    s.push(c);
                    owner.push(None);
                }
            }
        }
        for _ in g.text.bytes() {
            owner.push(Some(i));
        }
        s.push_str(&g.text);
        prev = Some(g);
    }
    (s, owner)
}

struct PageAnalysis {
    toks: Vec<Tok>,
    interp_glyphs: Vec<Glyph>,
    shows: Vec<ShowOp>,
    xuses: Vec<XUse>,
    inline_images: Vec<(usize, Rect)>,
    frames: Vec<Frame>,
    paths: Vec<PathPaint>,
    nested_marks: Vec<Rect>,
}

fn analyze_page(doc: &Document, page: &PageRef) -> Result<PageAnalysis, FormeError> {
    let content = page_content(doc, page.id)?;
    let toks = lex(&content);
    let mut it = Interp::new(doc);
    it.run(&toks, &page.resources, ID, false, b"page");
    Ok(PageAnalysis {
        toks,
        interp_glyphs: it.glyphs,
        shows: it.shows,
        xuses: it.xuses,
        inline_images: it.inline_images,
        frames: it.frames,
        paths: it.paths,
        nested_marks: it.nested_marks,
    })
}

/// Text drawn by an appearance stream (annotation), for matching.
fn appearance_text(doc: &Document, annot: &Dict) -> Result<String, ()> {
    let mut out = String::new();
    let Some(ap) = dict_get(annot, b"AP").and_then(|o| doc.resolve_dict(o)) else {
        return Ok(out);
    };
    let mut streams: Vec<&Obj> = vec![];
    for (_, v) in ap {
        match doc.resolve(v) {
            s @ Obj::Stream(..) => streams.push(s),
            Obj::Dict(states) => {
                for (_, sv) in states {
                    if let s @ Obj::Stream(..) = doc.resolve(sv) {
                        streams.push(s);
                    }
                }
            }
            _ => {}
        }
    }
    for s in streams {
        if let Obj::Stream(sd, raw) = s {
            let data = decode_stream(sd, raw).map_err(|_| ())?;
            let res = dict_get(sd, b"Resources")
                .and_then(|o| doc.resolve_dict(o))
                .cloned()
                .unwrap_or_default();
            let mut it = Interp::new(doc);
            it.run(&lex(&data), &res, ID, true, b"ap");
            let (t, _) = page_text(&it.glyphs);
            out.push_str(&t);
            out.push('\n');
        }
    }
    Ok(out)
}

/// Text strings reachable from `o` (following references, decoding text
/// streams such as rich-text `/RC`). Parent/page/popup links are not followed.
fn strings_in(doc: &Document, o: &Obj, out: &mut Vec<String>, depth: usize) {
    if depth > 8 {
        return;
    }
    match o {
        Obj::Str(s) => out.push(decode_text_string(s)),
        Obj::Ref(..) => strings_in(doc, doc.resolve(o), out, depth + 1),
        Obj::Array(a) => a.iter().for_each(|e| strings_in(doc, e, out, depth + 1)),
        Obj::Dict(d) => d
            .iter()
            .filter(|(k, _)| !matches!(k.as_slice(), b"Parent" | b"P" | b"Popup" | b"Pg" | b"AP"))
            .for_each(|(_, v)| strings_in(doc, v, out, depth + 1)),
        Obj::Stream(d, raw) => {
            let ty = dict_get(d, b"Type")
                .and_then(|o| o.as_name())
                .unwrap_or(b"");
            let sub = dict_get(d, b"Subtype")
                .and_then(|o| o.as_name())
                .unwrap_or(b"");
            if sub != b"Image" && sub != b"Form" && ty != b"XObject" {
                if let Ok(data) = decode_stream(d, raw) {
                    out.push(String::from_utf8_lossy(&data).into_owned());
                }
            }
        }
        _ => {}
    }
}

/// PDF text string: UTF-16BE with BOM, else PDFDocEncoding (≈ Latin-1).
fn decode_text_string(s: &[u8]) -> String {
    if s.len() >= 2 && s[0] == 0xFE && s[1] == 0xFF {
        let units: Vec<u16> = s[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    s.iter().map(|&b| b as char).collect()
}

// ─────────────────────────────── public entry points ───────────────────────────────

/// Find text matching `patterns`; regions in top-origin points.
pub fn find_text_regions(
    pdf: &[u8],
    patterns: &[RedactionPattern],
) -> Result<Vec<RedactionRegion>, FormeError> {
    if patterns.is_empty() {
        return Ok(vec![]);
    }
    let matcher = Matcher::new(patterns)?;
    let doc = Document::parse(pdf)?;
    if doc.is_encrypted() {
        return Err(refuse("the PDF is encrypted"));
    }
    let pages = collect_pages(&doc)?;
    let mut regions = vec![];
    for (pi, page) in pages.iter().enumerate() {
        page.check_geometry()?;
        let Ok(a) = analyze_page(&doc, page) else {
            continue;
        };
        let glyphs: Vec<Glyph> = a.interp_glyphs.into_iter().filter(|g| !g.nested).collect();
        let (text, owner) = page_text(&glyphs);
        for (s, e, color) in matcher.find(&text, Some(pi)) {
            // Group the matched glyphs by line into boxes.
            let mut boxes: Vec<Rect> = vec![];
            let mut last: Option<usize> = None;
            for gi in owner[s..e].iter().flatten() {
                if last == Some(*gi) {
                    continue;
                }
                last = Some(*gi);
                let r = glyphs[*gi].rect;
                match boxes.last_mut() {
                    Some(b) if (b.y0 - r.y0).abs() < (r.y1 - r.y0).abs() * 0.4 => *b = b.union(&r),
                    _ => boxes.push(r),
                }
            }
            for b in boxes {
                let d =
                    Rect::of_points(&[page.to_display(b.x0, b.y0), page.to_display(b.x1, b.y1)]);
                regions.push(RedactionRegion {
                    page: pi,
                    x: d.x0,
                    y: d.y0,
                    width: d.x1 - d.x0,
                    height: d.y1 - d.y0,
                    color: color.clone(),
                });
            }
        }
    }
    Ok(regions)
}

/// Redact `regions`, plus anything matching `patterns` outside page content
/// (annotations, bookmarks, structure tree, metadata). Always returns a full
/// rewrite of the document.
/// Returns the new PDF and whether anything was redacted or scrubbed.
pub fn redact(
    pdf: &[u8],
    regions: &[RedactionRegion],
    patterns: &[RedactionPattern],
) -> Result<(Vec<u8>, bool), FormeError> {
    let matcher = Matcher::new(patterns)?;
    let mut changed = !regions.is_empty();
    let mut doc = Document::parse(pdf)?;
    if doc.is_encrypted() {
        return Err(refuse("the PDF is encrypted; decrypt it first"));
    }
    let pages = collect_pages(&doc)?;
    if let Some(max) = regions.iter().map(|r| r.page).max() {
        if max >= pages.len() {
            return Err(FormeError::RenderError(format!(
                "Redaction references page {} but PDF only has {} pages",
                max,
                pages.len()
            )));
        }
    }

    if !matcher.is_empty() {
        let mut texts = vec![];
        if let Some(info) = dict_get(&doc.trailer, b"Info") {
            strings_in(&doc, doc.resolve(info), &mut texts, 0);
        }
        for o in doc.objects.values() {
            if let Obj::Stream(d, raw) = o {
                let t = dict_get(d, b"Type")
                    .and_then(|o| o.as_name())
                    .unwrap_or(b"");
                let st = dict_get(d, b"Subtype")
                    .and_then(|o| o.as_name())
                    .unwrap_or(b"");
                if t == b"EmbeddedFile" || (t == b"Metadata" && st == b"XML") {
                    if let Ok(data) = decode_stream(d, raw) {
                        texts.push(String::from_utf8_lossy(&data).into_owned());
                    }
                }
            }
        }
        if texts.iter().any(|t| matcher.matches(t)) {
            changed = true;
        }
    }

    let mut dropped_text: Vec<String> = vec![];
    let mut tainted_mcids: HashSet<(u32, i64)> = HashSet::new();
    // Regions per page (user space), for the annotation pass after all pages.
    let mut page_regions: HashMap<u32, Vec<Rect>> = HashMap::new();

    // ── Pages ──
    for (pi, page) in pages.iter().enumerate() {
        let rects = user_rects(regions, pi, page)?;
        let analysis = analyze_page(&doc, page);
        // Pattern matches inside forms (which this version does not rewrite).
        if !matcher.is_empty() {
            if let Ok(a) = &analysis {
                let nested: Vec<Glyph> = a
                    .interp_glyphs
                    .iter()
                    .filter(|g| g.nested)
                    .cloned()
                    .collect();
                if matcher.matches(&page_text(&nested).0) {
                    return Err(refuse("matching text is drawn inside a form XObject, which this version cannot rewrite"));
                }
            }
        }
        let page_props = dict_get(&page.resources, b"Properties")
            .and_then(|o| doc.resolve_dict(o))
            .cloned()
            .unwrap_or_default();
        let frame_sensitive = |a: &PageAnalysis, f: &Frame| -> bool {
            if matcher.is_empty() {
                return false;
            }
            let mut ss = vec![];
            if let Some(j) = f.operand_index {
                if let Tok::Operand(o, _) = &a.toks[j] {
                    strings_in(&doc, o, &mut ss, 0);
                }
            }
            if let Some(n) = &f.props_name {
                if let Some(v) = dict_get(&page_props, n) {
                    strings_in(&doc, doc.resolve(v), &mut ss, 0);
                }
            }
            ss.iter().any(|s| matcher.matches(s))
        };
        if rects.is_empty() {
            let Ok(a) = &analysis else { continue };
            if !a.frames.iter().any(|f| frame_sensitive(a, f)) {
                continue;
            }
        }
        let a = analysis?;
        let region_rects: Vec<Rect> = rects.iter().map(|(r, _)| *r).collect();

        let mut drop_ops: HashSet<usize> = HashSet::new();
        let mut replace: HashMap<usize, Vec<Tok>> = HashMap::new();
        // Forms are read, never rewritten: anything they draw under a
        // region is refused.
        let nested_hit = a.nested_marks.iter().any(|m| any_hit(m, &region_rects))
            || a.interp_glyphs
                .iter()
                .any(|g| g.nested && glyph_hit(&g.rect, &region_rects));
        if nested_hit {
            return Err(refuse("a redaction region covers content inside a form XObject, which this version cannot rewrite"));
        }
        for x in &a.xuses {
            if x.image && any_hit(&x.rect, &region_rects) {
                drop_ops.insert(x.op_index);
            }
        }
        // Vector paths: remove the geometry inside the regions.
        for pp in &a.paths {
            let Some(rep) = redact_path(&doc, &page.resources, pp, &region_rects)? else {
                continue;
            };
            for (s, e) in &pp.spans {
                for k in *s..=*e {
                    drop_ops.insert(k);
                }
            }
            drop_ops.insert(pp.paint_index);
            replace.insert(pp.paint_index, rep);
        }
        for (idx, r) in &a.inline_images {
            if any_hit(r, &region_rects) {
                drop_ops.insert(*idx);
            }
        }
        let mut frames_tainted: HashSet<usize> = HashSet::new();
        for s in &a.shows {
            let glyphs = &a.interp_glyphs[s.glyphs.0..s.glyphs.1];
            // Constructs this version does not rewrite: refused when they
            // may reach a region at all (any overlap, not the glyph rule).
            if s.type3 || s.clip_text {
                let near = !region_rects.is_empty()
                    && (s.unbounded || glyphs.iter().any(|g| any_hit(&g.rect, &region_rects)));
                if near && s.type3 {
                    return Err(refuse("text drawn with a Type3 font may lie under a redaction region; its glyph procedures cannot be rewritten by this version"));
                }
                if near && s.clip_text {
                    return Err(refuse("text used as a clipping path (rendering modes 4 to 7) lies under a redaction region, which this version cannot rewrite"));
                }
            }
            if !glyphs.iter().any(|g| glyph_hit(&g.rect, &region_rects)) {
                continue;
            }
            dropped_text.push(s.text.clone());
            for f in &s.open_frames {
                frames_tainted.insert(*f);
            }
            // Remove the operator and its operands; keep the advance.
            let first_operand = s.op_index - s.operands;
            for k in first_operand..=s.op_index {
                drop_ops.insert(k);
            }
            let mut rep = vec![];
            let op = match &a.toks[s.op_index] {
                Tok::Op(o) => o.clone(),
                _ => vec![],
            };
            if op == b"\"" {
                // aw ac string " → aw Tw ac Tc T*
                if let (Tok::Operand(aw, raw_aw), Tok::Operand(ac, raw_ac)) =
                    (&a.toks[first_operand], &a.toks[first_operand + 1])
                {
                    rep.push(Tok::Operand(aw.clone(), raw_aw.clone()));
                    rep.push(Tok::Op(b"Tw".to_vec()));
                    rep.push(Tok::Operand(ac.clone(), raw_ac.clone()));
                    rep.push(Tok::Op(b"Tc".to_vec()));
                }
                rep.push(Tok::Op(b"T*".to_vec()));
            } else if op == b"'" {
                rep.push(Tok::Op(b"T*".to_vec()));
            }
            let scale = s.fs * s.th;
            if scale.abs() > 1e-9 && s.advance.abs() > 1e-9 {
                let n = -(s.advance / scale) * 1000.0;
                let arr = Obj::Array(vec![Obj::Real(n)]);
                rep.push(Tok::Operand(arr.clone(), obj_bytes(&arr)));
                rep.push(Tok::Op(b"TJ".to_vec()));
            }
            replace.insert(s.op_index, rep);
        }

        // Marked content around removed text: drop alternate text.
        let mut toks = a.toks.clone();
        let mut prop_names_tainted: HashSet<Vec<u8>> = HashSet::new();
        for (fi, f) in a.frames.iter().enumerate() {
            if !(frames_tainted.contains(&fi) || f.tainted || frame_sensitive(&a, f)) {
                continue;
            }
            changed = true;
            if let Some(m) = f.mcid {
                tainted_mcids.insert((page.id, m));
            }
            if let Some(j) = f.operand_index {
                if let Tok::Operand(Obj::Dict(d), _) = &toks[j] {
                    let mut d2 = d.clone();
                    for k in [&b"ActualText"[..], b"Alt", b"E"] {
                        dict_remove(&mut d2, k);
                    }
                    let o = Obj::Dict(d2);
                    toks[j] = Tok::Operand(o.clone(), obj_bytes(&o));
                }
            }
            if let Some(n) = &f.props_name {
                prop_names_tainted.insert(n.clone());
            }
        }

        // New content: original (minus removed operators) inside q…Q, then
        // the opaque boxes in default user space.
        let mut kept: Vec<Tok> = vec![];
        for (i, t) in toks.iter().enumerate() {
            if let Some(rep) = replace.get(&i) {
                kept.extend(rep.iter().cloned());
                continue;
            }
            if !drop_ops.contains(&i) {
                kept.push(t.clone());
            }
        }
        let mut content = b"q\n".to_vec();
        content.extend_from_slice(&serialize(&kept));
        content.extend_from_slice(b"\nQ\n");
        for (r, color) in &rects {
            let (cr, cg, cb) =
                super::redaction::parse_hex_color(color.as_deref().unwrap_or("#000000"));
            let nums: Vec<String> = [cr, cg, cb, r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0]
                .iter()
                .map(|v| num(*v))
                .collect();
            content.extend_from_slice(
                format!(
                    "q {} {} {} rg {} {} {} {} re f Q\n",
                    nums[0], nums[1], nums[2], nums[3], nums[4], nums[5], nums[6]
                )
                .as_bytes(),
            );
        }
        let stream_id = doc.add(Obj::Stream(
            vec![(b"Filter".to_vec(), Obj::Name(b"FlateDecode".to_vec()))],
            flate(&content),
        ));

        // Page dictionary: new content, scrubbed properties, annotations.
        let mut pd = doc
            .get(page.id)
            .and_then(|o| o.as_dict())
            .cloned()
            .unwrap_or_default();
        dict_set(&mut pd, b"Contents", Obj::Ref(stream_id, 0));
        // Keep only XObjects the new content still draws, so removed images
        // (and forms) are no longer reachable and are not written.
        let mut res = page.resources.clone();
        let used: HashSet<Vec<u8>> = kept
            .windows(2)
            .filter_map(|w| match (&w[0], &w[1]) {
                (Tok::Operand(Obj::Name(n), _), Tok::Op(op)) if op == b"Do" => Some(n.clone()),
                _ => None,
            })
            .collect();
        if let Some(xo) = dict_get(&res, b"XObject")
            .and_then(|o| doc.resolve_dict(o))
            .cloned()
        {
            let pruned: Dict = xo.into_iter().filter(|(k, _)| used.contains(k)).collect();
            if pruned.is_empty() {
                dict_remove(&mut res, b"XObject");
            } else {
                dict_set(&mut res, b"XObject", Obj::Dict(pruned));
            }
        }
        {
            if let Some(props) = dict_get(&res, b"Properties")
                .and_then(|o| doc.resolve_dict(o))
                .cloned()
            {
                let mut props2 = props.clone();
                for (k, v) in props2.iter_mut() {
                    if prop_names_tainted.contains(k) {
                        if let Some(d) = doc.resolve_dict(v) {
                            let mut d2 = d.clone();
                            for key in [&b"ActualText"[..], b"Alt", b"E"] {
                                dict_remove(&mut d2, key);
                            }
                            *v = Obj::Dict(d2);
                        }
                    }
                }
                dict_set(&mut res, b"Properties", Obj::Dict(props2));
            }
        }
        dict_set(&mut pd, b"Resources", Obj::Dict(res));
        doc.objects.insert(page.id, Obj::Dict(pd));

        page_regions.insert(page.id, region_rects);
    }

    // ── Document level ──
    // "Sensitive": matches a pattern, or contains a word (4+ characters) of
    // text removed from a page — so hidden copies of removed text (link
    // URLs, notes, bookmarks, alternate text) go with it.
    let sensitive = |s: &str| -> bool {
        if matcher.matches(s) {
            return true;
        }
        dropped_text.iter().any(|d| {
            d.split_whitespace()
                .filter(|t| t.chars().count() >= 4)
                .any(|t| s.contains(t))
                || (d.trim().len() >= 4 && s.contains(d.trim()))
        })
    };
    // Annotations: over a region, or holding sensitive text — on every page,
    // after all pages so the removed text is known.
    for page in &pages {
        let rects = page_regions.get(&page.id).cloned().unwrap_or_default();
        changed |= filter_annotations(&mut doc, page.id, &rects, &sensitive)?;
    }
    changed |= scrub_document(&mut doc, &sensitive, &tainted_mcids)?;

    let root = dict_get(&doc.trailer, b"Root")
        .cloned()
        .ok_or_else(|| refuse("no catalog"))?;
    let info_id = doc.add(Obj::Dict(vec![(
        b"Producer".to_vec(),
        Obj::Str(b"Forme".to_vec()),
    )]));
    Ok((
        write_reachable(&doc, root, Some(Obj::Ref(info_id, 0)), pdf),
        changed,
    ))
}

/// Remove annotations on `page_id` that lie over a region, or whose text
/// matches. Widgets (form fields) are refused instead of removed.
fn filter_annotations(
    doc: &mut Document,
    page_id: u32,
    regions: &[Rect],
    sensitive: &dyn Fn(&str) -> bool,
) -> Result<bool, FormeError> {
    let Some(pd) = doc.get(page_id).and_then(|o| o.as_dict()).cloned() else {
        return Ok(false);
    };
    let Some(annots) = dict_get(&pd, b"Annots").map(|o| doc.resolve(o).clone()) else {
        return Ok(false);
    };
    let Obj::Array(list) = annots else {
        return Ok(false);
    };
    let mut keep = vec![];
    let mut removed: HashSet<u32> = HashSet::new();
    for a in &list {
        let Some(ad) = doc.resolve_dict(a).cloned() else {
            continue;
        };
        let sub = dict_get(&ad, b"Subtype")
            .and_then(|o| o.as_name())
            .unwrap_or(b"")
            .to_vec();
        let rect = match dict_get(&ad, b"Rect").map(|o| doc.resolve(o)) {
            Some(Obj::Array(v)) if v.len() == 4 => {
                let n: Vec<f64> = v
                    .iter()
                    .map(|o| doc.resolve(o).as_f64().unwrap_or(0.0))
                    .collect();
                Some(Rect {
                    x0: n[0].min(n[2]),
                    y0: n[1].min(n[3]),
                    x1: n[0].max(n[2]),
                    y1: n[1].max(n[3]),
                })
            }
            _ => None,
        };
        let over = rect.is_some_and(|r| any_hit(&r, regions));
        let mut texts = vec![];
        strings_in(doc, &Obj::Dict(ad.clone()), &mut texts, 0);
        let ap = appearance_text(doc, &ad);
        let ap_text = ap.as_ref().map(|s| s.as_str()).unwrap_or("");
        let text_hit = texts.iter().any(|s| sensitive(s)) || sensitive(ap_text);
        let embedded = matches!(
            sub.as_slice(),
            b"FileAttachment" | b"Sound" | b"Movie" | b"RichMedia" | b"3D" | b"Screen"
        );
        if sub == b"Widget" && (over || text_hit) {
            return Err(refuse("a form field lies under a redaction region or holds matching text; flatten the form first"));
        }
        if over && ap.is_err() {
            // Appearance we cannot read, over a region: removing is safe.
        }
        if over || text_hit || embedded {
            if let Obj::Ref(id, _) = a {
                removed.insert(*id);
            }
            continue;
        }
        keep.push(a.clone());
    }
    // Popups whose parent was removed.
    keep.retain(|a| {
        let parent = doc
            .resolve_dict(a)
            .and_then(|d| dict_get(d, b"Parent"))
            .and_then(|o| match o {
                Obj::Ref(id, _) => Some(*id),
                _ => None,
            });
        parent.is_none_or(|p| !removed.contains(&p))
    });
    if keep.len() != list.len() {
        let mut pd2 = pd.clone();
        if keep.is_empty() {
            dict_remove(&mut pd2, b"Annots");
        } else {
            dict_set(&mut pd2, b"Annots", Obj::Array(keep));
        }
        doc.objects.insert(page_id, Obj::Dict(pd2));
        return Ok(true);
    }
    Ok(false)
}

/// Remove document-level carriers of content and scrub what references the
/// redacted text: metadata, attachments, bookmarks, structure tree, form
/// field values.
fn scrub_document(
    doc: &mut Document,
    sensitive: &dyn Fn(&str) -> bool,
    tainted_mcids: &HashSet<(u32, i64)>,
) -> Result<bool, FormeError> {
    let mut hit = false;
    let Some(Obj::Ref(root_id, _)) = dict_get(&doc.trailer, b"Root").cloned() else {
        return Err(refuse("no catalog"));
    };
    let mut cat = doc
        .resolve_dict(&Obj::Ref(root_id, 0))
        .cloned()
        .unwrap_or_default();
    for k in [
        &b"Metadata"[..],
        b"AF",
        b"PieceInfo",
        b"SpiderInfo",
        b"AA",
        b"Collection",
        b"Perms",
        b"Legal",
    ] {
        dict_remove(&mut cat, k);
    }
    if let Some(oa) = dict_get(&cat, b"OpenAction").and_then(|o| doc.resolve_dict(o)) {
        if dict_get(oa, b"S").and_then(|o| o.as_name()) == Some(b"JavaScript") {
            dict_remove(&mut cat, b"OpenAction");
        }
    }
    if let Some(names) = dict_get(&cat, b"Names")
        .and_then(|o| doc.resolve_dict(o))
        .cloned()
    {
        let mut n2 = names;
        dict_remove(&mut n2, b"EmbeddedFiles");
        dict_remove(&mut n2, b"JavaScript");
        dict_set(&mut cat, b"Names", Obj::Dict(n2));
    }
    // Form fields: values that match are refused (flatten first); XFA data
    // is removed.
    if let Some(acro) = dict_get(&cat, b"AcroForm")
        .and_then(|o| doc.resolve_dict(o))
        .cloned()
    {
        let mut a2 = acro.clone();
        dict_remove(&mut a2, b"XFA");
        dict_remove(&mut a2, b"NeedsRendering");
        let mut field_strings = vec![];
        if let Some(fields) = dict_get(&acro, b"Fields") {
            let mut stack = vec![fields.clone()];
            let mut seen = HashSet::new();
            while let Some(o) = stack.pop() {
                if let Obj::Ref(id, _) = o {
                    if !seen.insert(id) {
                        continue;
                    }
                }
                match doc.resolve(&o) {
                    Obj::Array(a) => stack.extend(a.iter().cloned()),
                    Obj::Dict(d) => {
                        for k in [&b"V"[..], b"DV", b"T", b"TU", b"TM"] {
                            if let Some(v) = dict_get(d, k) {
                                strings_in(doc, doc.resolve(v), &mut field_strings, 0);
                            }
                        }
                        if let Some(kids) = dict_get(d, b"Kids") {
                            stack.push(kids.clone());
                        }
                    }
                    _ => {}
                }
                if seen.len() > 10_000 {
                    break;
                }
            }
        }
        if field_strings.iter().any(|s| sensitive(s)) {
            return Err(refuse(
                "a form field value holds matching text; flatten the form first",
            ));
        }
        dict_set(&mut cat, b"AcroForm", Obj::Dict(a2));
    }
    // Bookmarks.
    if let Some(Obj::Ref(oid, _)) = dict_get(&cat, b"Outlines").cloned() {
        let mut stack = vec![oid];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) || seen.len() > 100_000 {
                continue;
            }
            let Some(mut d) = doc.get(id).and_then(|o| o.as_dict()).cloned() else {
                continue;
            };
            for k in [&b"First"[..], b"Next"] {
                if let Some(Obj::Ref(n, _)) = dict_get(&d, k) {
                    stack.push(*n);
                }
            }
            if let Some(Obj::Str(t)) = dict_get(&d, b"Title").map(|o| doc.resolve(o)) {
                if sensitive(&decode_text_string(t)) {
                    dict_set(&mut d, b"Title", Obj::Str(b"[Redacted]".to_vec()));
                    hit = true;
                }
            }
            // Actions on bookmarks can carry text (URIs, JavaScript).
            if let Some(a) = dict_get(&d, b"A").map(|o| doc.resolve(o).clone()) {
                let mut ss = vec![];
                strings_in(doc, &a, &mut ss, 0);
                if ss.iter().any(|s| sensitive(s)) {
                    dict_remove(&mut d, b"A");
                    hit = true;
                }
            }
            doc.objects.insert(id, Obj::Dict(d));
        }
    }
    // Structure tree: alternate text tied to removed content, or matching.
    if let Some(Obj::Ref(sid, _)) = dict_get(&cat, b"StructTreeRoot").cloned() {
        let mut stack = vec![sid];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) || seen.len() > 200_000 {
                continue;
            }
            let Some(mut d) = doc.get(id).and_then(|o| o.as_dict()).cloned() else {
                continue;
            };
            let pg = match dict_get(&d, b"Pg") {
                Some(Obj::Ref(p, _)) => Some(*p),
                _ => None,
            };
            let mut touches_tainted = false;
            let mut kids = vec![];
            if let Some(k) = dict_get(&d, b"K") {
                let items: Vec<Obj> = match doc.resolve(k) {
                    Obj::Array(a) => a.clone(),
                    other => vec![if matches!(k, Obj::Ref(..)) {
                        k.clone()
                    } else {
                        other.clone()
                    }],
                };
                for it in items {
                    match &it {
                        Obj::Int(m) => {
                            if pg.is_some_and(|p| tainted_mcids.contains(&(p, *m))) {
                                touches_tainted = true;
                            }
                        }
                        Obj::Ref(kid, _) => {
                            if let Some(kd) = doc.get(*kid).and_then(|o| o.as_dict()) {
                                if let Some(m) = dict_get(kd, b"MCID").and_then(|o| o.as_f64()) {
                                    let p = match dict_get(kd, b"Pg") {
                                        Some(Obj::Ref(p, _)) => Some(*p),
                                        _ => pg,
                                    };
                                    if p.is_some_and(|p| tainted_mcids.contains(&(p, m as i64))) {
                                        touches_tainted = true;
                                    }
                                } else {
                                    kids.push(*kid);
                                }
                            }
                        }
                        Obj::Dict(kd) => {
                            if let Some(m) = dict_get(kd, b"MCID").and_then(|o| o.as_f64()) {
                                let p = match dict_get(kd, b"Pg") {
                                    Some(Obj::Ref(p, _)) => Some(*p),
                                    _ => pg,
                                };
                                if p.is_some_and(|p| tainted_mcids.contains(&(p, m as i64))) {
                                    touches_tainted = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            stack.extend(kids);
            let mut changed = false;
            for k in [&b"ActualText"[..], b"Alt", b"E", b"T"] {
                let hit = match dict_get(&d, k).map(|o| doc.resolve(o)) {
                    Some(Obj::Str(s)) => {
                        touches_tainted && k != b"T" || sensitive(&decode_text_string(s))
                    }
                    _ => false,
                };
                if hit {
                    dict_remove(&mut d, k);
                    changed = true;
                }
            }
            if changed {
                hit = true;
                doc.objects.insert(id, Obj::Dict(d));
            }
        }
    }
    doc.objects.insert(root_id, Obj::Dict(cat));

    // Everywhere: XMP metadata streams, associated/embedded files, private
    // application data, thumbnails, additional actions.
    let ids: Vec<u32> = doc.objects.keys().copied().collect();
    for id in ids {
        if let Some(d) = doc.objects.get_mut(&id).and_then(|o| o.as_dict_mut()) {
            for k in [&b"Metadata"[..], b"AF", b"PieceInfo", b"Thumb", b"AA"] {
                dict_remove(d, k);
            }
        }
    }
    Ok(hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_keeps_inline_images_whole() {
        let toks = lex(b"q BI /W 2 /H 1 /CS /G /BPC 8 ID \x28\x5b EI Q");
        assert!(matches!(toks[1], Tok::InlineImage(_)), "{toks:?}");
        assert!(matches!(&toks[2], Tok::Op(o) if o == b"Q"));
    }

    #[test]
    fn matrix_math() {
        let m = mul(
            &[2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
        );
        assert_eq!(apply(&m, 1.0, 1.0), (12.0, 22.0));
    }
}
