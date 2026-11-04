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

/// One `<text>` element, ready to shape.
#[derive(Debug, Clone)]
pub struct SvgText {
    /// The text, with SVG's default whitespace handling applied.
    pub text: String,
    /// Anchor point in the text element's own user space.
    pub x: f64,
    pub y: f64,
    /// The text element's user space in the SVG's root user space
    /// (ancestors' transforms, then its own), as `[a b c d e f]`.
    pub matrix: [f64; 6],
    /// A Forme font-family list (generic families mapped).
    pub font_family: String,
    /// In user units.
    pub font_size: f64,
    pub font_weight: u32,
    pub italic: bool,
    pub fill: (f64, f64, f64),
    /// Fill opacity times every ancestor's `opacity`.
    pub opacity: f64,
    pub anchor: TextAnchor,
    pub baseline: TextBaseline,
    /// `direction="rtl"`: `start` and `end` swap sides.
    pub rtl: bool,
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

    let font_size = font_size(inherited);
    let mut raw = String::new();
    collect_text(node, css, &mut raw, warn);
    let text = collapse_whitespace(&raw);
    if text.is_empty() {
        return None;
    }

    for (prop, label) in UNSUPPORTED {
        if own.contains_key(*prop)
            || inherited
                .get(prop)
                .is_some_and(|v| v != "normal" && v != "0")
        {
            warn(format!(
                "render defect: SVG text {label} is not supported; the text is drawn without it"
            ));
        }
    }
    if inherited.get("stroke").is_some_and(|v| v.trim() != "none") {
        warn(
            "render defect: a stroke on SVG text is not drawn; the text is filled only".to_string(),
        );
    }

    let fill = match inherited.get("fill").map(str::trim) {
        Some("none") => {
            warn("render defect: SVG text with fill=\"none\" is not drawn".to_string());
            return None;
        }
        Some(v) => paint_color(v, node.document(), inherited, warn),
        None => (0.0, 0.0, 0.0),
    };
    let fill_opacity = inherited
        .get("fill-opacity")
        .and_then(opacity_value)
        .unwrap_or(1.0);

    let first = |attr: &str| -> Option<f64> {
        node.attribute(attr)
            .and_then(|v| {
                v.split(|c: char| c.is_whitespace() || c == ',')
                    .find(|s| !s.is_empty())
            })
            .and_then(|v| length(Some(v), font_size))
    };
    let x = first("x").unwrap_or(0.0) + first("dx").unwrap_or(0.0);
    let y = first("y").unwrap_or(0.0) + first("dy").unwrap_or(0.0);

    let rtl = inherited.get("direction") == Some("rtl");
    let anchor = match inherited.get("text-anchor") {
        Some("middle") => TextAnchor::Middle,
        Some("end") => TextAnchor::End,
        _ => TextAnchor::Start,
    };
    let baseline = match own.get("dominant-baseline").map(|v| v.trim()) {
        Some("middle" | "central") => TextBaseline::Central,
        Some("hanging" | "text-before-edge" | "text-top") => TextBaseline::Top,
        Some("text-after-edge" | "text-bottom" | "ideographic") => TextBaseline::Bottom,
        _ => TextBaseline::Alphabetic,
    };

    Some(SvgText {
        text,
        x,
        y,
        matrix,
        font_family: font_family(inherited.get("font-family")),
        font_size,
        font_weight: font_weight(inherited.get("font-weight")),
        italic: inherited
            .get("font-style")
            .is_some_and(|v| v == "italic" || v == "oblique"),
        fill,
        opacity: opacity * fill_opacity,
        anchor,
        baseline,
        rtl,
    })
}

/// The text of a `<text>` and its `<tspan>`s. A `<tspan>` that moves or
/// restyles its text is drawn inline with the rest, and named.
fn collect_text(node: Node, css: &Css, out: &mut String, warn: &mut impl FnMut(String)) {
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            match child.tag_name().name() {
                "tspan" | "a" => {
                    let props = css.properties(child);
                    let moves = ["x", "y", "dx", "dy", "rotate"]
                        .iter()
                        .any(|a| child.attribute(*a).is_some());
                    let restyles = [
                        "fill",
                        "font-family",
                        "font-size",
                        "font-weight",
                        "font-style",
                        "baseline-shift",
                    ]
                    .iter()
                    .any(|p| props.contains_key(*p));
                    if moves || restyles {
                        warn("render defect: SVG <tspan> positions and styles are not supported; its text is drawn inline in the <text>'s style".to_string());
                    }
                    collect_text(child, css, out, warn);
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

/// SVG's default `xml:space` handling: newlines removed, tabs to spaces,
/// runs of spaces collapsed, leading and trailing space trimmed.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for ch in s.chars() {
        match ch {
            '\n' | '\r' => {}
            ' ' | '\t' => space = true,
            _ => {
                if space && !out.is_empty() {
                    out.push(' ');
                }
                space = false;
                out.push(ch);
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
        assert_eq!(t.text, "Jan");
        assert_eq!(t.font_family, "Helvetica");
        assert_eq!(t.font_size, 10.0);
        assert_eq!(t.anchor, TextAnchor::End);
        assert!((t.opacity - 0.5).abs() < 1e-9);
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
        assert_eq!(t.text, "A label");
        assert_eq!(t.font_family, "Inter, Times");
        assert!((t.font_size - 16.0).abs() < 1e-9);
        assert_eq!(t.font_weight, 700);
        assert!(t.italic);
        assert!((t.fill.1 - 110.0 / 255.0).abs() < 1e-9);
        assert_eq!((t.x, t.y), (5.0, 9.0));
    }

    #[test]
    fn hidden_and_unrendered_text_is_skipped() {
        let (t, _) = texts(&format!(
            r##"<svg {NS}><defs><text id="a">no</text></defs><text display="none">no</text><g style="display:none"><text>no</text></g><text visibility="hidden">no</text><text>yes</text></svg>"##
        ));
        assert_eq!(
            t.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["yes"]
        );
    }

    #[test]
    fn unsupported_text_features_are_named() {
        let (t, w) = texts(&format!(
            r##"<svg {NS}><text>a<tspan dy="5">b</tspan></text><text><textPath href="#p">c</textPath></text><text stroke="red" letter-spacing="2">d</text><text fill="none">e</text></svg>"##
        ));
        assert_eq!(
            t.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["ab", "d"]
        );
        for needle in [
            "<tspan>",
            "<textPath>",
            "stroke",
            "letter-spacing",
            "fill=\"none\"",
        ] {
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
        };
        let t = extract_texts(&doc, &defaults, &mut Vec::new());
        assert_eq!((t[0].font_family.as_str(), t[0].font_size), ("Inter", 11.0));
        assert!(t[0].italic && t[0].font_weight == 700);
        assert_eq!(
            (t[1].font_family.as_str(), t[1].font_size),
            ("Courier", 9.0)
        );
    }

    #[test]
    fn em_offsets_use_the_font_size() {
        let (t, _) = texts(&format!(
            r##"<svg {NS}><text font-size="20" y="10" dy="0.5em" dominant-baseline="central">x</text></svg>"##
        ));
        assert_eq!(t[0].y, 20.0);
        assert_eq!(t[0].baseline, TextBaseline::Central);
    }
}
