//! SVG import through usvg.
//!
//! usvg resolves the parts of SVG that a hand-rolled parser gets wrong or
//! skips: CSS (`<style>`, `class`, `style=""`), `transform`, `<use>`,
//! `<clipPath>`, gradients, inherited presentation attributes and units.
//! This module walks the resolved tree and lowers it to [`SvgCommand`]s in
//! raw SVG user space. The PDF writer applies the viewBox mapping and the
//! Y flip, so nothing here may apply them a second time.
//!
//! usvg is built without its `text` feature (it would add ~1.2 MB of font
//! machinery to the WASM bundle), so `<text>` is dropped by usvg. It, and
//! every other construct this lowering does not draw, is reported by name
//! instead of disappearing silently.

use super::{GradientKind, SvgCommand, SvgGradient, ViewBox};
use crate::style::{Color, GradientStop};
use usvg::roxmltree;

/// The result of importing one SVG: drawing commands plus a warning for
/// each construct that was not drawn as written.
pub struct SvgImport {
    pub commands: Vec<SvgCommand>,
    pub warnings: Vec<String>,
}

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// Root attributes that set the viewport. The engine's own Svg node owns
/// the viewport (its width, height and viewBox), so these are dropped
/// from a full `<svg>` document before usvg sees it.
const VIEWPORT_ATTRS: &[&str] = &[
    "width",
    "height",
    "viewBox",
    "preserveAspectRatio",
    "x",
    "y",
];

/// Import SVG markup. `content` is either the inner markup of an `<svg>`
/// element or a whole `<svg>` document. Falls back to the basic parser,
/// with a warning, when usvg cannot parse the markup.
pub fn import_svg(content: &str, view_box: ViewBox) -> SvgImport {
    let mut warnings = Vec::new();
    let source = match prepare_document(content, view_box) {
        Ok(s) => s,
        Err(e) => return fallback(content, view_box, e, warnings),
    };
    if let Ok(doc) = roxmltree::Document::parse(&source) {
        scan_dropped_elements(&doc, &mut warnings);
    }

    // usvg reads `<image href="file.png">` from the local disk by default.
    // Images are not drawn here, and a document must not be able to make
    // the renderer open local files, so both resolvers refuse.
    let opt = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = match usvg::Tree::from_str(&source, &opt) {
        Ok(t) => t,
        Err(e) => return fallback(content, view_box, e.to_string(), warnings),
    };

    let mut lower = Lower {
        commands: Vec::new(),
        warnings,
    };
    lower.group_children(tree.root(), 1.0);
    SvgImport {
        commands: lower.commands,
        warnings: lower.warnings,
    }
}

fn fallback(content: &str, view_box: ViewBox, err: String, mut warnings: Vec<String>) -> SvgImport {
    warnings.push(format!(
        "render defect: SVG could not be parsed ({err}); it is drawn with the basic SVG parser, which ignores CSS, transforms, clip paths and gradients"
    ));
    SvgImport {
        commands: super::parse_svg(content, view_box, view_box.width, view_box.height),
        warnings,
    }
}

/// The viewBox a whole `<svg>` document declares on its root, if any.
pub fn document_view_box(content: &str) -> Option<ViewBox> {
    let trimmed = content.trim_start();
    if !starts_with_svg_root(trimmed) {
        return None;
    }
    let source = with_svg_namespace(trimmed);
    let doc = roxmltree::Document::parse(&source).ok()?;
    doc.root_element()
        .attribute("viewBox")
        .and_then(super::parse_view_box)
        .filter(|vb| vb.width > 0.0 && vb.height > 0.0)
}

fn starts_with_svg_root(s: &str) -> bool {
    let mut rest = s;
    // Skip an XML declaration, comments and a doctype before the root.
    loop {
        rest = rest.trim_start();
        if rest.starts_with("<?") {
            match rest.find("?>") {
                Some(i) => rest = &rest[i + 2..],
                None => return false,
            }
        } else if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(i) => rest = &rest[i + 3..],
                None => return false,
            }
        } else if rest.starts_with("<!") {
            match rest.find('>') {
                Some(i) => rest = &rest[i + 1..],
                None => return false,
            }
        } else {
            break;
        }
    }
    rest.starts_with("<svg")
        && rest[4..].starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
}

