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
    /// Upper-bound extent in user space.
    extent: Rect,
    /// Total advance in text space (for the replacement).
    advance: f64,
    fs: f64,
    th: f64,
    text: String,
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
    /// For forms: contains text or images somewhere inside.
    form_has_content: bool,
}

struct Interp<'a> {
    doc: &'a Document,
    fonts: HashMap<Vec<u8>, Font>,
    glyphs: Vec<Glyph>,
    shows: Vec<ShowOp>,
    xuses: Vec<XUse>,
    inline_images: Vec<(usize, Rect)>,
    frames: Vec<Frame>,
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
        };
        let mut stack: Vec<Gs> = vec![];
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
                    match op.as_slice() {
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
                            let start_tm = tm;
                            let mut adv_total = 0.0;
                            let mut upper_total = 0.0;
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
                                            let gr = Rect::transformed(
                                                &trm,
                                                0.0,
                                                lo,
                                                adv.max(upper),
                                                hi,
                                            );
                                            let u = font.unicode(code);
                                            text.push_str(&u);
                                            self.glyphs.push(Glyph {
                                                text: u,
                                                rect: gr,
                                                nested,
                                            });
                                            tm = mul(&[1.0, 0.0, 0.0, 1.0, adv, 0.0], &tm);
                                            adv_total += adv;
                                            upper_total += upper.max(adv);
                                        }
                                    }
                                    o => {
                                        if let Some(n) = o.as_f64() {
                                            let adj = -n / 1000.0 * gs.fs * gs.th;
                                            tm = mul(&[1.0, 0.0, 0.0, 1.0, adj, 0.0], &tm);
                                            adv_total += adj;
                                            upper_total += adj.max(0.0);
                                        }
                                    }
                                }
                            }
                            if !parts.is_empty() {
                                drew = true;
                            }
                            if !nested {
                                let trm = mul(&start_tm, &gs.ctm);
                                let lo = gs.ts - DESCENT * gs.fs.abs();
                                let hi = gs.ts + ASCENT * gs.fs.abs();
                                let extent = Rect::transformed(
                                    &trm,
                                    adv_total.min(0.0),
                                    lo,
                                    upper_total.max(adv_total).max(0.0),
                                    hi,
                                );
                                let n_operands = match op.as_slice() {
                                    b"\"" => 3,
                                    _ => 1,
                                };
                                self.shows.push(ShowOp {
                                    op_index: i,
                                    operands: n_operands.min(ops.len()),
                                    extent,
                                    advance: adv_total,
                                    fs: gs.fs,
                                    th: gs.th,
                                    text,
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
                                            if !nested {
                                                self.xuses.push(XUse {
                                                    op_index: i,
                                                    rect: Rect::transformed(
                                                        &gs.ctm, 0.0, 0.0, 1.0, 1.0,
                                                    ),
                                                    image: true,
                                                    form_has_content: true,
                                                });
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
                                                // Unreadable form: assume it may hold content.
                                                Err(_) => true,
                                            };
                                            drew |= has;
                                            if !nested {
                                                self.xuses.push(XUse {
                                                    op_index: i,
                                                    rect: bbox,
                                                    image: false,
                                                    form_has_content: has,
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

// ─────────────────────────────── page model ───────────────────────────────

struct PageRef {
    id: u32,
    media: Rect,
    resources: Dict,
}

fn collect_pages(doc: &Document) -> Result<Vec<PageRef>, FormeError> {
    let root = doc
        .resolve_dict(dict_get(&doc.trailer, b"Root").ok_or_else(|| refuse("no catalog"))?)
        .ok_or_else(|| refuse("no catalog"))?;
    let pages = dict_get(root, b"Pages").ok_or_else(|| refuse("no page tree"))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    fn walk(
        doc: &Document,
        node: &Obj,
        inh_res: Option<Dict>,
        inh_media: Option<Rect>,
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
        let res = dict_get(d, b"Resources")
            .and_then(|o| doc.resolve_dict(o))
            .cloned()
            .or(inh_res);
        let media = match dict_get(d, b"MediaBox").map(|o| doc.resolve(o)) {
            Some(Obj::Array(a)) if a.len() == 4 => {
                let v: Vec<f64> = a
                    .iter()
                    .map(|o| doc.resolve(o).as_f64().unwrap_or(0.0))
                    .collect();
                Some(Rect {
                    x0: v[0].min(v[2]),
                    y0: v[1].min(v[3]),
                    x1: v[0].max(v[2]),
                    y1: v[1].max(v[3]),
                })
            }
            _ => inh_media,
        };
        match dict_get(d, b"Type").and_then(|o| o.as_name()) {
            Some(b"Pages") => {
                if let Some(Obj::Array(kids)) = dict_get(d, b"Kids").map(|o| doc.resolve(o)) {
                    for k in kids {
                        walk(doc, k, res.clone(), media, out, seen, depth + 1);
                    }
                }
            }
            _ => out.push(PageRef {
                id: *id,
                media: media.unwrap_or(Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 612.0,
                    y1: 792.0,
                }),
                resources: res.unwrap_or_default(),
            }),
        }
    }
    walk(doc, pages, None, None, &mut out, &mut seen, 0);
    Ok(out)
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

/// Regions on a page, in user space.
fn user_rects(
    regions: &[RedactionRegion],
    page_idx: usize,
    media: &Rect,
) -> Vec<(Rect, Option<String>)> {
    regions
        .iter()
        .filter(|r| r.page == page_idx)
        .map(|r| {
            let x0 = media.x0 + r.x;
            let y1 = media.y1 - r.y;
            (
                Rect {
                    x0,
                    y0: y1 - r.height,
                    x1: x0 + r.width,
                    y1,
                },
                r.color.clone(),
            )
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
                regions.push(RedactionRegion {
                    page: pi,
                    x: b.x0 - page.media.x0,
                    y: page.media.y1 - b.y1,
                    width: b.x1 - b.x0,
                    height: b.y1 - b.y0,
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
        let rects = user_rects(regions, pi, &page.media);
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
        for x in &a.xuses {
            if !any_hit(&x.rect, &region_rects) {
                continue;
            }
            if x.image {
                drop_ops.insert(x.op_index);
            } else if x.form_has_content {
                return Err(refuse("a redaction region covers text or images inside a form XObject, which this version cannot rewrite"));
            }
        }
        for (idx, r) in &a.inline_images {
            if any_hit(r, &region_rects) {
                drop_ops.insert(*idx);
            }
        }
        let mut frames_tainted: HashSet<usize> = HashSet::new();
        for s in &a.shows {
            if !any_hit(&s.extent, &region_rects) {
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
