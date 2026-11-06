//! SVG `<text>` extraction.
//!
//! usvg is built without its text feature, so it drops `<text>`. This
//! module reads the text elements from the same prepared document, resolves
//! the properties that position and style them (CSS from `<style>`,
//! `class` and `style=""`, presentation attributes, inheritance, the
//! transforms of every ancestor), and hands each one to layout, which
//! shapes it with Forme's own text pipeline. What it does not draw is named
//! in the warnings.

use std::collections::HashMap;
use std::str::FromStr;

use usvg::roxmltree::{self, Node};

use crate::style::FontFeature;

/// Where the anchor point sits on the text's advance (`text-anchor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

/// Which line of the text sits on `y` (`dominant-baseline`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextBaseline {
    Alphabetic,
    /// `middle` and `central`: halfway between ascent and descent.
    Central,
    /// `hanging`, `text-before-edge`, `text-top`: the ascent.
    Top,
    /// `text-after-edge`, `text-bottom`, `ideographic`: the descent.
    Bottom,
}

/// One `<text>` element, ready to lay out: its user space and its text
/// chunks.
#[derive(Debug, Clone)]
pub struct SvgText {
    /// The text element's user space in the SVG's root user space
    /// (ancestors' transforms, then its own), as `[a b c d e f]`.
    pub matrix: [f64; 6],
    /// `direction="rtl"`: `start` and `end` swap sides.
    pub rtl: bool,
    pub chunks: Vec<SvgTextChunk>,
}

/// A text chunk, as SVG defines it: text that starts at an absolute `x` or
/// `y` (or at the start of the `<text>`) and is aligned by `text-anchor` as
/// one unit. Without an `x` or `y` of its own, a chunk would continue the
/// previous one, so a new chunk always has one of them or is the first.
#[derive(Debug, Clone)]
pub struct SvgTextChunk {
    /// Absolute position; `None` keeps the current text position.
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub anchor: TextAnchor,
    pub baseline: TextBaseline,
    pub pieces: Vec<SvgTextPiece>,
}

/// A run of text in one style. `dx`/`dy` shift the current text position
/// before it, in user units.
#[derive(Debug, Clone)]
pub struct SvgTextPiece {
    pub text: String,
    pub dx: f64,
    pub dy: f64,
    /// A Forme font-family list (generic families mapped).
    pub font_family: String,
    /// In user units.
    pub font_size: f64,
    pub font_weight: u32,
    pub italic: bool,
    pub fill: (f64, f64, f64),
    /// Fill opacity times every ancestor's `opacity`.
    pub opacity: f64,
    /// OpenType features from `font-variant-numeric` and
    /// `font-feature-settings` (#245).
    pub font_features: Vec<FontFeature>,
}

/// Elements whose subtree is never rendered in place.
const NOT_RENDERED: &[&str] = &[
    "defs",
    "clipPath",
    "mask",
    "symbol",
    "pattern",
    "marker",
    "linearGradient",
    "radialGradient",
    "filter",
    "foreignObject",
    "style",
    "script",
    "title",
    "desc",
    "metadata",
];

/// Properties that inherit from the parent when an element leaves them
/// unset.
const INHERITED: &[&str] = &[
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "fill",
    "fill-opacity",
    "font-variant-numeric",
    "font-feature-settings",
    DEFAULT_FEATURES,
    "text-anchor",
    "dominant-baseline",
    "visibility",
    "direction",
    "color",
    "stroke",
    "letter-spacing",
    "word-spacing",
    "writing-mode",
];

/// Properties a `<text>` or `<tspan>` can set that this drawing ignores.
const UNSUPPORTED: &[(&str, &str)] = &[
    ("letter-spacing", "letter-spacing"),
    ("word-spacing", "word-spacing"),
    ("text-decoration", "text-decoration"),
    ("textLength", "textLength"),
    ("rotate", "rotate"),
];

/// Font properties the `<svg>` inherits from the document around it, as
/// SVG text in a browser inherits them from the HTML around the `<svg>`.
#[derive(Debug, Clone, Default)]
pub struct TextDefaults {
    /// A Forme family list.
    pub font_family: Option<String>,
    /// In SVG user units.
    pub font_size: Option<f64>,
    pub font_weight: Option<u32>,
    pub italic: bool,
    /// The OpenType features set around the `<svg>`.
    pub font_features: Vec<FontFeature>,
}

