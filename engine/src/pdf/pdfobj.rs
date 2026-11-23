//! # Minimal PDF object model: reader and full-rewrite writer
//!
//! Used by redaction, where two properties are security requirements:
//!
//! 1. **Read the document as it currently is.** Every object resolves to its
//!    latest definition: classic xref tables with `/Prev` chains, xref
//!    streams (with PNG predictors), object streams (`/ObjStm`) and hybrid
//!    files (`/XRefStm`). If the cross-reference data is missing or damaged,
//!    a header scan is used and the LAST definition of each object wins.
//! 2. **Write a self-contained file.** [`write_reachable`] serializes only
//!    the objects reachable from the new catalog (and `/Info`), renumbered,
//!    as a single revision with one classic xref and no `/Prev`. Earlier
//!    revisions, unreferenced objects and object streams from the input are
//!    never copied.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib;

use crate::error::FormeError;

/// An ordered PDF dictionary.
pub type Dict = Vec<(Vec<u8>, Obj)>;

#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(Vec<u8>),
    Name(Vec<u8>),
    Array(Vec<Obj>),
    Dict(Dict),
    Ref(u32, u16),
    /// Dictionary plus the raw (still encoded) stream bytes.
    Stream(Dict, Vec<u8>),
}

fn err(msg: impl Into<String>) -> FormeError {
    FormeError::RenderError(msg.into())
}

// ─────────────────────────── dictionary helpers ───────────────────────────

pub fn dict_get<'a>(d: &'a Dict, key: &[u8]) -> Option<&'a Obj> {
    d.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub fn dict_set(d: &mut Dict, key: &[u8], v: Obj) {
    if let Some(e) = d.iter_mut().find(|(k, _)| k == key) {
        e.1 = v;
    } else {
        d.push((key.to_vec(), v));
    }
}

pub fn dict_remove(d: &mut Dict, key: &[u8]) -> Option<Obj> {
    let i = d.iter().position(|(k, _)| k == key)?;
    Some(d.remove(i).1)
}

impl Obj {
    pub fn as_dict(&self) -> Option<&Dict> {
        match self {
            Obj::Dict(d) | Obj::Stream(d, _) => Some(d),
            _ => None,
        }
    }
    pub fn as_dict_mut(&mut self) -> Option<&mut Dict> {
        match self {
            Obj::Dict(d) | Obj::Stream(d, _) => Some(d),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Obj::Int(i) => Some(*i as f64),
            Obj::Real(r) => Some(*r),
            _ => None,
        }
    }
    pub fn as_name(&self) -> Option<&[u8]> {
        match self {
            Obj::Name(n) => Some(n),
            _ => None,
        }
    }
}

