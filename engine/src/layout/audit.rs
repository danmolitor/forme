//! Post-render content audit (opt-in, `RenderOptions::audit_content`).
//!
//! After layout, verify the laid-out pages against the input document and
//! report — through the render-defect channel — content that was dropped,
//! rendered fully off-page, painted in exactly its background's colour, or
//! clipped to a zero-size box. The engine's own layout tree is the source:
//! no PDF re-parsing, no rasterization.
//!
//! Everything here is a NET under the engine, catching what the engine does
//! not know it dropped. Cases the engine knowingly compromises on already
//! report themselves at the point of the compromise (atomic-row overflow,
//! clamped table columns, ...). The audit's design constraint is false
//! positives: a check that cries wolf gets turned off and then protects
//! nobody — so every check documents its exclusions and prefers a false
//! negative to a false alarm.
//!
//! Exclusions, by construction and by rule:
//! - `display: none` and subset-skipped content never reach the engine
//!   `Document` (the HTML mapper drops them, with their own warnings), so
//!   they cannot false-positive here — the audit compares against what the
//!   pipeline actually committed to render.
//! - `Fixed` (running headers/footers/margin boxes) and `Watermark`
//!   subtrees are excluded from the text accounting: their repetition is
//!   per-page and their parity/page-name scoping can legitimately render
//!   them zero times.
//! - Nodes with `text-overflow: ellipsis/clip` requested truncation; their
//!   text is excluded from the accounting.
//! - Page-number placeholders are substituted at render; the placeholder
//!   tokens are stripped from the expected text.
//! - Ligatures are exact, not approximated: glyphs carry their full
//!   cluster text (`PositionedGlyph::cluster_text`), the same
//!   reconstruction `LayoutInfo` uses.
//! - Comparison is case-folded, so `text-transform` needs no re-resolution
//!   (and a wrong-case rendering is out of scope).
//! - Off-page checks skip subtrees under `overflow: hidden` (deliberate
//!   clipping) and skip the horizontal direction when the page carries the
//!   `clip_content_x` body clip — the off-viewport-parking idiom is then
//!   deliberately invisible, exactly as in a browser.
//! - Colour checks fire only when the covering paint is a fully-containing
//!   solid at full opacity (or no paint at all, against the white page);
//!   gradients, images, partial overlaps, and any translucency make the
//!   ground "unknowable" and the check stays silent.

use std::collections::HashMap;

use crate::model::{Document, Node, NodeKind};
use crate::style::{Color, Overflow, TextOverflow};

use super::{DrawCommand, LayoutElement, LayoutPage};

const TAG: &str = "(content audit)";
const EPS: f64 = 1.0; // geometric slack, points
const COLOR_EPS: f64 = 0.5 / 255.0;

/// Entry point. Appends `render defect: ... (content audit)` strings.
pub fn audit_content(document: &Document, pages: &[LayoutPage], warnings: &mut Vec<String>) {
    audit_missing_text(document, pages, warnings);
    for (page_idx, page) in pages.iter().enumerate() {
        let mut ctx = PageCtx {
            page,
            page_no: page_idx + 1,
            paints: Vec::new(),
            warnings,
        };
        for el in &page.elements {
            walk_element(el, &mut ctx, false, 1.0);
        }
    }
    dedup(warnings);
}

fn dedup(warnings: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    warnings.retain(|w| !w.contains(TAG) || seen.insert(w.clone()));
}

// ── check 1: source text that rendered nowhere ───────────────────────────

fn audit_missing_text(document: &Document, pages: &[LayoutPage], warnings: &mut Vec<String>) {
    let mut rendered: HashMap<char, i64> = HashMap::new();
    for page in pages {
        for el in &page.elements {
            collect_rendered(el, &mut rendered);
        }
    }

    let mut expected: Vec<(Vec<char>, String)> = Vec::new(); // (folded chars, snippet)
    for node in &document.children {
        collect_expected(node, &mut expected);
    }

    let mut missing_total: i64 = 0;
    let mut first_snippet: Option<String> = None;
    for (chars, snippet) in &expected {
        let mut node_missing = 0i64;
        for c in chars {
            match rendered.get_mut(c) {
                Some(n) if *n > 0 => *n -= 1,
                _ => node_missing += 1,
            }
        }
        if node_missing > 0 {
            missing_total += node_missing;
            if first_snippet.is_none() {
                first_snippet = Some(snippet.clone());
            }
        }
    }
    if missing_total > 0 {
        let snippet = first_snippet.unwrap_or_default();
        warnings.push(format!(
            "render defect: {missing_total} character(s) of source text did not render — first missing in a node starting \"{snippet}\" {TAG}"
        ));
    }
}