/// Markup copied from HTML often omits the SVG namespace, which XML
/// parsing then does not apply. Add it to the root start tag.
fn with_svg_namespace(s: &str) -> String {
    let Some(start) = s.find("<svg") else {
        return s.to_string();
    };
    let tag_end = s[start..].find('>').map(|i| start + i).unwrap_or(s.len());
    let tag = &s[start..tag_end];
    let mut out = String::with_capacity(s.len() + 80);
    out.push_str(&s[..start + 4]);
    if !tag.contains("xmlns=") {
        out.push_str(&format!(" xmlns=\"{SVG_NS}\""));
    }
    if !tag.contains("xmlns:xlink") && s.contains("xlink:") {
        out.push_str(&format!(" xmlns:xlink=\"{XLINK_NS}\""));
    }
    out.push_str(&s[start + 4..]);
    out
}

/// Build the document handed to usvg: a root `<svg>` with no viewBox and
/// width/height equal to the viewBox size, so usvg's root transform is the
/// identity and every coordinate stays in raw user space.
fn prepare_document(content: &str, vb: ViewBox) -> Result<String, String> {
    let w = if vb.width > 0.0 { vb.width } else { 1.0 };
    let h = if vb.height > 0.0 { vb.height } else { 1.0 };
    let trimmed = content.trim_start();
    if !starts_with_svg_root(trimmed) {
        return Ok(format!(
            "<svg xmlns=\"{SVG_NS}\" xmlns:xlink=\"{XLINK_NS}\" width=\"{w}\" height=\"{h}\">{content}</svg>"
        ));
    }

    let source = with_svg_namespace(trimmed);
    let doc = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let mut tag = format!("<svg width=\"{w}\" height=\"{h}\"");
    for ns in root.namespaces() {
        match ns.name() {
            Some(prefix) => tag.push_str(&format!(" xmlns:{prefix}=\"{}\"", escape_attr(ns.uri()))),
            None => tag.push_str(&format!(" xmlns=\"{}\"", escape_attr(ns.uri()))),
        }
    }
    for attr in root.attributes() {
        if attr.namespace().is_none() && VIEWPORT_ATTRS.contains(&attr.name()) {
            continue;
        }
        let name = match attr.namespace() {
            Some(XLINK_NS) => format!("xlink:{}", attr.name()),
            Some(_) => continue,
            None => attr.name().to_string(),
        };
        tag.push_str(&format!(" {name}=\"{}\"", escape_attr(attr.value())));
    }
    tag.push('>');
    let inner = match (root.first_child(), root.last_child()) {
        (Some(first), Some(last)) => &source[first.range().start..last.range().end],
        _ => "",
    };
    Ok(format!("{tag}{inner}</svg>"))
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// Elements usvg drops without leaving a node behind: `<text>` (no text
/// feature), `<image>` (both resolvers refuse) and `<foreignObject>`.
fn scan_dropped_elements(doc: &roxmltree::Document, warnings: &mut Vec<String>) {
    let mut text = false;
    let mut image = false;
    let mut foreign = false;
    for node in doc.descendants().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "text" => text = true,
            "image" => image = true,
            "foreignObject" => foreign = true,
            _ => {}
        }
    }
    if text {
        warnings.push(
            "render defect: SVG <text> is not supported yet; the text is not drawn".to_string(),
        );
    }
    if image {
        warnings.push(
            "render defect: SVG <image> is not supported; the image is not drawn".to_string(),
        );
    }
    if foreign {
        warnings.push(
            "render defect: SVG <foreignObject> is not supported; its content is not drawn"
                .to_string(),
        );
    }
}

struct Lower {
    commands: Vec<SvgCommand>,
    warnings: Vec<String>,
}

fn is_identity(t: &usvg::Transform) -> bool {
    t.is_identity()
}

fn transform_cmd(t: &usvg::Transform) -> SvgCommand {
    SvgCommand::Transform(
        t.sx as f64,
        t.ky as f64,
        t.kx as f64,
        t.sy as f64,
        t.tx as f64,
        t.ty as f64,
    )
}

impl Lower {
    fn warn(&mut self, msg: &str) {
        if !self.warnings.iter().any(|w| w == msg) {
            self.warnings.push(msg.to_string());
        }
    }

    fn group_children(&mut self, group: &usvg::Group, alpha: f64) {
        for child in group.children() {
            match child {
                usvg::Node::Group(g) => self.group(g, alpha),
                usvg::Node::Path(p) => self.path(p, alpha),
                // Unreachable with the refusing resolvers, kept for safety.
                usvg::Node::Image(_) => {
                    self.warn("render defect: SVG <image> is not supported; the image is not drawn")
                }
                // Unreachable without usvg's text feature.
                usvg::Node::Text(_) => self
                    .warn("render defect: SVG <text> is not supported yet; the text is not drawn"),
            }
        }
    }