/// Read every rendered `<text>` element of a prepared document.
pub fn extract_texts(
    doc: &roxmltree::Document,
    defaults: &TextDefaults,
    warnings: &mut Vec<String>,
) -> Vec<SvgText> {
    let css = Css::new(doc);
    let mut out = Vec::new();
    let mut warn = |msg: String| {
        if !warnings.contains(&msg) {
            warnings.push(msg);
        }
    };
    walk(
        doc.root_element(),
        &css,
        &Inherited::root(defaults),
        IDENTITY,
        1.0,
        &mut out,
        &mut warn,
    );
    out
}

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` then `b` applied to a point: the matrix for `b · a` in column
/// vector terms, i.e. `a` is the outer (parent) transform.
fn concat(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

/// Inherited property values at a point in the tree.
#[derive(Clone)]
struct Inherited {
    values: HashMap<&'static str, String>,
}

impl Inherited {
    fn root(defaults: &TextDefaults) -> Self {
        let mut values = HashMap::new();
        if let Some(f) = &defaults.font_family {
            values.insert("font-family", f.clone());
        }
        if let Some(s) = defaults.font_size {
            values.insert("font-size", format!("{s}"));
        }
        if let Some(w) = defaults.font_weight {
            values.insert("font-weight", w.to_string());
        }
        if defaults.italic {
            values.insert("font-style", "italic".to_string());
        }
        // Kept in a slot of their own: an SVG's own font-variant-numeric or
        // font-feature-settings overrides them as CSS would (`normal` clears
        // an inherited numeric variant), which merging them into
        // font-feature-settings could not express.
        if !defaults.font_features.is_empty() {
            let settings = defaults
                .font_features
                .iter()
                .map(|f| format!("\"{}\" {}", f.tag, f.value))
                .collect::<Vec<_>>()
                .join(", ");
            values.insert(DEFAULT_FEATURES, settings);
        }
        Inherited { values }
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// The values a child sees: its own where set, ours otherwise.
    fn child(&self, own: &HashMap<String, String>) -> Self {
        let mut values = self.values.clone();
        for name in INHERITED {
            if let Some(v) = own.get(*name) {
                if v.trim() != "inherit" {
                    values.insert(name, v.trim().to_string());
                }
            }
        }
        Inherited { values }
    }
}

fn walk(
    node: Node,
    css: &Css,
    parent: &Inherited,
    parent_matrix: [f64; 6],
    parent_opacity: f64,
    out: &mut Vec<SvgText>,
    warn: &mut impl FnMut(String),
) {
    let name = node.tag_name().name();
    if NOT_RENDERED.contains(&name) {
        return;
    }
    let own = css.properties(node);
    if own.get("display").is_some_and(|v| v.trim() == "none") {
        return;
    }
    let inherited = parent.child(&own);
    let mut matrix = parent_matrix;
    // The root <svg> is the prepared wrapper; a nested one opens a new
    // viewport at its x/y.
    if name == "svg" && node.parent_element().is_some() {
        let x = length(node.attribute("x"), 16.0).unwrap_or(0.0);
        let y = length(node.attribute("y"), 16.0).unwrap_or(0.0);
        matrix = concat(matrix, [1.0, 0.0, 0.0, 1.0, x, y]);
        if node.attribute("viewBox").is_some() {
            warn("render defect: SVG <text> inside a nested <svg> with its own viewBox is placed without that viewBox's scaling".to_string());
        }
    }
    if let Some(t) = own.get("transform") {
        if let Ok(t) = svgtypes::Transform::from_str(t) {
            matrix = concat(matrix, [t.a, t.b, t.c, t.d, t.e, t.f]);
        }
    }
    let opacity = parent_opacity
        * own
            .get("opacity")
            .and_then(|v| opacity_value(v))
            .unwrap_or(1.0);

    if name == "text" {
        if let Some(t) = text_element(node, css, &own, &inherited, matrix, opacity, warn) {
            out.push(t);
        }
        return;
    }
    for child in node.children().filter(|c| c.is_element()) {
        walk(child, css, &inherited, matrix, opacity, out, warn);
    }
}

fn text_element(
    node: Node,
    css: &Css,
    own: &HashMap<String, String>,
    inherited: &Inherited,
    matrix: [f64; 6],
    opacity: f64,
    warn: &mut impl FnMut(String),
) -> Option<SvgText> {
    if inherited.get("visibility").is_some_and(|v| v != "visible") {
        return None;
    }
    if inherited
        .get("writing-mode")
        .is_some_and(|v| v.starts_with("tb") || v.starts_with("vertical"))
    {
        warn("render defect: vertical SVG text (writing-mode) is not supported; it is drawn horizontally".to_string());
    }
    let baseline = match own.get("dominant-baseline").map(|v| v.trim()) {
        Some("middle" | "central") => TextBaseline::Central,
        Some("hanging" | "text-before-edge" | "text-top") => TextBaseline::Top,
        Some("text-after-edge" | "text-bottom" | "ideographic") => TextBaseline::Bottom,
        _ => TextBaseline::Alphabetic,
    };
    let mut layout = TextLayout {
        css,
        doc: node.document(),
        baseline,
        chunks: Vec::new(),
        pending: (0.0, 0.0),
    };
    layout.element(node, own, inherited, opacity, warn);
    let chunks = collapse_chunks(layout.chunks);
    if chunks.is_empty() {
        return None;
    }
    Some(SvgText {
        matrix,
        rtl: inherited.get("direction") == Some("rtl"),
        chunks,
    })
}

/// Walks a `<text>` and its `<tspan>`s into chunks and pieces.
struct TextLayout<'a, 'd, 'input> {
    css: &'a Css<'d>,
    doc: &'d roxmltree::Document<'input>,
    baseline: TextBaseline,
    chunks: Vec<SvgTextChunk>,
    /// A `dx`/`dy` read but not yet given to a piece.
    pending: (f64, f64),
}

impl TextLayout<'_, '_, '_> {
    fn element(
        &mut self,
        node: Node,
        props: &HashMap<String, String>,
        inherited: &Inherited,
        opacity: f64,
        warn: &mut impl FnMut(String),
    ) {
        for (prop, label) in UNSUPPORTED {
            if props.contains_key(*prop)
                || inherited
                    .get(prop)
                    .is_some_and(|v| v != "normal" && v != "0")
            {
                warn(format!(
                    "render defect: SVG text {label} is not supported; the text is drawn without it"
                ));
            }
        }
        if props.contains_key("baseline-shift") {
            warn(
                "render defect: SVG baseline-shift is not supported; the text sits on its line"
                    .to_string(),
            );
        }
        let font_size = font_size(inherited);
        let mut first = |attr: &str| -> Option<f64> {
            let raw = node.attribute(attr)?;
            let mut values = raw
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty());
            let first = values.next();
            if values.next().is_some() {
                warn(format!(
                    "render defect: SVG text with a list of {attr} values (one per character) is not supported; the first value is used"
                ));
            }
            first.and_then(|v| length(Some(v), font_size))
        };
        let (x, y) = (first("x"), first("y"));
        let (dx, dy) = (first("dx").unwrap_or(0.0), first("dy").unwrap_or(0.0));
        if node.attribute("rotate").is_some() {
            warn(
                "render defect: SVG text rotate is not supported; the text is drawn without it"
                    .to_string(),
            );
        }

        // An absolute x or y starts a new chunk, aligned on its own; so does
        // the <text> itself.
        if x.is_some() || y.is_some() || self.chunks.is_empty() {
            let anchor = match inherited.get("text-anchor") {
                Some("middle") => TextAnchor::Middle,
                Some("end") => TextAnchor::End,
                _ => TextAnchor::Start,
            };
            self.chunks.push(SvgTextChunk {
                x,
                y,
                anchor,
                baseline: self.baseline,
                pieces: Vec::new(),
            });
            self.pending = (0.0, 0.0);
        }
        self.pending.0 += dx;
        self.pending.1 += dy;

        for child in node.children() {
            if child.is_text() {
                self.piece(child.text().unwrap_or(""), node, inherited, opacity, warn);
            } else if child.is_element() {
                match child.tag_name().name() {
                    "tspan" | "a" => {
                        let props = self.css.properties(child);
                        if props.get("display").is_some_and(|v| v.trim() == "none") {
                            continue;
                        }
                        let inner = inherited.child(&props);
                        if inner.get("visibility").is_some_and(|v| v != "visible") {
                            continue;
                        }
                        let child_opacity = opacity
                            * props
                                .get("opacity")
                                .and_then(|v| opacity_value(v))
                                .unwrap_or(1.0);
                        self.element(child, &props, &inner, child_opacity, warn);
                    }
                    "textPath" => {
                        warn(
                            "render defect: SVG <textPath> is not supported; its text is not drawn"
                                .to_string(),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn piece(
        &mut self,
        text: &str,
        node: Node,
        inherited: &Inherited,
        opacity: f64,
        warn: &mut impl FnMut(String),
    ) {
        if text.is_empty() {
            return;
        }
        if inherited.get("stroke").is_some_and(|v| v.trim() != "none") {
            warn(
                "render defect: a stroke on SVG text is not drawn; the text is filled only"
                    .to_string(),
            );
        }
        let fill = match inherited.get("fill").map(str::trim) {
            Some("none") => {
                warn("render defect: SVG text with fill=\"none\" is not drawn".to_string());
                return;
            }
            Some(v) => paint_color(v, self.doc, inherited, warn),
            None => (0.0, 0.0, 0.0),
        };
        let _ = node;
        let fill_opacity = inherited
            .get("fill-opacity")
            .and_then(opacity_value)
            .unwrap_or(1.0);
        let (dx, dy) = std::mem::take(&mut self.pending);
        let piece = SvgTextPiece {
            text: text.to_string(),
            dx,
            dy,
            font_family: font_family(inherited.get("font-family")),
            font_size: font_size(inherited),
            font_weight: font_weight(inherited.get("font-weight")),
            italic: inherited
                .get("font-style")
                .is_some_and(|v| v == "italic" || v == "oblique"),
            fill,
            opacity: opacity * fill_opacity,
            font_features: font_features(inherited, warn),
        };
        self.chunks
            .last_mut()
            .expect("a chunk is opened before any text")
            .pieces
            .push(piece);
    }
}

/// SVG's default `xml:space` handling across a whole `<text>`: newlines
/// removed, tabs to spaces, runs of spaces collapsed even where a run
/// crosses from one piece into the next. Space at the start and end of each
/// chunk is trimmed, so the indentation between two `<tspan>` lines does
/// not shift either line's anchoring. Empty pieces and chunks are dropped.
fn collapse_chunks(chunks: Vec<SvgTextChunk>) -> Vec<SvgTextChunk> {
    let mut out = Vec::new();
    for mut chunk in chunks {
        let mut prev_space = true; // at a chunk start, leading space goes
        for piece in &mut chunk.pieces {
            let mut text = String::with_capacity(piece.text.len());
            for ch in piece.text.chars() {
                match ch {
                    '\n' | '\r' => {}
                    ' ' | '\t' => {
                        if !prev_space {
                            text.push(' ');
                            prev_space = true;
                        }
                    }
                    _ => {
                        text.push(ch);
                        prev_space = false;
                    }
                }
            }
            piece.text = text;
        }
        // Trailing space of the chunk: on its last piece that has text.
        if let Some(last) = chunk.pieces.iter_mut().rev().find(|p| !p.text.is_empty()) {
            let trimmed = last.text.trim_end().len();
            last.text.truncate(trimmed);
        }
        // A piece left empty still carries its dx/dy to the next one.
        let mut carry = (0.0, 0.0);
        let mut pieces = Vec::new();
        for mut piece in chunk.pieces {
            piece.dx += carry.0;
            piece.dy += carry.1;
            if piece.text.is_empty() {
                carry = (piece.dx, piece.dy);
            } else {
                carry = (0.0, 0.0);
                pieces.push(piece);
            }
        }
        if !pieces.is_empty() {
            chunk.pieces = pieces;
            out.push(chunk);
        }
    }
    out
}

/// The inherited slot for features set around the `<svg>`, as a
/// font-feature-settings list. Not a CSS property, so no SVG can set it.
const DEFAULT_FEATURES: &str = "-forme-default-features";

/// `font-variant-numeric` keywords and the OpenType feature each turns on,
/// as the JS adapters and the HTML path map them.
const VARIANT_NUMERIC: &[(&str, &str)] = &[
    ("lining-nums", "lnum"),
    ("oldstyle-nums", "onum"),
    ("proportional-nums", "pnum"),
    ("tabular-nums", "tnum"),
    ("diagonal-fractions", "frac"),
    ("stacked-fractions", "afrc"),
    ("ordinal", "ordn"),
    ("slashed-zero", "zero"),
];

/// The features a piece is shaped with: `font-variant-numeric`, then
/// `font-feature-settings`, which wins for the same tag, as in CSS.
/// `normal` in either clears what it would set.
fn font_features(inherited: &Inherited, warn: &mut impl FnMut(String)) -> Vec<FontFeature> {
    let mut out: Vec<FontFeature> = Vec::new();
    let set = |tag: &str, value: u32, out: &mut Vec<FontFeature>| {
        out.retain(|f| f.tag != tag);
        out.push(FontFeature {
            tag: tag.to_string(),
            value,
        });
    };
    // Features from around the <svg>, unless the SVG sets its own: its
    // font-feature-settings replaces them all, its font-variant-numeric
    // replaces the numeric variants among them.
    if inherited.get("font-feature-settings").is_none() {
        if let Some(v) = inherited.get(DEFAULT_FEATURES) {
            for item in v.split(',') {
                let mut parts = item.split_whitespace();
                if let (Some(tag), Some(value)) = (parts.next(), parts.next()) {
                    let tag = tag.trim_matches('"');
                    let numeric = VARIANT_NUMERIC.iter().any(|(_, t)| *t == tag);
                    if numeric && inherited.get("font-variant-numeric").is_some() {
                        continue;
                    }
                    set(tag, value.parse().unwrap_or(1), &mut out);
                }
            }
        }
    }
    if let Some(v) = inherited.get("font-variant-numeric") {
        for word in v.split_whitespace() {
            if word == "normal" {
                continue;
            }
            match VARIANT_NUMERIC.iter().find(|(k, _)| *k == word) {
                Some((_, tag)) => set(tag, 1, &mut out),
                None => warn(format!(
                    "render defect: SVG text font-variant-numeric value \"{word}\" is not supported; it is ignored"
                )),
            }
        }
    }
    if let Some(v) = inherited.get("font-feature-settings") {
        if v.trim() != "normal" {
            for item in v.split(',') {
                let mut parts = item.split_whitespace();
                let Some(tag) = parts
                    .next()
                    .map(|t| t.trim_matches(|c| c == '"' || c == '\''))
                else {
                    continue;
                };
                if tag.len() != 4 || !tag.is_ascii() {
                    warn(format!(
                        "render defect: SVG text font-feature-settings tag \"{tag}\" is not a 4-letter OpenType tag; it is ignored"
                    ));
                    continue;
                }
                let value = match parts.next() {
                    None | Some("on") => 1,
                    Some("off") => 0,
                    Some(n) => n.parse().unwrap_or(1),
                };
                set(tag, value, &mut out);
            }
        }
    }
    out
}

fn opacity_value(v: &str) -> Option<f64> {
    let v = v.trim();
    let n = if let Some(p) = v.strip_suffix('%') {
        p.trim().parse::<f64>().ok()? / 100.0
    } else {
        v.parse::<f64>().ok()?
    };
    Some(n.clamp(0.0, 1.0))
}

/// A length in user units. `em` is relative to `font_size`; percentages
/// are not resolved (they need the viewport) and read as absent.
fn length(v: Option<&str>, font_size: f64) -> Option<f64> {
    let l = svgtypes::Length::from_str(v?.trim()).ok()?;
    use svgtypes::LengthUnit as U;
    let n = l.number;
    Some(match l.unit {
        U::None | U::Px => n,
        U::Em => n * font_size,
        U::Ex => n * font_size / 2.0,
        U::In => n * 96.0,
        U::Cm => n * 96.0 / 2.54,
        U::Mm => n * 96.0 / 25.4,
        U::Pt => n * 4.0 / 3.0,
        U::Pc => n * 16.0,
        U::Percent => return None,
    })
}

/// The computed font size in user units, from keywords, `%` or a length.
fn font_size(inherited: &Inherited) -> f64 {
    // `inherited` holds the nearest set value. Relative units in it are
    // resolved against the 16px default, not its own parent's size, an
    // approximation that only nested relative sizes notice.
    let v = match inherited.get("font-size") {
        Some(v) => v.trim(),
        None => return 16.0,
    };
    match v {
        "xx-small" => 9.0,
        "x-small" => 10.0,
        "small" => 13.0,
        "medium" => 16.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        _ => {
            if let Some(p) = v.strip_suffix('%') {
                p.trim()
                    .parse::<f64>()
                    .map(|p| 16.0 * p / 100.0)
                    .unwrap_or(16.0)
            } else {
                length(Some(v), 16.0).filter(|s| *s > 0.0).unwrap_or(16.0)
            }
        }
    }
}

fn font_weight(v: Option<&str>) -> u32 {
    match v.map(str::trim) {
        Some("bold" | "bolder") => 700,
        Some("lighter") => 300,
        Some(n) => n.parse::<u32>().unwrap_or(400).clamp(100, 900),
        None => 400,
    }
}

/// An SVG `font-family` list as a Forme family list. Generic families map
/// to the standard fonts, as the HTML path maps them.
fn font_family(v: Option<&str>) -> String {
    let Some(v) = v else {
        return "Helvetica".to_string();
    };
    let names: Vec<String> = v
        .split(',')
        .map(|n| {
            n.trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_string()
        })
        .filter(|n| !n.is_empty())
        .map(|n| match n.to_ascii_lowercase().as_str() {
            "sans-serif" | "system-ui" | "ui-sans-serif" => "Helvetica".to_string(),
            "serif" | "ui-serif" => "Times".to_string(),
            "monospace" | "ui-monospace" => "Courier".to_string(),
            _ => n,
        })
        .collect();
    if names.is_empty() {
        "Helvetica".to_string()
    } else {
        names.join(", ")
    }
}

/// A fill as an RGB colour. A gradient or pattern fill uses its first
/// stop's colour (or black), and is named.
fn paint_color(
    v: &str,
    doc: &roxmltree::Document,
    inherited: &Inherited,
    warn: &mut impl FnMut(String),
) -> (f64, f64, f64) {
    let rgb = |c: svgtypes::Color| {
        (
            c.red as f64 / 255.0,
            c.green as f64 / 255.0,
            c.blue as f64 / 255.0,
        )
    };
    match svgtypes::Paint::from_str(v) {
        Ok(svgtypes::Paint::Color(c)) => rgb(c),
        Ok(svgtypes::Paint::CurrentColor) => inherited
            .get("color")
            .and_then(|c| svgtypes::Color::from_str(c).ok())
            .map(rgb)
            .unwrap_or((0.0, 0.0, 0.0)),
        Ok(svgtypes::Paint::FuncIRI(id, _)) => {
            warn("render defect: a gradient or pattern fill on SVG text is not supported; the text uses the first stop colour".to_string());
            doc.descendants()
                .find(|n| n.attribute("id") == Some(id))
                .and_then(|g| g.children().find(|s| s.has_tag_name("stop")))
                .and_then(|s| {
                    s.attribute("stop-color").or_else(|| {
                        s.attribute("style").and_then(|st| {
                            simplecss::DeclarationTokenizer::from(st)
                                .find(|d| d.name == "stop-color")
                                .map(|d| d.value)
                        })
                    })
                })
                .and_then(|c| svgtypes::Color::from_str(c).ok())
                .map(rgb)
                .unwrap_or((0.0, 0.0, 0.0))
        }
        _ => (0.0, 0.0, 0.0),
    }
}

/// The CSS rules of a document's `<style>` elements, applied the way usvg
/// applies them to shapes: presentation attributes, then matching rules in
/// specificity and source order, then `style=""`, with `!important`
/// declarations last.
struct Css<'a> {
    rules: Vec<(simplecss::Selector<'a>, Vec<simplecss::Declaration<'a>>)>,
}

impl<'a> Css<'a> {
    fn new(doc: &'a roxmltree::Document) -> Self {
        let mut sheet = simplecss::StyleSheet::new();
        for node in doc.descendants().filter(|n| n.has_tag_name("style")) {
            if let Some(t) = node.attribute("type") {
                if t != "text/css" && !t.is_empty() {
                    continue;
                }
            }
            for child in node.children() {
                if let Some(text) = child.text() {
                    sheet.parse_more(text);
                }
            }
        }
        let mut rules: Vec<_> = sheet
            .rules
            .into_iter()
            .map(|r| (r.selector, r.declarations))
            .collect();
        // Stable: equal specificity keeps source order.
        rules.sort_by_key(|(s, _)| s.specificity());
        Css { rules }
    }

    fn properties(&self, node: Node) -> HashMap<String, String> {
        let mut props: HashMap<String, String> = HashMap::new();
        for attr in node.attributes() {
            if attr.namespace().is_none() && attr.name() != "style" {
                props.insert(attr.name().to_string(), attr.value().to_string());
            }
        }
        let mut important: Vec<(String, String)> = Vec::new();
        let el = XmlNode(node);
        for (selector, decls) in &self.rules {
            if selector.matches(&el) {
                for d in decls {
                    if d.important {
                        important.push((d.name.to_string(), d.value.to_string()));
                    } else {
                        props.insert(d.name.to_string(), d.value.to_string());
                    }
                }
            }
        }
        if let Some(style) = node.attribute("style") {
            for d in simplecss::DeclarationTokenizer::from(style) {
                if d.important {
                    important.push((d.name.to_string(), d.value.to_string()));
                } else {
                    props.insert(d.name.to_string(), d.value.to_string());
                }
            }
        }
        for (name, value) in important {
            props.insert(name, value);
        }
        props
    }
}

#[derive(Clone, Copy)]
struct XmlNode<'a, 'input>(Node<'a, 'input>);

impl simplecss::Element for XmlNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(XmlNode)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(XmlNode)
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag_name().name() == local_name
    }

    fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
        self.0
            .attribute(local_name)
            .is_some_and(|value| operator.matches(value))
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
        matches!(class, simplecss::PseudoClass::FirstChild) && self.prev_sibling_element().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first piece of a text.
    fn p(t: &SvgText) -> &SvgTextPiece {
        &t.chunks[0].pieces[0]
    }

    /// All of a text's characters, chunk by chunk, piece by piece.
    fn s(t: &SvgText) -> String {
        t.chunks
            .iter()
            .flat_map(|c| c.pieces.iter().map(|p| p.text.as_str()))
            .collect()
    }

    fn texts(svg: &str) -> (Vec<SvgText>, Vec<String>) {
        let doc = roxmltree::Document::parse(svg).unwrap();
        let mut w = Vec::new();
        let t = extract_texts(&doc, &TextDefaults::default(), &mut w);
        (t, w)
    }

    const NS: &str = r#"xmlns="http://www.w3.org/2000/svg""#;

    #[test]
    fn vega_label() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><g transform="translate(40,10)"><text text-anchor="end" transform="translate(20,7) rotate(315) translate(0,8)" font-family="sans-serif" font-size="10px" fill="#000" opacity="0.5">Jan</text></g></svg>"##
        ));
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(t.len(), 1);
        let t = &t[0];
        assert_eq!(s(t), "Jan");
        assert_eq!(p(t).font_family, "Helvetica");
        assert_eq!(p(t).font_size, 10.0);
        assert_eq!(t.chunks[0].anchor, TextAnchor::End);
        assert!((p(t).opacity - 0.5).abs() < 1e-9);
        // translate(40,10) translate(20,7) rotate(315) translate(0,8):
        // the origin lands at (60,17) + rotate(315)·(0,8).
        let a = 315f64.to_radians();
        assert!(
            (t.matrix[4] - (60.0 - 8.0 * a.sin())).abs() < 1e-9,
            "{:?}",
            t.matrix
        );
        assert!(
            (t.matrix[5] - (17.0 + 8.0 * a.cos())).abs() < 1e-9,
            "{:?}",
            t.matrix
        );
    }

    #[test]
    fn css_class_and_inheritance() {
        let (t, _) = texts(&format!(
            r##"<svg {NS}><style>.lbl {{ font-family: 'Inter', serif; font-size: 12pt; font-weight: bold }} g text {{ fill: #006E58 }}</style><g font-style="italic"><text class="lbl" x="5" y="9">  A
 label  </text></g></svg>"##
        ));
        let t = &t[0];
        assert_eq!(s(t), "A label");
        assert_eq!(p(t).font_family, "Inter, Times");
        assert!((p(t).font_size - 16.0).abs() < 1e-9);
        assert_eq!(p(t).font_weight, 700);
        assert!(p(t).italic);
        assert!((p(t).fill.1 - 110.0 / 255.0).abs() < 1e-9);
        assert_eq!((t.chunks[0].x, t.chunks[0].y), (Some(5.0), Some(9.0)));
    }

    #[test]
    fn hidden_and_unrendered_text_is_skipped() {
        let (t, _) = texts(&format!(
            r##"<svg {NS}><defs><text id="a">no</text></defs><text display="none">no</text><g style="display:none"><text>no</text></g><text visibility="hidden">no</text><text>yes</text></svg>"##
        ));
        assert_eq!(t.iter().map(s).collect::<Vec<_>>(), ["yes"]);
    }

    #[test]
    fn unsupported_text_features_are_named() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><text>a<tspan dy="5">b</tspan></text><text><textPath href="#p">c</textPath></text><text stroke="red" letter-spacing="2">d</text><text fill="none">e</text></svg>"##
        ));
        assert_eq!(t.iter().map(s).collect::<Vec<_>>(), ["ab", "d"]);
        // A <tspan> that moves is laid out now (#238), so it is not named.
        assert!(!w.iter().any(|m| m.contains("<tspan>")), "{w:?}");
        for needle in ["<textPath>", "stroke", "letter-spacing", "fill=\"none\""] {
            assert!(w.iter().any(|m| m.contains(needle)), "{needle}: {w:?}");
        }
    }

    #[test]
    fn document_fonts_are_inherited_unless_set() {
        let src = format!(
            r##"<svg {NS}><text>a</text><text font-family="monospace" font-size="9">b</text></svg>"##
        );
        let doc = roxmltree::Document::parse(&src).unwrap();
        let defaults = TextDefaults {
            font_family: Some("Inter".into()),
            font_size: Some(11.0),
            font_weight: Some(700),
            italic: true,
            font_features: Vec::new(),
        };
        let t = extract_texts(&doc, &defaults, &mut Vec::new());
        assert_eq!(
            (p(&t[0]).font_family.as_str(), p(&t[0]).font_size),
            ("Inter", 11.0)
        );
        assert!(p(&t[0]).italic && p(&t[0]).font_weight == 700);
        assert_eq!(
            (p(&t[1]).font_family.as_str(), p(&t[1]).font_size),
            ("Courier", 9.0)
        );
    }

    #[test]
    fn em_offsets_use_the_font_size() {
        let (t, _) = texts(&format!(
            r##"<svg {NS}><text font-size="20" y="10" dy="0.5em" dominant-baseline="central">x</text></svg>"##
        ));
        assert_eq!(t[0].chunks[0].y, Some(10.0));
        assert_eq!(p(&t[0]).dy, 10.0);
        assert_eq!(t[0].chunks[0].baseline, TextBaseline::Central);
    }

    #[test]
    fn vega_two_line_label_is_two_chunks() {
        // #238: Vega's axis label ["Sep", "2025"].
        let (t, w) = texts(&format!(
            r##"<svg {NS}><text text-anchor="middle" transform="translate(100,20)" font-size="10"><tspan>Sep</tspan><tspan x="0" dy="12">2025</tspan></text></svg>"##
        ));
        assert!(w.is_empty(), "{w:?}");
        let c = &t[0].chunks;
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].x, s(&t[0]).as_str()), (None, "Sep2025"));
        assert_eq!((c[1].x, c[1].y), (Some(0.0), None));
        assert_eq!(c[1].pieces[0].text, "2025");
        assert_eq!(c[1].pieces[0].dy, 12.0);
        assert!(c.iter().all(|c| c.anchor == TextAnchor::Middle));
    }

    #[test]
    fn tspan_keeps_its_own_style() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><text font-size="10" fill="#000">Total: <tspan font-weight="bold" fill="#c00">42</tspan> items</text></svg>"##
        ));
        assert!(w.is_empty(), "{w:?}");
        let pieces = &t[0].chunks[0].pieces;
        let texts: Vec<&str> = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, ["Total: ", "42", " items"]);
        assert_eq!((pieces[1].font_weight, pieces[1].fill.0), (700, 0.8));
        assert_eq!(pieces[0].font_weight, 400);
    }

    #[test]
    fn whitespace_collapses_across_tspans_and_trims_at_chunks() {
        let (t, _) = texts(&format!(
            "<svg {NS}><text>\n  <tspan>Reserved </tspan>\n  <tspan x=\"0\" dy=\"12\"> Offers</tspan>\n</text></svg>"
        ));
        let c = &t[0].chunks;
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0].pieces
                .iter()
                .map(|p| p.text.as_str())
                .collect::<String>(),
            "Reserved"
        );
        assert_eq!(
            c[1].pieces
                .iter()
                .map(|p| p.text.as_str())
                .collect::<String>(),
            "Offers"
        );
    }

    #[test]
    fn per_character_position_lists_are_named() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><text x="1 2 3" y="5">abc</text></svg>"##
        ));
        assert_eq!(t[0].chunks[0].x, Some(1.0));
        assert!(w.iter().any(|m| m.contains("list of x values")), "{w:?}");
    }

    #[test]
    fn font_features_are_read_and_inherited() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><style>.t {{ font-feature-settings: "tnum" 1, "liga" off }}</style><g font-variant-numeric="slashed-zero tabular-nums"><text class="t">1</text><text font-variant-numeric="normal">2</text></g></svg>"##
        ));
        assert!(w.is_empty(), "{w:?}");
        let tags = |t: &SvgText| {
            p(t).font_features
                .iter()
                .map(|f| (f.tag.clone(), f.value))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            tags(&t[0]),
            [("zero".into(), 1), ("tnum".into(), 1), ("liga".into(), 0)]
        );
        assert!(
            tags(&t[1]).is_empty(),
            "normal clears the inherited numeric variants"
        );
    }
}