fn collect_rendered(el: &LayoutElement, out: &mut HashMap<char, i64>) {
    if let DrawCommand::Text { lines, .. } = &el.draw {
        for line in lines {
            // Ligature glyphs carry their full cluster in `cluster_text`
            // (the same reconstruction `LayoutInfo` uses), so the
            // accounting is exact even under shaping.
            let raw: String = line
                .glyphs
                .iter()
                .flat_map(|g| {
                    g.cluster_text
                        .as_deref()
                        .unwrap_or("")
                        .chars()
                        .chain(if g.cluster_text.is_none() {
                            Some(g.char_value)
                        } else {
                            None
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            for c in fold(&raw) {
                *out.entry(c).or_insert(0) += 1;
            }
        }
    }
    for child in &el.children {
        collect_rendered(child, out);
    }
}

fn collect_expected(node: &Node, out: &mut Vec<(Vec<char>, String)>) {
    match &node.kind {
        // Per-page furniture: repetition and parity/page-name scoping make
        // per-document accounting meaningless (a `:left`-only margin box on
        // a one-page document legitimately renders zero times).
        NodeKind::Fixed { .. } | NodeKind::Watermark { .. } => return,
        NodeKind::Text { content, runs, .. } | NodeKind::Heading { content, runs, .. } => {
            // Requested truncation: absence is the feature.
            if matches!(
                node.style.text_overflow,
                Some(TextOverflow::Ellipsis) | Some(TextOverflow::Clip)
            ) {
                return;
            }
            let text: String = if runs.is_empty() {
                content.clone()
            } else {
                runs.iter().map(|r| r.content.as_str()).collect()
            };
            // Substituted at render time with the live page numbers.
            let text = text
                .replace("{{pageNumber}}", "")
                .replace("{{totalPages}}", "");
            let folded = fold(&text);
            if !folded.is_empty() {
                out.push((folded, snippet(&text)));
            }
        }
        _ => {}
    }
    for child in &node.children {
        collect_expected(child, out);
    }
}

/// Case-fold and drop whitespace and invisible formatting characters.
/// Ligatures need no special handling: glyphs carry their full cluster
/// text (`PositionedGlyph::cluster_text`), so the rendered side is
/// reconstructed exactly. Case-folding makes the accounting immune to
/// `text-transform` without re-resolving styles (a wrong-CASE rendering
/// is out of scope by the same token).
fn fold(text: &str) -> Vec<char> {
    text.to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{00AD}' && *c != '\u{200B}')
        .collect()
}

fn snippet(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s: String = collapsed.chars().take(48).collect();
    if collapsed.chars().count() > 48 {
        s.push('…');
    }
    s.replace('"', "'")
}

// ── checks 2–4: geometry and colour, one pre-order pass per page ─────────

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn of(el: &LayoutElement) -> Rect {
        Rect {
            x: el.x,
            y: el.y,
            w: el.width,
            h: el.height,
        }
    }
    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    fn contains(&self, o: &Rect) -> bool {
        self.x <= o.x + EPS
            && self.y <= o.y + EPS
            && self.x + self.w >= o.x + o.w - EPS
            && self.y + self.h >= o.y + o.h - EPS
    }
}

enum Paint {
    /// Fully opaque solid rectangle — a knowable ground.
    Solid(Rect, Color),
    /// Image, gradient, or translucent paint — ground unknowable; any
    /// overlap silences the colour check rather than guessing.
    Unknown(Rect),
}

struct PageCtx<'a> {
    page: &'a LayoutPage,
    page_no: usize,
    paints: Vec<Paint>,
    warnings: &'a mut Vec<String>,
}

fn walk_element(el: &LayoutElement, ctx: &mut PageCtx, clipped: bool, opacity: f64) {
    let opacity = opacity * el.opacity;
    let rect = Rect::of(el);

    // Check 4: a zero-size clipping box swallowing real content. Clipping
    // to literally nothing is never a deliberate design.
    if el.overflow == Overflow::Hidden
        && (el.width <= 0.5 || el.height <= 0.5)
        && subtree_has_content(el)
    {
        ctx.warnings.push(format!(
            "render defect: content clipped to a zero-size box ({:.0}x{:.0}pt) {TAG}",
            el.width, el.height
        ));
    }

    if let DrawCommand::Text { lines, color, .. } = &el.draw {
        let has_glyphs = lines.iter().any(|l| !l.glyphs.is_empty());
        if has_glyphs {
            audit_text_element(el, rect, *color, ctx, clipped, opacity);
        }
    }

    // Register this element as painted ground for later siblings/descendants.
    match &el.draw {
        DrawCommand::Rect {
            background,
            background_gradient,
            opacity: rect_opacity,
            ..
        } => {
            let cumulative = opacity * rect_opacity;
            match (background, background_gradient) {
                (Some(c), None) if cumulative >= 0.999 && c.a >= 0.999 => {
                    ctx.paints.push(Paint::Solid(rect, *c));
                }
                (Some(_), _) | (None, Some(_)) => ctx.paints.push(Paint::Unknown(rect)),
                (None, None) => {} // border-only: covers nothing
            }
        }
        DrawCommand::Image { .. } | DrawCommand::ImagePlaceholder | DrawCommand::Svg { .. } => {
            ctx.paints.push(Paint::Unknown(rect));
        }
        _ => {}
    }

    let clipped = clipped || el.overflow == Overflow::Hidden;
    for child in &el.children {
        walk_element(child, ctx, clipped, opacity);
    }
}

fn audit_text_element(
    el: &LayoutElement,
    rect: Rect,
    color: Color,
    ctx: &mut PageCtx,
    clipped: bool,
    opacity: f64,
) {
    // Check 2: rendered fully off the page. Skipped under overflow:hidden
    // (deliberate clipping); the horizontal direction is skipped under the
    // page-level body clip, where off-viewport parking is deliberate.
    if !clipped {
        let page_w = ctx.page.width;
        let page_h = ctx.page.height;
        let off_x = rect.x + rect.w <= EPS || rect.x >= page_w - EPS;
        let off_y = rect.y + rect.h <= EPS || rect.y >= page_h - EPS;
        let x_clip = ctx.page.config.clip_content_x;
        if (off_x && !x_clip) || off_y {
            ctx.warnings.push(format!(
                "render defect: text rendered fully off-page (page {} at {:.0},{:.0}; page is {:.0}x{:.0}pt) — \"{}\" {TAG}",
                ctx.page_no, rect.x, rect.y, page_w, page_h,
                element_text_snippet(el)
            ));
            return; // off-page text needs no colour check
        }
    }

    // Check 3: text painted in exactly the colour of the ground under it.
    if opacity < 0.999 {
        return; // translucent text: ground blending is unknowable here
    }
    if color.a <= 0.001 {
        ctx.warnings.push(format!(
            "render defect: invisible text (fully transparent colour) — \"{}\" {TAG}",
            element_text_snippet(el)
        ));
        return;
    }
    let last_overlap = ctx.paints.iter().rev().find(|p| match p {
        Paint::Solid(r, _) | Paint::Unknown(r) => r.overlaps(&rect),
    });
    match last_overlap {
        None => {
            // Nothing painted under it: the ground is the white page.
            if color_eq(color, Color::white()) {
                ctx.warnings.push(format!(
                    "render defect: invisible text (white on the white page) — \"{}\" {TAG}",
                    element_text_snippet(el)
                ));
            }
        }
        Some(Paint::Solid(r, bg)) if r.contains(&rect) && color_eq(color, *bg) => {
            ctx.warnings.push(format!(
                "render defect: invisible text (colour matches its background) — \"{}\" {TAG}",
                element_text_snippet(el)
            ));
        }
        // Partial overlap or unknowable paint: stay silent.
        _ => {}
    }
}

fn color_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < COLOR_EPS && (a.g - b.g).abs() < COLOR_EPS && (a.b - b.b).abs() < COLOR_EPS
}

fn subtree_has_content(el: &LayoutElement) -> bool {
    match &el.draw {
        DrawCommand::Text { lines, .. } => lines.iter().any(|l| !l.glyphs.is_empty()),
        DrawCommand::Image { .. } | DrawCommand::Svg { .. } => true,
        _ => el.children.iter().any(subtree_has_content),
    }
}

fn element_text_snippet(el: &LayoutElement) -> String {
    let mut text = String::new();
    collect_element_text(el, &mut text);
    snippet(&text)
}

fn collect_element_text(el: &LayoutElement, out: &mut String) {
    if let DrawCommand::Text { lines, .. } = &el.draw {
        for line in lines {
            if out.len() > 64 {
                return;
            }
            for g in &line.glyphs {
                match &g.cluster_text {
                    Some(t) => out.push_str(t),
                    None => out.push(g.char_value),
                }
            }
            out.push(' ');
        }
    }
    for child in &el.children {
        collect_element_text(child, out);
    }
}

impl Color {
    fn white() -> Color {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Metadata, PageConfig};

    fn text_node(content: &str) -> Node {
        Node {
            kind: NodeKind::Text {
                content: content.to_string(),
                href: None,
                runs: vec![],
            },
            style: Default::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    fn doc(children: Vec<Node>) -> Document {
        Document {
            children,
            metadata: Metadata::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            named_pages: Default::default(),
            fonts: vec![],
            default_style: None,
            tagged: false,
            pdfa: None,
            pdf_ua: false,
            embedded_data: None,
            attachments: vec![],
            zugferd: None,
            flatten_forms: false,
            certification: None,
        }
    }

    /// The core net: layout a document, then delete a text element from the
    /// laid-out pages (simulating an engine bug that silently drops
    /// content), and the audit must name the missing text.
    #[test]
    fn deleted_text_element_is_reported_missing() {
        let d = doc(vec![
            text_node("the quick brown fox"),
            text_node("jumps over the lazy dog"),
        ]);
        let fc = crate::font::FontContext::new();
        let engine = super::super::LayoutEngine::new();
        let mut pages = engine.layout(&d, &fc);

        // Sanity: intact pages audit clean.
        let mut clean = Vec::new();
        audit_content(&d, &pages, &mut clean);
        assert!(clean.is_empty(), "intact layout flagged: {clean:?}");

        // Remove the first text container from page 1.
        fn remove_first_text(els: &mut Vec<LayoutElement>) -> bool {
            for i in 0..els.len() {
                if matches!(els[i].draw, DrawCommand::Text { .. }) {
                    els.remove(i);
                    return true;
                }
                if remove_first_text(&mut els[i].children) {
                    return true;
                }
            }
            false
        }
        assert!(remove_first_text(&mut pages[0].elements));

        let mut w = Vec::new();
        audit_content(&d, &pages, &mut w);
        assert!(
            w.iter()
                .any(|m| m.contains("did not render") && m.contains("the quick brown fox")),
            "dropped text not reported: {w:?}"
        );
    }

    fn page_with(elements: Vec<LayoutElement>) -> LayoutPage {
        LayoutPage {
            width: 595.0,
            height: 842.0,
            elements,
            fixed_header: vec![],
            fixed_footer: vec![],
            watermarks: vec![],
            config: PageConfig::default(),
            page_name: None,
        }
    }

    fn text_element(x: f64, y: f64, w: f64, h: f64, color: Color, text: &str) -> LayoutElement {
        use super::super::{PositionedGlyph, TextLine};
        use crate::style::{FontStyle, TextDecoration};
        let glyphs = text
            .chars()
            .map(|c| PositionedGlyph {
                glyph_id: c as u16,
                x_offset: 0.0,
                y_offset: 0.0,
                x_advance: 6.0,
                font_size: 12.0,
                font_family: "Helvetica".into(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                char_value: c,
                color: None,
                href: None,
                text_decoration: TextDecoration::None,
                letter_spacing: 0.0,
                cluster_text: None,
            })
            .collect();
        LayoutElement {
            x,
            y,
            width: w,
            height: h,
            draw: DrawCommand::Text {
                lines: vec![TextLine {
                    x,
                    y,
                    glyphs,
                    width: w,
                    height: h,
                    word_spacing: 0.0,
                }],
                color,
                text_decoration: TextDecoration::None,
                opacity: 1.0,
            },
            children: vec![],
            node_type: None,
            resolved_style: None,
            source_location: None,
            href: None,
            bookmark: None,
            alt: None,
            is_header_row: false,
            col_span: 1,
            overflow: Overflow::Visible,
            opacity: 1.0,
        }
    }

    #[test]
    fn zero_size_clipping_box_with_content_flags() {
        let mut clipper = text_element(
            10.0,
            10.0,
            100.0,
            0.0,
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            "",
        );
        clipper.draw = DrawCommand::None;
        clipper.overflow = Overflow::Hidden;
        clipper.children = vec![text_element(
            10.0,
            10.0,
            100.0,
            14.0,
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            "swallowed",
        )];
        let pages = vec![page_with(vec![clipper])];
        let mut w = Vec::new();
        audit_content(&doc(vec![]), &pages, &mut w);
        assert!(
            w.iter().any(|m| m.contains("zero-size")),
            "zero-size clip not flagged: {w:?}"
        );
    }

    #[test]
    fn fully_transparent_text_flags() {
        let el = text_element(
            10.0,
            10.0,
            100.0,
            14.0,
            Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 0.0,
            },
            "vanished",
        );
        let pages = vec![page_with(vec![el])];
        let mut w = Vec::new();
        audit_content(&doc(vec![]), &pages, &mut w);
        assert!(
            w.iter().any(|m| m.contains("transparent")),
            "alpha-zero text not flagged: {w:?}"
        );
    }

    #[test]
    fn fold_is_case_and_whitespace_insensitive() {
        assert_eq!(fold("Office  File"), fold("office\nfile"));
        assert_eq!(fold("soft\u{00AD}hyphen"), fold("softhyphen"));
    }
}