    fn group(&mut self, g: &usvg::Group, alpha: f64) {
        if g.mask().is_some() {
            self.warn(
                "render defect: SVG <mask> is not supported; the masked content is drawn unmasked",
            );
        }
        if !g.filters().is_empty() {
            self.warn(
                "render defect: SVG <filter> is not supported; the content is drawn unfiltered",
            );
        }
        // Group opacity is applied to each path inside (ExtGState alpha
        // replaces, so it is multiplied down the tree). Overlapping
        // children inside a translucent group therefore show through each
        // other, where a browser composites the group first.
        let alpha = alpha * g.opacity().get() as f64;
        let needs_state = !is_identity(&g.transform()) || g.clip_path().is_some();
        if needs_state {
            self.commands.push(SvgCommand::SaveState);
            if !is_identity(&g.transform()) {
                self.commands.push(transform_cmd(&g.transform()));
            }
            if let Some(clip) = g.clip_path() {
                self.clip_path(clip);
            }
        }
        self.group_children(g, alpha);
        if needs_state {
            self.commands.push(SvgCommand::RestoreState);
        }
    }

    /// Intersect the clip with a `<clipPath>`. Its shapes are baked
    /// through their transforms into one path, so the clip is their union
    /// under the nonzero rule (exact for one shape, and for shapes that
    /// do not overlap with opposite windings).
    fn clip_path(&mut self, clip: &usvg::ClipPath) {
        if let Some(inner) = clip.clip_path() {
            self.clip_path(inner);
        }
        let mut shapes = Vec::new();
        collect_clip_shapes(clip.root(), clip.transform(), &mut shapes);
        if shapes.is_empty() {
            // An empty clip path clips everything away.
            self.commands.push(SvgCommand::MoveTo(0.0, 0.0));
            self.commands.push(SvgCommand::ClosePath);
            self.commands.push(SvgCommand::ClipNonZero);
            return;
        }
        let even_odd = shapes.len() == 1 && shapes[0].1 == usvg::FillRule::EvenOdd;
        for (path, _) in &shapes {
            push_path_data(&mut self.commands, path);
        }
        self.commands.push(if even_odd {
            SvgCommand::ClipEvenOdd
        } else {
            SvgCommand::ClipNonZero
        });
    }

    fn path(&mut self, p: &usvg::Path, alpha: f64) {
        if !p.is_visible() {
            return;
        }
        let fill = p.fill().and_then(|f| self.fill_paint(f, alpha));
        let stroke = p.stroke().and_then(|s| self.stroke_paint(s, alpha));
        if fill.is_none() && stroke.is_none() {
            return;
        }
        let stroke_first = p.paint_order() == usvg::PaintOrder::StrokeAndFill;

        match (fill, stroke) {
            // One solid fill and one stroke at the same alpha: a single B.
            (Some(Paint::Solid(fc, fa, eo)), Some((sc, sa, ref style)))
                if (fa - sa).abs() < 1e-9 && !stroke_first =>
            {
                self.commands.push(SvgCommand::SaveState);
                self.set_alpha(fa);
                self.commands.push(SvgCommand::SetFill(fc.0, fc.1, fc.2));
                self.commands.push(SvgCommand::SetStroke(sc.0, sc.1, sc.2));
                self.stroke_style(style);
                push_path_data(&mut self.commands, p.data());
                self.commands.push(if eo {
                    SvgCommand::FillAndStrokeEvenOdd
                } else {
                    SvgCommand::FillAndStroke
                });
                self.commands.push(SvgCommand::RestoreState);
            }
            (fill, stroke) => {
                if stroke_first {
                    if let Some(s) = &stroke {
                        self.emit_stroke(p, s);
                    }
                    if let Some(f) = fill {
                        self.emit_fill(p, f);
                    }
                } else {
                    if let Some(f) = fill {
                        self.emit_fill(p, f);
                    }
                    if let Some(s) = &stroke {
                        self.emit_stroke(p, s);
                    }
                }
            }
        }
    }

    fn set_alpha(&mut self, a: f64) {
        if a < 1.0 {
            self.commands.push(SvgCommand::SetOpacity(a));
        }
    }

    fn emit_fill(&mut self, p: &usvg::Path, fill: Paint) {
        self.commands.push(SvgCommand::SaveState);
        match fill {
            Paint::Solid(c, a, eo) => {
                self.set_alpha(a);
                self.commands.push(SvgCommand::SetFill(c.0, c.1, c.2));
                push_path_data(&mut self.commands, p.data());
                self.commands.push(if eo {
                    SvgCommand::FillEvenOdd
                } else {
                    SvgCommand::Fill
                });
            }
            Paint::Gradient(g, a, eo) => {
                self.set_alpha(a);
                push_path_data(&mut self.commands, p.data());
                self.commands.push(if eo {
                    SvgCommand::ClipEvenOdd
                } else {
                    SvgCommand::ClipNonZero
                });
                self.commands.push(SvgCommand::PaintGradient(g));
            }
        }
        self.commands.push(SvgCommand::RestoreState);
    }