// ─────────────────────────────── lexer/parser ───────────────────────────────

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Recursive-descent parser over a byte slice.
pub struct Parser<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Self {
        Parser { data, pos }
    }

    pub fn skip_ws(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                while self.pos < self.data.len() && !matches!(self.data[self.pos], b'\r' | b'\n') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn regular_token(&mut self) -> &'a [u8] {
        let start = self.pos;
        while self.pos < self.data.len()
            && !is_ws(self.data[self.pos])
            && !is_delim(self.data[self.pos])
        {
            self.pos += 1;
        }
        &self.data[start..self.pos]
    }

    /// Parse one object. Indirect references `N G R` are recognized here.
    pub fn parse(&mut self) -> Result<Obj, FormeError> {
        self.parse_depth(0)
    }

    fn parse_depth(&mut self, depth: usize) -> Result<Obj, FormeError> {
        if depth > 256 {
            return Err(err("PDF object nesting too deep"));
        }
        self.skip_ws();
        let b = self
            .peek()
            .ok_or_else(|| err("unexpected end of PDF data"))?;
        match b {
            b'/' => {
                self.pos += 1;
                let raw = self.regular_token();
                Ok(Obj::Name(decode_name(raw)))
            }
            b'(' => Ok(Obj::Str(self.literal_string()?)),
            b'<' if self.data.get(self.pos + 1) == Some(&b'<') => {
                self.pos += 2;
                let mut d = Dict::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b'>') if self.data.get(self.pos + 1) == Some(&b'>') => {
                            self.pos += 2;
                            break;
                        }
                        Some(b'/') => {
                            self.pos += 1;
                            let k = decode_name(self.regular_token());
                            let v = self.parse_depth(depth + 1)?;
                            d.push((k, v));
                        }
                        None => return Err(err("unterminated dictionary")),
                        _ => {
                            // Malformed entry: skip one token to make progress.
                            let _ = self.parse_depth(depth + 1)?;
                        }
                    }
                }
                Ok(Obj::Dict(d))
            }
            b'<' => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.data.len() && self.data[self.pos] != b'>' {
                    self.pos += 1;
                }
                let hex = &self.data[start..self.pos];
                self.pos += 1;
                Ok(Obj::Str(decode_hex(hex)))
            }
            b'[' => {
                self.pos += 1;
                let mut a = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        None => return Err(err("unterminated array")),
                        _ => a.push(self.parse_depth(depth + 1)?),
                    }
                }
                Ok(Obj::Array(fold_refs(a)))
            }
            _ => {
                let tok = self.regular_token();
                if tok.is_empty() {
                    self.pos += 1;
                    return Ok(Obj::Null);
                }
                match tok {
                    b"true" => return Ok(Obj::Bool(true)),
                    b"false" => return Ok(Obj::Bool(false)),
                    b"null" => return Ok(Obj::Null),
                    _ => {}
                }
                let s = std::str::from_utf8(tok).unwrap_or("");
                if let Ok(i) = s.parse::<i64>() {
                    // Possible reference: N G R
                    let save = self.pos;
                    self.skip_ws();
                    let g = self.regular_token();
                    if let Ok(gen) = std::str::from_utf8(g).unwrap_or("x").parse::<u16>() {
                        self.skip_ws();
                        if self.peek() == Some(b'R')
                            && self
                                .data
                                .get(self.pos + 1)
                                .is_none_or(|c| is_ws(*c) || is_delim(*c))
                        {
                            self.pos += 1;
                            return Ok(Obj::Ref(i.max(0) as u32, gen));
                        }
                    }
                    self.pos = save;
                    return Ok(Obj::Int(i));
                }
                if let Ok(f) = s.parse::<f64>() {
                    return Ok(Obj::Real(f));
                }
                // An operator-like keyword inside object syntax: treat as name-less null.
                Ok(Obj::Null)
            }
        }
    }

    fn literal_string(&mut self) -> Result<Vec<u8>, FormeError> {
        self.pos += 1; // (
        let mut out = Vec::new();
        let mut depth = 1;
        while self.pos < self.data.len() {
            let c = self.data[self.pos];
            self.pos += 1;
            match c {
                b'\\' => {
                    let Some(&n) = self.data.get(self.pos) else {
                        break;
                    };
                    self.pos += 1;
                    match n {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = (n - b'0') as u32;
                            for _ in 0..2 {
                                match self.data.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        b'\r' => {
                            if self.data.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        Err(err("unterminated string"))
    }
}

/// `[1 0 R]` parses as Int Int Null(R)… — the parser already folds N G R at
/// the scalar level, so arrays need no extra work. Kept as a hook.
fn fold_refs(a: Vec<Obj>) -> Vec<Obj> {
    a
}

fn decode_name(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'#' && i + 2 < raw.len() {
            if let (Some(h), Some(l)) = (
                hexval(raw.get(i + 1).copied()),
                hexval(raw.get(i + 2).copied()),
            ) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}

fn hexval(b: Option<u8>) -> Option<u8> {
    let b = b?;
    (b as char).to_digit(16).map(|d| d as u8)
}

pub fn decode_hex(hex: &[u8]) -> Vec<u8> {
    let digits: Vec<u8> = hex
        .iter()
        .copied()
        .filter(|b| b.is_ascii_hexdigit())
        .collect();
    let mut out = Vec::with_capacity(digits.len() / 2 + 1);
    let mut i = 0;
    while i < digits.len() {
        let h = hexval(Some(digits[i])).unwrap_or(0);
        let l = if i + 1 < digits.len() {
            hexval(Some(digits[i + 1])).unwrap_or(0)
        } else {
            0
        };
        out.push(h * 16 + l);
        i += 2;
    }
    out
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from >= h.len() || n.is_empty() || n.len() > h.len() {
        return None;
    }
    h[from..]
        .windows(n.len())
        .position(|w| w == n)
        .map(|p| p + from)
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    if n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len())
        .rev()
        .find(|&i| &h[i..i + n.len()] == n)
}

// ───────────────────────────────── filters ─────────────────────────────────

fn filters_of(dict: &Dict) -> Vec<Vec<u8>> {
    match dict_get(dict, b"Filter") {
        Some(Obj::Name(n)) => vec![n.clone()],
        Some(Obj::Array(a)) => a
            .iter()
            .filter_map(|o| o.as_name().map(|n| n.to_vec()))
            .collect(),
        _ => vec![],
    }
}

fn decode_parms_of(dict: &Dict, idx: usize) -> Option<Dict> {
    match dict_get(dict, b"DecodeParms").or_else(|| dict_get(dict, b"DP")) {
        Some(Obj::Dict(d)) if idx == 0 => Some(d.clone()),
        Some(Obj::Array(a)) => a.get(idx).and_then(|o| o.as_dict().cloned()),
        _ => None,
    }
}

/// Decode stream data through its filters. Supports FlateDecode (with PNG and
/// TIFF-2 predictors), ASCIIHexDecode and ASCII85Decode. Other filters
/// (image codecs, LZW, RunLength) return an error: callers that must read
/// the content treat that as "cannot redact safely".
pub fn decode_stream(dict: &Dict, raw: &[u8]) -> Result<Vec<u8>, FormeError> {
    let mut data = raw.to_vec();
    for (i, f) in filters_of(dict).iter().enumerate() {
        data = match f.as_slice() {
            b"FlateDecode" | b"Fl" => {
                let inflated = decompress_to_vec_zlib(&data)
                    .or_else(|_| miniz_oxide::inflate::decompress_to_vec(&data))
                    .map_err(|e| err(format!("FlateDecode failed: {e:?}")))?;
                apply_predictor(inflated, decode_parms_of(dict, i).as_ref())?
            }
            b"ASCIIHexDecode" | b"AHx" => {
                let end = data.iter().position(|&b| b == b'>').unwrap_or(data.len());
                decode_hex(&data[..end])
            }
            b"ASCII85Decode" | b"A85" => ascii85(&data)?,
            other => {
                return Err(err(format!(
                    "unsupported stream filter /{}",
                    String::from_utf8_lossy(other)
                )))
            }
        };
    }
    Ok(data)
}

fn apply_predictor(data: Vec<u8>, parms: Option<&Dict>) -> Result<Vec<u8>, FormeError> {
    let Some(p) = parms else { return Ok(data) };
    let predictor = dict_get(p, b"Predictor")
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0) as i64;
    if predictor < 2 {
        return Ok(data);
    }
    let colors = dict_get(p, b"Colors")
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0) as usize;
    let bpc = dict_get(p, b"BitsPerComponent")
        .and_then(|o| o.as_f64())
        .unwrap_or(8.0) as usize;
    let columns = dict_get(p, b"Columns")
        .and_then(|o| o.as_f64())
        .unwrap_or(1.0) as usize;
    let bpp = (colors * bpc).div_ceil(8);
    let row_len = (colors * bpc * columns).div_ceil(8);
    if predictor == 2 {
        let mut out = data;
        if bpc == 8 {
            for row in out.chunks_mut(row_len.max(1)) {
                for i in bpp..row.len() {
                    row[i] = row[i].wrapping_add(row[i - bpp]);
                }
            }
        }
        return Ok(out);
    }
    // PNG predictors: each row prefixed with a filter-type byte.
    let mut out = Vec::with_capacity(data.len());
    let mut prev = vec![0u8; row_len];
    for chunk in data.chunks(row_len + 1) {
        if chunk.is_empty() {
            break;
        }
        let ft = chunk[0];
        let mut row: Vec<u8> = chunk[1..].to_vec();
        row.resize(row_len, 0);
        for i in 0..row_len {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            row[i] = match ft {
                0 => row[i],
                1 => row[i].wrapping_add(a),
                2 => row[i].wrapping_add(b),
                3 => row[i].wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => {
                    let p = a as i16 + b as i16 - c as i16;
                    let (pa, pb, pc) = (
                        (p - a as i16).abs(),
                        (p - b as i16).abs(),
                        (p - c as i16).abs(),
                    );
                    let pred = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    row[i].wrapping_add(pred)
                }
                _ => return Err(err("invalid PNG predictor")),
            };
        }
        out.extend_from_slice(&row);
        prev = row;
    }
    Ok(out)
}

fn ascii85(data: &[u8]) -> Result<Vec<u8>, FormeError> {
    let mut out = Vec::new();
    let mut group = [0u32; 5];
    let mut n = 0;
    let body = match find(data, b"~>", 0) {
        Some(e) => &data[..e],
        None => data,
    };
    let body = body.strip_prefix(b"<~").unwrap_or(body);
    for &c in body {
        if is_ws(c) {
            continue;
        }
        if c == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&c) {
            return Err(err("invalid ASCII85 data"));
        }
        group[n] = (c - b'!') as u32;
        n += 1;
        if n == 5 {
            let v = group.iter().fold(0u64, |acc, &d| acc * 85 + d as u64) as u32;
            out.extend_from_slice(&v.to_be_bytes());
            n = 0;
        }
    }
    if n > 0 {
        for g in group.iter_mut().skip(n) {
            *g = 84;
        }
        let v = group.iter().fold(0u64, |acc, &d| acc * 85 + d as u64) as u32;
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    Ok(out)
}

// ───────────────────────────────── document ─────────────────────────────────

#[derive(Clone, Copy, Debug)]
enum Loc {
    Offset(usize),
    InStream(u32, usize),
}

/// A parsed PDF: every live object at its latest definition.
pub struct Document {
    pub objects: BTreeMap<u32, Obj>,
    pub trailer: Dict,
}

impl Document {
    pub fn parse(data: &[u8]) -> Result<Document, FormeError> {
        if find(data, b"%PDF-", 0).is_none_or(|p| p > 1024) {
            return Err(err("not a PDF file"));
        }
        let (locs, trailer) = match read_xref_chain(data) {
            Ok((locs, trailer)) if !locs.is_empty() && dict_get(&trailer, b"Root").is_some() => {
                (locs, trailer)
            }
            _ => scan_objects(data)?,
        };
        let mut objects = BTreeMap::new();
        let mut objstm_cache: HashMap<u32, Vec<(u32, Obj)>> = HashMap::new();
        let mut direct: BTreeMap<u32, Loc> = BTreeMap::new();
        for (&id, &loc) in &locs {
            direct.insert(id, loc);
        }
        // First pass: plain objects (object streams are themselves plain).
        for (&id, loc) in &direct {
            if let Loc::Offset(off) = loc {
                if let Some(o) = parse_indirect_at(data, *off, id, &locs) {
                    objects.insert(id, o);
                } else if let Some(o) = scan_for_object(data, id, &locs) {
                    objects.insert(id, o);
                }
            }
        }
        // Second pass: objects inside object streams.
        for (&id, loc) in &direct {
            if let Loc::InStream(stm, idx) = loc {
                if !objstm_cache.contains_key(stm) {
                    let members = match objects.get(stm) {
                        Some(Obj::Stream(d, raw)) => expand_objstm(d, raw).unwrap_or_default(),
                        _ => vec![],
                    };
                    objstm_cache.insert(*stm, members);
                }
                let members = &objstm_cache[stm];
                let found = members
                    .iter()
                    .find(|(mid, _)| *mid == id)
                    .or_else(|| members.get(*idx));
                if let Some((mid, o)) = found {
                    if *mid == id {
                        objects.insert(id, o.clone());
                    }
                }
            }
        }
        Ok(Document { objects, trailer })
    }

    pub fn get(&self, id: u32) -> Option<&Obj> {
        self.objects.get(&id)
    }

    /// Follow references (bounded) to a direct object.
    pub fn resolve<'a>(&'a self, mut o: &'a Obj) -> &'a Obj {
        for _ in 0..32 {
            match o {
                Obj::Ref(id, _) => match self.objects.get(id) {
                    Some(next) => o = next,
                    None => return &Obj::Null,
                },
                _ => return o,
            }
        }
        &Obj::Null
    }

    pub fn resolve_dict<'a>(&'a self, o: &'a Obj) -> Option<&'a Dict> {
        self.resolve(o).as_dict()
    }

    pub fn is_encrypted(&self) -> bool {
        dict_get(&self.trailer, b"Encrypt").is_some()
    }

    pub fn next_id(&self) -> u32 {
        self.objects.keys().next_back().copied().unwrap_or(0) + 1
    }

    /// Add an object and return its id.
    pub fn add(&mut self, o: Obj) -> u32 {
        let id = self.next_id();
        self.objects.insert(id, o);
        id
    }
}

fn parse_indirect_at(data: &[u8], off: usize, id: u32, locs: &BTreeMap<u32, Loc>) -> Option<Obj> {
    if off >= data.len() {
        return None;
    }
    let mut p = Parser::new(data, off);
    p.skip_ws();
    let n = p.regular_token();
    if std::str::from_utf8(n).ok()?.parse::<u32>().ok()? != id {
        return None;
    }
    p.skip_ws();
    let _gen = p.regular_token();
    p.skip_ws();
    if p.regular_token() != b"obj" {
        return None;
    }
    parse_object_body(&mut p, data, locs)
}

/// After `N G obj`: the value, and a stream body when present.
fn parse_object_body(p: &mut Parser, data: &[u8], locs: &BTreeMap<u32, Loc>) -> Option<Obj> {
    let v = p.parse().ok()?;
    let save = p.pos;
    p.skip_ws();
    if let Obj::Dict(d) = &v {
        if data[p.pos..].starts_with(b"stream") {
            let mut start = p.pos + 6;
            if data.get(start) == Some(&b'\r') {
                start += 1;
            }
            if data.get(start) == Some(&b'\n') {
                start += 1;
            }
            let declared = match dict_get(d, b"Length") {
                Some(Obj::Int(n)) if *n >= 0 => Some(*n as usize),
                Some(Obj::Ref(rid, _)) => match locs.get(rid) {
                    Some(Loc::Offset(o)) => {
                        let mut lp = Parser::new(data, *o);
                        lp.skip_ws();
                        lp.regular_token();
                        lp.skip_ws();
                        lp.regular_token();
                        lp.skip_ws();
                        lp.regular_token();
                        lp.parse().ok().and_then(|o| o.as_f64()).map(|f| f as usize)
                    }
                    _ => None,
                },
                _ => None,
            };
            let end = match declared {
                Some(len)
                    if start + len <= data.len() && {
                        let mut q = start + len;
                        while q < data.len() && is_ws(data[q]) {
                            q += 1;
                        }
                        data[q..].starts_with(b"endstream")
                    } =>
                {
                    start + len
                }
                _ => {
                    let e = find(data, b"endstream", start)?;
                    let mut e2 = e;
                    if e2 > start && data[e2 - 1] == b'\n' {
                        e2 -= 1;
                    }
                    if e2 > start && data[e2 - 1] == b'\r' {
                        e2 -= 1;
                    }
                    e2
                }
            };
            return Some(Obj::Stream(d.clone(), data[start..end].to_vec()));
        }
    }
    p.pos = save;
    Some(v)
}

fn expand_objstm(d: &Dict, raw: &[u8]) -> Result<Vec<(u32, Obj)>, FormeError> {
    let data = decode_stream(d, raw)?;
    let n = dict_get(d, b"N").and_then(|o| o.as_f64()).unwrap_or(0.0) as usize;
    let first = dict_get(d, b"First")
        .and_then(|o| o.as_f64())
        .unwrap_or(0.0) as usize;
    let mut p = Parser::new(&data, 0);
    let mut pairs = Vec::new();
    for _ in 0..n {
        let id = p.parse()?.as_f64().unwrap_or(-1.0);
        let off = p.parse()?.as_f64().unwrap_or(-1.0);
        if id < 0.0 || off < 0.0 {
            break;
        }
        pairs.push((id as u32, off as usize));
    }
    let mut out = Vec::new();
    for (id, off) in pairs {
        let mut q = Parser::new(&data, first + off);
        if let Ok(o) = q.parse() {
            out.push((id, o));
        }
    }
    Ok(out)
}

/// Follow startxref → xref sections (tables or streams) → /Prev, keeping the
/// FIRST location seen for each object (the newest section is read first).
fn read_xref_chain(data: &[u8]) -> Result<(BTreeMap<u32, Loc>, Dict), FormeError> {
    let sx = rfind(data, b"startxref").ok_or_else(|| err("no startxref"))?;
    let mut p = Parser::new(data, sx + 9);
    let mut off = p.parse()?.as_f64().ok_or_else(|| err("bad startxref"))? as usize;
    let mut locs: BTreeMap<u32, Loc> = BTreeMap::new();
    let mut trailer: Option<Dict> = None;
    let mut seen = HashSet::new();
    let empty = BTreeMap::new();
    while seen.insert(off) && off < data.len() {
        let mut q = Parser::new(data, off);
        q.skip_ws();
        let sect_trailer: Dict;
        if data[q.pos..].starts_with(b"xref") {
            q.pos += 4;
            loop {
                q.skip_ws();
                if data[q.pos..].starts_with(b"trailer") {
                    q.pos += 7;
                    break;
                }
                let start = q
                    .parse()?
                    .as_f64()
                    .ok_or_else(|| err("bad xref subsection"))? as u32;
                let count = q
                    .parse()?
                    .as_f64()
                    .ok_or_else(|| err("bad xref subsection"))? as u32;
                for i in 0..count {
                    q.skip_ws();
                    let line_off = std::str::from_utf8(q.regular_token())
                        .unwrap_or("0")
                        .parse::<usize>()
                        .unwrap_or(0);
                    q.skip_ws();
                    q.regular_token();
                    q.skip_ws();
                    let kind = q.regular_token();
                    let id = start + i;
                    locs.entry(id).or_insert_with(|| {
                        if kind == b"n" && line_off > 0 {
                            Loc::Offset(line_off)
                        } else {
                            // Free in the newest section: it must not be
                            // resurrected from an older one.
                            Loc::Offset(usize::MAX)
                        }
                    });
                }
            }
            sect_trailer = match q.parse()? {
                Obj::Dict(d) => d,
                _ => return Err(err("bad trailer")),
            };
            if let Some(stm) = dict_get(&sect_trailer, b"XRefStm").and_then(|o| o.as_f64()) {
                if let Ok((_, extra)) = read_xref_stream(data, stm as usize, &empty) {
                    for (id, l) in extra {
                        locs.entry(id).or_insert(l);
                    }
                }
            }
        } else {
            let (d, entries) = read_xref_stream(data, off, &empty)?;
            for (id, l) in entries {
                locs.entry(id).or_insert(l);
            }
            sect_trailer = d;
        }
        if trailer.is_none() {
            trailer = Some(sect_trailer.clone());
        }
        match dict_get(&sect_trailer, b"Prev").and_then(|o| o.as_f64()) {
            Some(prev) => off = prev as usize,
            None => break,
        }
    }
    locs.retain(|_, l| !matches!(l, Loc::Offset(usize::MAX)));
    let mut trailer = trailer.ok_or_else(|| err("no trailer"))?;
    for k in [
        &b"Prev"[..],
        b"XRefStm",
        b"Type",
        b"W",
        b"Index",
        b"Filter",
        b"DecodeParms",
        b"Length",
    ] {
        dict_remove(&mut trailer, k);
    }
    Ok((locs, trailer))
}

fn read_xref_stream(
    data: &[u8],
    off: usize,
    locs: &BTreeMap<u32, Loc>,
) -> Result<(Dict, Vec<(u32, Loc)>), FormeError> {
    let mut p = Parser::new(data, off);
    p.skip_ws();
    p.regular_token();
    p.skip_ws();
    p.regular_token();
    p.skip_ws();
    if p.regular_token() != b"obj" {
        return Err(err("xref offset does not point to an object"));
    }
    let Some(Obj::Stream(d, raw)) = parse_object_body(&mut p, data, locs) else {
        return Err(err("xref stream expected"));
    };
    if dict_get(&d, b"Type").and_then(|o| o.as_name()) != Some(b"XRef") {
        return Err(err("not an xref stream"));
    }
    let rows = decode_stream(&d, &raw)?;
    let w: Vec<usize> = match dict_get(&d, b"W") {
        Some(Obj::Array(a)) => a
            .iter()
            .map(|o| o.as_f64().unwrap_or(0.0) as usize)
            .collect(),
        _ => return Err(err("xref stream without /W")),
    };
    if w.len() != 3 {
        return Err(err("bad /W"));
    }
    let size = dict_get(&d, b"Size")
        .and_then(|o| o.as_f64())
        .unwrap_or(0.0) as u32;
    let index: Vec<u32> = match dict_get(&d, b"Index") {
        Some(Obj::Array(a)) => a.iter().map(|o| o.as_f64().unwrap_or(0.0) as u32).collect(),
        _ => vec![0, size],
    };
    let row_len: usize = w.iter().sum();
    let field = |row: &[u8], start: usize, len: usize| {
        row[start..start + len]
            .iter()
            .fold(0u64, |a, &b| (a << 8) | b as u64)
    };
    let mut entries = Vec::new();
    let mut r = 0;
    for pair in index.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        for i in 0..pair[1] {
            if (r + 1) * row_len > rows.len() {
                break;
            }
            let row = &rows[r * row_len..(r + 1) * row_len];
            r += 1;
            let t = if w[0] == 0 { 1 } else { field(row, 0, w[0]) };
            let f2 = field(row, w[0], w[1]);
            let f3 = field(row, w[0] + w[1], w[2]);
            let id = pair[0] + i;
            match t {
                1 => entries.push((id, Loc::Offset(f2 as usize))),
                2 => entries.push((id, Loc::InStream(f2 as u32, f3 as usize))),
                _ => entries.push((id, Loc::Offset(usize::MAX))),
            }
        }
    }
    Ok((d, entries))
}

/// Fallback when cross-reference data is unusable: find every `N G obj`
/// header at a line start; the last definition of each id wins.
fn scan_objects(data: &[u8]) -> Result<(BTreeMap<u32, Loc>, Dict), FormeError> {
    let mut locs = BTreeMap::new();
    let mut i = 0;
    while let Some(pos) = find(data, b" obj", i) {
        i = pos + 4;
        // walk back over "N G"
        let mut s = pos;
        let mut fields = 0;
        let mut j = pos;
        while j > 0 && fields < 2 {
            let mut k = j;
            while k > 0 && data[k - 1].is_ascii_digit() {
                k -= 1;
            }
            if k == j {
                break;
            }
            fields += 1;
            s = k;
            j = k;
            if fields < 2 {
                if j > 0 && data[j - 1] == b' ' {
                    j -= 1;
                } else {
                    break;
                }
            }
        }
        if fields == 2 && (s == 0 || matches!(data[s - 1], b'\n' | b'\r' | b' ' | b'>')) {
            let id = std::str::from_utf8(&data[s..])
                .ok()
                .and_then(|t| t.split_whitespace().next())
                .and_then(|t| t.parse::<u32>().ok());
            if let Some(id) = id {
                locs.insert(id, Loc::Offset(s));
            }
        }
    }
    // Trailer: the last trailer dictionary, or an xref stream dict with /Root.
    let mut trailer = None;
    if let Some(t) = rfind(data, b"trailer") {
        let mut p = Parser::new(data, t + 7);
        if let Ok(Obj::Dict(d)) = p.parse() {
            trailer = Some(d);
        }
    }
    if trailer.is_none() {
        for (&id, loc) in locs.iter().rev() {
            if let Loc::Offset(off) = loc {
                if let Some(Obj::Stream(d, _)) = parse_indirect_at(data, *off, id, &BTreeMap::new())
                {
                    if dict_get(&d, b"Root").is_some() {
                        trailer = Some(d);
                        break;
                    }
                }
            }
        }
    }
    let mut trailer = trailer.ok_or_else(|| err("cannot find the PDF trailer"))?;
    for k in [&b"Prev"[..], b"XRefStm"] {
        dict_remove(&mut trailer, k);
    }
    Ok((locs, trailer))
}

fn scan_for_object(data: &[u8], id: u32, locs: &BTreeMap<u32, Loc>) -> Option<Obj> {
    // Last header for this id at a line start.
    let needle = format!("{id} ");
    let mut best = None;
    let mut i = 0;
    while let Some(pos) = find(data, needle.as_bytes(), i) {
        i = pos + 1;
        if pos > 0 && !matches!(data[pos - 1], b'\n' | b'\r') {
            continue;
        }
        if parse_indirect_at(data, pos, id, locs).is_some() {
            best = Some(pos);
        }
    }
    parse_indirect_at(data, best?, id, locs)
}

// ───────────────────────────────── writer ─────────────────────────────────

fn write_name(out: &mut Vec<u8>, n: &[u8]) {
    out.push(b'/');
    for &b in n {
        if b <= b' ' || b >= 0x7f || is_delim(b) || b == b'#' {
            out.extend_from_slice(format!("#{b:02X}").as_bytes());
        } else {
            out.push(b);
        }
    }
}

fn write_string(out: &mut Vec<u8>, s: &[u8]) {
    if s.iter().all(|&b| (0x20..0x7f).contains(&b)) {
        out.push(b'(');
        for &b in s {
            if matches!(b, b'(' | b')' | b'\\') {
                out.push(b'\\');
            }
            out.push(b);
        }
        out.push(b')');
        return;
    }
    out.push(b'<');
    for b in s {
        out.extend_from_slice(format!("{b:02X}").as_bytes());
    }
    out.push(b'>');
}

fn write_real(out: &mut Vec<u8>, f: f64) {
    if f.is_finite() && f == f.trunc() && f.abs() < 1e15 {
        out.extend_from_slice(format!("{}", f as i64).as_bytes());
    } else if f.is_finite() {
        let s = format!("{f:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        out.extend_from_slice(s.as_bytes());
    } else {
        out.push(b'0');
    }
}

/// Serialize an object, renumbering references through `map`. References to
/// objects not in the output become `null`.
pub fn write_obj(out: &mut Vec<u8>, o: &Obj, map: &HashMap<u32, u32>) {
    match o {
        Obj::Null => out.extend_from_slice(b"null"),
        Obj::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Obj::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
        Obj::Real(f) => write_real(out, *f),
        Obj::Str(s) => write_string(out, s),
        Obj::Name(n) => write_name(out, n),
        Obj::Array(a) => {
            out.push(b'[');
            for (i, e) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                write_obj(out, e, map);
            }
            out.push(b']');
        }
        Obj::Dict(d) => write_dict(out, d, map, None),
        Obj::Ref(id, _) => match map.get(id) {
            Some(n) => out.extend_from_slice(format!("{n} 0 R").as_bytes()),
            None => out.extend_from_slice(b"null"),
        },
        Obj::Stream(d, data) => {
            write_dict(out, d, map, Some(data.len()));
            out.extend_from_slice(b"\nstream\n");
            out.extend_from_slice(data);
            out.extend_from_slice(b"\nendstream");
        }
    }
}

fn write_dict(out: &mut Vec<u8>, d: &Dict, map: &HashMap<u32, u32>, length: Option<usize>) {
    out.extend_from_slice(b"<<");
    for (k, v) in d {
        if length.is_some() && k == b"Length" {
            continue;
        }
        out.push(b' ');
        write_name(out, k);
        out.push(b' ');
        write_obj(out, v, map);
    }
    if let Some(len) = length {
        out.extend_from_slice(format!(" /Length {len}").as_bytes());
    }
    out.extend_from_slice(b" >>");
}

/// Collect object ids reachable from `roots`.
fn reachable(doc: &Document, roots: &[&Obj]) -> Vec<u32> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<u32> = VecDeque::new();
    fn refs_in(o: &Obj, acc: &mut Vec<u32>) {
        match o {
            Obj::Ref(id, _) => acc.push(*id),
            Obj::Array(a) => a.iter().for_each(|e| refs_in(e, acc)),
            Obj::Dict(d) | Obj::Stream(d, _) => d.iter().for_each(|(_, v)| refs_in(v, acc)),
            _ => {}
        }
    }
    let mut start = Vec::new();
    for r in roots {
        refs_in(r, &mut start);
    }
    for id in start {
        if seen.insert(id) {
            queue.push_back(id);
        }
    }
    while let Some(id) = queue.pop_front() {
        let Some(o) = doc.objects.get(&id) else {
            continue;
        };
        order.push(id);
        let mut acc = Vec::new();
        refs_in(o, &mut acc);
        for r in acc {
            if seen.insert(r) {
                queue.push_back(r);
            }
        }
    }
    order
}

/// Write a complete single-revision PDF holding only the objects reachable
/// from `root` (a reference to the catalog) and `info`. Objects are
/// renumbered from 1; the output has one classic xref and no `/Prev`.
pub fn write_reachable(doc: &Document, root: Obj, info: Option<Obj>, id_seed: &[u8]) -> Vec<u8> {
    let mut roots: Vec<&Obj> = vec![&root];
    if let Some(i) = &info {
        roots.push(i);
    }
    let order = reachable(doc, &roots);
    let map: HashMap<u32, u32> = order
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i as u32 + 1))
        .collect();
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(order.len());
    for &id in &order {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", map[&id]).as_bytes());
        write_obj(&mut out, &doc.objects[&id], &map);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let size = order.len() + 1;
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    let digest = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(id_seed);
        h.update(&out);
        h.finalize()
    };
    let id_hex: String = digest[..16].iter().map(|b| format!("{b:02X}")).collect();
    let mut trailer = format!("trailer\n<< /Size {size} /Root ");
    let mut tmp = Vec::new();
    write_obj(&mut tmp, &root, &map);
    trailer.push_str(&String::from_utf8_lossy(&tmp));
    if let Some(i) = &info {
        let mut t2 = Vec::new();
        write_obj(&mut t2, i, &map);
        trailer.push_str(" /Info ");
        trailer.push_str(&String::from_utf8_lossy(&t2));
    }
    trailer.push_str(&format!(
        " /ID [<{id_hex}> <{id_hex}>] >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    out.extend_from_slice(trailer.as_bytes());
    out
}

/// Deflate data for a new stream.
pub fn flate(data: &[u8]) -> Vec<u8> {
    compress_to_vec_zlib(data, 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_refs_and_containers() {
        let mut p = Parser::new(
            b"<< /A 1 /B 2 0 R /C [3 4 0 R (x\\)y) <414243>] /D#20E 7 /F 1.5 >>",
            0,
        );
        let o = p.parse().unwrap();
        let d = o.as_dict().unwrap();
        assert_eq!(dict_get(d, b"A"), Some(&Obj::Int(1)));
        assert_eq!(dict_get(d, b"B"), Some(&Obj::Ref(2, 0)));
        assert_eq!(
            dict_get(d, b"C"),
            Some(&Obj::Array(vec![
                Obj::Int(3),
                Obj::Ref(4, 0),
                Obj::Str(b"x)y".to_vec()),
                Obj::Str(b"ABC".to_vec())
            ]))
        );
        assert!(dict_get(d, b"D E").is_some());
        assert_eq!(dict_get(d, b"F"), Some(&Obj::Real(1.5)));
    }

    #[test]
    fn round_trips_through_the_writer() {
        let src = b"<< /T (a\\(b) /N /Name#23x /A [1 -2.25 true null] >>";
        let o = Parser::new(src, 0).parse().unwrap();
        let mut out = Vec::new();
        write_obj(&mut out, &o, &HashMap::new());
        let again = Parser::new(&out, 0).parse().unwrap();
        assert_eq!(o, again);
    }

    #[test]
    fn ascii85_decodes() {
        assert_eq!(ascii85(b"<~87cURD]i,\"Ebo80~>").unwrap(), b"Hello World!");
    }
}