    fn emit_stroke(&mut self, p: &usvg::Path, s: &((f64, f64, f64), f64, StrokeStyle)) {
        let (c, a, style) = s;
        self.commands.push(SvgCommand::SaveState);
        self.set_alpha(*a);
        self.commands.push(SvgCommand::SetStroke(c.0, c.1, c.2));
        self.stroke_style(style);
        push_path_data(&mut self.commands, p.data());
        self.commands.push(SvgCommand::Stroke);
        self.commands.push(SvgCommand::RestoreState);
    }

    fn stroke_style(&mut self, s: &StrokeStyle) {
        self.commands.push(SvgCommand::SetStrokeWidth(s.width));
        // Always explicit, so a stroke never inherits a cap or join set by
        // content drawn earlier in the same graphics state.
        self.commands.push(SvgCommand::SetLineCap(s.cap));
        self.commands.push(SvgCommand::SetLineJoin(s.join));
        if (s.miter_limit - 4.0).abs() > 1e-9 {
            self.commands.push(SvgCommand::SetMiterLimit(s.miter_limit));
        }
        if let Some((dashes, offset)) = &s.dash {
            self.commands
                .push(SvgCommand::SetDash(dashes.clone(), *offset));
        }
    }

    fn fill_paint(&mut self, f: &usvg::Fill, alpha: f64) -> Option<Paint> {
        let a = alpha * f.opacity().get() as f64;
        let eo = f.rule() == usvg::FillRule::EvenOdd;
        match f.paint() {
            usvg::Paint::Color(c) => Some(Paint::Solid(rgb(*c), a, eo)),
            usvg::Paint::LinearGradient(lg) => {
                let kind = GradientKind::Linear {
                    x1: lg.x1() as f64,
                    y1: lg.y1() as f64,
                    x2: lg.x2() as f64,
                    y2: lg.y2() as f64,
                };
                let (g, stop_alpha) = self.gradient(kind, lg)?;
                Some(Paint::Gradient(g, a * stop_alpha, eo))
            }
            usvg::Paint::RadialGradient(rg) => {
                let kind = GradientKind::Radial {
                    cx: rg.cx() as f64,
                    cy: rg.cy() as f64,
                    r: rg.r().get() as f64,
                    fx: rg.fx() as f64,
                    fy: rg.fy() as f64,
                    fr: rg.fr().get() as f64,
                };
                let (g, stop_alpha) = self.gradient(kind, rg)?;
                Some(Paint::Gradient(g, a * stop_alpha, eo))
            }
            usvg::Paint::Pattern(_) => {
                self.warn(
                    "render defect: SVG <pattern> fills are not supported; the fill is not drawn",
                );
                None
            }
        }
    }

    fn stroke_paint(
        &mut self,
        s: &usvg::Stroke,
        alpha: f64,
    ) -> Option<((f64, f64, f64), f64, StrokeStyle)> {
        let a = alpha * s.opacity().get() as f64;
        let color = match s.paint() {
            usvg::Paint::Color(c) => rgb(*c),
            usvg::Paint::LinearGradient(_) | usvg::Paint::RadialGradient(_) => {
                self.warn("render defect: SVG gradient strokes are not supported; the stroke uses the first stop color");
                match s.paint() {
                    usvg::Paint::LinearGradient(g) => first_stop(g),
                    usvg::Paint::RadialGradient(g) => first_stop(g),
                    _ => (0.0, 0.0, 0.0),
                }
            }
            usvg::Paint::Pattern(_) => {
                self.warn("render defect: SVG <pattern> strokes are not supported; the stroke is not drawn");
                return None;
            }
        };
        let style = StrokeStyle {
            width: s.width().get() as f64,
            cap: match s.linecap() {
                usvg::LineCap::Butt => 0,
                usvg::LineCap::Round => 1,
                usvg::LineCap::Square => 2,
            },
            join: match s.linejoin() {
                usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => 0,
                usvg::LineJoin::Round => 1,
                usvg::LineJoin::Bevel => 2,
            },
            miter_limit: s.miterlimit().get() as f64,
            dash: s
                .dasharray()
                .map(|d| (d.iter().map(|v| *v as f64).collect(), s.dashoffset() as f64)),
        };
        Some((color, a, style))
    }

    /// Lower a usvg gradient. Returns the gradient and an alpha factor:
    /// PDF shadings carry no alpha, so stop opacity is honored only when
    /// every stop has the same one.
    fn gradient(
        &mut self,
        kind: GradientKind,
        base: &usvg::BaseGradient,
    ) -> Option<(Box<SvgGradient>, f64)> {
        let stops = base.stops();
        if stops.is_empty() {
            return None;
        }
        if base.spread_method() != usvg::SpreadMethod::Pad {
            self.warn("render defect: SVG gradient spreadMethod reflect/repeat is not supported; the gradient pads");
        }
        let first_opacity = stops[0].opacity().get() as f64;
        let uniform = stops
            .iter()
            .all(|s| (s.opacity().get() as f64 - first_opacity).abs() < 1e-6);
        if !uniform {
            self.warn("render defect: SVG gradient stops with differing stop-opacity are not supported; the stops are drawn opaque");
        }
        let stops = stops
            .iter()
            .map(|s| {
                let (r, g, b) = rgb(s.color());
                GradientStop {
                    position: s.offset().get() as f64,
                    color: Color { r, g, b, a: 1.0 },
                }
            })
            .collect();
        let t = base.transform();
        Some((
            Box::new(SvgGradient {
                kind,
                transform: [
                    t.sx as f64,
                    t.ky as f64,
                    t.kx as f64,
                    t.sy as f64,
                    t.tx as f64,
                    t.ty as f64,
                ],
                stops,
            }),
            if uniform { first_opacity } else { 1.0 },
        ))
    }
}

enum Paint {
    Solid((f64, f64, f64), f64, bool),
    Gradient(Box<SvgGradient>, f64, bool),
}

struct StrokeStyle {
    width: f64,
    cap: u32,
    join: u32,
    miter_limit: f64,
    dash: Option<(Vec<f64>, f64)>,
}

fn rgb(c: usvg::Color) -> (f64, f64, f64) {
    (
        c.red as f64 / 255.0,
        c.green as f64 / 255.0,
        c.blue as f64 / 255.0,
    )
}

fn first_stop(g: &usvg::BaseGradient) -> (f64, f64, f64) {
    g.stops()
        .first()
        .map(|s| rgb(s.color()))
        .unwrap_or((0.0, 0.0, 0.0))
}

/// Collect the shapes of a clip path's subtree with their transforms
/// baked in (PDF cannot change the CTM while building a clip path).
fn collect_clip_shapes(
    group: &usvg::Group,
    ts: usvg::Transform,
    out: &mut Vec<(usvg::tiny_skia_path::Path, usvg::FillRule)>,
) {
    for child in group.children() {
        match child {
            usvg::Node::Group(g) => collect_clip_shapes(g, ts.pre_concat(g.transform()), out),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let rule = p
                    .fill()
                    .map(|f| f.rule())
                    .unwrap_or(usvg::FillRule::NonZero);
                if let Some(path) = p.data().clone().transform(ts) {
                    out.push((path, rule));
                }
            }
            _ => {}
        }
    }
}

fn push_path_data(out: &mut Vec<SvgCommand>, path: &usvg::tiny_skia_path::Path) {
    use usvg::tiny_skia_path::PathSegment;
    let mut last = (0.0f64, 0.0f64);
    let mut start = last;
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                last = (p.x as f64, p.y as f64);
                start = last;
                out.push(SvgCommand::MoveTo(last.0, last.1));
            }
            PathSegment::LineTo(p) => {
                last = (p.x as f64, p.y as f64);
                out.push(SvgCommand::LineTo(last.0, last.1));
            }
            PathSegment::QuadTo(c, p) => {
                // Elevate the quadratic to a cubic.
                let (cx, cy) = (c.x as f64, c.y as f64);
                let (x, y) = (p.x as f64, p.y as f64);
                out.push(SvgCommand::CurveTo(
                    last.0 + 2.0 / 3.0 * (cx - last.0),
                    last.1 + 2.0 / 3.0 * (cy - last.1),
                    x + 2.0 / 3.0 * (cx - x),
                    y + 2.0 / 3.0 * (cy - y),
                    x,
                    y,
                ));
                last = (x, y);
            }
            PathSegment::CubicTo(c1, c2, p) => {
                last = (p.x as f64, p.y as f64);
                out.push(SvgCommand::CurveTo(
                    c1.x as f64,
                    c1.y as f64,
                    c2.x as f64,
                    c2.y as f64,
                    last.0,
                    last.1,
                ));
            }
            PathSegment::Close => {
                // Closing returns the current point to the subpath start.
                last = start;
                out.push(SvgCommand::ClosePath);
            }
        }
    }
}
