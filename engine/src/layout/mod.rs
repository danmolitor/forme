//! # Page-Aware Layout Engine
//!
//! This is the heart of Forme and the reason it exists.
//!
//! ## The Problem With Every Other Engine
//!
//! Most PDF renderers do this:
//! 1. Lay out all content on an infinitely tall canvas
//! 2. Slice the canvas into pages
//! 3. Try to fix the things that broke at slice points
//!
//! Step 3 is where everything falls apart. Flexbox layouts collapse because
//! the flex algorithm ran on the pre-sliced dimensions. Table rows get split
//! in the wrong places. Headers don't repeat. Content gets "mashed together."
//!
//! ## How Forme Works
//!
//! Forme never creates an infinite canvas. The layout algorithm is:
//!
//! 1. Open a page with known dimensions and remaining space
//! 2. Place each child node. Before placing, ask: "does this fit?"
//! 3. If it fits: place it, reduce remaining space
//! 4. If it doesn't fit and is unbreakable: start a new page, place it there
//! 5. If it doesn't fit and is breakable: place what fits, split the rest
//!    to a new page, and RE-RUN flex layout on both fragments
//! 6. For tables: when splitting, clone the header rows onto the new page
//!
//! The key insight in step 5: when a flex container splits across pages,
//! BOTH fragments get their own independent flex layout pass. This is why
//! react-pdf's flex breaks on page wrap — it runs flex once on the whole
//! container and then slices, so the flex calculations are wrong on both
//! halves. We run flex AFTER splitting.

pub mod flex;
pub mod grid;
pub mod page_break;

use std::cell::RefCell;
use std::collections::HashMap;

use serde::Serialize;

use crate::font::FontContext;
use crate::model::*;
use crate::style::*;
use crate::text::bidi;
use crate::text::shaping;
use crate::text::{BrokenLine, RunBrokenLine, StyledChar, TextLayout};

/// A bookmark entry collected during layout.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkEntry {
    pub title: String,
    pub page_index: usize,
    pub y: f64,
}

// ── Serializable layout metadata (for debug overlays / dev tools) ───

/// Complete layout metadata for all pages.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutInfo {
    pub pages: Vec<PageInfo>,
}

/// Layout metadata for a single page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub width: f64,
    pub height: f64,
    pub content_x: f64,
    pub content_y: f64,
    pub content_width: f64,
    pub content_height: f64,
    pub elements: Vec<ElementInfo>,
}

/// Serializable snapshot of ResolvedStyle for the inspector panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyleInfo {
    // Box model
    pub margin: Edges,
    pub padding: Edges,
    pub border_width: Edges,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f64>,
    // Flex
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<AlignItems>,
    pub flex_wrap: FlexWrap,
    pub align_content: AlignContent,
    pub flex_grow: f64,
    pub flex_shrink: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_basis: Option<String>,
    pub gap: f64,
    pub row_gap: f64,
    pub column_gap: f64,
    // Text
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: u32,
    pub font_style: FontStyle,
    pub line_height: f64,
    pub text_align: TextAlign,
    pub letter_spacing: f64,
    pub text_decoration: TextDecoration,
    pub text_transform: TextTransform,
    // Visual
    pub color: Color,
    pub background_color: Option<Color>,
    pub border_color: EdgeValues<Color>,
    pub border_style: EdgeValues<crate::style::BorderStyle>,
    pub border_radius: CornerValues,
    pub opacity: f64,
    // Positioning
    pub position: Position,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<f64>,
    // Overflow
    pub overflow: Overflow,
    // Page behavior
    pub breakable: bool,
    pub break_before: bool,
    pub min_widow_lines: u32,
    pub min_orphan_lines: u32,
}

fn size_constraint_to_str(sc: &SizeConstraint) -> Option<String> {
    match sc {
        SizeConstraint::Auto => None,
        SizeConstraint::Fixed(v) => Some(format!("{v}")),
    }
}

impl ElementStyleInfo {
    fn from_resolved(style: &ResolvedStyle) -> Self {
        ElementStyleInfo {
            margin: style.margin.to_edges(),
            padding: style.padding,
            border_width: style.border_width,
            width: size_constraint_to_str(&style.width),
            height: size_constraint_to_str(&style.height),
            min_width: if style.min_width > 0.0 {
                Some(style.min_width)
            } else {
                None
            },
            min_height: if style.min_height > 0.0 {
                Some(style.min_height)
            } else {
                None
            },
            max_width: if style.max_width.is_finite() {
                Some(style.max_width)
            } else {
                None
            },
            max_height: if style.max_height.is_finite() {
                Some(style.max_height)
            } else {
                None
            },
            flex_direction: style.flex_direction,
            justify_content: style.justify_content,
            align_items: style.align_items,
            align_self: style.align_self,
            flex_wrap: style.flex_wrap,
            align_content: style.align_content,
            flex_grow: style.flex_grow,
            flex_shrink: style.flex_shrink,
            flex_basis: size_constraint_to_str(&style.flex_basis),
            gap: style.gap,
            row_gap: style.row_gap,
            column_gap: style.column_gap,
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            font_weight: style.font_weight,
            font_style: style.font_style,
            line_height: style.line_height,
            text_align: style.text_align,
            letter_spacing: style.letter_spacing,
            text_decoration: style.text_decoration,
            text_transform: style.text_transform,
            color: style.color,
            background_color: style.background_color,
            border_color: style.border_color,
            border_style: style.border_style,
            border_radius: style.border_radius,
            opacity: style.opacity,
            position: style.position,
            top: style.top,
            right: style.right,
            bottom: style.bottom,
            left: style.left,
            overflow: style.overflow,
            breakable: style.breakable,
            break_before: style.break_before,
            min_widow_lines: style.min_widow_lines,
            min_orphan_lines: style.min_orphan_lines,
        }
    }
}

impl Default for ElementStyleInfo {
    fn default() -> Self {
        ElementStyleInfo {
            margin: Edges::default(),
            padding: Edges::default(),
            border_width: Edges::default(),
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            flex_direction: FlexDirection::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_self: None,
            flex_wrap: FlexWrap::default(),
            align_content: AlignContent::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            gap: 0.0,
            row_gap: 0.0,
            column_gap: 0.0,
            font_family: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: 400,
            font_style: FontStyle::default(),
            line_height: 1.4,
            text_align: TextAlign::default(),
            letter_spacing: 0.0,
            text_decoration: TextDecoration::None,
            text_transform: TextTransform::None,
            color: Color::BLACK,
            background_color: None,
            border_color: EdgeValues::uniform(Color::BLACK),
            border_style: EdgeValues::uniform(crate::style::BorderStyle::Solid),
            border_radius: CornerValues::uniform(0.0),
            opacity: 1.0,
            position: Position::default(),
            top: None,
            right: None,
            bottom: None,
            left: None,
            overflow: Overflow::default(),
            breakable: false,
            break_before: false,
            min_widow_lines: 2,
            min_orphan_lines: 2,
        }
    }
}

/// Layout metadata for a single positioned element (hierarchical).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// DrawCommand-based kind (Rect, Text, Image, etc.) for backward compat.
    pub kind: String,
    /// Logical node type (View, Text, Image, TableRow, etc.).
    pub node_type: String,
    /// Resolved style snapshot for the inspector panel.
    pub style: ElementStyleInfo,
    /// Child elements (preserves hierarchy).
    pub children: Vec<ElementInfo>,
    /// Source code location for click-to-source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,
    /// Text content extracted from TextLine draw commands (for component tree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    /// Optional hyperlink URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// Optional bookmark title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,
}

impl LayoutInfo {
    /// Extract serializable layout metadata from laid-out pages.
    pub fn from_pages(pages: &[LayoutPage]) -> Self {
        LayoutInfo {
            pages: pages
                .iter()
                .map(|page| {
                    let (page_w, page_h) = page.config.size.dimensions();
                    let content_x = page.config.margin.left;
                    let content_y = page.config.margin.top;
                    let content_width = page_w - page.config.margin.horizontal();
                    let content_height = page_h - page.config.margin.vertical();

                    let elements = Self::build_element_tree(&page.elements);

                    PageInfo {
                        width: page_w,
                        height: page_h,
                        content_x,
                        content_y,
                        content_width,
                        content_height,
                        elements,
                    }
                })
                .collect(),
        }
    }

    fn build_element_tree(elems: &[LayoutElement]) -> Vec<ElementInfo> {
        elems
            .iter()
            .map(|elem| {
                let kind = match &elem.draw {
                    DrawCommand::None => "None",
                    DrawCommand::Rect { .. } => "Rect",
                    DrawCommand::Text { .. } => "Text",
                    DrawCommand::Image { .. } => "Image",
                    DrawCommand::ImagePlaceholder => "ImagePlaceholder",
                    DrawCommand::Svg { .. } => "Svg",
                    DrawCommand::Barcode { .. } => "Barcode",
                    DrawCommand::QrCode { .. } => "QrCode",
                    DrawCommand::Chart { .. } => "Chart",
                    DrawCommand::Watermark { .. } => "Watermark",
                    DrawCommand::FormField { .. } => "FormField",
                };
                let text_content = match &elem.draw {
                    DrawCommand::Text { lines, .. } => {
                        let text: String = lines
                            .iter()
                            .flat_map(|line| {
                                line.glyphs.iter().flat_map(|g| {
                                    // Use cluster_text for ligatures (e.g., "fi" → 2 chars)
                                    g.cluster_text.as_deref().unwrap_or("").chars().chain(
                                        if g.cluster_text.is_none() {
                                            Some(g.char_value)
                                        } else {
                                            None
                                        },
                                    )
                                })
                            })
                            .collect();
                        if text.is_empty() {
                            None
                        } else {
                            Some(text)
                        }
                    }
                    _ => None,
                };
                let node_type = elem.node_type.clone().unwrap_or_else(|| kind.to_string());
                let style = elem
                    .resolved_style
                    .as_ref()
                    .map(ElementStyleInfo::from_resolved)
                    .unwrap_or_default();
                ElementInfo {
                    x: elem.x,
                    y: elem.y,
                    width: elem.width,
                    height: elem.height,
                    kind: kind.to_string(),
                    node_type,
                    style,
                    children: Self::build_element_tree(&elem.children),
                    source_location: elem.source_location.clone(),
                    text_content,
                    href: elem.href.clone(),
                    bookmark: elem.bookmark.clone(),
                }
            })
            .collect()
    }
}

/// A fully laid-out page ready for PDF serialization.
#[derive(Debug, Clone)]
pub struct LayoutPage {
    pub width: f64,
    pub height: f64,
    pub elements: Vec<LayoutElement>,
    /// Fixed header nodes to inject after layout (internal use).
    pub(crate) fixed_header: Vec<(Node, f64)>,
    /// Fixed footer nodes to inject after layout (internal use).
    pub(crate) fixed_footer: Vec<(Node, f64)>,
    /// Watermark nodes to inject after layout (internal use).
    pub(crate) watermarks: Vec<Node>,
    /// Page config needed for fixed element layout (internal use).
    pub(crate) config: PageConfig,
}

/// A positioned element on a page.
#[derive(Debug, Clone)]
pub struct LayoutElement {
    /// Absolute position on the page (top-left corner).
    pub x: f64,
    pub y: f64,
    /// Dimensions including padding and border, excluding margin.
    pub width: f64,
    pub height: f64,
    /// The visual properties to draw.
    pub draw: DrawCommand,
    /// Child elements (positioned relative to page, not parent).
    pub children: Vec<LayoutElement>,
    /// Logical node type for dev tools (e.g. "View", "Text", "Image").
    pub node_type: Option<String>,
    /// Resolved style snapshot for inspector panel.
    pub resolved_style: Option<ResolvedStyle>,
    /// Source code location for click-to-source in the dev inspector.
    pub source_location: Option<SourceLocation>,
    /// Optional hyperlink URL for link annotations.
    pub href: Option<String>,
    /// Optional bookmark title for PDF outline entries.
    pub bookmark: Option<String>,
    /// Optional alt text for images and SVGs (accessibility).
    pub alt: Option<String>,
    /// Whether this is a table header row (for tagged PDF: TH vs TD).
    pub is_header_row: bool,
    /// Number of columns this table cell spans (for tagged PDF: /ColSpan).
    /// 1 for every non-cell element and for unspanned cells.
    pub col_span: u32,
    /// Overflow behavior (Visible or Hidden). When Hidden, PDF clips children.
    pub overflow: Overflow,
    /// Opacity for the entire element including its children (0.0–1.0). The
    /// PDF serializer wraps `write_element` in a `q\n/GS{n} gs ... Q` block
    /// when this is < 1.0, so descendants render at the cumulative alpha.
    /// Default is 1.0 (no extra wrap).
    pub opacity: f64,
}

/// Return a human-readable name for a NodeKind variant.
fn node_kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::View => "View",
        NodeKind::Text { .. } => "Text",
        NodeKind::Heading { level: 1, .. } => "H1",
        NodeKind::Heading { level: 2, .. } => "H2",
        NodeKind::Heading { level: 3, .. } => "H3",
        NodeKind::Heading { level: 4, .. } => "H4",
        NodeKind::Heading { level: 5, .. } => "H5",
        // Default clamps levels outside 1..=6 to H6 (matches HTML's
        // tolerance: invalid levels still render as the deepest heading
        // rather than vanishing).
        NodeKind::Heading { .. } => "H6",
        NodeKind::List { .. } => "List",
        NodeKind::ListItem => "ListItem",
        NodeKind::Image { .. } => "Image",
        NodeKind::Table { .. } => "Table",
        NodeKind::TableRow { .. } => "TableRow",
        NodeKind::TableCell { .. } => "TableCell",
        NodeKind::Fixed {
            position: FixedPosition::Header,
            ..
        } => "FixedHeader",
        NodeKind::Fixed {
            position: FixedPosition::Footer,
            ..
        } => "FixedFooter",
        NodeKind::Page { .. } => "Page",
        NodeKind::PageBreak => "PageBreak",
        NodeKind::Svg { .. } => "Svg",
        NodeKind::Canvas { .. } => "Canvas",
        NodeKind::Barcode { .. } => "Barcode",
        NodeKind::QrCode { .. } => "QrCode",
        NodeKind::BarChart { .. } => "BarChart",
        NodeKind::LineChart { .. } => "LineChart",
        NodeKind::PieChart { .. } => "PieChart",
        NodeKind::AreaChart { .. } => "AreaChart",
        NodeKind::DotPlot { .. } => "DotPlot",
        NodeKind::Watermark { .. } => "Watermark",
        NodeKind::TextField { .. } => "TextField",
        NodeKind::Checkbox { .. } => "Checkbox",
        NodeKind::Dropdown { .. } => "Dropdown",
        NodeKind::RadioButton { .. } => "RadioButton",
    }
}

/// Build the zero-height marker element that carries a container's `bookmark`
/// into the PDF outline, or `None` if the node has no bookmark.
///
/// Both container paths (`layout_view`'s fits branch and `layout_breakable_view`)
/// go through this so the marker is the *single* carrier of the bookmark on
/// either path. That matters twice over:
///
/// - `layout_breakable_view` can skip building a wrapper entirely (see
///   `needs_wrapper`), so without a marker an unstyled overflowing view would
///   lose its bookmark outright.
/// - `collect_bookmarks` walks every element and every descendant, so leaving
///   the bookmark on *both* the marker and its enclosing wrapper emits the
///   outline entry twice. One carrier, one entry.
///
/// `node_type` must be set explicitly: leaving it `None` makes the LayoutInfo
/// serializer fall back to `kind.to_string()`, which leaks the
/// `DrawCommand::None` variant name into `nodeType` as the string "None" — not
/// a value in the public `ElementNodeType` union.
fn bookmark_marker(node: &Node, x: f64, y: f64) -> Option<LayoutElement> {
    node.bookmark.as_ref().map(|title| LayoutElement {
        x,
        y,
        width: 0.0,
        height: 0.0,
        draw: DrawCommand::None,
        children: vec![],
        node_type: Some("Bookmark".to_string()),
        resolved_style: None,
        source_location: None,
        href: None,
        bookmark: Some(title.clone()),
        alt: None,
        is_header_row: false,
        col_span: 1,
        overflow: Overflow::default(),
        opacity: 1.0,
    })
}

// ─── List marker helpers ────────────────────────────────────────────

/// Produce the visible marker text for a list item at the given index.
/// For unordered lists, returns the bullet glyph (or empty for `None`).
/// For ordered lists, returns the index in the chosen numbering system
/// followed by a period (e.g. "3.", "iii.", "c.").
fn format_marker(idx: u32, ordered: bool, marker_type: ListMarkerType) -> String {
    if !ordered {
        // v1: Disc/Circle/Square all render as "•" (U+2022 BULLET),
        // which is in standard fonts' WinAnsi range. Proper distinct
        // glyphs for circle/square are a follow-up.
        return match marker_type {
            ListMarkerType::None => String::new(),
            _ => "•".to_string(),
        };
    }
    let body = match marker_type {
        ListMarkerType::LowerAlpha => to_alpha(idx, false),
        ListMarkerType::UpperAlpha => to_alpha(idx, true),
        ListMarkerType::LowerRoman => to_roman(idx, false),
        ListMarkerType::UpperRoman => to_roman(idx, true),
        ListMarkerType::None => return String::new(),
        // Decimal + every unordered variant routed here (caller shouldn't
        // mix unordered marker_type with ordered=true, but be permissive).
        _ => idx.to_string(),
    };
    format!("{body}.")
}

/// Convert a 1-based index to an alphabetic marker (a, b, ..., z, aa,
/// ab, ...). `upper = true` returns uppercase letters.
fn to_alpha(mut n: u32, upper: bool) -> String {
    if n == 0 {
        return String::new();
    }
    let base = if upper { b'A' } else { b'a' };
    let mut bytes: Vec<u8> = Vec::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        bytes.push(base + rem);
        n = (n - 1) / 26;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap_or_default()
}

/// Convert a 1-based index to a Roman numeral. `upper = true` returns
/// uppercase. Falls back to the decimal representation for n outside
/// 1..=3999 (Roman numerals lose meaning past that).
fn to_roman(n: u32, upper: bool) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    const UPPER: &[(&str, u32)] = &[
        ("M", 1000),
        ("CM", 900),
        ("D", 500),
        ("CD", 400),
        ("C", 100),
        ("XC", 90),
        ("L", 50),
        ("XL", 40),
        ("X", 10),
        ("IX", 9),
        ("V", 5),
        ("IV", 4),
        ("I", 1),
    ];
    const LOWER: &[(&str, u32)] = &[
        ("m", 1000),
        ("cm", 900),
        ("d", 500),
        ("cd", 400),
        ("c", 100),
        ("xc", 90),
        ("l", 50),
        ("xl", 40),
        ("x", 10),
        ("ix", 9),
        ("v", 5),
        ("iv", 4),
        ("i", 1),
    ];
    let table = if upper { UPPER } else { LOWER };
    let mut out = String::new();
    let mut remaining = n;
    for &(sym, val) in table {
        while remaining >= val {
            out.push_str(sym);
            remaining -= val;
        }
    }
    out
}

/// Reserve a left-side gutter wide enough to fit the widest marker the
/// list will render at its font size, plus a small gap. v1 uses an
/// approximation based on font-size × char-count rather than measuring
/// each marker through the FontContext — works well in practice for the
/// standard latin fonts and avoids threading font measurement through
/// the layout entry-point.
fn compute_marker_gutter_width(
    ordered: bool,
    marker_type: ListMarkerType,
    start: u32,
    n_items: u32,
    style: &ResolvedStyle,
) -> f64 {
    if matches!(marker_type, ListMarkerType::None) {
        return 0.0;
    }
    // Approximate average character advance — close enough for the gutter.
    let approx_char_w = style.font_size * 0.6;
    let gap = 6.0_f64;
    if !ordered {
        // Bullet glyph (one char) + gap.
        return approx_char_w + gap;
    }
    // Pick the widest marker the list will ever emit (the last one).
    let last_idx = start + n_items.saturating_sub(1);
    let widest = match marker_type {
        ListMarkerType::Decimal => format_marker(last_idx, true, ListMarkerType::Decimal),
        ListMarkerType::LowerAlpha => format_marker(last_idx, true, ListMarkerType::LowerAlpha),
        ListMarkerType::UpperAlpha => format_marker(last_idx, true, ListMarkerType::UpperAlpha),
        ListMarkerType::LowerRoman => format_marker(last_idx, true, ListMarkerType::LowerRoman),
        ListMarkerType::UpperRoman => format_marker(last_idx, true, ListMarkerType::UpperRoman),
        _ => format_marker(last_idx, true, ListMarkerType::Decimal),
    };
    widest.chars().count() as f64 * approx_char_w + gap
}

/// Configuration for an interactive PDF form field.
#[derive(Debug, Clone)]
pub enum FormFieldType {
    TextField {
        value: Option<String>,
        placeholder: Option<String>,
        multiline: bool,
        password: bool,
        read_only: bool,
        max_length: Option<u32>,
        font_size: f64,
    },
    Checkbox {
        checked: bool,
        read_only: bool,
    },
    Dropdown {
        options: Vec<String>,
        value: Option<String>,
        read_only: bool,
        font_size: f64,
    },
    RadioButton {
        value: String,
        checked: bool,
        read_only: bool,
    },
}

/// What to actually draw for this element.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Nothing to draw (just a layout container).
    None,
    /// Draw a rectangle (background, border).
    Rect {
        background: Option<Color>,
        border_width: Edges,
        border_color: EdgeValues<Color>,
        border_style: EdgeValues<crate::style::BorderStyle>,
        border_radius: CornerValues,
        opacity: f64,
        /// Optional drop shadow rendered before the background. Boxed
        /// to keep the `DrawCommand` enum's largest variant size down.
        box_shadow: Option<Box<crate::style::BoxShadow>>,
        /// Optional gradient paint. When `Some`, takes precedence over
        /// `background` (solid color). Boxed for the same enum-size
        /// reason as `box_shadow`.
        background_gradient: Option<Box<crate::style::Background>>,
    },
    /// Draw text.
    Text {
        lines: Vec<TextLine>,
        color: Color,
        text_decoration: TextDecoration,
        opacity: f64,
    },
    /// Draw an image.
    Image {
        image_data: crate::image_loader::LoadedImage,
    },
    /// Draw a grey placeholder rectangle (fallback when image loading fails).
    ImagePlaceholder,
    /// Draw SVG vector graphics.
    Svg {
        commands: Vec<crate::svg::SvgCommand>,
        /// Display width (the rendered box width in points).
        width: f64,
        /// Display height (the rendered box height in points).
        height: f64,
        /// SVG viewBox origin / dimensions. When the user omits viewBox these
        /// default to `(0, 0, width, height)` so the scale comes out to 1 and
        /// the content stream behaves as if no transform was applied.
        viewbox_min_x: f64,
        viewbox_min_y: f64,
        viewbox_width: f64,
        viewbox_height: f64,
        /// When true, clip content to [0, 0, width, height] (used by Canvas).
        clip: bool,
    },
    /// Draw a 1D barcode as filled rectangles.
    Barcode {
        bars: Vec<u8>,
        bar_width: f64,
        height: f64,
        color: Color,
    },
    /// Draw a QR code as filled rectangles.
    QrCode {
        modules: Vec<Vec<bool>>,
        module_size: f64,
        color: Color,
    },
    /// Draw a chart as a list of drawing primitives.
    Chart {
        primitives: Vec<crate::chart::ChartPrimitive>,
    },
    /// Draw a watermark (rotated text with opacity).
    Watermark {
        lines: Vec<TextLine>,
        color: Color,
        opacity: f64,
        angle_rad: f64,
        /// Font family used (for PDF font registration).
        font_family: String,
    },
    /// An interactive PDF form field (AcroForm widget annotation).
    FormField {
        field_type: FormFieldType,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct TextLine {
    pub x: f64,
    pub y: f64,
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f64,
    pub height: f64,
    /// Extra width added to each space character for justification (PDF `Tw` operator).
    pub word_spacing: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    /// Glyph ID. For custom fonts with shaping, this is a real GID from GSUB.
    /// For standard fonts, this is `char as u16` (Unicode codepoint).
    pub glyph_id: u16,
    /// X position relative to line start.
    pub x_offset: f64,
    /// Y offset from GPOS (e.g., mark positioning). Usually 0.0.
    pub y_offset: f64,
    /// Actual advance width of this glyph in points (from shaping or font metrics).
    pub x_advance: f64,
    pub font_size: f64,
    pub font_family: String,
    pub font_weight: u32,
    pub font_style: FontStyle,
    /// The character this glyph represents. For ligatures, the first char of the cluster.
    pub char_value: char,
    /// Per-glyph color (for text runs with different colors).
    pub color: Option<Color>,
    /// Per-glyph href (for inline links within runs).
    pub href: Option<String>,
    /// Per-glyph text decoration (for runs with different decorations).
    pub text_decoration: TextDecoration,
    /// Letter spacing applied to this glyph.
    pub letter_spacing: f64,
    /// For ligature glyphs, the full cluster text (e.g., "fi" for an fi ligature).
    /// `None` for 1:1 char-to-glyph mappings.
    pub cluster_text: Option<String>,
}

/// Shift a layout element and all its nested content (children, text lines)
/// down by `dy` points. Used to reposition footer elements after layout.
fn offset_element_y(el: &mut LayoutElement, dy: f64) {
    el.y += dy;
    if let DrawCommand::Text { ref mut lines, .. } = el.draw {
        for line in lines.iter_mut() {
            line.y += dy;
        }
    }
    for child in &mut el.children {
        offset_element_y(child, dy);
    }
}

/// Shift a layout element and all its nested content horizontally by `dx` points.
fn offset_element_x(el: &mut LayoutElement, dx: f64) {
    el.x += dx;
    if let DrawCommand::Text { ref mut lines, .. } = el.draw {
        for line in lines.iter_mut() {
            line.x += dx;
        }
    }
    for child in &mut el.children {
        offset_element_x(child, dx);
    }
}

/// After flex-grow expands an element's height, redistribute its children
/// vertically according to its justify-content setting. Only meaningful for
/// column containers whose height was just increased by flex-grow.
fn reapply_justify_content(elem: &mut LayoutElement) {
    let style = match elem.resolved_style {
        Some(ref s) => s,
        None => return,
    };
    if matches!(style.justify_content, JustifyContent::FlexStart) {
        return;
    }
    if elem.children.is_empty() {
        return;
    }

    let padding_top = style.padding.top + style.border_width.top;
    let padding_bottom = style.padding.bottom + style.border_width.bottom;
    let inner_h = elem.height - padding_top - padding_bottom;
    let content_top = elem.y + padding_top;

    // Find the span of children content
    let last_child = &elem.children[elem.children.len() - 1];
    let children_bottom = last_child.y + last_child.height;
    let children_span = children_bottom - content_top;
    let slack = inner_h - children_span;
    if slack < 0.001 {
        return;
    }

    let n = elem.children.len();
    let offsets: Vec<f64> = match style.justify_content {
        JustifyContent::FlexEnd => vec![slack; n],
        JustifyContent::Center => vec![slack / 2.0; n],
        JustifyContent::SpaceBetween => {
            if n <= 1 {
                vec![0.0; n]
            } else {
                let per_gap = slack / (n - 1) as f64;
                (0..n).map(|i| i as f64 * per_gap).collect()
            }
        }
        JustifyContent::SpaceAround => {
            let space = slack / n as f64;
            (0..n).map(|i| space / 2.0 + i as f64 * space).collect()
        }
        JustifyContent::SpaceEvenly => {
            let space = slack / (n + 1) as f64;
            (0..n).map(|i| (i + 1) as f64 * space).collect()
        }
        JustifyContent::FlexStart => unreachable!(),
    };

    for (i, child) in elem.children.iter_mut().enumerate() {
        let dy = offsets[i];
        if dy.abs() > 0.001 {
            offset_element_y(child, dy);
        }
    }
}

/// Apply a text transform to a string.
fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => {
            let mut result = String::with_capacity(text.len());
            let mut prev_is_whitespace = true;
            for ch in text.chars() {
                if prev_is_whitespace && ch.is_alphabetic() {
                    for upper in ch.to_uppercase() {
                        result.push(upper);
                    }
                } else {
                    result.push(ch);
                }
                prev_is_whitespace = ch.is_whitespace();
            }
            result
        }
    }
}

/// Sentinel character for `{{pageNumber}}` placeholder.
/// A single char that is atomic (can't be split by line breaking), measured
/// as the width of "00", and recognized by the PDF serializer for replacement.
pub const PAGE_NUMBER_SENTINEL: char = '\x02';

/// Sentinel character for `{{totalPages}}` placeholder.
pub const TOTAL_PAGES_SENTINEL: char = '\x03';

/// Replace page number placeholders with single sentinel characters.
/// The sentinels are measured as the width of "00" by the font system,
/// are atomic (single char, so line breaking can't split them), and are
/// replaced with actual values by the PDF serializer.
fn substitute_page_placeholders(text: &str) -> String {
    if text.contains("{{pageNumber}}") || text.contains("{{totalPages}}") {
        text.replace("{{pageNumber}}", &PAGE_NUMBER_SENTINEL.to_string())
            .replace("{{totalPages}}", &TOTAL_PAGES_SENTINEL.to_string())
    } else {
        text.to_string()
    }
}

/// Apply a text transform to a single character, given whether it's the first
/// letter of a word (for Capitalize).
fn apply_char_transform(ch: char, transform: TextTransform, is_word_start: bool) -> char {
    match transform {
        TextTransform::None => ch,
        TextTransform::Uppercase => ch.to_uppercase().next().unwrap_or(ch),
        TextTransform::Lowercase => ch.to_lowercase().next().unwrap_or(ch),
        TextTransform::Capitalize => {
            if is_word_start && ch.is_alphabetic() {
                ch.to_uppercase().next().unwrap_or(ch)
            } else {
                ch
            }
        }
    }
}

/// The main layout engine.
pub struct LayoutEngine {
    text_layout: TextLayout,
    image_dim_cache: RefCell<HashMap<String, (u32, u32)>>,
}

/// Tracks where we are on the current page during layout.
#[derive(Debug, Clone)]
struct PageCursor {
    config: PageConfig,
    /// The config subsequent pages use — differs from `config` only on a
    /// first page created from `Document::first_page` (@page :first).
    base_config: PageConfig,
    /// 0-based index of this page within the document. Kept in sync with
    /// the `pages` vec at document level; table cell-overflow pages can
    /// briefly skew it, which only matters for First/NotFirst filters and
    /// resolves at injection time where the real index is used.
    page_index: usize,
    content_width: f64,
    content_height: f64,
    y: f64,
    elements: Vec<LayoutElement>,
    fixed_header: Vec<(Node, f64)>,
    fixed_footer: Vec<(Node, f64)>,
    /// Watermark nodes stored for repetition on every page.
    watermarks: Vec<Node>,
    content_x: f64,
    content_y: f64,
    /// Extra Y offset applied on continuation pages (e.g. parent view's padding+border)
    continuation_top_offset: f64,
    /// The nearest positioned-ancestor content box `(x, y, width, height)` —
    /// the containing block for `position: absolute` descendants. Defaults to
    /// the page content box; updated when layout descends into a `relative`/
    /// `absolute` element and restored on the way out.
    containing_block: (f64, f64, f64, f64),
    /// `@page :left` / `:right` configs, when the document declares them.
    /// Flow layout ALWAYS uses `config`'s geometry; the parity config is
    /// applied as the finalized page's config plus a constant x translation
    /// of flow content (mirrored margins preserve content width by
    /// construction, so a translation is exact — never a re-layout). Docs
    /// without parity configs keep these `None` and run the exact same
    /// instructions as before.
    left_config: Option<PageConfig>,
    right_config: Option<PageConfig>,
    /// The config this page presents as (margin boxes, PDF margins,
    /// LayoutInfo). Equals `config` unless a parity config selected.
    display_config: Option<PageConfig>,
    /// The margin-left flow content was ACTUALLY anchored at. Flowing
    /// containers capture the first page's content_x and carry it across
    /// page breaks (the bake), so the parity translation must be relative
    /// to this anchor, not to the current cursor's nominal config.
    flow_anchor_left: f64,
}

impl PageCursor {
    fn new(config: &PageConfig) -> Self {
        let (page_w, page_h) = config.size.dimensions();
        let content_width = page_w - config.margin.horizontal();
        let content_height = page_h - config.margin.vertical();

        Self {
            config: config.clone(),
            base_config: config.clone(),
            page_index: 0,
            content_width,
            content_height,
            y: 0.0,
            elements: Vec::new(),
            fixed_header: Vec::new(),
            fixed_footer: Vec::new(),
            watermarks: Vec::new(),
            content_x: config.margin.left,
            content_y: config.margin.top,
            continuation_top_offset: 0.0,
            containing_block: (
                config.margin.left,
                config.margin.top,
                content_width,
                content_height,
            ),
            left_config: None,
            right_config: None,
            display_config: None,
            flow_anchor_left: config.margin.left,
        }
    }

    /// Select the parity display config for a 1-based page number. Page 1
    /// is a RIGHT page (CSS Paged Media, left-to-right page progression);
    /// `:first` outranks parity and is handled at cursor creation.
    fn parity_config_for(&self, page_number: usize) -> Option<PageConfig> {
        if page_number % 2 == 1 {
            self.right_config.clone()
        } else {
            self.left_config.clone()
        }
    }

    /// First-page cursor: lays out with `first`'s geometry while
    /// subsequent pages fall back to `base` (@page :first).
    fn new_first(first: &PageConfig, base: &PageConfig) -> Self {
        let mut cursor = PageCursor::new(first);
        cursor.base_config = base.clone();
        cursor
    }

    fn fixed_page_filter(node: &Node) -> crate::model::FixedPageFilter {
        match &node.kind {
            NodeKind::Fixed { pages, .. } => *pages,
            _ => crate::model::FixedPageFilter::All,
        }
    }

    fn remaining_height(&self) -> f64 {
        let footer_height: f64 = self
            .fixed_footer
            .iter()
            .filter(|(n, _)| Self::fixed_page_filter(n).applies(self.page_index))
            .map(|(_, h)| *h)
            .sum();
        (self.content_height - self.y - footer_height).max(0.0)
    }

    fn finalize(&self) -> LayoutPage {
        let (page_w, page_h) = self.config.size.dimensions();

        // Parity translation: flow content was laid out at the base
        // horizontal geometry; a selected :left/:right config shifts it by
        // the constant margin-left delta. Only flow elements exist at this
        // point — fixed elements, margin boxes, and watermarks are injected
        // later from the page's own (parity) config and must NOT translate.
        let (elements, config) = match &self.display_config {
            Some(display) => {
                let dx = display.margin.left - self.flow_anchor_left;
                let mut elements = self.elements.clone();
                if dx != 0.0 {
                    fn shift(els: &mut [LayoutElement], dx: f64) {
                        for el in els {
                            el.x += dx;
                            shift(&mut el.children, dx);
                        }
                    }
                    shift(&mut elements, dx);
                }
                (elements, display.clone())
            }
            None => (self.elements.clone(), self.config.clone()),
        };

        LayoutPage {
            width: page_w,
            height: page_h,
            elements,
            fixed_header: self.fixed_header.clone(),
            fixed_footer: self.fixed_footer.clone(),
            watermarks: self.watermarks.clone(),
            config,
        }
    }

    fn new_page(&self) -> Self {
        // Subsequent pages use the base config — identical to `config`
        // except when this cursor was a first page with its own geometry.
        let mut cursor = PageCursor::new(&self.base_config);
        cursor.page_index = self.page_index + 1;
        cursor.left_config = self.left_config.clone();
        cursor.right_config = self.right_config.clone();
        cursor.display_config = cursor.parity_config_for(cursor.page_index + 1);
        cursor.flow_anchor_left = self.flow_anchor_left;
        cursor.fixed_header = self.fixed_header.clone();
        cursor.fixed_footer = self.fixed_footer.clone();
        cursor.watermarks = self.watermarks.clone();
        cursor.continuation_top_offset = self.continuation_top_offset;

        let header_height: f64 = cursor
            .fixed_header
            .iter()
            .filter(|(n, _)| Self::fixed_page_filter(n).applies(cursor.page_index))
            .map(|(_, h)| *h)
            .sum();
        cursor.y = header_height + cursor.continuation_top_offset;

        cursor
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            text_layout: TextLayout::new(),
            image_dim_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Look up cached image dimensions, or load and cache them.
    fn get_image_dimensions(&self, src: &str) -> Option<(u32, u32)> {
        if let Some(dims) = self.image_dim_cache.borrow().get(src) {
            return Some(*dims);
        }
        if let Ok(dims) = crate::image_loader::load_image_dimensions(src) {
            self.image_dim_cache
                .borrow_mut()
                .insert(src.to_string(), dims);
            Some(dims)
        } else {
            None
        }
    }

    /// Main entry point: lay out a document into pages.
    pub fn layout(&self, document: &Document, font_context: &FontContext) -> Vec<LayoutPage> {
        let mut pages: Vec<LayoutPage> = Vec::new();
        let mut cursor = match &document.first_page {
            Some(first) => PageCursor::new_first(first, &document.default_page),
            None => PageCursor::new(&document.default_page),
        };
        // @page :left / :right parity configs. Page 1 is a RIGHT page (CSS
        // Paged Media, LTR page progression); :first outranks :right on
        // page 1, so the parity display only applies when :first is absent.
        // Explicit <Page> nodes carry their own configs and do not
        // participate in parity selection.
        cursor.left_config = document.left_page.clone();
        cursor.right_config = document.right_page.clone();
        if document.first_page.is_none() {
            cursor.display_config = cursor.parity_config_for(1);
        }

        // Build a root resolved style from document default_style + lang
        let base = document.default_style.clone().unwrap_or_default();
        let root_style = Style {
            lang: base.lang.clone().or(document.metadata.lang.clone()),
            ..base
        }
        .resolve(None, cursor.content_width);

        for node in &document.children {
            match &node.kind {
                NodeKind::Page { config } => {
                    if !cursor.elements.is_empty() || cursor.y > 0.0 {
                        pages.push(cursor.finalize());
                    }
                    cursor = PageCursor::new(config);
                    cursor.page_index = pages.len();

                    // Build a page-level root style that carries document lang
                    // AND has a fixed height matching the page content area.
                    // The fixed height ensures flex-grow page-level detection
                    // works correctly (layout_children uses parent height).
                    // Resolve the Page node's own style so properties like
                    // fontFamily set on <Page style={...}> inherit to children.
                    let mut page_root = node.style.resolve(Some(&root_style), cursor.content_width);
                    page_root.height = SizeConstraint::Fixed(cursor.content_height);

                    let cx = cursor.content_x;
                    let cw = cursor.content_width;
                    self.layout_children(
                        &node.children,
                        &node.style,
                        &mut cursor,
                        &mut pages,
                        cx,
                        cw,
                        Some(&page_root),
                        font_context,
                    );
                }
                NodeKind::PageBreak => {
                    pages.push(cursor.finalize());
                    cursor = cursor.new_page();
                }
                _ => {
                    let cx = cursor.content_x;
                    let cw = cursor.content_width;
                    self.layout_node(
                        node,
                        &mut cursor,
                        &mut pages,
                        cx,
                        cw,
                        Some(&root_style),
                        font_context,
                        None,
                        None,
                    );
                }
            }
        }

        if !cursor.elements.is_empty() || cursor.y > 0.0 {
            pages.push(cursor.finalize());
        }

        self.inject_fixed_elements(&mut pages, font_context);

        pages
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_node(
        &self,
        node: &Node,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        parent_style: Option<&ResolvedStyle>,
        font_context: &FontContext,
        cross_axis_height: Option<f64>,
        forced_outer_width: Option<f64>,
    ) {
        let mut style = node.style.resolve(parent_style, available_width);

        // When a flex row stretches a child, inject the cross-axis height so
        // justify-content, flex-grow, and other height-dependent logic works.
        if let Some(h) = cross_axis_height {
            if matches!(style.height, SizeConstraint::Auto) {
                style.height = SizeConstraint::Fixed(h);
            }
        }

        // When a flex parent has already resolved this child's outer width
        // (via flex-basis / flex-grow / flex-shrink distribution), override
        // style.width so layout_view uses the distributed value instead of
        // re-resolving the raw percentage against the constrained width.
        if let Some(w) = forced_outer_width {
            style.width = SizeConstraint::Fixed(w);
        }

        // A forced break-before with no in-flow content yet on the page has
        // nothing to break from, so it is suppressed — the same rule Chrome's
        // print path applies. This covers both the document start and a
        // consecutive forced break (which would otherwise emit a blank page).
        // Migrated wkhtmltopdf-era templates set page-break-before on every
        // section including the first; without this guard the document opens
        // with a blank page. Keyed on committed in-flow boxes rather than
        // `cursor.y` because the body's own margin (and any running header)
        // advances `y` above zero before the first block is ever laid out —
        // the content-bearing half of the swallowed-siblings guard above.
        if style.break_before && !cursor.elements.is_empty() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        // Remember where this node's elements begin so a `position: relative`
        // offset can shift its paint after normal-flow layout (below).
        let elem_start = cursor.elements.len();

        match &node.kind {
            NodeKind::PageBreak => {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }

            NodeKind::Fixed { position, pages } => {
                let height = self.measure_node_height(node, available_width, &style, font_context);
                match position {
                    FixedPosition::Header => {
                        cursor.fixed_header.push((node.clone(), height));
                        // Space is only consumed on pages the element
                        // actually appears on (CSS :first suppression).
                        if pages.applies(cursor.page_index) {
                            cursor.y += height;
                        }
                    }
                    FixedPosition::Footer => {
                        cursor.fixed_footer.push((node.clone(), height));
                    }
                }
            }

            NodeKind::Watermark { .. } => {
                // Watermarks take zero layout height — just store on cursor for injection
                cursor.watermarks.push(node.clone());
            }

            NodeKind::TextField {
                name,
                value,
                placeholder,
                width: field_w,
                height: field_h,
                multiline,
                password,
                read_only,
                max_length,
                font_size,
            } => {
                self.layout_form_field(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *field_w,
                    *field_h,
                    DrawCommand::FormField {
                        field_type: FormFieldType::TextField {
                            value: value.clone(),
                            placeholder: placeholder.clone(),
                            multiline: *multiline,
                            password: *password,
                            read_only: *read_only,
                            max_length: *max_length,
                            font_size: *font_size,
                        },
                        name: name.clone(),
                    },
                    "TextField",
                );
            }

            NodeKind::Checkbox {
                name,
                checked,
                width: field_w,
                height: field_h,
                read_only,
            } => {
                self.layout_form_field(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *field_w,
                    *field_h,
                    DrawCommand::FormField {
                        field_type: FormFieldType::Checkbox {
                            checked: *checked,
                            read_only: *read_only,
                        },
                        name: name.clone(),
                    },
                    "Checkbox",
                );
            }

            NodeKind::Dropdown {
                name,
                options,
                value,
                width: field_w,
                height: field_h,
                read_only,
                font_size,
            } => {
                self.layout_form_field(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *field_w,
                    *field_h,
                    DrawCommand::FormField {
                        field_type: FormFieldType::Dropdown {
                            options: options.clone(),
                            value: value.clone(),
                            read_only: *read_only,
                            font_size: *font_size,
                        },
                        name: name.clone(),
                    },
                    "Dropdown",
                );
            }

            NodeKind::RadioButton {
                name,
                value,
                checked,
                width: field_w,
                height: field_h,
                read_only,
            } => {
                self.layout_form_field(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *field_w,
                    *field_h,
                    DrawCommand::FormField {
                        field_type: FormFieldType::RadioButton {
                            value: value.clone(),
                            checked: *checked,
                            read_only: *read_only,
                        },
                        name: name.clone(),
                    },
                    "RadioButton",
                );
            }

            NodeKind::Text {
                content,
                href,
                runs,
            } => {
                self.layout_text(
                    content,
                    href.as_deref(),
                    runs,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                    node.source_location.as_ref(),
                    node.bookmark.as_deref(),
                    None,
                );
            }

            NodeKind::Heading {
                content,
                href,
                runs,
                ..
            } => {
                // Headings lay out exactly like Text but tag the wrapping
                // element as "H1".."H6" via node_type_override so the
                // tagged-PDF builder picks up the semantic role. Style
                // defaults (size, weight, margins) come from the React layer.
                let heading_role = node_kind_name(&node.kind); // "H1".."H6"
                self.layout_text(
                    content,
                    href.as_deref(),
                    runs,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                    node.source_location.as_ref(),
                    node.bookmark.as_deref(),
                    Some(heading_role),
                );
            }

            NodeKind::List {
                ordered,
                marker_type,
                start,
            } => {
                self.layout_list(
                    node,
                    *ordered,
                    *marker_type,
                    *start,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                );
            }

            NodeKind::ListItem => {
                // A bare ListItem outside of a List is just a container —
                // fall back to view-style layout. Real list rendering goes
                // through layout_list which spawns each ListItem with the
                // proper marker.
                self.layout_view(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                );
            }

            NodeKind::Image { width, height, .. } => {
                self.layout_image(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    *width,
                    *height,
                );
            }

            NodeKind::Table { columns } => {
                self.layout_table(
                    node,
                    &style,
                    columns,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                );
            }

            NodeKind::View | NodeKind::Page { .. } => {
                self.layout_view(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                );
            }

            NodeKind::TableRow { .. } | NodeKind::TableCell { .. } => {
                self.layout_view(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    font_context,
                );
            }

            NodeKind::Svg {
                width: svg_w,
                height: svg_h,
                view_box,
                content,
            } => {
                self.layout_svg(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    *svg_w,
                    *svg_h,
                    view_box.as_deref(),
                    content,
                );
            }

            NodeKind::Barcode {
                data,
                format,
                width: explicit_width,
                height: bar_height,
            } => {
                self.layout_barcode(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    data,
                    *format,
                    *explicit_width,
                    *bar_height,
                );
            }

            NodeKind::QrCode {
                data,
                size: explicit_size,
            } => {
                self.layout_qrcode(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    data,
                    *explicit_size,
                );
            }

            NodeKind::Canvas {
                width: canvas_w,
                height: canvas_h,
                operations,
            } => {
                self.layout_canvas(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    available_width,
                    *canvas_w,
                    *canvas_h,
                    operations,
                );
            }

            NodeKind::BarChart {
                data,
                width: chart_w,
                height: chart_h,
                color,
                show_labels,
                show_values,
                show_grid,
                title,
            } => {
                let config = crate::chart::bar::BarChartConfig {
                    color: color.clone(),
                    show_labels: *show_labels,
                    show_values: *show_values,
                    show_grid: *show_grid,
                    title: title.clone(),
                };
                let primitives = crate::chart::bar::build(*chart_w, *chart_h, data, &config);
                self.layout_chart(
                    node, &style, cursor, pages, x, *chart_w, *chart_h, primitives, "BarChart",
                );
            }

            NodeKind::LineChart {
                series,
                labels,
                width: chart_w,
                height: chart_h,
                show_points,
                show_grid,
                title,
            } => {
                let config = crate::chart::line::LineChartConfig {
                    show_points: *show_points,
                    show_grid: *show_grid,
                    title: title.clone(),
                };
                let primitives =
                    crate::chart::line::build(*chart_w, *chart_h, series, labels, &config);
                self.layout_chart(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *chart_w,
                    *chart_h,
                    primitives,
                    "LineChart",
                );
            }

            NodeKind::PieChart {
                data,
                width: chart_w,
                height: chart_h,
                donut,
                show_legend,
                title,
            } => {
                let config = crate::chart::pie::PieChartConfig {
                    donut: *donut,
                    show_legend: *show_legend,
                    title: title.clone(),
                };
                let primitives = crate::chart::pie::build(*chart_w, *chart_h, data, &config);
                self.layout_chart(
                    node, &style, cursor, pages, x, *chart_w, *chart_h, primitives, "PieChart",
                );
            }

            NodeKind::AreaChart {
                series,
                labels,
                width: chart_w,
                height: chart_h,
                show_grid,
                title,
            } => {
                let config = crate::chart::area::AreaChartConfig {
                    show_grid: *show_grid,
                    title: title.clone(),
                };
                let primitives =
                    crate::chart::area::build(*chart_w, *chart_h, series, labels, &config);
                self.layout_chart(
                    node,
                    &style,
                    cursor,
                    pages,
                    x,
                    *chart_w,
                    *chart_h,
                    primitives,
                    "AreaChart",
                );
            }

            NodeKind::DotPlot {
                groups,
                width: chart_w,
                height: chart_h,
                x_min,
                x_max,
                y_min,
                y_max,
                x_label,
                y_label,
                show_legend,
                dot_size,
            } => {
                let config = crate::chart::dot::DotPlotConfig {
                    x_min: *x_min,
                    x_max: *x_max,
                    y_min: *y_min,
                    y_max: *y_max,
                    x_label: x_label.clone(),
                    y_label: y_label.clone(),
                    show_legend: *show_legend,
                    dot_size: *dot_size,
                };
                let primitives = crate::chart::dot::build(*chart_w, *chart_h, groups, &config);
                self.layout_chart(
                    node, &style, cursor, pages, x, *chart_w, *chart_h, primitives, "DotPlot",
                );
            }
        }

        // position: relative — the element kept its normal-flow space (cursor.y
        // was advanced as usual above); now paint it and its content offset by
        // top/left/right/bottom. `left`/`top` shift positive, `right`/`bottom`
        // negative; siblings are unaffected because flow already advanced.
        // `position` defaults to Relative, so the presence of offsets is the
        // real discriminator — the mapper only sets offsets on a positioned
        // element, and Absolute is handled separately in `layout_children`.
        if matches!(style.position, Position::Relative)
            && (style.top.is_some()
                || style.left.is_some()
                || style.right.is_some()
                || style.bottom.is_some())
        {
            let dx = style.left.unwrap_or(0.0) - style.right.unwrap_or(0.0);
            let dy = style.top.unwrap_or(0.0) - style.bottom.unwrap_or(0.0);
            for el in &mut cursor.elements[elem_start..] {
                if dx != 0.0 {
                    offset_element_x(el, dx);
                }
                if dy != 0.0 {
                    offset_element_y(el, dy);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_view(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        font_context: &FontContext,
    ) {
        let padding = &style.padding;
        let margin = &style.margin.to_edges();
        let border = &style.border_width;

        let outer_width = match style.width {
            SizeConstraint::Fixed(w) => w,
            SizeConstraint::Auto => available_width - margin.horizontal(),
        }
        // min wins over max on conflict, per CSS.
        .min(style.max_width)
        .max(style.min_width);
        let inner_width = outer_width - padding.horizontal() - border.horizontal();

        let children_height =
            self.measure_children_height(&node.children, inner_width, style, font_context);
        let total_height = match style.height {
            SizeConstraint::Fixed(h) => h,
            SizeConstraint::Auto => children_height + padding.vertical() + border.vertical(),
        }
        .max(style.min_height);

        let node_x = x + margin.left;

        let fits = total_height <= cursor.remaining_height() - margin.vertical();

        if fits || !style.breakable {
            if !fits && !style.breakable {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }

            // Snapshot-and-collect: lay out children first, then wrap in parent
            let rect_y = cursor.content_y + cursor.y + margin.top;
            let snapshot = cursor.elements.len();

            // Pushed after the snapshot so it's drained into the rect's
            // children below — same shape the breakable path produces. Sits at
            // `rect_y`, exactly where the bookmark used to resolve when it rode
            // on `rect_element`, so the outline destination is unchanged.
            if let Some(marker) = bookmark_marker(node, node_x, rect_y) {
                cursor.elements.push(marker);
            }

            let saved_y = cursor.y;
            cursor.y += margin.top + padding.top + border.top;

            let children_x = node_x + padding.left + border.left;
            let is_grid =
                matches!(style.display, Display::Grid) && style.grid_template_columns.is_some();
            if is_grid {
                self.layout_grid_children(
                    &node.children,
                    style,
                    cursor,
                    pages,
                    children_x,
                    inner_width,
                    font_context,
                );
            } else {
                self.layout_children(
                    &node.children,
                    &node.style,
                    cursor,
                    pages,
                    children_x,
                    inner_width,
                    Some(style),
                    font_context,
                );
            }

            // Collect child elements that were pushed during layout
            let child_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();

            let rect_element = LayoutElement {
                x: node_x,
                y: rect_y,
                width: outer_width,
                height: total_height,
                draw: DrawCommand::Rect {
                    background: style.background_color,
                    border_width: style.border_width,
                    border_color: style.border_color,
                    border_style: style.border_style,
                    border_radius: style.border_radius,
                    opacity: 1.0,
                    box_shadow: style.box_shadow.map(Box::new),
                    background_gradient: style.background.clone().map(Box::new),
                },
                children: child_elements,
                node_type: Some(node_kind_name(&node.kind).to_string()),
                resolved_style: Some(style.clone()),
                source_location: node.source_location.clone(),
                href: node.href.clone(),
                // The marker above owns the bookmark now. Carrying it here too
                // would make `collect_bookmarks` emit the outline entry twice.
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: style.overflow,
                opacity: style.opacity,
            };
            cursor.elements.push(rect_element);

            cursor.y = saved_y + total_height + margin.vertical();
        } else {
            self.layout_breakable_view(
                node,
                style,
                cursor,
                pages,
                node_x,
                outer_width,
                inner_width,
                font_context,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_breakable_view(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        node_x: f64,
        outer_width: f64,
        inner_width: f64,
        font_context: &FontContext,
    ) {
        let padding = &style.padding;
        let border = &style.border_width;
        let margin = &style.margin.to_edges();

        // Save state before child layout for page-break detection
        let initial_page_count = pages.len();
        let snapshot = cursor.elements.len();
        let rect_start_y = cursor.content_y + cursor.y + margin.top;

        // Emit a zero-height marker element so the bookmark gets into the PDF
        // outline. Deliberately placed at `rect_start_y` — the view's outer top
        // edge — BEFORE the cursor advances past padding/border, so every
        // container path resolves a bookmark to the same coordinate. It used to
        // sit at the content top, which is inset by padding + border, so an
        // unstyled overflowing view landed lower than an otherwise identical
        // styled or non-overflowing one.
        if let Some(marker) = bookmark_marker(node, node_x, rect_start_y) {
            cursor.elements.push(marker);
        }

        cursor.y += margin.top + padding.top + border.top;
        let prev_continuation_offset = cursor.continuation_top_offset;
        cursor.continuation_top_offset = padding.top + border.top;

        let children_x = node_x + padding.left + border.left;
        let is_grid =
            matches!(style.display, Display::Grid) && style.grid_template_columns.is_some();
        if is_grid {
            self.layout_grid_children(
                &node.children,
                style,
                cursor,
                pages,
                children_x,
                inner_width,
                font_context,
            );
        } else {
            self.layout_children(
                &node.children,
                &node.style,
                cursor,
                pages,
                children_x,
                inner_width,
                Some(style),
                font_context,
            );
        }

        cursor.continuation_top_offset = prev_continuation_offset;

        // Check if this view has any visual styling worth wrapping
        let has_visual = style.background_color.is_some()
            || style.border_width.top > 0.0
            || style.border_width.right > 0.0
            || style.border_width.bottom > 0.0
            || style.border_width.left > 0.0;
        // Also wrap when flex_grow > 0 so the flex-grow code finds a proper wrapper element
        let needs_wrapper = has_visual || style.flex_grow > 0.0;

        if !needs_wrapper {
            // No visual styling and no flex-grow — skip wrapping
            cursor.y += padding.bottom + border.bottom + margin.bottom;
            return;
        }

        let draw_cmd = DrawCommand::Rect {
            background: style.background_color,
            border_width: style.border_width,
            border_color: style.border_color,
            border_style: style.border_style,
            border_radius: style.border_radius,
            opacity: 1.0,
            box_shadow: style.box_shadow.map(Box::new),
            background_gradient: style.background.clone().map(Box::new),
        };

        if pages.len() == initial_page_count {
            // No page breaks: simple wrap (same as non-breakable path)
            let child_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
            let rect_height =
                cursor.content_y + cursor.y + padding.bottom + border.bottom - rect_start_y;
            cursor.elements.push(LayoutElement {
                x: node_x,
                y: rect_start_y,
                width: outer_width,
                height: rect_height,
                draw: draw_cmd,
                children: child_elements,
                node_type: Some(node_kind_name(&node.kind).to_string()),
                resolved_style: Some(style.clone()),
                source_location: node.source_location.clone(),
                href: node.href.clone(),
                // The marker above owns the bookmark. `collect_bookmarks`
                // recurses into children, and the marker was drained into
                // `child_elements` — carrying it here too emits the outline
                // entry twice for one `bookmark` prop.
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: style.overflow,
                opacity: style.opacity,
            });
        } else {
            // Page breaks occurred: wrap elements on each page with clone semantics

            // A. First page — wrap elements from snapshot onward
            let page = &mut pages[initial_page_count];
            let footer_h: f64 = page.fixed_footer.iter().map(|(_, h)| *h).sum();
            let page_content_bottom =
                page.config.margin.top + (page.height - page.config.margin.vertical()) - footer_h;
            let our_elements: Vec<LayoutElement> = page.elements.drain(snapshot..).collect();
            if !our_elements.is_empty() {
                let rect_height = page_content_bottom - rect_start_y;
                page.elements.push(LayoutElement {
                    x: node_x,
                    y: rect_start_y,
                    width: outer_width,
                    height: rect_height,
                    draw: draw_cmd.clone(),
                    children: our_elements,
                    node_type: Some(node_kind_name(&node.kind).to_string()),
                    resolved_style: Some(style.clone()),
                    source_location: node.source_location.clone(),
                    href: node.href.clone(),
                    // Marker owns it — see the no-page-break branch above.
                    bookmark: None,
                    alt: None,
                    is_header_row: false,
                    col_span: 1,
                    overflow: Overflow::default(),
                    opacity: 1.0,
                });
            }

            // B. Intermediate pages — wrap ALL elements
            for page in &mut pages[initial_page_count + 1..] {
                let header_h: f64 = page.fixed_header.iter().map(|(_, h)| *h).sum();
                let content_top = page.config.margin.top + header_h;
                let footer_h: f64 = page.fixed_footer.iter().map(|(_, h)| *h).sum();
                let content_bottom = page.config.margin.top
                    + (page.height - page.config.margin.vertical())
                    - footer_h;
                let all_elements: Vec<LayoutElement> = std::mem::take(&mut page.elements);
                if !all_elements.is_empty() {
                    page.elements.push(LayoutElement {
                        x: node_x,
                        y: content_top,
                        width: outer_width,
                        height: content_bottom - content_top,
                        draw: draw_cmd.clone(),
                        children: all_elements,
                        node_type: Some(node_kind_name(&node.kind).to_string()),
                        resolved_style: Some(style.clone()),
                        source_location: node.source_location.clone(),
                        href: None,
                        bookmark: None,
                        alt: None,
                        is_header_row: false,
                        col_span: 1,
                        overflow: Overflow::default(),
                        opacity: 1.0,
                    });
                }
            }

            // C. Current page (cursor.elements) — wrap ALL elements
            let all_elements: Vec<LayoutElement> = std::mem::take(&mut cursor.elements);
            if !all_elements.is_empty() {
                let header_h: f64 = cursor.fixed_header.iter().map(|(_, h)| *h).sum();
                let content_top = cursor.content_y + header_h;
                let rect_height =
                    cursor.content_y + cursor.y + padding.bottom + border.bottom - content_top;
                cursor.elements.push(LayoutElement {
                    x: node_x,
                    y: content_top,
                    width: outer_width,
                    height: rect_height,
                    draw: draw_cmd,
                    children: all_elements,
                    node_type: Some(node_kind_name(&node.kind).to_string()),
                    resolved_style: Some(style.clone()),
                    source_location: node.source_location.clone(),
                    href: None,
                    bookmark: None,
                    alt: None,
                    is_header_row: false,
                    col_span: 1,
                    overflow: Overflow::default(),
                    opacity: 1.0,
                });
            }
        }

        cursor.y += padding.bottom + border.bottom + margin.bottom;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_children(
        &self,
        children: &[Node],
        _parent_raw_style: &Style,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        content_x: f64,
        available_width: f64,
        parent_style: Option<&ResolvedStyle>,
        font_context: &FontContext,
    ) {
        // Save parent content box position for absolute children
        let parent_box_y = cursor.content_y + cursor.y;
        let parent_box_x = content_x;

        // If this container is *explicitly* positioned it becomes the
        // containing block for its absolute descendants. Update the cursor's
        // containing block for the duration of this subtree; restore after the
        // second pass. (`position` defaults to Relative, so only the explicit
        // `positioned` flag counts.)
        let parent_positioned = parent_style.map(|s| s.positioned).unwrap_or(false);
        let saved_cb = cursor.containing_block;
        if parent_positioned {
            let cb_height = parent_style
                .and_then(|ps| match ps.height {
                    SizeConstraint::Fixed(h) => {
                        Some(h - ps.padding.vertical() - ps.border_width.vertical())
                    }
                    SizeConstraint::Auto => None,
                })
                .unwrap_or(saved_cb.3);
            cursor.containing_block = (parent_box_x, parent_box_y, available_width, cb_height);
        }

        // Separate absolute vs flow children
        let (flow_children, abs_children): (Vec<&Node>, Vec<&Node>) = children
            .iter()
            .partition(|child| !matches!(child.style.position, Some(Position::Absolute)));

        let direction = parent_style
            .map(|s| s.flex_direction)
            .unwrap_or(FlexDirection::Column);

        let row_gap = parent_style.map(|s| s.row_gap).unwrap_or(0.0);
        let column_gap = parent_style.map(|s| s.column_gap).unwrap_or(0.0);

        // First pass: flow children
        match direction {
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                let items: Vec<&Node> = if matches!(direction, FlexDirection::ColumnReverse) {
                    flow_children.into_iter().rev().collect()
                } else {
                    flow_children
                };

                let justify = parent_style
                    .map(|s| s.justify_content)
                    .unwrap_or(JustifyContent::FlexStart);
                let align = parent_style
                    .map(|s| s.align_items)
                    .unwrap_or(AlignItems::Stretch);

                let start_y = cursor.y;
                let initial_pages = pages.len();

                // Track each child's element range for align-items adjustment
                let mut child_ranges: Vec<(usize, usize)> = Vec::new();

                for (i, child) in items.iter().enumerate() {
                    if i > 0 {
                        cursor.y += row_gap;
                    }
                    let child_start = cursor.elements.len();

                    // Auto margins take priority over align-items for cross-axis positioning.
                    // For column flex, horizontal auto margins center or push the child.
                    let child_margin = &child.style.resolve(parent_style, available_width).margin;
                    let has_auto_h = child_margin.has_auto_horizontal();

                    // For align-items Center/FlexEnd, measure child width and adjust x.
                    // Returns (child_x, layout_width): layout_width is what we pass
                    // to layout_node. For Fixed-width children (incl. percentage),
                    // we pass available_width so percentages re-resolve correctly.
                    // For Auto-width children, we pass the intrinsic width so they
                    // don't stretch to fill the parent.
                    let (child_x, layout_w) = if has_auto_h {
                        let child_style = child.style.resolve(parent_style, available_width);
                        let has_explicit_width =
                            matches!(child_style.width, SizeConstraint::Fixed(_));
                        let intrinsic = self
                            .measure_intrinsic_width(child, &child_style, font_context)
                            .min(available_width);
                        let w = match child_style.width {
                            SizeConstraint::Fixed(fw) => fw,
                            // Auto width + max-width is the centered-column
                            // idiom: the block fills, the clamp shrinks it,
                            // auto margins split what's left. Plain auto
                            // keeps the engine's shrink-to-fit behavior.
                            SizeConstraint::Auto if child_style.max_width.is_finite() => {
                                (available_width - child_margin.horizontal())
                                    .min(child_style.max_width)
                            }
                            SizeConstraint::Auto => intrinsic,
                        }
                        .min(child_style.max_width)
                        .max(child_style.min_width);
                        let lw = if has_explicit_width {
                            available_width
                        } else {
                            w
                        };
                        let fixed_h = child_margin.horizontal();
                        let slack = (available_width - w - fixed_h).max(0.0);
                        let auto_left = child_margin.left.is_auto();
                        let auto_right = child_margin.right.is_auto();
                        let ml = match (auto_left, auto_right) {
                            (true, true) => slack / 2.0,
                            (true, false) => slack,
                            (false, true) => 0.0,
                            (false, false) => 0.0,
                        };
                        (content_x + child_margin.left.resolve() + ml, lw)
                    } else if !matches!(align, AlignItems::Stretch | AlignItems::FlexStart) {
                        let child_style = child.style.resolve(parent_style, available_width);
                        let has_explicit_width =
                            matches!(child_style.width, SizeConstraint::Fixed(_));
                        let intrinsic = self
                            .measure_intrinsic_width(child, &child_style, font_context)
                            .min(available_width);
                        let w = match child_style.width {
                            SizeConstraint::Fixed(fw) => fw,
                            SizeConstraint::Auto => intrinsic,
                        };
                        let lw = if has_explicit_width {
                            available_width
                        } else {
                            w
                        };
                        match align {
                            AlignItems::Center => (content_x + (available_width - w) / 2.0, lw),
                            AlignItems::FlexEnd => (content_x + available_width - w, lw),
                            _ => (content_x, available_width),
                        }
                    } else {
                        (content_x, available_width)
                    };

                    self.layout_node(
                        child,
                        cursor,
                        pages,
                        child_x,
                        layout_w,
                        parent_style,
                        font_context,
                        None,
                        None,
                    );

                    child_ranges.push((child_start, cursor.elements.len()));
                }

                // flex-grow: distribute extra vertical space proportionally
                // Compute container inner height from parent style or page content area
                let container_inner_h: Option<f64> = parent_style
                    .and_then(|ps| match ps.height {
                        SizeConstraint::Fixed(h) => {
                            Some(h - ps.padding.vertical() - ps.border_width.vertical())
                        }
                        SizeConstraint::Auto => None,
                    })
                    .or_else(|| {
                        // Page-level: use remaining content height from start
                        if parent_style.is_none() {
                            Some(cursor.content_height - start_y)
                        } else {
                            None
                        }
                    });

                if let Some(inner_h) = container_inner_h {
                    if pages.len() == initial_pages {
                        let child_styles: Vec<ResolvedStyle> = items
                            .iter()
                            .map(|child| child.style.resolve(parent_style, available_width))
                            .collect();
                        let total_grow: f64 = child_styles.iter().map(|s| s.flex_grow).sum();
                        if total_grow > 0.0 {
                            let children_total = cursor.y - start_y;
                            let slack = (inner_h - children_total).max(0.0);
                            if slack > 0.0 {
                                let mut cumulative_shift = 0.0_f64;
                                for (i, cs) in child_styles.iter().enumerate() {
                                    let (start, end) = child_ranges[i];
                                    if cumulative_shift > 0.001 {
                                        for j in start..end {
                                            offset_element_y(
                                                &mut cursor.elements[j],
                                                cumulative_shift,
                                            );
                                        }
                                    }
                                    if cs.flex_grow > 0.0 {
                                        let extra = slack * (cs.flex_grow / total_grow);
                                        // Expand the container element's height
                                        if start < end {
                                            let elem = &mut cursor.elements[end - 1];
                                            elem.height += extra;
                                            reapply_justify_content(elem);
                                        }
                                        cumulative_shift += extra;
                                    }
                                }
                                cursor.y += cumulative_shift;
                            }
                        }
                    }
                }

                // Auto vertical margin pass: distribute any remaining slack to
                // children with marginTop/marginBottom: Auto. Per CSS flex spec,
                // this runs AFTER flex-grow and BEFORE justify-content — auto
                // margins consume free space first, leaving nothing for
                // justify-content. Mirrors the cross-axis handling in
                // layout_flex_row (~2256-2267) but applied to the main axis here.
                if let Some(inner_h) = container_inner_h {
                    if pages.len() == initial_pages {
                        let auto_styles: Vec<ResolvedStyle> = items
                            .iter()
                            .map(|child| child.style.resolve(parent_style, available_width))
                            .collect();
                        let total_autos: usize = auto_styles
                            .iter()
                            .map(|s| {
                                s.margin.top.is_auto() as usize + s.margin.bottom.is_auto() as usize
                            })
                            .sum();
                        if total_autos > 0 {
                            let children_total = cursor.y - start_y;
                            let total_slack = (inner_h - children_total).max(0.0);
                            if total_slack > 0.0 {
                                let per_auto = total_slack / total_autos as f64;
                                let mut cumulative_shift = 0.0_f64;
                                for (i, cs) in auto_styles.iter().enumerate() {
                                    let (start, end) = child_ranges[i];
                                    let mt_auto = cs.margin.top.is_auto();
                                    let mb_auto = cs.margin.bottom.is_auto();
                                    // mt-auto pushes THIS child down by per_auto;
                                    // any cumulative_shift from earlier children
                                    // (including their mb-auto carryover) applies too.
                                    let this_child_shift =
                                        cumulative_shift + if mt_auto { per_auto } else { 0.0 };
                                    if this_child_shift > 0.001 {
                                        for j in start..end {
                                            offset_element_y(
                                                &mut cursor.elements[j],
                                                this_child_shift,
                                            );
                                        }
                                    }
                                    // mb-auto adds slack between this child and
                                    // any subsequent ones (carried forward).
                                    cumulative_shift =
                                        this_child_shift + if mb_auto { per_auto } else { 0.0 };
                                }
                                cursor.y += cumulative_shift;
                            }
                        }
                    }
                }

                // justify-content: redistribute children vertically when parent has fixed height
                let needs_justify =
                    !matches!(justify, JustifyContent::FlexStart) && pages.len() == initial_pages;
                if needs_justify {
                    // Use container_inner_h if available, otherwise compute from parent style
                    let justify_inner_h = container_inner_h.or_else(|| {
                        parent_style.and_then(|ps| match ps.height {
                            SizeConstraint::Fixed(h) => {
                                Some(h - ps.padding.vertical() - ps.border_width.vertical())
                            }
                            SizeConstraint::Auto => None,
                        })
                    });
                    if let Some(inner_h) = justify_inner_h {
                        let children_total = cursor.y - start_y;
                        let slack = inner_h - children_total;
                        if slack > 0.0 {
                            let n = child_ranges.len();
                            let offsets: Vec<f64> = match justify {
                                JustifyContent::FlexEnd => vec![slack; n],
                                JustifyContent::Center => vec![slack / 2.0; n],
                                JustifyContent::SpaceBetween => {
                                    if n <= 1 {
                                        vec![0.0; n]
                                    } else {
                                        let per_gap = slack / (n - 1) as f64;
                                        (0..n).map(|i| i as f64 * per_gap).collect()
                                    }
                                }
                                JustifyContent::SpaceAround => {
                                    let space = slack / n as f64;
                                    (0..n).map(|i| space / 2.0 + i as f64 * space).collect()
                                }
                                JustifyContent::SpaceEvenly => {
                                    let space = slack / (n + 1) as f64;
                                    (0..n).map(|i| (i + 1) as f64 * space).collect()
                                }
                                JustifyContent::FlexStart => vec![0.0; n],
                            };
                            for (i, &(start, end)) in child_ranges.iter().enumerate() {
                                let dy = offsets[i];
                                if dy.abs() > 0.001 {
                                    for j in start..end {
                                        offset_element_y(&mut cursor.elements[j], dy);
                                    }
                                }
                            }
                            cursor.y += *offsets.last().unwrap_or(&0.0);
                        }
                    }
                }
            }

            FlexDirection::Row | FlexDirection::RowReverse => {
                let flow_owned: Vec<Node> = flow_children.into_iter().cloned().collect();
                self.layout_flex_row(
                    &flow_owned,
                    cursor,
                    pages,
                    content_x,
                    available_width,
                    parent_style,
                    column_gap,
                    row_gap,
                    font_context,
                );
            }
        }

        // The containing block for these absolutes: the direct parent when it
        // is positioned (preserving the auto-height lazy computation), else the
        // nearest positioned ancestor / page carried on the cursor. This is the
        // v0-divergence retirement — an absolute inside an *unpositioned* parent
        // now escapes to its nearest positioned ancestor, matching browsers.
        let (cb_x, cb_y, cb_w, cb_h) = if parent_positioned {
            let ph = parent_style
                .and_then(|ps| match ps.height {
                    SizeConstraint::Fixed(h) => {
                        Some(h - ps.padding.vertical() - ps.border_width.vertical())
                    }
                    SizeConstraint::Auto => None,
                })
                .unwrap_or(cursor.content_y + cursor.y - parent_box_y);
            (parent_box_x, parent_box_y, available_width, ph)
        } else {
            cursor.containing_block
        };

        // Second pass: absolute children
        for abs_child in &abs_children {
            let abs_style = abs_child.style.resolve(parent_style, cb_w);

            // Measure intrinsic size
            let child_width = match abs_style.width {
                SizeConstraint::Fixed(w) => w,
                SizeConstraint::Auto => {
                    // If both left and right are set, stretch width
                    if let (Some(l), Some(r)) = (abs_style.left, abs_style.right) {
                        (cb_w - l - r).max(0.0)
                    } else {
                        self.measure_intrinsic_width(abs_child, &abs_style, font_context)
                    }
                }
            };

            let child_height = match abs_style.height {
                SizeConstraint::Fixed(h) => h,
                SizeConstraint::Auto => {
                    self.measure_node_height(abs_child, child_width, &abs_style, font_context)
                }
            };

            // Position relative to the containing block.
            let abs_x = if let Some(l) = abs_style.left {
                cb_x + l
            } else if let Some(r) = abs_style.right {
                cb_x + cb_w - r - child_width
            } else {
                cb_x
            };

            let abs_y = if let Some(t) = abs_style.top {
                cb_y + t
            } else if let Some(b) = abs_style.bottom {
                cb_y + cb_h - b - child_height
            } else {
                cb_y
            };

            // Lay out the absolute child into a temporary cursor
            let mut abs_cursor = PageCursor::new(&cursor.config);
            abs_cursor.y = 0.0;
            abs_cursor.content_x = abs_x;
            abs_cursor.content_y = abs_y;

            self.layout_node(
                abs_child,
                &mut abs_cursor,
                &mut Vec::new(),
                abs_x,
                child_width,
                parent_style,
                font_context,
                None,
                None,
            );

            // Add absolute elements to the current cursor (renders on top)
            cursor.elements.extend(abs_cursor.elements);
        }

        // Restore the containing block for the caller's remaining siblings.
        cursor.containing_block = saved_cb;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_flex_row(
        &self,
        children: &[Node],
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        content_x: f64,
        available_width: f64,
        parent_style: Option<&ResolvedStyle>,
        column_gap: f64,
        row_gap: f64,
        font_context: &FontContext,
    ) {
        if children.is_empty() {
            return;
        }

        let flex_wrap = parent_style
            .map(|s| s.flex_wrap)
            .unwrap_or(FlexWrap::NoWrap);

        // Phase 1: resolve styles and measure base widths for all items
        // flex_basis takes precedence over width for flex items (per CSS spec)
        let items: Vec<FlexItem> = children
            .iter()
            .map(|child| {
                let style = child.style.resolve(parent_style, available_width);
                let base_width = match style.flex_basis {
                    SizeConstraint::Fixed(w) => w,
                    SizeConstraint::Auto => match style.width {
                        SizeConstraint::Fixed(w) => w,
                        SizeConstraint::Auto => {
                            self.measure_intrinsic_width(child, &style, font_context)
                        }
                    },
                };
                let min_content_width = self.measure_min_content_width(child, &style, font_context);
                FlexItem {
                    node: child,
                    style,
                    base_width,
                    min_content_width,
                }
            })
            .collect();

        // Phase 2: determine wrap lines
        let base_widths: Vec<f64> = items.iter().map(|i| i.base_width).collect();
        let lines = match flex_wrap {
            FlexWrap::NoWrap => {
                vec![flex::WrapLine {
                    start: 0,
                    end: items.len(),
                }]
            }
            FlexWrap::Wrap => flex::partition_into_lines(&base_widths, column_gap, available_width),
            FlexWrap::WrapReverse => {
                let mut l = flex::partition_into_lines(&base_widths, column_gap, available_width);
                l.reverse();
                l
            }
        };

        if lines.is_empty() {
            return;
        }

        // Phase 3: lay out each line
        let justify = parent_style.map(|s| s.justify_content).unwrap_or_default();

        // We need mutable final_widths per line, so collect into a vec
        let mut final_widths: Vec<f64> = items.iter().map(|i| i.base_width).collect();

        let initial_pages_count = pages.len();
        let flex_start_y = cursor.y;
        let mut line_infos: Vec<(usize, usize, f64)> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let line_items = &items[line.start..line.end];
            let line_count = line.end - line.start;
            let line_gap = column_gap * (line_count as f64 - 1.0).max(0.0);
            let distributable = available_width - line_gap;

            // Flex distribution for this line
            let total_base: f64 = line_items.iter().map(|i| i.base_width).sum();
            let remaining = distributable - total_base;

            if remaining > 0.0 {
                let total_grow: f64 = line_items.iter().map(|i| i.style.flex_grow).sum();
                if total_grow > 0.0 {
                    for (j, item) in line_items.iter().enumerate() {
                        final_widths[line.start + j] =
                            item.base_width + remaining * (item.style.flex_grow / total_grow);
                    }
                }
            } else if remaining < 0.0 {
                let total_shrink: f64 = line_items
                    .iter()
                    .map(|i| i.style.flex_shrink * i.base_width)
                    .sum();
                if total_shrink > 0.0 {
                    for (j, item) in line_items.iter().enumerate() {
                        let factor = (item.style.flex_shrink * item.base_width) / total_shrink;
                        let w = item.base_width + remaining * factor;
                        let floor = item.style.min_width.max(item.min_content_width);
                        final_widths[line.start + j] = w.max(floor);
                    }
                }
            }

            // Measure line height
            let line_height: f64 = line_items
                .iter()
                .enumerate()
                .map(|(j, item)| {
                    let fw = final_widths[line.start + j];
                    self.measure_node_height(item.node, fw, &item.style, font_context)
                        + item.style.margin.vertical()
                })
                .fold(0.0f64, f64::max);

            // Page break check for this line. The `cursor.y > 0.0` guard
            // matches the other break sites: when the current page is
            // already empty, moving to a fresh page can't gain space — a
            // line taller than a full page would otherwise emit a blank
            // page and then overflow anyway (found by the HTML spike's
            // taller-than-page flex item).
            if line_height > cursor.remaining_height() && cursor.y > 0.0 {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }

            // Add row_gap between lines (not before first)
            if line_idx > 0 {
                cursor.y += row_gap;
            }

            let row_start_y = cursor.y;

            // Justify-content for this line
            let actual_total: f64 = (line.start..line.end).map(|i| final_widths[i]).sum();
            let slack = available_width - actual_total - line_gap;

            let (start_offset, between_extra) = match justify {
                JustifyContent::FlexStart => (0.0, 0.0),
                JustifyContent::FlexEnd => (slack, 0.0),
                JustifyContent::Center => (slack / 2.0, 0.0),
                JustifyContent::SpaceBetween => {
                    if line_count > 1 {
                        (0.0, slack / (line_count as f64 - 1.0))
                    } else {
                        (0.0, 0.0)
                    }
                }
                JustifyContent::SpaceAround => {
                    let s = slack / line_count as f64;
                    (s / 2.0, s)
                }
                JustifyContent::SpaceEvenly => {
                    let s = slack / (line_count as f64 + 1.0);
                    (s, s)
                }
            };

            let line_elem_start = cursor.elements.len();
            let mut x = content_x + start_offset;

            for (j, item) in line_items.iter().enumerate() {
                if j > 0 {
                    x += column_gap + between_extra;
                }

                let fw = final_widths[line.start + j];

                let align = item
                    .style
                    .align_self
                    .unwrap_or(parent_style.map(|s| s.align_items).unwrap_or_default());

                let item_height =
                    self.measure_node_height(item.node, fw, &item.style, font_context);

                // Auto margins on cross axis take priority over align-items
                let has_auto_v = item.style.margin.has_auto_vertical();
                let y_offset = if has_auto_v {
                    let fixed_v = item.style.margin.vertical();
                    let slack = (line_height - item_height - fixed_v).max(0.0);
                    let auto_top = item.style.margin.top.is_auto();
                    let auto_bottom = item.style.margin.bottom.is_auto();
                    match (auto_top, auto_bottom) {
                        (true, true) => slack / 2.0,
                        (true, false) => slack,
                        (false, true) => 0.0,
                        (false, false) => 0.0,
                    }
                } else {
                    match align {
                        AlignItems::FlexStart => 0.0,
                        AlignItems::FlexEnd => {
                            line_height - item_height - item.style.margin.vertical()
                        }
                        AlignItems::Center => {
                            (line_height - item_height - item.style.margin.vertical()) / 2.0
                        }
                        AlignItems::Stretch => 0.0,
                        AlignItems::Baseline => 0.0,
                    }
                };

                // When stretch applies and item has no explicit height, pass
                // the cross-axis height so inner layout sees a fixed container.
                // Auto margins prevent stretch.
                let cross_h = if matches!(align, AlignItems::Stretch)
                    && matches!(item.style.height, SizeConstraint::Auto)
                    && !has_auto_v
                {
                    let stretch_h = line_height - item.style.margin.vertical();
                    if stretch_h > item_height {
                        Some(stretch_h)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let saved_y = cursor.y;
                cursor.y = row_start_y + y_offset;

                self.layout_node(
                    item.node,
                    cursor,
                    pages,
                    x,
                    available_width,
                    parent_style,
                    font_context,
                    cross_h,
                    Some(fw),
                );

                cursor.y = saved_y;
                x += fw;
            }

            cursor.y = row_start_y + line_height;
            line_infos.push((line_elem_start, cursor.elements.len(), line_height));
        }

        // Apply align-content redistribution for wrapped flex lines
        if pages.len() == initial_pages_count && !line_infos.is_empty() {
            let align_content = parent_style.map(|s| s.align_content).unwrap_or_default();
            if !matches!(align_content, AlignContent::FlexStart)
                && !matches!(flex_wrap, FlexWrap::NoWrap)
            {
                if let Some(parent) = parent_style {
                    if let SizeConstraint::Fixed(container_h) = parent.height {
                        let inner_h = container_h
                            - parent.padding.vertical()
                            - parent.border_width.vertical();
                        let total_used = cursor.y - flex_start_y;
                        let slack = inner_h - total_used;
                        if slack > 0.0 {
                            let n = line_infos.len();
                            let offsets: Vec<f64> = match align_content {
                                AlignContent::FlexEnd => vec![slack; n],
                                AlignContent::Center => vec![slack / 2.0; n],
                                AlignContent::SpaceBetween => {
                                    if n <= 1 {
                                        vec![0.0; n]
                                    } else {
                                        let per_gap = slack / (n - 1) as f64;
                                        (0..n).map(|i| i as f64 * per_gap).collect()
                                    }
                                }
                                AlignContent::SpaceAround => {
                                    let space = slack / n as f64;
                                    (0..n).map(|i| space / 2.0 + i as f64 * space).collect()
                                }
                                AlignContent::SpaceEvenly => {
                                    let space = slack / (n + 1) as f64;
                                    (0..n).map(|i| (i + 1) as f64 * space).collect()
                                }
                                AlignContent::Stretch => {
                                    let extra = slack / n as f64;
                                    (0..n).map(|i| i as f64 * extra).collect()
                                }
                                AlignContent::FlexStart => vec![0.0; n],
                            };
                            for (i, &(start, end, _)) in line_infos.iter().enumerate() {
                                let dy = offsets[i];
                                if dy.abs() > 0.001 {
                                    for j in start..end {
                                        offset_element_y(&mut cursor.elements[j], dy);
                                    }
                                }
                            }
                            cursor.y += *offsets.last().unwrap_or(&0.0);
                        }
                    }
                }
            }
        }
    }

    // ─── Lists ─────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn layout_list(
        &self,
        node: &Node,
        ordered: bool,
        marker_type: ListMarkerType,
        start: u32,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        font_context: &FontContext,
    ) {
        let margin = &style.margin.to_edges();
        let padding = &style.padding;

        cursor.y += margin.top;

        let list_x = x + margin.left;
        let outer_width = available_width - margin.horizontal();
        let inner_width = outer_width - padding.horizontal();

        // Count items so we can size the marker gutter for the widest
        // marker the list will produce (e.g. "12." needs more space than "1.")
        let n_items = node
            .children
            .iter()
            .filter(|c| matches!(c.kind, NodeKind::ListItem))
            .count() as u32;

        let marker_gutter =
            compute_marker_gutter_width(ordered, marker_type, start, n_items, style);

        let list_inner_x = list_x + padding.left;
        let content_x = list_inner_x + marker_gutter;
        let content_width = (inner_width - marker_gutter).max(0.0);

        // Snapshot for wrapping the items in a single List container
        // element (so tagged-PDF picks up the /L role on the whole list).
        let snapshot = cursor.elements.len();
        let list_start_y = cursor.content_y + cursor.y;
        cursor.y += padding.top;

        let mut item_index: u32 = 0;
        for child in &node.children {
            if !matches!(child.kind, NodeKind::ListItem) {
                continue;
            }
            let marker_idx = start + item_index;
            self.layout_list_item(
                child,
                marker_idx,
                ordered,
                marker_type,
                marker_gutter,
                style,
                cursor,
                pages,
                list_inner_x,
                content_x,
                content_width,
                font_context,
            );
            item_index += 1;
        }

        cursor.y += padding.bottom;

        // Wrap collected item elements in a List container
        let item_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
        let list_height = cursor.content_y + cursor.y - list_start_y;
        cursor.elements.push(LayoutElement {
            x: list_x,
            y: list_start_y,
            width: outer_width,
            height: list_height,
            draw: DrawCommand::None,
            children: item_elements,
            node_type: Some("List".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: None,
            bookmark: node.bookmark.clone(),
            alt: None,
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += margin.bottom;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_list_item(
        &self,
        item: &Node,
        marker_idx: u32,
        ordered: bool,
        marker_type: ListMarkerType,
        marker_gutter: f64,
        parent_style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        list_inner_x: f64,
        content_x: f64,
        content_width: f64,
        font_context: &FontContext,
    ) {
        let item_style = item.style.resolve(Some(parent_style), content_width);
        let item_margin = item_style.margin.to_edges();

        cursor.y += item_margin.top;
        let item_start_y = cursor.content_y + cursor.y;
        let item_snapshot = cursor.elements.len();

        // 1. Render the marker. Save cursor.y, lay out marker as a tiny
        //    Text node at list_inner_x with width = marker_gutter, then
        //    restore cursor.y so the content lays out at the same line.
        let marker_str = format_marker(marker_idx, ordered, marker_type);
        if !marker_str.is_empty() {
            let saved_y = cursor.y;
            self.layout_text(
                &marker_str,
                None,
                &[],
                &item_style,
                cursor,
                pages,
                list_inner_x,
                marker_gutter,
                font_context,
                None,
                None,
                Some("Lbl"),
            );
            cursor.y = saved_y;
        }

        // 2. Lay out item children at content_x using the standard
        //    layout_children path. Wrapping inside a long item naturally
        //    indents to content_x for every line because that's the x
        //    we hand to layout_children — no special hanging-indent
        //    logic required, since the marker is a separate element.
        self.layout_children(
            &item.children,
            &item.style,
            cursor,
            pages,
            content_x,
            content_width,
            Some(&item_style),
            font_context,
        );

        // 3. Wrap marker + content in a ListItem container element
        //    (tagged PDF picks up /LI from the node_type).
        let item_children: Vec<LayoutElement> = cursor.elements.drain(item_snapshot..).collect();
        let item_height = cursor.content_y + cursor.y - item_start_y;
        let item_width = content_x + content_width - list_inner_x;
        cursor.elements.push(LayoutElement {
            x: list_inner_x,
            y: item_start_y,
            width: item_width,
            height: item_height,
            draw: DrawCommand::None,
            children: item_children,
            node_type: Some("ListItem".to_string()),
            resolved_style: Some(item_style.clone()),
            source_location: item.source_location.clone(),
            href: None,
            bookmark: item.bookmark.clone(),
            alt: None,
            is_header_row: false,
            col_span: 1,
            overflow: item_style.overflow,
            opacity: item_style.opacity,
        });

        cursor.y += item_margin.bottom;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_table(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        column_defs: &[ColumnDef],
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        font_context: &FontContext,
    ) {
        let padding = &style.padding;
        let margin = &style.margin.to_edges();
        let border = &style.border_width;

        let table_x = x + margin.left;
        let table_width = match style.width {
            SizeConstraint::Fixed(w) => w,
            SizeConstraint::Auto => available_width - margin.horizontal(),
        };
        let inner_width = table_width - padding.horizontal() - border.horizontal();

        let col_widths = self.resolve_column_widths(column_defs, inner_width, &node.children);

        let mut header_rows: Vec<&Node> = Vec::new();
        let mut body_rows: Vec<&Node> = Vec::new();

        for child in &node.children {
            match &child.kind {
                NodeKind::TableRow { is_header: true } => header_rows.push(child),
                _ => body_rows.push(child),
            }
        }

        // break-inside: avoid (wrap: false). Row-by-row pagination below
        // ignores breakability, so an unbreakable table that doesn't fit
        // must move to a fresh page here — whole — before any row lands.
        // A table taller than a full page falls through to normal
        // pagination: breaking is unavoidable and splitting beats clipping.
        if !style.breakable {
            let total_height: f64 = node
                .children
                .iter()
                .map(|r| self.measure_table_row_height(r, &col_widths, style, font_context))
                .sum::<f64>()
                + padding.vertical()
                + border.vertical();
            let fresh_page_available = cursor.content_height
                - cursor.fixed_header.iter().map(|(_, h)| *h).sum::<f64>()
                - cursor.fixed_footer.iter().map(|(_, h)| *h).sum::<f64>();
            if total_height > cursor.remaining_height()
                && total_height <= fresh_page_available
                && cursor.y > 0.0
            {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }
        }

        // Snapshot-and-collect state for the Table wrapper element (same
        // clone-semantics fragment wrapping as layout_breakable_view). Two
        // consumers need a real Table container: table-level border and
        // background have no paint target without one, and structural
        // consumers (tagged PDF /Table, pdf-testkit's extractor) otherwise
        // have to synthesize the table from loose rows.
        let initial_page_count = pages.len();
        let snapshot = cursor.elements.len();
        let rect_start_y = cursor.content_y + cursor.y + margin.top;

        cursor.y += margin.top + padding.top + border.top;

        let cell_x_start = table_x + padding.left + border.left;

        // Initial-header pre-fit check. Covers three related symptoms:
        //
        //   * Original issue 4 ("doubled, sliding column"): table starts low
        //     enough that the header didn't fit. Each header cell's inner
        //     content triggered a widow/orphan page-break via layout_text,
        //     and layout_table_row's cell-overflow path committed those
        //     breaks as spurious "trial" pages.
        //   * Orphan header: header fits in remaining space but the first
        //     body row doesn't, so the header gets drawn at the bottom of
        //     the current page with no rows beneath it, then redrawn on
        //     the next page above the actual rows.
        //   * Long-token header (issue 2 reproduction): a single header
        //     cell wraps to many lines because of a no-break-opportunity
        //     token. Even though the pre-check would fire on header height
        //     alone, including the first body row makes the fit decision
        //     symmetric with body-row checks below and avoids edge cases
        //     where rounding leaves the header just barely fitting while
        //     no body row will ever land on the same page.
        //
        // Fold the first body row into the fit calculation so we never
        // leave an orphan header behind. Cap at fresh-page available
        // height: if the combined block is genuinely taller than a page,
        // page-breaking can't help — fall through and let the
        // `!is_header` cell-overflow guard in layout_table_row handle it.
        if !header_rows.is_empty() {
            let total_header_h: f64 = header_rows
                .iter()
                .map(|r| self.measure_table_row_height(r, &col_widths, style, font_context))
                .sum();
            let first_body_h = body_rows
                .first()
                .map(|r| self.measure_table_row_height(r, &col_widths, style, font_context))
                .unwrap_or(0.0);

            let needed = total_header_h + first_body_h;
            let fresh_page_available = cursor.content_height
                - cursor.fixed_header.iter().map(|(_, h)| *h).sum::<f64>()
                - cursor.fixed_footer.iter().map(|(_, h)| *h).sum::<f64>();

            if needed > cursor.remaining_height() && needed <= fresh_page_available {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
                cursor.y += padding.top + border.top;
            }
        }

        for header_row in &header_rows {
            self.layout_table_row(
                header_row,
                &col_widths,
                style,
                cursor,
                cell_x_start,
                font_context,
                pages,
            );
        }

        for body_row in &body_rows {
            let row_height =
                self.measure_table_row_height(body_row, &col_widths, style, font_context);

            if row_height > cursor.remaining_height() {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();

                cursor.y += padding.top + border.top;
                for header_row in &header_rows {
                    self.layout_table_row(
                        header_row,
                        &col_widths,
                        style,
                        cursor,
                        cell_x_start,
                        font_context,
                        pages,
                    );
                }
            }

            self.layout_table_row(
                body_row,
                &col_widths,
                style,
                cursor,
                cell_x_start,
                font_context,
                pages,
            );
        }

        // Wrap the laid-out rows in a Table container element. Always
        // emitted (structural consumers need it even without visuals); the
        // draw command is a Rect only when there's something to paint.
        let has_visual = style.background_color.is_some()
            || style.background.is_some()
            || style.border_width.top > 0.0
            || style.border_width.right > 0.0
            || style.border_width.bottom > 0.0
            || style.border_width.left > 0.0;
        let draw_cmd = if has_visual {
            DrawCommand::Rect {
                background: style.background_color,
                border_width: style.border_width,
                border_color: style.border_color,
                border_style: style.border_style,
                border_radius: style.border_radius,
                opacity: 1.0,
                box_shadow: style.box_shadow.map(Box::new),
                background_gradient: style.background.clone().map(Box::new),
            }
        } else {
            DrawCommand::None
        };
        let make_wrapper =
            |y: f64, height: f64, children: Vec<LayoutElement>, draw| LayoutElement {
                x: table_x,
                y,
                width: table_width,
                height,
                draw,
                children,
                node_type: Some(node_kind_name(&node.kind).to_string()),
                resolved_style: Some(style.clone()),
                source_location: node.source_location.clone(),
                href: node.href.clone(),
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: style.opacity,
            };

        let table_bottom_y = cursor.content_y + cursor.y + padding.bottom + border.bottom;

        if pages.len() == initial_page_count {
            // No page breaks: simple wrap.
            let child_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
            cursor.elements.push(make_wrapper(
                rect_start_y,
                table_bottom_y - rect_start_y,
                child_elements,
                draw_cmd,
            ));
        } else {
            // Page breaks occurred: clone-semantics fragment per page,
            // mirroring layout_breakable_view.

            // A. The page the table started on — wrap from the snapshot.
            let page = &mut pages[initial_page_count];
            let footer_h: f64 = page.fixed_footer.iter().map(|(_, h)| *h).sum();
            let page_content_bottom =
                page.config.margin.top + (page.height - page.config.margin.vertical()) - footer_h;
            let our_elements: Vec<LayoutElement> = page.elements.drain(snapshot..).collect();
            if !our_elements.is_empty() {
                page.elements.push(make_wrapper(
                    rect_start_y,
                    page_content_bottom - rect_start_y,
                    our_elements,
                    draw_cmd.clone(),
                ));
            }

            // B. Intermediate pages — entirely table content.
            for page in &mut pages[initial_page_count + 1..] {
                let header_h: f64 = page.fixed_header.iter().map(|(_, h)| *h).sum();
                let content_top = page.config.margin.top + header_h;
                let footer_h: f64 = page.fixed_footer.iter().map(|(_, h)| *h).sum();
                let content_bottom = page.config.margin.top
                    + (page.height - page.config.margin.vertical())
                    - footer_h;
                let all_elements: Vec<LayoutElement> = std::mem::take(&mut page.elements);
                if !all_elements.is_empty() {
                    page.elements.push(make_wrapper(
                        content_top,
                        content_bottom - content_top,
                        all_elements,
                        draw_cmd.clone(),
                    ));
                }
            }

            // C. Current page — everything on it is table content.
            let all_elements: Vec<LayoutElement> = std::mem::take(&mut cursor.elements);
            if !all_elements.is_empty() {
                let header_h: f64 = cursor.fixed_header.iter().map(|(_, h)| *h).sum();
                let content_top = cursor.content_y + header_h;
                cursor.elements.push(make_wrapper(
                    content_top,
                    table_bottom_y - content_top,
                    all_elements,
                    draw_cmd,
                ));
            }
        }

        cursor.y += padding.bottom + border.bottom + margin.bottom;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_table_row(
        &self,
        row: &Node,
        col_widths: &[f64],
        parent_style: &ResolvedStyle,
        cursor: &mut PageCursor,
        start_x: f64,
        font_context: &FontContext,
        pages: &mut Vec<LayoutPage>,
    ) {
        let row_style = row
            .style
            .resolve(Some(parent_style), col_widths.iter().sum());

        let row_height = self.measure_table_row_height(row, col_widths, parent_style, font_context);
        let row_bl = self.row_baseline(row, &row_style, col_widths);
        let row_y = cursor.content_y + cursor.y;
        let total_width: f64 = col_widths.iter().sum();

        let is_header = matches!(row.kind, NodeKind::TableRow { is_header: true });

        // Snapshot before laying out cells — we'll collect them as row children
        let row_snapshot = cursor.elements.len();

        let mut all_overflow_pages: Vec<LayoutPage> = Vec::new();
        let mut cell_x = start_x;
        // Track the column index separately from the cell index: a colspan
        // cell consumes several columns' widths, and the next cell must
        // start past ALL of them. Indexing col_widths by cell position put
        // every cell after a colspan one slot too far left (found by the
        // HTML input path's totals rows).
        let mut col_idx = 0usize;
        for cell in row.children.iter() {
            let span = match &cell.kind {
                NodeKind::TableCell { col_span, .. } => (*col_span).max(1) as usize,
                _ => 1,
            };
            let col_width: f64 = col_widths.iter().skip(col_idx).take(span).copied().sum();
            col_idx += span;

            let cell_style = cell.style.resolve(Some(&row_style), col_width);

            // Snapshot before cell content — we'll collect as cell children
            let cell_snapshot = cursor.elements.len();

            let inner_width =
                col_width - cell_style.padding.horizontal() - cell_style.border_width.horizontal();

            let content_x = cell_x + cell_style.padding.left + cell_style.border_width.left;
            let saved_y = cursor.y;
            cursor.y += cell_style.padding.top + cell_style.border_width.top;

            // vertical-align: middle/bottom/baseline — the row box height is
            // already resolved (measured above the loop), so offset this cell's
            // content within it. Top is the default and costs nothing.
            if !matches!(cell_style.vertical_align, crate::style::VerticalAlign::Top) {
                let content_h: f64 = cell
                    .children
                    .iter()
                    .map(|ch| {
                        let ch_style = ch.style.resolve(Some(&cell_style), inner_width);
                        self.measure_node_height(ch, inner_width, &ch_style, font_context)
                    })
                    .sum();
                let inner_row =
                    row_height - cell_style.padding.vertical() - cell_style.border_width.vertical();
                let slack = (inner_row - content_h).max(0.0);
                cursor.y += match cell_style.vertical_align {
                    crate::style::VerticalAlign::Middle => slack / 2.0,
                    crate::style::VerticalAlign::Bottom => slack,
                    // Shove this cell down so its first baseline lands on the
                    // row baseline (the max first-baseline distance across the
                    // row's baseline cells). measure_table_row_height grew the
                    // row to fit this, so it never clips.
                    crate::style::VerticalAlign::Baseline => row_bl
                        .map(|b| {
                            let d = self.cell_baseline_distance(cell, &cell_style, inner_width);
                            (b - d).max(0.0)
                        })
                        .unwrap_or(0.0),
                    crate::style::VerticalAlign::Top => 0.0,
                };
            }

            // Save cursor state in case cell content triggers page breaks
            let cursor_before_cell = cursor.clone();
            let mut cell_pages: Vec<LayoutPage> = Vec::new();
            for child in &cell.children {
                self.layout_node(
                    child,
                    cursor,
                    &mut cell_pages,
                    content_x,
                    inner_width,
                    Some(&cell_style),
                    font_context,
                    None,
                    None,
                );
            }

            // If cell content triggered page breaks, collect overflow and restore cursor
            if !cell_pages.is_empty() {
                let post_break_elements = std::mem::take(&mut cursor.elements);
                if let Some(last_page) = cell_pages.last_mut() {
                    last_page.elements.extend(post_break_elements);
                }
                // Belt-and-suspenders for issue 4: header rows are designed to
                // be re-emitted on each continuation page and must never
                // legitimately produce mid-row page breaks. If they somehow do
                // (e.g. a future regression that puts headers in a tight spot
                // again), drop the trial pages rather than committing them.
                if !is_header {
                    all_overflow_pages.extend(cell_pages);
                }
                *cursor = cursor_before_cell;
            }

            cursor.y = saved_y;

            // Collect cell content elements
            let cell_children: Vec<LayoutElement> =
                cursor.elements.drain(cell_snapshot..).collect();

            // Always push a cell element (with or without visual styling) to preserve hierarchy
            cursor.elements.push(LayoutElement {
                x: cell_x,
                y: row_y,
                width: col_width,
                height: row_height,
                draw: if cell_style.background_color.is_some()
                    || cell_style.border_width.horizontal() > 0.0
                    || cell_style.border_width.vertical() > 0.0
                {
                    DrawCommand::Rect {
                        background: cell_style.background_color,
                        border_width: cell_style.border_width,
                        border_color: cell_style.border_color,
                        border_style: cell_style.border_style,
                        border_radius: cell_style.border_radius,
                        opacity: 1.0,
                        box_shadow: cell_style.box_shadow.map(Box::new),
                        background_gradient: cell_style.background.clone().map(Box::new),
                    }
                } else {
                    DrawCommand::None
                },
                children: cell_children,
                node_type: Some("TableCell".to_string()),
                resolved_style: Some(cell_style.clone()),
                source_location: cell.source_location.clone(),
                href: None,
                bookmark: cell.bookmark.clone(),
                alt: None,
                is_header_row: is_header,
                col_span: span as u32,
                overflow: Overflow::default(),
                opacity: 1.0,
            });

            cell_x += col_width;
        }

        // Collect all cell elements as row children
        let row_children: Vec<LayoutElement> = cursor.elements.drain(row_snapshot..).collect();
        cursor.elements.push(LayoutElement {
            x: start_x,
            y: row_y,
            width: total_width,
            height: row_height,
            draw: if let Some(bg) = row_style.background_color {
                DrawCommand::Rect {
                    background: Some(bg),
                    border_width: Edges::default(),
                    border_color: EdgeValues::uniform(Color::BLACK),
                    border_style: EdgeValues::uniform(crate::style::BorderStyle::Solid),
                    border_radius: CornerValues::uniform(0.0),
                    opacity: 1.0,
                    box_shadow: row_style.box_shadow.map(Box::new),
                    background_gradient: row_style.background.clone().map(Box::new),
                }
            } else {
                DrawCommand::None
            },
            children: row_children,
            node_type: Some("TableRow".to_string()),
            resolved_style: Some(row_style.clone()),
            source_location: row.source_location.clone(),
            href: None,
            bookmark: row.bookmark.clone(),
            alt: None,
            is_header_row: is_header,
            col_span: 1,
            overflow: row_style.overflow,
            opacity: row_style.opacity,
        });

        // Append any overflow pages from cells that exceeded page height
        pages.extend(all_overflow_pages);

        cursor.y += row_height;
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn layout_text(
        &self,
        content: &str,
        href: Option<&str>,
        runs: &[TextRun],
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        font_context: &FontContext,
        source_location: Option<&SourceLocation>,
        bookmark: Option<&str>,
        // Optional node_type label for the wrapping Text element. Defaults
        // to "Text". Headings pass "H1".."H6" so tagged-PDF picks up the
        // semantic role; everything else passes None.
        node_type_override: Option<&str>,
    ) {
        let margin = &style.margin.to_edges();
        let text_x = x + margin.left;
        // Honor an explicit/resolved fixed width for the text box; only fall back
        // to available_width when width is Auto. In a flex row, available_width is
        // the parent row's content width (used for percentage resolution) while the
        // child's own distributed width arrives via style.width — see layout_node's
        // forced_outer_width. layout_view already works this way; this keeps leaf
        // text consistent so textAlign/justify use the real box, not the row width.
        let text_width = match style.width {
            SizeConstraint::Fixed(w) => (w - margin.horizontal()).max(0.0),
            SizeConstraint::Auto => available_width - margin.horizontal(),
        };

        cursor.y += margin.top;

        // Runs path: if runs are provided, use multi-style line breaking
        if !runs.is_empty() {
            self.layout_text_runs(
                runs,
                href,
                style,
                cursor,
                pages,
                text_x,
                text_width,
                font_context,
                source_location,
                bookmark,
                node_type_override,
            );
            cursor.y += margin.bottom;
            return;
        }

        let content = substitute_page_placeholders(content);
        let transformed = apply_text_transform(&content, style.text_transform);
        let justify = matches!(style.text_align, TextAlign::Justify);
        let lines = match style.line_breaking {
            LineBreaking::Optimal => self.text_layout.break_into_lines_optimal(
                font_context,
                &transformed,
                text_width,
                style.font_size,
                &style.font_family,
                style.font_weight,
                style.font_style,
                style.letter_spacing,
                style.hyphens,
                style.lang.as_deref(),
                justify,
            ),
            LineBreaking::Greedy => self.text_layout.break_into_lines(
                font_context,
                &transformed,
                text_width,
                style.font_size,
                &style.font_family,
                style.font_weight,
                style.font_style,
                style.letter_spacing,
                style.hyphens,
                style.lang.as_deref(),
            ),
        };

        // Apply text overflow truncation (single-line modes)
        let lines = match style.text_overflow {
            TextOverflow::Ellipsis => self.text_layout.truncate_with_ellipsis(
                font_context,
                lines,
                text_width,
                style.font_size,
                &style.font_family,
                style.font_weight,
                style.font_style,
                style.letter_spacing,
            ),
            TextOverflow::Clip => self.text_layout.truncate_clip(
                font_context,
                lines,
                text_width,
                style.font_size,
                &style.font_family,
                style.font_weight,
                style.font_style,
                style.letter_spacing,
            ),
            TextOverflow::Wrap => lines,
        };

        let line_height = style.font_size * style.line_height;

        // Widow/orphan control: decide how to break before placing lines
        let line_heights: Vec<f64> = vec![line_height; lines.len()];
        let decision = page_break::decide_break(
            cursor.remaining_height(),
            &line_heights,
            true,
            style.min_orphan_lines as usize,
            style.min_widow_lines as usize,
        );

        // Snapshot-and-collect: accumulate line elements, wrap in parent
        let mut snapshot = cursor.elements.len();
        let mut container_start_y = cursor.content_y + cursor.y;
        let mut is_first_element = true;

        // Handle move-to-next-page decision (orphan control)
        if matches!(decision, page_break::BreakDecision::MoveToNextPage) {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
            snapshot = cursor.elements.len();
            container_start_y = cursor.content_y + cursor.y;
        }

        // For split decisions, track the widow/orphan-adjusted first break point
        let forced_break_at = match decision {
            page_break::BreakDecision::Split {
                items_on_current_page,
            } => Some(items_on_current_page),
            _ => None,
        };
        let mut first_break_done = false;

        for (line_idx, line) in lines.iter().enumerate() {
            // Widow/orphan-controlled first break, then normal overflow checks
            let needs_break = if let Some(break_at) = forced_break_at {
                if !first_break_done && line_idx == break_at {
                    true
                } else {
                    line_height > cursor.remaining_height()
                }
            } else {
                line_height > cursor.remaining_height()
            };

            if needs_break {
                first_break_done = true;
                // Flush accumulated lines into a Text container on this page
                let line_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
                if !line_elements.is_empty() {
                    let container_height = cursor.content_y + cursor.y - container_start_y;
                    cursor.elements.push(LayoutElement {
                        x: text_x,
                        y: container_start_y,
                        width: text_width,
                        height: container_height,
                        draw: DrawCommand::None,
                        children: line_elements,
                        node_type: Some(node_type_override.unwrap_or("Text").to_string()),
                        resolved_style: Some(style.clone()),
                        source_location: source_location.cloned(),
                        href: href.map(|s| s.to_string()),
                        bookmark: if is_first_element {
                            bookmark.map(|s| s.to_string())
                        } else {
                            None
                        },
                        alt: None,
                        is_header_row: false,
                        col_span: 1,
                        overflow: Overflow::default(),
                        opacity: 1.0,
                    });
                    is_first_element = false;
                }

                pages.push(cursor.finalize());
                *cursor = cursor.new_page();

                // Reset snapshot for new page
                snapshot = cursor.elements.len();
                container_start_y = cursor.content_y + cursor.y;
            }

            let glyphs = self.build_positioned_glyphs_single_style(line, style, href, font_context);

            // Use actual rendered width from glyphs for alignment (may differ from
            // line.width when per-char measurement is used for line breaking but
            // shaping is used for glyph placement).
            let rendered_width = if glyphs.is_empty() {
                line.width
            } else {
                let last = &glyphs[glyphs.len() - 1];
                (last.x_offset + last.x_advance).max(line.width * 0.5)
            };

            let line_x = match style.text_align {
                TextAlign::Left => text_x,
                TextAlign::Right => text_x + text_width - rendered_width,
                TextAlign::Center => text_x + (text_width - rendered_width) / 2.0,
                TextAlign::Justify => text_x,
            };

            // Justify: compute extra word spacing so the line fills the column width.
            // Use the sum of natural glyph advances (what PDF Tj actually renders)
            // rather than KP-adjusted positions, which bake justification into
            // char_positions and make slack ≈ 0.
            //
            // User-set `word_spacing` is the base; when text is justified, the
            // computed slack-per-space is added on top.
            let is_last_line = line_idx == lines.len() - 1;
            let user_ws = style.word_spacing;
            let (justified_width, word_spacing) =
                if matches!(style.text_align, TextAlign::Justify) && !is_last_line {
                    let last_non_space = glyphs.iter().rposition(|g| g.char_value != ' ');
                    let (natural_width, space_count) = if let Some(idx) = last_non_space {
                        let w: f64 = glyphs[..=idx].iter().map(|g| g.x_advance).sum();
                        let s = glyphs[..=idx]
                            .iter()
                            .filter(|g| g.char_value == ' ')
                            .count();
                        (w, s)
                    } else {
                        (0.0, 0)
                    };
                    let slack = text_width - natural_width;
                    let ws = if space_count > 0 && slack.abs() > 0.01 {
                        slack / space_count as f64
                    } else {
                        0.0
                    };
                    (text_width, user_ws + ws)
                } else {
                    (rendered_width, user_ws)
                };

            let text_line = TextLine {
                x: line_x,
                y: cursor.content_y + cursor.y + style.font_size,
                glyphs,
                width: justified_width,
                height: line_height,
                word_spacing,
            };

            cursor.elements.push(LayoutElement {
                x: line_x,
                y: cursor.content_y + cursor.y,
                width: justified_width,
                height: line_height,
                draw: DrawCommand::Text {
                    lines: vec![text_line],
                    color: style.color,
                    text_decoration: style.text_decoration,
                    opacity: 1.0,
                },
                children: vec![],
                node_type: Some("TextLine".to_string()),
                resolved_style: Some(style.clone()),
                source_location: None,
                href: href.map(|s| s.to_string()),
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: 1.0,
            });

            cursor.y += line_height;
        }

        // Wrap remaining lines into a Text container
        let line_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
        if !line_elements.is_empty() {
            let container_height = cursor.content_y + cursor.y - container_start_y;
            cursor.elements.push(LayoutElement {
                x: text_x,
                y: container_start_y,
                width: text_width,
                height: container_height,
                draw: DrawCommand::None,
                children: line_elements,
                node_type: Some(node_type_override.unwrap_or("Text").to_string()),
                resolved_style: Some(style.clone()),
                source_location: source_location.cloned(),
                href: href.map(|s| s.to_string()),
                bookmark: if is_first_element {
                    bookmark.map(|s| s.to_string())
                } else {
                    None
                },
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: 1.0,
            });
        }

        cursor.y += margin.bottom;
    }

    /// Layout text runs with per-run styling.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn layout_text_runs(
        &self,
        runs: &[TextRun],
        parent_href: Option<&str>,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        text_x: f64,
        text_width: f64,
        font_context: &FontContext,
        source_location: Option<&SourceLocation>,
        bookmark: Option<&str>,
        // Same role as in layout_text — None defaults to "Text".
        node_type_override: Option<&str>,
    ) {
        // Build StyledChar list from runs
        let mut styled_chars: Vec<StyledChar> = Vec::new();
        for run in runs {
            let run_style = run.style.resolve(Some(style), text_width);
            let run_href = run.href.as_deref().or(parent_href);
            let transform = run_style.text_transform;
            let run_content = substitute_page_placeholders(&run.content);
            let mut prev_is_whitespace = true;
            for ch in run_content.chars() {
                let transformed_ch = apply_char_transform(ch, transform, prev_is_whitespace);
                prev_is_whitespace = ch.is_whitespace();
                styled_chars.push(StyledChar {
                    ch: transformed_ch,
                    font_family: run_style.font_family.clone(),
                    font_size: run_style.font_size,
                    font_weight: run_style.font_weight,
                    font_style: run_style.font_style,
                    color: run_style.color,
                    href: run_href.map(|s| s.to_string()),
                    text_decoration: run_style.text_decoration,
                    letter_spacing: run_style.letter_spacing,
                });
            }
        }

        // Break into lines
        let justify = matches!(style.text_align, TextAlign::Justify);
        let broken_lines = match style.line_breaking {
            LineBreaking::Optimal => self.text_layout.break_runs_into_lines_optimal(
                font_context,
                &styled_chars,
                text_width,
                style.hyphens,
                style.lang.as_deref(),
                justify,
            ),
            LineBreaking::Greedy => self.text_layout.break_runs_into_lines(
                font_context,
                &styled_chars,
                text_width,
                style.hyphens,
                style.lang.as_deref(),
            ),
        };

        // Apply text overflow truncation (single-line modes)
        let broken_lines = match style.text_overflow {
            TextOverflow::Ellipsis => {
                self.text_layout
                    .truncate_runs_with_ellipsis(font_context, broken_lines, text_width)
            }
            TextOverflow::Clip => {
                self.text_layout
                    .truncate_runs_clip(font_context, broken_lines, text_width)
            }
            TextOverflow::Wrap => broken_lines,
        };

        let line_height = style.font_size * style.line_height;

        // Widow/orphan control for text runs
        let line_heights: Vec<f64> = vec![line_height; broken_lines.len()];
        let decision = page_break::decide_break(
            cursor.remaining_height(),
            &line_heights,
            true,
            style.min_orphan_lines as usize,
            style.min_widow_lines as usize,
        );

        let mut snapshot = cursor.elements.len();
        let mut container_start_y = cursor.content_y + cursor.y;
        let mut is_first_element = true;

        if matches!(decision, page_break::BreakDecision::MoveToNextPage) {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
            snapshot = cursor.elements.len();
            container_start_y = cursor.content_y + cursor.y;
        }

        let forced_break_at = match decision {
            page_break::BreakDecision::Split {
                items_on_current_page,
            } => Some(items_on_current_page),
            _ => None,
        };
        let mut first_break_done = false;

        for (line_idx, run_line) in broken_lines.iter().enumerate() {
            let needs_break = if let Some(break_at) = forced_break_at {
                if !first_break_done && line_idx == break_at {
                    true
                } else {
                    line_height > cursor.remaining_height()
                }
            } else {
                line_height > cursor.remaining_height()
            };

            if needs_break {
                first_break_done = true;
                let line_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
                if !line_elements.is_empty() {
                    let container_height = cursor.content_y + cursor.y - container_start_y;
                    cursor.elements.push(LayoutElement {
                        x: text_x,
                        y: container_start_y,
                        width: text_width,
                        height: container_height,
                        draw: DrawCommand::None,
                        children: line_elements,
                        node_type: Some(node_type_override.unwrap_or("Text").to_string()),
                        resolved_style: Some(style.clone()),
                        source_location: source_location.cloned(),
                        href: parent_href.map(|s| s.to_string()),
                        bookmark: if is_first_element {
                            bookmark.map(|s| s.to_string())
                        } else {
                            None
                        },
                        alt: None,
                        is_header_row: false,
                        col_span: 1,
                        overflow: Overflow::default(),
                        opacity: 1.0,
                    });
                    is_first_element = false;
                }

                pages.push(cursor.finalize());
                *cursor = cursor.new_page();

                snapshot = cursor.elements.len();
                container_start_y = cursor.content_y + cursor.y;
            }

            let line_x = match style.text_align {
                TextAlign::Left => text_x,
                TextAlign::Right => text_x + text_width - run_line.width,
                TextAlign::Center => text_x + (text_width - run_line.width) / 2.0,
                TextAlign::Justify => text_x,
            };

            let glyphs = self.build_positioned_glyphs_runs(run_line, font_context, style.direction);

            // Justify: compute extra word spacing so the line fills the column width.
            // Use the sum of natural glyph advances (what PDF Tj actually renders)
            // rather than KP-adjusted line width.
            //
            // User-set `word_spacing` is the base; when text is justified, the
            // computed slack-per-space is added on top.
            let is_last_line = line_idx == broken_lines.len() - 1;
            let user_ws = style.word_spacing;
            let (justified_width, word_spacing) =
                if matches!(style.text_align, TextAlign::Justify) && !is_last_line {
                    let last_non_space = glyphs.iter().rposition(|g| g.char_value != ' ');
                    let (natural_width, space_count) = if let Some(idx) = last_non_space {
                        let w: f64 = glyphs[..=idx].iter().map(|g| g.x_advance).sum();
                        let s = glyphs[..=idx]
                            .iter()
                            .filter(|g| g.char_value == ' ')
                            .count();
                        (w, s)
                    } else {
                        (0.0, 0)
                    };
                    let slack = text_width - natural_width;
                    let ws = if space_count > 0 && slack.abs() > 0.01 {
                        slack / space_count as f64
                    } else {
                        0.0
                    };
                    (text_width, user_ws + ws)
                } else {
                    (run_line.width, user_ws)
                };

            let text_line = TextLine {
                x: line_x,
                y: cursor.content_y + cursor.y + style.font_size,
                glyphs,
                width: justified_width,
                height: line_height,
                word_spacing,
            };

            // Determine text decoration: use the run's decoration if any glyph has one
            let text_dec = run_line
                .chars
                .iter()
                .find(|sc| !matches!(sc.text_decoration, TextDecoration::None))
                .map(|sc| sc.text_decoration)
                .unwrap_or(style.text_decoration);

            cursor.elements.push(LayoutElement {
                x: line_x,
                y: cursor.content_y + cursor.y,
                width: justified_width,
                height: line_height,
                draw: DrawCommand::Text {
                    lines: vec![text_line],
                    color: style.color,
                    text_decoration: text_dec,
                    opacity: 1.0,
                },
                children: vec![],
                node_type: Some("TextLine".to_string()),
                resolved_style: Some(style.clone()),
                source_location: None,
                href: parent_href.map(|s| s.to_string()),
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: 1.0,
            });

            cursor.y += line_height;
        }

        let line_elements: Vec<LayoutElement> = cursor.elements.drain(snapshot..).collect();
        if !line_elements.is_empty() {
            let container_height = cursor.content_y + cursor.y - container_start_y;
            cursor.elements.push(LayoutElement {
                x: text_x,
                y: container_start_y,
                width: text_width,
                height: container_height,
                draw: DrawCommand::None,
                children: line_elements,
                node_type: Some(node_type_override.unwrap_or("Text").to_string()),
                resolved_style: Some(style.clone()),
                source_location: source_location.cloned(),
                href: parent_href.map(|s| s.to_string()),
                bookmark: if is_first_element {
                    bookmark.map(|s| s.to_string())
                } else {
                    None
                },
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: 1.0,
            });
        }
    }

    /// Build PositionedGlyphs for a single-style BrokenLine.
    /// For custom fonts, shapes the line text to get real glyph IDs.
    /// For standard fonts, uses char-as-u16 glyph IDs.
    fn build_positioned_glyphs_single_style(
        &self,
        line: &BrokenLine,
        style: &ResolvedStyle,
        href: Option<&str>,
        font_context: &FontContext,
    ) -> Vec<PositionedGlyph> {
        let italic = matches!(style.font_style, FontStyle::Italic | FontStyle::Oblique);
        let line_text: String = line.chars.iter().collect();
        let direction = style.direction;
        // Check if BiDi processing is needed
        let has_bidi = !bidi::is_pure_ltr(&line_text, direction);

        // Segment by font — handles both explicit fallback chains and
        // automatic builtin font fallback (Noto Sans for non-Latin chars)
        let font_runs = crate::font::fallback::segment_by_font(
            &line.chars,
            &style.font_family,
            style.font_weight,
            italic,
            font_context.registry(),
        );
        let needs_per_char_fallback = font_runs.len() > 1
            || (font_runs.len() == 1 && font_runs[0].family != style.font_family);

        // Per-char fallback path: segment by font within each BiDi run
        if needs_per_char_fallback {
            let bidi_runs = if has_bidi {
                bidi::analyze_bidi(&line_text, direction)
            } else {
                vec![crate::text::bidi::BidiRun {
                    char_start: 0,
                    char_end: line.chars.len(),
                    level: unicode_bidi::Level::ltr(),
                    is_rtl: false,
                }]
            };

            let mut all_glyphs = Vec::new();
            let mut bidi_levels = Vec::new();
            let mut x = 0.0_f64;

            // Process each BiDi run
            for bidi_run in &bidi_runs {
                // Within this BiDi run, sub-segment by font
                for font_run in &font_runs {
                    // Intersect font_run with bidi_run
                    let start = font_run.start.max(bidi_run.char_start);
                    let end = font_run.end.min(bidi_run.char_end);
                    if start >= end {
                        continue;
                    }

                    let sub_chars: Vec<char> = line.chars[start..end].to_vec();
                    let sub_text: String = sub_chars.iter().collect();
                    let resolved_family = &font_run.family;

                    if let Some(font_data) =
                        font_context.font_data(resolved_family, style.font_weight, italic)
                    {
                        if let Some(shaped) = shaping::shape_text_with_direction(
                            &sub_text,
                            font_data,
                            bidi_run.is_rtl,
                        ) {
                            let units_per_em = font_context.units_per_em(
                                resolved_family,
                                style.font_weight,
                                italic,
                            );
                            let scale = style.font_size / units_per_em as f64;

                            for sg in &shaped {
                                let cluster = sg.cluster as usize;
                                let char_value = sub_chars.get(cluster).copied().unwrap_or(' ');

                                let cluster_text = if shaped.len() < sub_chars.len() {
                                    let cluster_end =
                                        self.find_cluster_end(&shaped, sg, sub_chars.len());
                                    if cluster_end > cluster + 1 {
                                        Some(
                                            sub_chars[cluster..cluster_end]
                                                .iter()
                                                .collect::<String>(),
                                        )
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                let glyph_x = x + sg.x_offset as f64 * scale;
                                let glyph_y = sg.y_offset as f64 * scale;
                                let advance = sg.x_advance as f64 * scale + style.letter_spacing;

                                all_glyphs.push(PositionedGlyph {
                                    glyph_id: sg.glyph_id,
                                    x_offset: glyph_x,
                                    y_offset: glyph_y,
                                    x_advance: advance,
                                    font_size: style.font_size,
                                    font_family: resolved_family.clone(),
                                    font_weight: style.font_weight,
                                    font_style: style.font_style,
                                    char_value,
                                    color: Some(style.color),
                                    href: href.map(|s| s.to_string()),
                                    text_decoration: style.text_decoration,
                                    letter_spacing: style.letter_spacing,
                                    cluster_text,
                                });
                                bidi_levels.push(bidi_run.level);
                                x += advance;
                            }
                            continue;
                        }
                    }

                    // Fallback: standard font or shaping failure for this sub-segment
                    for i in start..end {
                        let ch = line.chars[i];
                        let glyph_x = x;
                        let char_width = font_context.char_width(
                            ch,
                            resolved_family,
                            style.font_weight,
                            italic,
                            style.font_size,
                        );
                        let advance = char_width + style.letter_spacing;
                        all_glyphs.push(PositionedGlyph {
                            glyph_id: ch as u16,
                            x_offset: glyph_x,
                            y_offset: 0.0,
                            x_advance: advance,
                            font_size: style.font_size,
                            font_family: resolved_family.clone(),
                            font_weight: style.font_weight,
                            font_style: style.font_style,
                            char_value: ch,
                            color: Some(style.color),
                            href: href.map(|s| s.to_string()),
                            text_decoration: style.text_decoration,
                            letter_spacing: style.letter_spacing,
                            cluster_text: None,
                        });
                        bidi_levels.push(bidi_run.level);
                        x += advance;
                    }
                }
            }

            // Apply BiDi visual reordering if needed
            if has_bidi && !all_glyphs.is_empty() {
                all_glyphs = bidi::reorder_line_glyphs(all_glyphs, &bidi_levels);
                bidi::reposition_after_reorder(&mut all_glyphs, 0.0);
            }
            return all_glyphs;
        }

        // Original single-font path (no comma in font_family)
        // Try shaping for custom fonts
        if let Some(font_data) =
            font_context.font_data(&style.font_family, style.font_weight, italic)
        {
            if has_bidi {
                // BiDi path: analyze runs, shape each with correct direction
                let bidi_runs = bidi::analyze_bidi(&line_text, direction);
                let units_per_em =
                    font_context.units_per_em(&style.font_family, style.font_weight, italic);
                let scale = style.font_size / units_per_em as f64;

                let mut all_glyphs = Vec::new();
                let mut bidi_levels = Vec::new();
                let mut x = 0.0_f64;

                for run in &bidi_runs {
                    let run_chars: Vec<char> = line.chars[run.char_start..run.char_end].to_vec();
                    let run_text: String = run_chars.iter().collect();

                    if let Some(shaped) =
                        shaping::shape_text_with_direction(&run_text, font_data, run.is_rtl)
                    {
                        for sg in &shaped {
                            let cluster = sg.cluster as usize;
                            let char_value = run_chars.get(cluster).copied().unwrap_or(' ');

                            let cluster_text = if shaped.len() < run_chars.len() {
                                let cluster_end =
                                    self.find_cluster_end(&shaped, sg, run_chars.len());
                                if cluster_end > cluster + 1 {
                                    Some(run_chars[cluster..cluster_end].iter().collect::<String>())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let glyph_x = x + sg.x_offset as f64 * scale;
                            let glyph_y = sg.y_offset as f64 * scale;
                            let advance = sg.x_advance as f64 * scale + style.letter_spacing;

                            all_glyphs.push(PositionedGlyph {
                                glyph_id: sg.glyph_id,
                                x_offset: glyph_x,
                                y_offset: glyph_y,
                                x_advance: advance,
                                font_size: style.font_size,
                                font_family: style.font_family.clone(),
                                font_weight: style.font_weight,
                                font_style: style.font_style,
                                char_value,
                                color: Some(style.color),
                                href: href.map(|s| s.to_string()),
                                text_decoration: style.text_decoration,
                                letter_spacing: style.letter_spacing,
                                cluster_text,
                            });
                            bidi_levels.push(run.level);

                            x += advance;
                        }
                    }
                }

                // Reorder glyphs visually and reposition
                let mut glyphs = bidi::reorder_line_glyphs(all_glyphs, &bidi_levels);
                bidi::reposition_after_reorder(&mut glyphs, 0.0);
                return glyphs;
            }

            // Pure LTR path: shape normally
            if let Some(shaped) = shaping::shape_text(&line_text, font_data) {
                let units_per_em =
                    font_context.units_per_em(&style.font_family, style.font_weight, italic);
                let scale = style.font_size / units_per_em as f64;

                return self.shaped_glyphs_to_positioned(
                    &shaped,
                    &line.chars,
                    &line.char_positions,
                    scale,
                    style.font_size,
                    &style.font_family,
                    style.font_weight,
                    style.font_style,
                    Some(style.color),
                    href,
                    style.text_decoration,
                    style.letter_spacing,
                );
            }
        }

        // Fallback: standard fonts or shaping failure
        let mut glyphs: Vec<PositionedGlyph> = line
            .chars
            .iter()
            .enumerate()
            .map(|(j, ch)| {
                let glyph_x = line.char_positions.get(j).copied().unwrap_or(0.0);
                let char_width = font_context.char_width(
                    *ch,
                    &style.font_family,
                    style.font_weight,
                    italic,
                    style.font_size,
                );
                PositionedGlyph {
                    glyph_id: *ch as u16,
                    x_offset: glyph_x,
                    y_offset: 0.0,
                    x_advance: char_width,
                    font_size: style.font_size,
                    font_family: style.font_family.clone(),
                    font_weight: style.font_weight,
                    font_style: style.font_style,
                    char_value: *ch,
                    color: Some(style.color),
                    href: href.map(|s| s.to_string()),
                    text_decoration: style.text_decoration,
                    letter_spacing: style.letter_spacing,
                    cluster_text: None,
                }
            })
            .collect();

        // For standard fonts with BiDi text, still reorder visually
        if has_bidi && !glyphs.is_empty() {
            let bidi_runs = bidi::analyze_bidi(&line_text, direction);
            let mut levels = Vec::with_capacity(glyphs.len());
            let mut char_idx = 0;
            for run in &bidi_runs {
                for _ in run.char_start..run.char_end {
                    if char_idx < glyphs.len() {
                        levels.push(run.level);
                        char_idx += 1;
                    }
                }
            }
            // Pad if needed
            while levels.len() < glyphs.len() {
                levels.push(unicode_bidi::Level::ltr());
            }
            glyphs = bidi::reorder_line_glyphs(glyphs, &levels);
            bidi::reposition_after_reorder(&mut glyphs, 0.0);
        }

        glyphs
    }

    /// Build PositionedGlyphs for a multi-style RunBrokenLine.
    /// Shapes contiguous runs of the same custom font, with BiDi support.
    /// When a StyledChar has a comma-separated font_family, resolves each
    /// character to a single font before grouping for shaping.
    fn build_positioned_glyphs_runs(
        &self,
        run_line: &RunBrokenLine,
        font_context: &FontContext,
        direction: Direction,
    ) -> Vec<PositionedGlyph> {
        let chars = &run_line.chars;
        if chars.is_empty() {
            return vec![];
        }

        // Pre-resolve per-char font families from comma chains.
        // This produces a vec of resolved single family names, one per char.
        let resolved_families: Vec<String> = chars
            .iter()
            .map(|sc| {
                if !sc.font_family.contains(',') {
                    sc.font_family.clone()
                } else {
                    let italic = matches!(sc.font_style, FontStyle::Italic | FontStyle::Oblique);
                    let (_, family) = font_context.registry().resolve_for_char(
                        &sc.font_family,
                        sc.ch,
                        sc.font_weight,
                        italic,
                    );
                    family
                }
            })
            .collect();

        let line_text: String = chars.iter().map(|c| c.ch).collect();
        let has_bidi = !bidi::is_pure_ltr(&line_text, direction);
        let bidi_runs = if has_bidi {
            Some(bidi::analyze_bidi(&line_text, direction))
        } else {
            None
        };

        let mut glyphs = Vec::new();
        let mut bidi_levels = Vec::new();
        let mut i = 0;

        while i < chars.len() {
            let sc = &chars[i];
            let italic = matches!(sc.font_style, FontStyle::Italic | FontStyle::Oblique);
            let resolved_family = &resolved_families[i];

            // Determine if this char is in an RTL BiDi run
            let is_rtl = bidi_runs.as_ref().is_some_and(|runs| {
                runs.iter()
                    .any(|r| i >= r.char_start && i < r.char_end && r.is_rtl)
            });

            // Check for custom font with shaping (using resolved single family)
            if let Some(font_data) = font_context.font_data(resolved_family, sc.font_weight, italic)
            {
                // Find contiguous run with same resolved font AND same BiDi direction
                let run_start = i;
                let mut run_end = i + 1;
                while run_end < chars.len() {
                    let next = &chars[run_end];
                    let next_italic =
                        matches!(next.font_style, FontStyle::Italic | FontStyle::Oblique);
                    let next_is_rtl = bidi_runs.as_ref().is_some_and(|runs| {
                        runs.iter()
                            .any(|r| run_end >= r.char_start && run_end < r.char_end && r.is_rtl)
                    });
                    // Group by resolved family, not original comma chain
                    if resolved_families[run_end] == *resolved_family
                        && next.font_weight == sc.font_weight
                        && next_italic == italic
                        && (next.font_size - sc.font_size).abs() < 0.001
                        && next_is_rtl == is_rtl
                    {
                        run_end += 1;
                    } else {
                        break;
                    }
                }

                let run_text: String = chars[run_start..run_end].iter().map(|c| c.ch).collect();
                if let Some(shaped) =
                    shaping::shape_text_with_direction(&run_text, font_data, is_rtl)
                {
                    let units_per_em =
                        font_context.units_per_em(resolved_family, sc.font_weight, italic);
                    let scale = sc.font_size / units_per_em as f64;

                    // Build char positions for this run segment
                    let run_chars: Vec<char> =
                        chars[run_start..run_end].iter().map(|c| c.ch).collect();
                    let run_positions: Vec<f64> = (run_start..run_end)
                        .map(|j| run_line.char_positions.get(j).copied().unwrap_or(0.0))
                        .collect();

                    // Build glyphs with resolved single family on each glyph
                    let mut run_glyphs = self.shaped_glyphs_to_positioned_runs(
                        &shaped,
                        &chars[run_start..run_end],
                        &run_chars,
                        &run_positions,
                        scale,
                    );
                    // Override font_family to the resolved single family
                    for g in &mut run_glyphs {
                        g.font_family = resolved_family.clone();
                    }
                    // Track BiDi levels for each glyph
                    let run_level = if is_rtl {
                        unicode_bidi::Level::rtl()
                    } else {
                        unicode_bidi::Level::ltr()
                    };
                    for _ in &run_glyphs {
                        bidi_levels.push(run_level);
                    }
                    glyphs.extend(run_glyphs);
                    i = run_end;
                    continue;
                }
            }

            // Fallback: unshaped glyph (using resolved family)
            let glyph_x = run_line.char_positions.get(i).copied().unwrap_or(0.0);
            let char_width = font_context.char_width(
                sc.ch,
                resolved_family,
                sc.font_weight,
                italic,
                sc.font_size,
            );
            glyphs.push(PositionedGlyph {
                glyph_id: sc.ch as u16,
                x_offset: glyph_x,
                y_offset: 0.0,
                x_advance: char_width,
                font_size: sc.font_size,
                font_family: resolved_family.clone(),
                font_weight: sc.font_weight,
                font_style: sc.font_style,
                char_value: sc.ch,
                color: Some(sc.color),
                href: sc.href.clone(),
                text_decoration: sc.text_decoration,
                letter_spacing: sc.letter_spacing,
                cluster_text: None,
            });
            bidi_levels.push(if is_rtl {
                unicode_bidi::Level::rtl()
            } else {
                unicode_bidi::Level::ltr()
            });
            i += 1;
        }

        // Apply BiDi visual reordering if needed
        if has_bidi && !glyphs.is_empty() {
            glyphs = bidi::reorder_line_glyphs(glyphs, &bidi_levels);
            bidi::reposition_after_reorder(&mut glyphs, 0.0);
        }

        glyphs
    }

    /// Convert shaped glyphs to PositionedGlyphs for single-style text.
    #[allow(clippy::too_many_arguments)]
    fn shaped_glyphs_to_positioned(
        &self,
        shaped: &[shaping::ShapedGlyph],
        chars: &[char],
        _char_positions: &[f64],
        scale: f64,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        color: Option<Color>,
        href: Option<&str>,
        text_decoration: TextDecoration,
        letter_spacing: f64,
    ) -> Vec<PositionedGlyph> {
        let mut result = Vec::with_capacity(shaped.len());
        let mut x = 0.0_f64;

        for sg in shaped {
            let cluster = sg.cluster as usize;
            let char_value = chars.get(cluster).copied().unwrap_or(' ');

            // Determine cluster text for ligatures
            let cluster_text = if shaped.len() < chars.len() {
                // There are fewer glyphs than chars: likely ligatures.
                // Find end of this cluster.
                let cluster_end = self.find_cluster_end(shaped, sg, chars.len());
                if cluster_end > cluster + 1 {
                    Some(chars[cluster..cluster_end].iter().collect::<String>())
                } else {
                    None
                }
            } else {
                None
            };

            // Use shaped position
            let glyph_x = x + sg.x_offset as f64 * scale;
            let glyph_y = sg.y_offset as f64 * scale;
            let advance = sg.x_advance as f64 * scale + letter_spacing;

            result.push(PositionedGlyph {
                glyph_id: sg.glyph_id,
                x_offset: glyph_x,
                y_offset: glyph_y,
                x_advance: advance,
                font_size,
                font_family: font_family.to_string(),
                font_weight,
                font_style,
                char_value,
                color,
                href: href.map(|s| s.to_string()),
                text_decoration,
                letter_spacing,
                cluster_text,
            });

            x += advance;
        }

        result
    }

    /// Convert shaped glyphs to PositionedGlyphs for multi-style runs.
    fn shaped_glyphs_to_positioned_runs(
        &self,
        shaped: &[shaping::ShapedGlyph],
        styled_chars: &[StyledChar],
        chars: &[char],
        char_positions: &[f64],
        scale: f64,
    ) -> Vec<PositionedGlyph> {
        let mut result = Vec::with_capacity(shaped.len());
        // Use the first char position as the base offset for this run
        let base_x = char_positions.first().copied().unwrap_or(0.0);
        let mut x = 0.0_f64;

        for sg in shaped {
            let cluster = sg.cluster as usize;
            let sc = styled_chars.get(cluster).unwrap_or(&styled_chars[0]);
            let char_value = chars.get(cluster).copied().unwrap_or(' ');

            let cluster_text = if shaped.len() < chars.len() {
                let cluster_end = self.find_cluster_end(shaped, sg, chars.len());
                if cluster_end > cluster + 1 {
                    Some(chars[cluster..cluster_end].iter().collect::<String>())
                } else {
                    None
                }
            } else {
                None
            };

            let glyph_x = base_x + x + sg.x_offset as f64 * scale;
            let glyph_y = sg.y_offset as f64 * scale;
            let advance = sg.x_advance as f64 * scale + sc.letter_spacing;

            result.push(PositionedGlyph {
                glyph_id: sg.glyph_id,
                x_offset: glyph_x,
                y_offset: glyph_y,
                x_advance: advance,
                font_size: sc.font_size,
                font_family: sc.font_family.clone(),
                font_weight: sc.font_weight,
                font_style: sc.font_style,
                char_value,
                color: Some(sc.color),
                href: sc.href.clone(),
                text_decoration: sc.text_decoration,
                letter_spacing: sc.letter_spacing,
                cluster_text,
            });

            x += advance;
        }

        result
    }

    /// Find the end index of a cluster in shaped glyphs.
    fn find_cluster_end(
        &self,
        shaped: &[shaping::ShapedGlyph],
        current: &shaping::ShapedGlyph,
        num_chars: usize,
    ) -> usize {
        // Find the next glyph's cluster value
        for sg in shaped {
            if sg.cluster > current.cluster {
                return sg.cluster as usize;
            }
        }
        // Last glyph: cluster extends to end of text
        num_chars
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_image(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        explicit_width: Option<f64>,
        explicit_height: Option<f64>,
    ) {
        let margin = &style.margin.to_edges();

        // Try to load the image from the node's src field
        let src = match &node.kind {
            NodeKind::Image { src, .. } => src.as_str(),
            _ => "",
        };

        let loaded = if !src.is_empty() {
            crate::image_loader::load_image(src).ok()
        } else {
            None
        };

        // Compute display dimensions with aspect ratio preservation
        let (img_width, img_height) = if let Some(ref img) = loaded {
            let intrinsic_w = img.width_px as f64;
            let intrinsic_h = img.height_px as f64;
            let aspect = if intrinsic_w > 0.0 {
                intrinsic_h / intrinsic_w
            } else {
                0.75
            };

            match (explicit_width, explicit_height) {
                (Some(w), Some(h)) => (w, h),
                (Some(w), None) => (w, w * aspect),
                (None, Some(h)) => (h / aspect, h),
                (None, None) => {
                    let max_w = available_width - margin.horizontal();
                    let w = intrinsic_w.min(max_w);
                    (w, w * aspect)
                }
            }
        } else {
            // Fallback dimensions when image can't be loaded
            let w = explicit_width.unwrap_or(available_width - margin.horizontal());
            let h = explicit_height.unwrap_or(w * 0.75);
            (w, h)
        };

        let total_height = img_height + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let draw = if let Some(image_data) = loaded {
            DrawCommand::Image { image_data }
        } else {
            DrawCommand::ImagePlaceholder
        };

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: img_width,
            height: img_height,
            draw,
            children: vec![],
            node_type: Some(node_kind_name(&node.kind).to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += img_height + margin.bottom;
    }

    /// Layout an SVG element as a fixed-size box.
    #[allow(clippy::too_many_arguments)]
    fn layout_svg(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        _available_width: f64,
        svg_width: f64,
        svg_height: f64,
        view_box: Option<&str>,
        content: &str,
    ) {
        let margin = &style.margin.to_edges();
        let total_height = svg_height + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let vb = view_box
            .and_then(crate::svg::parse_view_box)
            .unwrap_or(crate::svg::ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: svg_width,
                height: svg_height,
            });

        let commands = crate::svg::parse_svg(content, vb, svg_width, svg_height);

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: svg_width,
            height: svg_height,
            draw: DrawCommand::Svg {
                commands,
                width: svg_width,
                height: svg_height,
                viewbox_min_x: vb.min_x,
                viewbox_min_y: vb.min_y,
                viewbox_width: vb.width,
                viewbox_height: vb.height,
                clip: false,
            },
            children: vec![],
            node_type: Some("Svg".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += svg_height + margin.bottom;
    }

    /// Convert CanvasOps to SvgCommands, reusing the existing SVG rendering pipeline.
    fn canvas_ops_to_svg_commands(operations: &[CanvasOp]) -> Vec<crate::svg::SvgCommand> {
        use crate::svg::SvgCommand;

        let mut commands = Vec::new();
        let mut cur_x = 0.0_f64;
        let mut cur_y = 0.0_f64;

        for op in operations {
            match op {
                CanvasOp::MoveTo { x, y } => {
                    commands.push(SvgCommand::MoveTo(*x, *y));
                    cur_x = *x;
                    cur_y = *y;
                }
                CanvasOp::LineTo { x, y } => {
                    commands.push(SvgCommand::LineTo(*x, *y));
                    cur_x = *x;
                    cur_y = *y;
                }
                CanvasOp::BezierCurveTo {
                    cp1x,
                    cp1y,
                    cp2x,
                    cp2y,
                    x,
                    y,
                } => {
                    commands.push(SvgCommand::CurveTo(*cp1x, *cp1y, *cp2x, *cp2y, *x, *y));
                    cur_x = *x;
                    cur_y = *y;
                }
                CanvasOp::QuadraticCurveTo { cpx, cpy, x, y } => {
                    // Convert quadratic to cubic bezier
                    let cp1x = cur_x + 2.0 / 3.0 * (*cpx - cur_x);
                    let cp1y = cur_y + 2.0 / 3.0 * (*cpy - cur_y);
                    let cp2x = *x + 2.0 / 3.0 * (*cpx - *x);
                    let cp2y = *y + 2.0 / 3.0 * (*cpy - *y);
                    commands.push(SvgCommand::CurveTo(cp1x, cp1y, cp2x, cp2y, *x, *y));
                    cur_x = *x;
                    cur_y = *y;
                }
                CanvasOp::ClosePath => {
                    commands.push(SvgCommand::ClosePath);
                }
                CanvasOp::Rect {
                    x,
                    y,
                    width,
                    height,
                } => {
                    commands.push(SvgCommand::MoveTo(*x, *y));
                    commands.push(SvgCommand::LineTo(*x + *width, *y));
                    commands.push(SvgCommand::LineTo(*x + *width, *y + *height));
                    commands.push(SvgCommand::LineTo(*x, *y + *height));
                    commands.push(SvgCommand::ClosePath);
                    cur_x = *x;
                    cur_y = *y;
                }
                CanvasOp::Circle { cx, cy, r } => {
                    commands.extend(crate::svg::ellipse_commands(*cx, *cy, *r, *r));
                }
                CanvasOp::Ellipse { cx, cy, rx, ry } => {
                    commands.extend(crate::svg::ellipse_commands(*cx, *cy, *rx, *ry));
                }
                CanvasOp::Arc {
                    cx,
                    cy,
                    r,
                    start_angle,
                    end_angle,
                    counterclockwise,
                } => {
                    // Approximate arc with line segments matching HTML Canvas arc() semantics.
                    // Canvas coords are Y-down (like HTML Canvas), and the PDF Y-flip
                    // preserves visual positions, so standard trig (cy + r*sin) is correct.
                    let steps = 32;
                    let mut sweep = end_angle - start_angle;
                    if !counterclockwise && sweep < 0.0 {
                        sweep += 2.0 * std::f64::consts::PI;
                    }
                    if *counterclockwise && sweep > 0.0 {
                        sweep -= 2.0 * std::f64::consts::PI;
                    }
                    for i in 0..=steps {
                        let t = *start_angle + sweep * (i as f64 / steps as f64);
                        let px = cx + r * t.cos();
                        let py = cy + r * t.sin();
                        if i == 0 {
                            commands.push(SvgCommand::MoveTo(px, py));
                        } else {
                            commands.push(SvgCommand::LineTo(px, py));
                        }
                    }
                }
                CanvasOp::Stroke => commands.push(SvgCommand::Stroke),
                CanvasOp::Fill => commands.push(SvgCommand::Fill),
                CanvasOp::FillAndStroke => commands.push(SvgCommand::FillAndStroke),
                CanvasOp::SetFillColor { r, g, b } => {
                    // Canvas API uses 0-255, PDF/SVG pipeline uses 0-1
                    commands.push(SvgCommand::SetFill(r / 255.0, g / 255.0, b / 255.0));
                }
                CanvasOp::SetStrokeColor { r, g, b } => {
                    commands.push(SvgCommand::SetStroke(r / 255.0, g / 255.0, b / 255.0));
                }
                CanvasOp::SetLineWidth { width } => {
                    commands.push(SvgCommand::SetStrokeWidth(*width));
                }
                CanvasOp::SetLineCap { cap } => {
                    commands.push(SvgCommand::SetLineCap(*cap));
                }
                CanvasOp::SetLineJoin { join } => {
                    commands.push(SvgCommand::SetLineJoin(*join));
                }
                CanvasOp::Save => commands.push(SvgCommand::SaveState),
                CanvasOp::Restore => commands.push(SvgCommand::RestoreState),
            }
        }

        commands
    }

    /// Layout a canvas element as a fixed-size box with vector graphics.
    #[allow(clippy::too_many_arguments)]
    fn layout_canvas(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        _available_width: f64,
        canvas_width: f64,
        canvas_height: f64,
        operations: &[CanvasOp],
    ) {
        let margin = style.margin.to_edges();
        let total_height = canvas_height + margin.top + margin.bottom;

        // Page break check
        if cursor.remaining_height() < total_height && cursor.y > 0.0 {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let svg_commands = Self::canvas_ops_to_svg_commands(operations);

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: canvas_width,
            height: canvas_height,
            draw: DrawCommand::Svg {
                commands: svg_commands,
                width: canvas_width,
                height: canvas_height,
                // Canvas constructs commands in display coordinates, so the
                // viewBox matches the display box 1:1 — scale comes out to 1.
                viewbox_min_x: 0.0,
                viewbox_min_y: 0.0,
                viewbox_width: canvas_width,
                viewbox_height: canvas_height,
                clip: true,
            },
            children: vec![],
            node_type: Some("Canvas".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += canvas_height + margin.bottom;
    }

    /// Layout a 1D barcode as a row of vector rectangles.
    #[allow(clippy::too_many_arguments)]
    /// Layout a chart as a single unbreakable block of drawing primitives.
    #[allow(clippy::too_many_arguments)]
    fn layout_chart(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        chart_width: f64,
        chart_height: f64,
        primitives: Vec<crate::chart::ChartPrimitive>,
        node_type_name: &str,
    ) {
        let margin = &style.margin.to_edges();
        let total_height = chart_height + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let draw = DrawCommand::Chart { primitives };

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: chart_width,
            height: chart_height,
            draw,
            children: vec![],
            node_type: Some(node_type_name.to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += chart_height + margin.bottom;
    }

    /// Layout a form field as a fixed-size leaf node.
    #[allow(clippy::too_many_arguments)]
    fn layout_form_field(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        field_width: f64,
        field_height: f64,
        draw: DrawCommand,
        node_type_name: &str,
    ) {
        let margin = &style.margin.to_edges();
        let total_height = field_height + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: field_width,
            height: field_height,
            draw,
            children: vec![],
            node_type: Some(node_type_name.to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += field_height + margin.bottom;
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_barcode(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        data: &str,
        format: crate::barcode::BarcodeFormat,
        explicit_width: Option<f64>,
        bar_height: f64,
    ) {
        let margin = &style.margin.to_edges();
        let display_width = explicit_width.unwrap_or(available_width - margin.horizontal());
        let total_height = bar_height + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let draw = match crate::barcode::generate_barcode(data, format) {
            Ok(barcode_data) => {
                let bar_width = if barcode_data.bars.is_empty() {
                    0.0
                } else {
                    display_width / barcode_data.bars.len() as f64
                };
                DrawCommand::Barcode {
                    bars: barcode_data.bars,
                    bar_width,
                    height: bar_height,
                    color: style.color,
                }
            }
            Err(_) => DrawCommand::None,
        };

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: display_width,
            height: bar_height,
            draw,
            children: vec![],
            node_type: Some("Barcode".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += bar_height + margin.bottom;
    }

    /// Layout a QR code as a square block of vector rectangles.
    #[allow(clippy::too_many_arguments)]
    fn layout_qrcode(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        data: &str,
        explicit_size: Option<f64>,
    ) {
        let margin = &style.margin.to_edges();
        let display_size = explicit_size.unwrap_or(available_width - margin.horizontal());
        let total_height = display_size + margin.vertical();

        if total_height > cursor.remaining_height() {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        cursor.y += margin.top;

        let draw = match crate::qrcode::generate_qr(data) {
            Ok(matrix) => {
                let module_size = display_size / matrix.size as f64;
                DrawCommand::QrCode {
                    modules: matrix.modules,
                    module_size,
                    color: style.color,
                }
            }
            Err(_) => DrawCommand::None,
        };

        cursor.elements.push(LayoutElement {
            x: x + margin.left,
            y: cursor.content_y + cursor.y,
            width: display_size,
            height: display_size,
            draw,
            children: vec![],
            node_type: Some("QrCode".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: node.href.clone(),
            bookmark: node.bookmark.clone(),
            alt: node.alt.clone(),
            is_header_row: false,
            col_span: 1,
            overflow: style.overflow,
            opacity: style.opacity,
        });

        cursor.y += display_size + margin.bottom;
    }

    // ── Measurement helpers ─────────────────────────────────────

    fn measure_node_height(
        &self,
        node: &Node,
        available_width: f64,
        style: &ResolvedStyle,
        font_context: &FontContext,
    ) -> f64 {
        match &node.kind {
            // Headings lay out exactly like Text (see the layout arm), so they
            // must measure the same way — otherwise a heading falls through to
            // the container `_` arm, measures ~0 (it has no children), and a
            // parent's auto-height omits it.
            NodeKind::Text { content, runs, .. } | NodeKind::Heading { content, runs, .. } => {
                // Mirror layout_text: a fixed width drives line-breaking, so height
                // measurement must use the same width or it will under-count lines.
                let measure_width = match style.width {
                    SizeConstraint::Fixed(w) => (w - style.margin.horizontal()).max(0.0),
                    SizeConstraint::Auto => available_width - style.margin.horizontal(),
                };
                if !runs.is_empty() {
                    // Measure runs
                    let mut styled_chars: Vec<StyledChar> = Vec::new();
                    for run in runs {
                        let run_style = run.style.resolve(Some(style), measure_width);
                        let run_content = substitute_page_placeholders(&run.content);
                        for ch in run_content.chars() {
                            styled_chars.push(StyledChar {
                                ch,
                                font_family: run_style.font_family.clone(),
                                font_size: run_style.font_size,
                                font_weight: run_style.font_weight,
                                font_style: run_style.font_style,
                                color: run_style.color,
                                href: None,
                                text_decoration: run_style.text_decoration,
                                letter_spacing: run_style.letter_spacing,
                            });
                        }
                    }
                    let broken_lines = self.text_layout.break_runs_into_lines(
                        font_context,
                        &styled_chars,
                        measure_width,
                        style.hyphens,
                        style.lang.as_deref(),
                    );
                    let line_height = style.font_size * style.line_height;
                    (broken_lines.len() as f64) * line_height + style.padding.vertical()
                } else {
                    let content = substitute_page_placeholders(content);
                    let lines = self.text_layout.break_into_lines(
                        font_context,
                        &content,
                        measure_width,
                        style.font_size,
                        &style.font_family,
                        style.font_weight,
                        style.font_style,
                        style.letter_spacing,
                        style.hyphens,
                        style.lang.as_deref(),
                    );
                    let line_height = style.font_size * style.line_height;
                    (lines.len() as f64) * line_height + style.padding.vertical()
                }
            }
            NodeKind::Image {
                src,
                width: explicit_w,
                height: explicit_h,
            } => {
                // 1. style.height takes precedence
                if let SizeConstraint::Fixed(h) = style.height {
                    return h + style.padding.vertical();
                }
                // 2. Explicit height prop
                if let Some(h) = explicit_h {
                    return *h + style.padding.vertical();
                }
                // 3. Compute from real image aspect ratio (header-only read, no pixel decode)
                let aspect = self
                    .get_image_dimensions(src)
                    .map(|(w, h)| if w > 0 { h as f64 / w as f64 } else { 0.75 })
                    .unwrap_or(0.75);
                let w = if let SizeConstraint::Fixed(w) = style.width {
                    w
                } else {
                    explicit_w.unwrap_or(available_width - style.margin.horizontal())
                };
                w * aspect + style.padding.vertical()
            }
            NodeKind::Svg { height, .. } => *height + style.margin.vertical(),
            NodeKind::Barcode { height, .. } => *height + style.margin.vertical(),
            NodeKind::QrCode { size, .. } => {
                let display_size = size.unwrap_or(available_width - style.margin.horizontal());
                display_size + style.margin.vertical()
            }
            NodeKind::Canvas { height, .. } => *height + style.margin.vertical(),
            NodeKind::BarChart { height, .. }
            | NodeKind::LineChart { height, .. }
            | NodeKind::PieChart { height, .. }
            | NodeKind::AreaChart { height, .. }
            | NodeKind::DotPlot { height, .. } => *height + style.margin.vertical(),
            NodeKind::TextField { height, .. }
            | NodeKind::Checkbox { height, .. }
            | NodeKind::Dropdown { height, .. }
            | NodeKind::RadioButton { height, .. } => *height + style.margin.vertical(),
            NodeKind::Watermark { .. } => 0.0, // Watermarks take zero layout height
            NodeKind::Table { columns } => {
                // Use the same column-resolution + per-row max-of-cells helpers
                // that `layout_table` uses, so measurement matches what the
                // engine actually renders. Without this arm, Table fell into the
                // generic `_` branch which column-summed each row's children,
                // and (since TableRow also lacked an arm) over-counted row
                // heights by a factor of (cell count).
                if let SizeConstraint::Fixed(h) = style.height {
                    return h;
                }
                let outer_width = match style.width {
                    SizeConstraint::Fixed(w) => w,
                    SizeConstraint::Auto => available_width - style.margin.horizontal(),
                };
                let inner_width =
                    outer_width - style.padding.horizontal() - style.border_width.horizontal();
                let col_widths = self.resolve_column_widths(columns, inner_width, &node.children);
                let row_gap = style.row_gap;
                let mut total = 0.0;
                for (i, row) in node.children.iter().enumerate() {
                    if i > 0 {
                        total += row_gap;
                    }
                    total += self.measure_table_row_height(row, &col_widths, style, font_context);
                }
                total + style.padding.vertical() + style.border_width.vertical()
            }
            NodeKind::TableRow { .. } => {
                // Standalone-row fallback (rare): a TableRow measured outside
                // a Table context has no ColumnDef source, so split
                // available_width evenly across cells — matches what
                // resolve_column_widths does when its defs vec is empty.
                let n = node.children.len().max(1);
                let usable = (available_width - style.margin.horizontal()).max(0.0);
                let col_w = usable / n as f64;
                let col_widths = vec![col_w; n];
                self.measure_table_row_height(node, &col_widths, style, font_context)
            }
            _ => {
                // If a fixed height is specified, use it directly
                if let SizeConstraint::Fixed(h) = style.height {
                    return h;
                }
                // Match layout_view: when width is Auto, margin reduces the
                // outer width; min/max clamp identically or measured heights
                // disagree with laid-out widths.
                let outer_width = match style.width {
                    SizeConstraint::Fixed(w) => w,
                    SizeConstraint::Auto => available_width - style.margin.horizontal(),
                }
                .min(style.max_width)
                .max(style.min_width);
                let inner_width =
                    outer_width - style.padding.horizontal() - style.border_width.horizontal();
                let children_height =
                    self.measure_children_height(&node.children, inner_width, style, font_context);
                (children_height + style.padding.vertical() + style.border_width.vertical())
                    .max(style.min_height)
            }
        }
    }

    fn measure_children_height(
        &self,
        children: &[Node],
        available_width: f64,
        parent_style: &ResolvedStyle,
        font_context: &FontContext,
    ) -> f64 {
        // Grid layout: measure using actual grid placement instead of stacking
        if matches!(parent_style.display, Display::Grid) {
            if let Some(template_cols) = &parent_style.grid_template_columns {
                let num_columns = template_cols.len();
                if num_columns > 0 && !children.is_empty() {
                    let col_gap = parent_style.column_gap;
                    let row_gap = parent_style.row_gap;

                    let content_sizes: Vec<f64> = template_cols
                        .iter()
                        .map(|track| {
                            if matches!(track, GridTrackSize::Auto) {
                                available_width / num_columns as f64
                            } else {
                                0.0
                            }
                        })
                        .collect();

                    let col_widths = grid::resolve_tracks(
                        template_cols,
                        available_width,
                        col_gap,
                        &content_sizes,
                    );

                    let placements: Vec<Option<&GridPlacement>> = children
                        .iter()
                        .map(|child| child.style.grid_placement.as_ref())
                        .collect();

                    let item_placements = grid::place_items(&placements, num_columns);
                    let num_rows = grid::compute_num_rows(&item_placements);

                    if num_rows == 0 {
                        return 0.0;
                    }

                    let mut row_heights = vec![0.0_f64; num_rows];
                    for placement in &item_placements {
                        let cell_width = grid::span_width(
                            placement.col_start,
                            placement.col_end,
                            &col_widths,
                            col_gap,
                        );
                        let child = &children[placement.child_index];
                        let child_style = child.style.resolve(Some(parent_style), cell_width);
                        let h =
                            self.measure_node_height(child, cell_width, &child_style, font_context);
                        let span = placement.row_end - placement.row_start;
                        let per_row = h / span as f64;
                        for rh in row_heights
                            .iter_mut()
                            .take(placement.row_end.min(num_rows))
                            .skip(placement.row_start)
                        {
                            if per_row > *rh {
                                *rh = per_row;
                            }
                        }
                    }

                    let total_row_gap = row_gap * (num_rows as f64 - 1.0).max(0.0);
                    return row_heights.iter().sum::<f64>() + total_row_gap;
                }
            }
        }

        let direction = parent_style.flex_direction;
        let row_gap = parent_style.row_gap;
        let column_gap = parent_style.column_gap;

        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                // Measure base widths for all children
                // flex_basis takes precedence over width (matching layout_flex_row)
                let styles: Vec<ResolvedStyle> = children
                    .iter()
                    .map(|child| child.style.resolve(Some(parent_style), available_width))
                    .collect();

                let base_widths: Vec<f64> = children
                    .iter()
                    .zip(&styles)
                    .map(|(child, style)| match style.flex_basis {
                        SizeConstraint::Fixed(w) => w,
                        SizeConstraint::Auto => match style.width {
                            SizeConstraint::Fixed(w) => w,
                            SizeConstraint::Auto => self
                                .measure_intrinsic_width(child, style, font_context)
                                .min(available_width),
                        },
                    })
                    .collect();

                let lines = match parent_style.flex_wrap {
                    FlexWrap::NoWrap => {
                        vec![flex::WrapLine {
                            start: 0,
                            end: children.len(),
                        }]
                    }
                    FlexWrap::Wrap | FlexWrap::WrapReverse => {
                        flex::partition_into_lines(&base_widths, column_gap, available_width)
                    }
                };

                // Apply flex grow/shrink to get final widths (matching layout_flex_row)
                let mut final_widths = base_widths.clone();
                for line in &lines {
                    let line_count = line.end - line.start;
                    let line_gap = column_gap * (line_count as f64 - 1.0).max(0.0);
                    let distributable = available_width - line_gap;
                    let total_base: f64 = base_widths[line.start..line.end].iter().sum();
                    let remaining = distributable - total_base;

                    if remaining > 0.0 {
                        let total_grow: f64 = styles[line.start..line.end]
                            .iter()
                            .map(|s| s.flex_grow)
                            .sum();
                        if total_grow > 0.0 {
                            for (j, s) in styles[line.start..line.end].iter().enumerate() {
                                final_widths[line.start + j] = base_widths[line.start + j]
                                    + remaining * (s.flex_grow / total_grow);
                            }
                        }
                    } else if remaining < 0.0 {
                        let total_shrink: f64 = styles[line.start..line.end]
                            .iter()
                            .enumerate()
                            .map(|(j, s)| s.flex_shrink * base_widths[line.start + j])
                            .sum();
                        if total_shrink > 0.0 {
                            for (j, s) in styles[line.start..line.end].iter().enumerate() {
                                let factor =
                                    (s.flex_shrink * base_widths[line.start + j]) / total_shrink;
                                let w = base_widths[line.start + j] + remaining * factor;
                                final_widths[line.start + j] = w.max(s.min_width);
                            }
                        }
                    }
                }

                let mut total = 0.0;
                for (i, line) in lines.iter().enumerate() {
                    let line_height: f64 = children[line.start..line.end]
                        .iter()
                        .enumerate()
                        .map(|(j, child)| {
                            let fw = final_widths[line.start + j];
                            let child_style = child.style.resolve(Some(parent_style), fw);
                            self.measure_node_height(child, fw, &child_style, font_context)
                                + child_style.margin.vertical()
                        })
                        .fold(0.0f64, f64::max);
                    total += line_height;
                    if i > 0 {
                        total += row_gap;
                    }
                }
                total
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                let mut total = 0.0;
                for (i, child) in children.iter().enumerate() {
                    let child_style = child.style.resolve(Some(parent_style), available_width);
                    let child_height = self.measure_node_height(
                        child,
                        available_width,
                        &child_style,
                        font_context,
                    );
                    total += child_height + child_style.margin.vertical();
                    if i > 0 {
                        total += row_gap;
                    }
                }
                total
            }
        }
    }

    /// Measure intrinsic width of a node (used for flex row sizing).
    fn measure_intrinsic_width(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        font_context: &FontContext,
    ) -> f64 {
        match &node.kind {
            NodeKind::Svg { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Text { content, runs, .. } | NodeKind::Heading { content, runs, .. } => {
                // Runs-based text measures per run with each run's own
                // resolved style — `content` is empty (or a shadow copy)
                // when runs are present, so measuring it alone reports a
                // zero/approximate width and flex rows collapse the node
                // to one character per line.
                let text_width = if !runs.is_empty() {
                    runs.iter()
                        .map(|run| {
                            let run_style = run.style.resolve(Some(style), 0.0);
                            let run_content = substitute_page_placeholders(&run.content);
                            let transformed =
                                apply_text_transform(&run_content, run_style.text_transform);
                            let italic = matches!(
                                run_style.font_style,
                                FontStyle::Italic | FontStyle::Oblique
                            );
                            // A hard break ('\n') restarts the line: the
                            // intrinsic width of multi-line text is the
                            // widest line, so measure segments separately.
                            transformed
                                .split('\n')
                                .map(|segment| {
                                    font_context.measure_string(
                                        segment,
                                        &run_style.font_family,
                                        run_style.font_weight,
                                        italic,
                                        run_style.font_size,
                                        run_style.letter_spacing,
                                    )
                                })
                                .fold(0.0f64, f64::max)
                        })
                        .sum()
                } else {
                    let content = substitute_page_placeholders(content);
                    let transformed = apply_text_transform(&content, style.text_transform);
                    let italic = matches!(style.font_style, FontStyle::Italic | FontStyle::Oblique);
                    transformed
                        .split('\n')
                        .map(|segment| {
                            font_context.measure_string(
                                segment,
                                &style.font_family,
                                style.font_weight,
                                italic,
                                style.font_size,
                                style.letter_spacing,
                            )
                        })
                        .fold(0.0f64, f64::max)
                };
                // Add tiny epsilon to prevent exact-boundary line wrapping when
                // this width is later used as max_width for line breaking
                text_width + 0.01 + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Image {
                src, width, height, ..
            } => {
                let w = if let SizeConstraint::Fixed(w) = style.width {
                    w
                } else if let Some(w) = width {
                    *w
                } else if let Some((iw, ih)) = self.get_image_dimensions(src) {
                    let pixel_w = iw as f64;
                    let pixel_h = ih as f64;
                    let aspect = if pixel_w > 0.0 {
                        pixel_h / pixel_w
                    } else {
                        0.75
                    };
                    // Check for height constraint (style or node prop)
                    let constrained_h = match style.height {
                        SizeConstraint::Fixed(h) => Some(h),
                        SizeConstraint::Auto => *height,
                    };
                    if let Some(h) = constrained_h {
                        h / aspect
                    } else {
                        pixel_w
                    }
                } else {
                    100.0
                };
                w + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Barcode { width, .. } => {
                let w = width.unwrap_or(0.0);
                w + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::QrCode { size, .. } => {
                let display_size = size.unwrap_or(0.0);
                display_size + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Canvas { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::BarChart { width, .. }
            | NodeKind::LineChart { width, .. }
            | NodeKind::PieChart { width, .. }
            | NodeKind::AreaChart { width, .. }
            | NodeKind::DotPlot { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::TextField { width, .. } | NodeKind::Dropdown { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Checkbox { width, .. } | NodeKind::RadioButton { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Watermark { .. } => 0.0, // Watermarks take zero width
            _ => {
                // Recursively measure children's intrinsic widths
                if node.children.is_empty() {
                    style.padding.horizontal() + style.margin.horizontal()
                } else {
                    let direction = style.flex_direction;
                    let gap = style.gap;
                    let mut total = 0.0f64;
                    for (i, child) in node.children.iter().enumerate() {
                        let child_style = child.style.resolve(Some(style), 0.0);
                        let child_width =
                            self.measure_intrinsic_width(child, &child_style, font_context);
                        match direction {
                            FlexDirection::Row | FlexDirection::RowReverse => {
                                total += child_width;
                                if i > 0 {
                                    total += gap;
                                }
                            }
                            _ => {
                                total = total.max(child_width);
                            }
                        }
                    }
                    total
                        + style.padding.horizontal()
                        + style.margin.horizontal()
                        + style.border_width.horizontal()
                }
            }
        }
    }

    /// Measure the min-content width of a node — the minimum width needed
    /// to render without breaking unbreakable words. For Text nodes this is
    /// the widest single word; for containers it's the max of children.
    pub fn measure_min_content_width(
        &self,
        node: &Node,
        style: &ResolvedStyle,
        font_context: &FontContext,
    ) -> f64 {
        match &node.kind {
            NodeKind::Text { content, runs, .. } | NodeKind::Heading { content, runs, .. } => {
                let word_width = if !runs.is_empty() {
                    // For styled runs, measure each run's widest word
                    runs.iter()
                        .map(|run| {
                            let run_style = run.style.resolve(Some(style), 0.0);
                            let run_content = substitute_page_placeholders(&run.content);
                            let transformed =
                                apply_text_transform(&run_content, run_style.text_transform);
                            self.text_layout.measure_widest_word(
                                font_context,
                                &transformed,
                                run_style.font_size,
                                &run_style.font_family,
                                run_style.font_weight,
                                run_style.font_style,
                                run_style.letter_spacing,
                                style.hyphens,
                                style.lang.as_deref(),
                            )
                        })
                        .fold(0.0f64, f64::max)
                } else {
                    let content = substitute_page_placeholders(content);
                    let transformed = apply_text_transform(&content, style.text_transform);
                    self.text_layout.measure_widest_word(
                        font_context,
                        &transformed,
                        style.font_size,
                        &style.font_family,
                        style.font_weight,
                        style.font_style,
                        style.letter_spacing,
                        style.hyphens,
                        style.lang.as_deref(),
                    )
                };
                word_width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Image { width, .. } => {
                width.unwrap_or(0.0) + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Svg { width, .. } => {
                *width + style.padding.horizontal() + style.margin.horizontal()
            }
            _ => {
                if node.children.is_empty() {
                    style.padding.horizontal()
                        + style.margin.horizontal()
                        + style.border_width.horizontal()
                } else {
                    let mut max_child_min = 0.0f64;
                    for child in &node.children {
                        let child_style = child.style.resolve(Some(style), 0.0);
                        let child_min =
                            self.measure_min_content_width(child, &child_style, font_context);
                        max_child_min = max_child_min.max(child_min);
                    }
                    max_child_min
                        + style.padding.horizontal()
                        + style.margin.horizontal()
                        + style.border_width.horizontal()
                }
            }
        }
    }

    /// The font size of a cell's first text line. In this engine the baseline
    /// sits exactly `font_size` below the line-box top (there is no font-ascent
    /// metric), so this IS the first-baseline offset from the content-box top.
    /// Walks to the first text-producing descendant; falls back to the cell's
    /// own font size when there is none.
    fn cell_first_line_font_size(&self, cell: &Node, cell_style: &ResolvedStyle, w: f64) -> f64 {
        fn first(node: &Node, parent: &ResolvedStyle, w: f64) -> Option<f64> {
            for ch in &node.children {
                let s = ch.style.resolve(Some(parent), w);
                match &ch.kind {
                    NodeKind::Text { .. } | NodeKind::Heading { .. } => return Some(s.font_size),
                    _ => {
                        if let Some(f) = first(ch, &s, w) {
                            return Some(f);
                        }
                    }
                }
            }
            None
        }
        first(cell, cell_style, w).unwrap_or(cell_style.font_size)
    }

    /// Distance from a cell's border-box top to its first text baseline:
    /// `padding.top + border.top + first-line font_size`.
    fn cell_baseline_distance(
        &self,
        cell: &Node,
        cell_style: &ResolvedStyle,
        inner_width: f64,
    ) -> f64 {
        cell_style.padding.top
            + cell_style.border_width.top
            + self.cell_first_line_font_size(cell, cell_style, inner_width)
    }

    /// The row baseline: the max first-baseline distance across the row's
    /// `vertical-align: baseline` cells. `None` when no cell asks for baseline.
    fn row_baseline(
        &self,
        row: &Node,
        row_style: &ResolvedStyle,
        col_widths: &[f64],
    ) -> Option<f64> {
        let mut b: Option<f64> = None;
        let mut col_idx = 0usize;
        for cell in row.children.iter() {
            let span = match &cell.kind {
                NodeKind::TableCell { col_span, .. } => (*col_span).max(1) as usize,
                _ => 1,
            };
            let col_width: f64 = col_widths.iter().skip(col_idx).take(span).copied().sum();
            col_idx += span;
            let cell_style = cell.style.resolve(Some(row_style), col_width);
            if matches!(cell_style.vertical_align, VerticalAlign::Baseline) {
                let iw = col_width
                    - cell_style.padding.horizontal()
                    - cell_style.border_width.horizontal();
                let d = self.cell_baseline_distance(cell, &cell_style, iw);
                b = Some(b.map_or(d, |m: f64| m.max(d)));
            }
        }
        b
    }

    fn measure_table_row_height(
        &self,
        row: &Node,
        col_widths: &[f64],
        parent_style: &ResolvedStyle,
        font_context: &FontContext,
    ) -> f64 {
        let row_style = row
            .style
            .resolve(Some(parent_style), col_widths.iter().sum());
        let mut max_height: f64 = 0.0;
        // Precompute the row baseline so a baseline-shoved cell can grow the row
        // rather than clip (the risk site).
        let row_bl = self.row_baseline(row, &row_style, col_widths);

        let mut col_idx = 0usize;
        for cell in row.children.iter() {
            let span = match &cell.kind {
                NodeKind::TableCell { col_span, .. } => (*col_span).max(1) as usize,
                _ => 1,
            };
            let col_width: f64 = col_widths.iter().skip(col_idx).take(span).copied().sum();
            col_idx += span;
            let cell_style = cell.style.resolve(Some(&row_style), col_width);
            let inner_width =
                col_width - cell_style.padding.horizontal() - cell_style.border_width.horizontal();

            let mut cell_content_height = 0.0;
            for child in &cell.children {
                let child_style = child.style.resolve(Some(&cell_style), inner_width);
                cell_content_height +=
                    self.measure_node_height(child, inner_width, &child_style, font_context);
            }

            let mut total = cell_content_height
                + cell_style.padding.vertical()
                + cell_style.border_width.vertical();
            // A baseline cell is shoved down by `row_baseline - its own baseline
            // distance`; the row must be tall enough to fit that shove, or the
            // cell content clips.
            if matches!(cell_style.vertical_align, VerticalAlign::Baseline) {
                if let Some(b) = row_bl {
                    let d = self.cell_baseline_distance(cell, &cell_style, inner_width);
                    total += (b - d).max(0.0);
                }
            }
            // CSS 2.1 §17.5.3: `height` on a table cell is a MINIMUM — the cell
            // grows to fit its content but never shrinks below the specified
            // height. This is the slack `vertical-align: middle/bottom` needs to
            // be visible. Auto-height cells are unaffected; content taller than
            // the height still wins. No clipping, and rows stay atomic (an
            // over-tall row overflows whole, it is not sliced).
            if let SizeConstraint::Fixed(h) = cell_style.height {
                total = total.max(h);
            }
            max_height = max_height.max(total);
        }

        max_height.max(row_style.min_height)
    }

    fn resolve_column_widths(
        &self,
        defs: &[ColumnDef],
        available_width: f64,
        children: &[Node],
    ) -> Vec<f64> {
        if defs.is_empty() {
            let num_cols = children.first().map(|row| row.children.len()).unwrap_or(1);
            return vec![available_width / num_cols as f64; num_cols];
        }

        let mut widths = Vec::new();
        let mut remaining = available_width;
        let mut auto_count = 0;

        for def in defs {
            match def.width {
                ColumnWidth::Fixed(w) => {
                    widths.push(w);
                    remaining -= w;
                }
                ColumnWidth::Fraction(f) => {
                    let w = available_width * f;
                    widths.push(w);
                    remaining -= w;
                }
                ColumnWidth::Auto => {
                    widths.push(0.0);
                    auto_count += 1;
                }
            }
        }

        if auto_count > 0 {
            let auto_width = remaining / auto_count as f64;
            for (i, def) in defs.iter().enumerate() {
                if matches!(def.width, ColumnWidth::Auto) {
                    widths[i] = auto_width;
                }
            }
        }

        widths
    }

    fn inject_fixed_elements(&self, pages: &mut [LayoutPage], font_context: &FontContext) {
        for (page_index, page) in pages.iter_mut().enumerate() {
            // Inject watermarks behind all content
            if !page.watermarks.is_empty() {
                let (page_w, page_h) = page.config.size.dimensions();
                let cx = page_w / 2.0;
                let cy = page_h / 2.0;

                let mut watermark_elements = Vec::new();
                for wm_node in &page.watermarks {
                    if let NodeKind::Watermark {
                        text,
                        font_size,
                        angle,
                    } = &wm_node.kind
                    {
                        let style = wm_node.style.resolve(None, page_w);
                        let color = style.color;
                        let opacity = style.opacity;
                        let angle_rad = angle.to_radians();

                        // Build positioned glyphs for the watermark text
                        let italic =
                            matches!(style.font_style, FontStyle::Italic | FontStyle::Oblique);

                        // Try shaping, fall back to per-char measurement
                        let shaped = self.text_layout.shape_text(
                            font_context,
                            text,
                            &style.font_family,
                            style.font_weight,
                            style.font_style,
                        );

                        let mut glyphs = Vec::new();
                        let mut x_pos = 0.0;
                        let text_chars: Vec<char> = text.chars().collect();

                        if let Some(shaped_glyphs) = shaped {
                            // Use shaped glyphs (custom fonts)
                            let units_per_em = font_context.units_per_em(
                                &style.font_family,
                                style.font_weight,
                                italic,
                            ) as f64;

                            for sg in &shaped_glyphs {
                                let advance = sg.x_advance as f64 / units_per_em * *font_size;
                                let cluster_idx = sg.cluster as usize;
                                let ch = text_chars.get(cluster_idx).copied().unwrap_or(' ');
                                glyphs.push(PositionedGlyph {
                                    glyph_id: sg.glyph_id,
                                    char_value: ch,
                                    x_offset: x_pos,
                                    y_offset: 0.0,
                                    x_advance: advance,
                                    font_size: *font_size,
                                    font_family: style.font_family.clone(),
                                    font_weight: style.font_weight,
                                    font_style: style.font_style,
                                    color: Some(color),
                                    href: None,
                                    text_decoration: TextDecoration::None,
                                    letter_spacing: style.letter_spacing,
                                    cluster_text: None,
                                });
                                x_pos += advance + style.letter_spacing;
                            }
                        } else {
                            // Per-char measurement (standard fonts)
                            for &ch in &text_chars {
                                let w = font_context.char_width(
                                    ch,
                                    &style.font_family,
                                    style.font_weight,
                                    italic,
                                    *font_size,
                                );
                                glyphs.push(PositionedGlyph {
                                    glyph_id: ch as u16,
                                    char_value: ch,
                                    x_offset: x_pos,
                                    y_offset: 0.0,
                                    x_advance: w,
                                    font_size: *font_size,
                                    font_family: style.font_family.clone(),
                                    font_weight: style.font_weight,
                                    font_style: style.font_style,
                                    color: Some(color),
                                    href: None,
                                    text_decoration: TextDecoration::None,
                                    letter_spacing: style.letter_spacing,
                                    cluster_text: None,
                                });
                                x_pos += w + style.letter_spacing;
                            }
                        }

                        let text_width = x_pos;

                        let line = TextLine {
                            x: 0.0,
                            y: 0.0,
                            glyphs,
                            width: text_width,
                            height: *font_size,
                            word_spacing: 0.0,
                        };

                        watermark_elements.push(LayoutElement {
                            x: cx,
                            y: cy,
                            width: text_width,
                            height: *font_size,
                            draw: DrawCommand::Watermark {
                                lines: vec![line],
                                color,
                                opacity,
                                angle_rad,
                                font_family: style.font_family.clone(),
                            },
                            children: vec![],
                            node_type: Some("Watermark".to_string()),
                            resolved_style: None,
                            source_location: None,
                            href: None,
                            bookmark: None,
                            alt: None,
                            is_header_row: false,
                            col_span: 1,
                            overflow: Overflow::default(),
                            opacity: 1.0,
                        });
                    }
                }

                // Prepend watermark elements so they render behind all content
                watermark_elements.append(&mut page.elements);
                page.elements = watermark_elements;
                page.watermarks.clear();
            }

            if page.fixed_header.is_empty() && page.fixed_footer.is_empty() {
                continue;
            }

            // Lay out headers at top of content area
            if !page.fixed_header.is_empty() {
                let mut hdr_cursor = PageCursor::new(&page.config);
                for (node, _h) in &page.fixed_header {
                    // The enumerate index is the authoritative page number
                    // for First/NotFirst filtering.
                    if !PageCursor::fixed_page_filter(node).applies(page_index) {
                        continue;
                    }
                    let cw = hdr_cursor.content_width;
                    let cx = hdr_cursor.content_x;
                    let style = node.style.resolve(None, cw);
                    self.layout_view(
                        node,
                        &style,
                        &mut hdr_cursor,
                        &mut Vec::new(),
                        cx,
                        cw,
                        font_context,
                    );
                }
                // Prepend header elements so they draw behind body content
                let mut combined = hdr_cursor.elements;
                combined.append(&mut page.elements);
                page.elements = combined;
            }

            // Lay out footers at bottom of content area.
            // We lay out from y=0 (so there's plenty of room and no spurious
            // page breaks), then shift all resulting elements down to the
            // correct footer position.
            if !page.fixed_footer.is_empty() {
                let mut ftr_cursor = PageCursor::new(&page.config);
                let total_ftr: f64 = page
                    .fixed_footer
                    .iter()
                    .filter(|(n, _)| PageCursor::fixed_page_filter(n).applies(page_index))
                    .map(|(_, h)| *h)
                    .sum();
                let target_y = ftr_cursor.content_height - total_ftr;
                // Layout from y=0
                for (node, _h) in &page.fixed_footer {
                    if !PageCursor::fixed_page_filter(node).applies(page_index) {
                        continue;
                    }
                    let cw = ftr_cursor.content_width;
                    let cx = ftr_cursor.content_x;
                    let style = node.style.resolve(None, cw);
                    self.layout_view(
                        node,
                        &style,
                        &mut ftr_cursor,
                        &mut Vec::new(),
                        cx,
                        cw,
                        font_context,
                    );
                }
                // Shift all footer elements down to the target position.
                // Elements already have content_y baked in, so we just offset
                // by target_y (which is relative to content area top).
                for el in &mut ftr_cursor.elements {
                    offset_element_y(el, target_y);
                }
                page.elements.extend(ftr_cursor.elements);
            }

            // Clean up internal fields
            page.fixed_header.clear();
            page.fixed_footer.clear();
        }
    }

    /// Layout children as a CSS Grid.
    ///
    /// Uses the grid track definitions from the parent style to create a 2D grid,
    /// places children into cells, and lays out each child within its cell bounds.
    #[allow(clippy::too_many_arguments)]
    fn layout_grid_children(
        &self,
        children: &[Node],
        parent_style: &ResolvedStyle,
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
        x: f64,
        available_width: f64,
        font_context: &FontContext,
    ) {
        let template_cols = match &parent_style.grid_template_columns {
            Some(cols) => cols,
            None => return, // No columns defined, nothing to do
        };

        let num_columns = template_cols.len();
        if num_columns == 0 || children.is_empty() {
            return;
        }

        let col_gap = parent_style.column_gap;
        let row_gap = parent_style.row_gap;

        // Resolve column widths
        // For auto tracks, we need content sizes. Use a rough measure.
        let content_sizes: Vec<f64> = template_cols
            .iter()
            .map(|track| {
                if matches!(track, GridTrackSize::Auto) {
                    // Measure the widest child that falls in this column
                    // (approximation: use available_width / num_columns)
                    available_width / num_columns as f64
                } else {
                    0.0
                }
            })
            .collect();

        let col_widths =
            grid::resolve_tracks(template_cols, available_width, col_gap, &content_sizes);

        // Collect grid placements from children's styles
        let placements: Vec<Option<&GridPlacement>> = children
            .iter()
            .map(|child| child.style.grid_placement.as_ref())
            .collect();

        // Place items in the grid
        let item_placements = grid::place_items(&placements, num_columns);
        let num_rows = grid::compute_num_rows(&item_placements);

        if num_rows == 0 {
            return;
        }

        // Measure each item's height at its resolved cell width
        let mut item_heights: Vec<f64> = vec![0.0; children.len()];
        for placement in &item_placements {
            let cell_width =
                grid::span_width(placement.col_start, placement.col_end, &col_widths, col_gap);
            let child = &children[placement.child_index];
            let child_style = child.style.resolve(Some(parent_style), cell_width);
            item_heights[placement.child_index] =
                self.measure_node_height(child, cell_width, &child_style, font_context);
        }

        // Compute row heights: max height of all items in each row
        let template_rows = parent_style.grid_template_rows.as_deref();
        let mut row_heights = vec![0.0_f64; num_rows];
        for placement in &item_placements {
            let h = item_heights[placement.child_index];
            let span = placement.row_end - placement.row_start;
            let per_row = h / span as f64;
            for rh in row_heights
                .iter_mut()
                .take(placement.row_end.min(num_rows))
                .skip(placement.row_start)
            {
                if per_row > *rh {
                    *rh = per_row;
                }
            }
        }

        // Apply template row sizes if provided
        if let Some(template) = template_rows {
            let auto_row = parent_style.grid_auto_rows.as_ref();
            for (r, rh) in row_heights.iter_mut().enumerate() {
                let track = template.get(r).or(auto_row);
                if let Some(track) = track {
                    match track {
                        GridTrackSize::Pt(pts) => *rh = *pts,
                        GridTrackSize::Auto => {} // keep computed
                        _ => {}                   // Fr for rows is complex, skip for now
                    }
                }
            }
        }

        // Layout each row
        for (row, &row_height) in row_heights.iter().enumerate().take(num_rows) {
            // Check page break: treat each row as unbreakable. The whole row
            // moves to the next page so all columns share the same baseline
            // (otherwise each cell's layout_node would page-break individually
            // and scatter the columns across separate pages).
            if row_height > cursor.remaining_height() {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }

            let row_start_y = cursor.y;

            // Layout items in this row
            for placement in &item_placements {
                if placement.row_start != row {
                    continue; // Only process items starting in this row
                }

                let cell_x = x + grid::column_x_offset(placement.col_start, &col_widths, col_gap);
                let cell_width =
                    grid::span_width(placement.col_start, placement.col_end, &col_widths, col_gap);

                let child = &children[placement.child_index];

                self.layout_node(
                    child,
                    cursor,
                    pages,
                    cell_x,
                    cell_width,
                    Some(parent_style),
                    font_context,
                    None,
                    None,
                );
                // Restore y to row baseline (items don't affect each other's y)
                cursor.y = row_start_y;
            }

            cursor.y = row_start_y + row_height + row_gap;
        }

        // Remove trailing gap
        if num_rows > 0 {
            cursor.y -= row_gap;
        }
    }
}

struct FlexItem<'a> {
    node: &'a Node,
    style: ResolvedStyle,
    base_width: f64,
    min_content_width: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontContext;

    fn make_text(content: &str, font_size: f64) -> Node {
        Node {
            kind: NodeKind::Text {
                content: content.to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_size: Some(font_size),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    fn make_styled_view(style: Style, children: Vec<Node>) -> Node {
        Node {
            kind: NodeKind::View,
            style,
            children,
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    fn make_runs_text(runs: Vec<crate::model::TextRun>) -> Node {
        Node {
            kind: NodeKind::Text {
                content: String::new(),
                href: None,
                runs,
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    #[test]
    fn intrinsic_width_measures_runs_not_just_content() {
        // Found by the HTML input path: a runs-based Text node (empty
        // `content`) used to measure ~0 intrinsic width, so flex rows
        // collapsed it to one character per line.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let runs_node = make_runs_text(vec![
            crate::model::TextRun {
                content: "Hello ".to_string(),
                style: Style::default(),
                href: None,
            },
            crate::model::TextRun {
                content: "World".to_string(),
                style: Style {
                    font_weight: Some(700),
                    ..Default::default()
                },
                href: None,
            },
        ]);
        let plain_node = make_text("Hello World", 12.0);

        let runs_style = runs_node.style.resolve(None, 0.0);
        let plain_style = plain_node.style.resolve(None, 0.0);
        let runs_w = engine.measure_intrinsic_width(&runs_node, &runs_style, &font_context);
        let plain_w = engine.measure_intrinsic_width(&plain_node, &plain_style, &font_context);

        // Must be in the same ballpark as the plain-content equivalent
        // (slightly wider: the second run is bold).
        assert!(
            runs_w >= plain_w,
            "runs width ({runs_w}) must not undershoot plain width ({plain_w})"
        );
        assert!(
            runs_w < plain_w * 1.5,
            "runs width ({runs_w}) should be close to plain width ({plain_w})"
        );
    }

    #[test]
    fn intrinsic_width_of_multiline_text_is_widest_line() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let multiline = make_text("123 Main St\nSpringfield, IL 62704", 12.0);
        let widest = make_text("Springfield, IL 62704", 12.0);

        let m_style = multiline.style.resolve(None, 0.0);
        let w_style = widest.style.resolve(None, 0.0);
        let m_w = engine.measure_intrinsic_width(&multiline, &m_style, &font_context);
        let w_w = engine.measure_intrinsic_width(&widest, &w_style, &font_context);

        assert!(
            (m_w - w_w).abs() < 0.01,
            "multiline intrinsic width ({m_w}) must equal its widest line ({w_w})"
        );
    }

    #[test]
    fn intrinsic_width_of_heading_measures_its_text() {
        // Heading used to fall through to the children-recursion arm and
        // measure zero (headings are leaves).
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let heading = Node {
            kind: NodeKind::Heading {
                level: 1,
                content: "Invoice #2024-001".to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_size: Some(24.0),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        };
        let style = heading.style.resolve(None, 0.0);
        let w = engine.measure_intrinsic_width(&heading, &style, &font_context);
        assert!(w > 100.0, "24pt heading text must measure wide, got {w}");
    }

    #[test]
    fn measure_node_height_of_wrapping_heading_matches_text() {
        // A heading that wraps to multiple lines must contribute its full
        // height to a parent's auto-height, exactly like Text. Previously
        // Heading had no arm in `measure_node_height` and fell through to the
        // container `_` arm (children-recursion), measuring ~0 — so an
        // auto-height View wrapping a multi-line heading collapsed, shifting
        // every sibling below it.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let content = "Annual Performance Review";
        let heading = Node {
            kind: NodeKind::Heading {
                level: 1,
                content: content.to_string(),
                href: None,
                runs: vec![],
            },
            style: Style {
                font_size: Some(32.0),
                ..Default::default()
            },
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        };
        let text = make_text(content, 32.0);

        // A width narrow enough to force the 32pt title onto more than one line.
        let width = 200.0;
        let h_style = heading.style.resolve(None, width);
        let t_style = text.style.resolve(None, width);
        let h_height = engine.measure_node_height(&heading, width, &h_style, &font_context);
        let t_height = engine.measure_node_height(&text, width, &t_style, &font_context);

        assert!(
            h_height > 32.0,
            "a wrapping 32pt heading must measure more than one line, got {h_height}"
        );
        assert!(
            (h_height - t_height).abs() < 0.01,
            "heading height ({h_height}) must equal the same text's height ({t_height})"
        );
    }

    #[test]
    fn intrinsic_width_flex_row_sums_children() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let child1 = make_text("Hello", 14.0);
        let child2 = make_text("World", 14.0);

        let child1_style = child1.style.resolve(None, 0.0);
        let child2_style = child2.style.resolve(None, 0.0);
        let child1_w = engine.measure_intrinsic_width(&child1, &child1_style, &font_context);
        let child2_w = engine.measure_intrinsic_width(&child2, &child2_style, &font_context);

        let row = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Row),
                ..Default::default()
            },
            vec![make_text("Hello", 14.0), make_text("World", 14.0)],
        );
        let row_style = row.style.resolve(None, 0.0);
        let row_w = engine.measure_intrinsic_width(&row, &row_style, &font_context);

        assert!(
            (row_w - (child1_w + child2_w)).abs() < 0.01,
            "Row intrinsic width ({}) should equal sum of children ({} + {})",
            row_w,
            child1_w,
            child2_w
        );
    }

    #[test]
    fn intrinsic_width_flex_column_takes_max() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let short = make_text("Hi", 14.0);
        let long = make_text("Hello World", 14.0);

        let short_style = short.style.resolve(None, 0.0);
        let long_style = long.style.resolve(None, 0.0);
        let short_w = engine.measure_intrinsic_width(&short, &short_style, &font_context);
        let long_w = engine.measure_intrinsic_width(&long, &long_style, &font_context);

        let col = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                ..Default::default()
            },
            vec![make_text("Hi", 14.0), make_text("Hello World", 14.0)],
        );
        let col_style = col.style.resolve(None, 0.0);
        let col_w = engine.measure_intrinsic_width(&col, &col_style, &font_context);

        assert!(
            (col_w - long_w).abs() < 0.01,
            "Column intrinsic width ({}) should equal max child ({}, short was {})",
            col_w,
            long_w,
            short_w
        );
    }

    #[test]
    fn intrinsic_width_nested_containers() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let inner = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Row),
                ..Default::default()
            },
            vec![make_text("A", 12.0), make_text("B", 12.0)],
        );
        let inner_style = inner.style.resolve(None, 0.0);
        let inner_w = engine.measure_intrinsic_width(&inner, &inner_style, &font_context);

        let outer = make_styled_view(
            Style::default(),
            vec![make_styled_view(
                Style {
                    flex_direction: Some(FlexDirection::Row),
                    ..Default::default()
                },
                vec![make_text("A", 12.0), make_text("B", 12.0)],
            )],
        );
        let outer_style = outer.style.resolve(None, 0.0);
        let outer_w = engine.measure_intrinsic_width(&outer, &outer_style, &font_context);

        assert!(
            (outer_w - inner_w).abs() < 0.01,
            "Nested container ({}) should match inner container ({})",
            outer_w,
            inner_w
        );
    }

    #[test]
    fn intrinsic_width_row_with_gap() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let no_gap = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Row),
                ..Default::default()
            },
            vec![make_text("A", 12.0), make_text("B", 12.0)],
        );
        let with_gap = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Row),
                gap: Some(10.0),
                ..Default::default()
            },
            vec![make_text("A", 12.0), make_text("B", 12.0)],
        );

        let no_gap_style = no_gap.style.resolve(None, 0.0);
        let with_gap_style = with_gap.style.resolve(None, 0.0);
        let no_gap_w = engine.measure_intrinsic_width(&no_gap, &no_gap_style, &font_context);
        let with_gap_w = engine.measure_intrinsic_width(&with_gap, &with_gap_style, &font_context);

        assert!(
            (with_gap_w - no_gap_w - 10.0).abs() < 0.01,
            "Gap should add 10pt: with_gap={}, no_gap={}",
            with_gap_w,
            no_gap_w
        );
    }

    #[test]
    fn intrinsic_width_empty_container() {
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let padding = 8.0;
        let empty = make_styled_view(
            Style {
                padding: Some(Edges::uniform(padding)),
                ..Default::default()
            },
            vec![],
        );
        let style = empty.style.resolve(None, 0.0);
        let w = engine.measure_intrinsic_width(&empty, &style, &font_context);

        assert!(
            (w - padding * 2.0).abs() < 0.01,
            "Empty container width ({}) should equal horizontal padding ({})",
            w,
            padding * 2.0
        );
    }

    // ── Fix 1: min-content width prevents text wrapping in flex shrink ──

    #[test]
    fn flex_shrink_respects_min_content_width() {
        // A flex row with a short-text child ("SALE") and a large sibling.
        // The shrink algorithm should not compress the short-text child below
        // the width of the word "SALE".
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let sale_text = make_text("SALE", 12.0);
        let sale_style = sale_text.style.resolve(None, 0.0);
        let sale_word_width =
            engine.measure_min_content_width(&sale_text, &sale_style, &font_context);
        assert!(
            sale_word_width > 0.0,
            "SALE should have non-zero min-content width"
        );

        // Row with 100pt available; child1 wants 80pt, child2 (SALE) wants 60pt.
        // Total = 140pt, overflow = 40pt. Without floor, SALE would shrink below word width.
        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Row),
                width: Some(Dimension::Pt(100.0)),
                ..Default::default()
            },
            vec![
                make_styled_view(
                    Style {
                        width: Some(Dimension::Pt(80.0)),
                        flex_shrink: Some(1.0),
                        ..Default::default()
                    },
                    vec![],
                ),
                make_styled_view(
                    Style {
                        width: Some(Dimension::Pt(60.0)),
                        flex_shrink: Some(1.0),
                        ..Default::default()
                    },
                    vec![make_text("SALE", 12.0)],
                ),
            ],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        assert!(!pages.is_empty());

        // The SALE child (second flex item) should not be narrower than its min-content width
        // Walk the layout tree: Page -> View (container) -> second child
        let page = &pages[0];
        // Find the container (the View with children)
        let container_el = page.elements.iter().find(|e| e.children.len() == 2);
        assert!(
            container_el.is_some(),
            "Should find container with 2 children"
        );
        let sale_child = &container_el.unwrap().children[1];
        assert!(
            sale_child.width >= sale_word_width - 0.01,
            "SALE child width ({}) should be >= min-content width ({})",
            sale_child.width,
            sale_word_width
        );
    }

    // ── Fix 2: column justify-content and align-items ──

    #[test]
    fn column_justify_content_center() {
        // A column container with fixed height 200pt and a single child of ~20pt.
        // With justify-content: center, the child should be roughly centered vertically.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(200.0)),
                justify_content: Some(JustifyContent::Center),
                ..Default::default()
            },
            vec![make_text("Centered", 12.0)],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        // The container should have one child, and that child should be
        // offset roughly to the vertical center
        let container_el = page.elements.iter().find(|e| !e.children.is_empty());
        assert!(
            container_el.is_some(),
            "Should find container with children"
        );
        let container_el = container_el.unwrap();
        let child = &container_el.children[0];

        // Child y should be container.y + roughly (200 - child_height) / 2
        let child_offset = child.y - container_el.y;
        let expected_offset = (200.0 - child.height) / 2.0;
        assert!(
            (child_offset - expected_offset).abs() < 2.0,
            "Child offset ({}) should be near center ({})",
            child_offset,
            expected_offset
        );
    }

    #[test]
    fn column_align_items_center() {
        // A column container with a narrow text child.
        // With align-items: center, the child should be horizontally centered.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                width: Some(Dimension::Pt(300.0)),
                align_items: Some(AlignItems::Center),
                ..Default::default()
            },
            vec![make_text("Hi", 12.0)],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page.elements.iter().find(|e| !e.children.is_empty());
        assert!(container_el.is_some());
        let container_el = container_el.unwrap();
        let child = &container_el.children[0];

        // Child should be centered within the 300pt container
        let child_center = child.x + child.width / 2.0;
        let container_center = container_el.x + container_el.width / 2.0;
        assert!(
            (child_center - container_center).abs() < 2.0,
            "Child center ({}) should be near container center ({})",
            child_center,
            container_center
        );
    }

    // ── Fix 3: absolute positioning relative to parent ──

    #[test]
    fn absolute_child_positioned_relative_to_parent() {
        // A POSITIONED parent (position: relative) with an absolute child using
        // top: 10, left: 10. The child resolves against the parent — now the
        // correct CSS behavior, since the parent is a positioned ancestor.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let parent = make_styled_view(
            Style {
                position: Some(crate::model::Position::Relative),
                margin: Some(MarginEdges::from_edges(Edges {
                    top: 50.0,
                    left: 50.0,
                    ..Default::default()
                })),
                width: Some(Dimension::Pt(200.0)),
                height: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_styled_view(
                Style {
                    position: Some(crate::model::Position::Absolute),
                    top: Some(10.0),
                    left: Some(10.0),
                    width: Some(Dimension::Pt(50.0)),
                    height: Some(Dimension::Pt(50.0)),
                    ..Default::default()
                },
                vec![],
            )],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![parent],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        // Find the parent container (has the absolute child inside it or as sibling)
        // Absolute children are added to cursor.elements, so they'll be inside the parent
        let parent_el = page
            .elements
            .iter()
            .find(|e| e.width > 190.0 && e.width < 210.0);
        assert!(parent_el.is_some(), "Should find the 200x200 parent");
        let parent_el = parent_el.unwrap();

        // The absolute child should be at parent.x + 10, parent.y + 10
        let abs_child = parent_el
            .children
            .iter()
            .find(|e| e.width > 45.0 && e.width < 55.0);
        assert!(abs_child.is_some(), "Should find 50x50 absolute child");
        let abs_child = abs_child.unwrap();

        let expected_x = parent_el.x + 10.0;
        let expected_y = parent_el.y + 10.0;
        assert!(
            (abs_child.x - expected_x).abs() < 1.0,
            "Absolute child x ({}) should be parent.x + 10 ({})",
            abs_child.x,
            expected_x
        );
        assert!(
            (abs_child.y - expected_y).abs() < 1.0,
            "Absolute child y ({}) should be parent.y + 10 ({})",
            abs_child.y,
            expected_y
        );
    }

    #[test]
    fn absolute_escapes_unpositioned_parent_to_page() {
        // Same shape, but the parent is UNpositioned. Under browser semantics
        // the absolute child resolves against the nearest positioned ancestor —
        // here none exists, so the page content box, NOT the parent. This is
        // the retired v0 divergence.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();
        let parent = make_styled_view(
            Style {
                margin: Some(MarginEdges::from_edges(Edges {
                    top: 50.0,
                    left: 50.0,
                    ..Default::default()
                })),
                width: Some(Dimension::Pt(200.0)),
                height: Some(Dimension::Pt(200.0)),
                ..Default::default()
            },
            vec![make_styled_view(
                Style {
                    position: Some(crate::model::Position::Absolute),
                    top: Some(10.0),
                    left: Some(10.0),
                    width: Some(Dimension::Pt(50.0)),
                    height: Some(Dimension::Pt(50.0)),
                    ..Default::default()
                },
                vec![],
            )],
        );
        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![parent],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };
        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];
        let parent_el = page
            .elements
            .iter()
            .find(|e| e.width > 190.0 && e.width < 210.0)
            .expect("parent");
        let abs_child = parent_el
            .children
            .iter()
            .find(|e| e.width > 45.0 && e.width < 55.0)
            .expect("abs child");
        let page_left = PageConfig::default().margin.left;
        let page_top = PageConfig::default().margin.top;
        assert!(
            (abs_child.x - (page_left + 10.0)).abs() < 1.0,
            "absolute escapes to the page: x {} should be page_left + 10 ({})",
            abs_child.x,
            page_left + 10.0
        );
        assert!(
            (abs_child.y - (page_top + 10.0)).abs() < 1.0,
            "absolute escapes to the page: y {} should be page_top + 10 ({})",
            abs_child.y,
            page_top + 10.0
        );
        assert!(
            abs_child.x < parent_el.x,
            "child must no longer be parent-relative (parent is 50pt further in)"
        );
    }

    #[test]
    fn text_transform_none_passthrough() {
        assert_eq!(
            apply_text_transform("Hello World", TextTransform::None),
            "Hello World"
        );
    }

    #[test]
    fn text_transform_uppercase() {
        assert_eq!(
            apply_text_transform("hello world", TextTransform::Uppercase),
            "HELLO WORLD"
        );
    }

    #[test]
    fn text_transform_lowercase() {
        assert_eq!(
            apply_text_transform("HELLO WORLD", TextTransform::Lowercase),
            "hello world"
        );
    }

    #[test]
    fn text_transform_capitalize() {
        assert_eq!(
            apply_text_transform("hello world", TextTransform::Capitalize),
            "Hello World"
        );
        assert_eq!(
            apply_text_transform("  hello  world  ", TextTransform::Capitalize),
            "  Hello  World  "
        );
        assert_eq!(
            apply_text_transform("already Capitalized", TextTransform::Capitalize),
            "Already Capitalized"
        );
    }

    #[test]
    fn text_transform_capitalize_empty() {
        assert_eq!(apply_text_transform("", TextTransform::Capitalize), "");
    }

    #[test]
    fn apply_char_transform_uppercase() {
        assert_eq!(
            apply_char_transform('a', TextTransform::Uppercase, false),
            'A'
        );
        assert_eq!(
            apply_char_transform('A', TextTransform::Uppercase, false),
            'A'
        );
    }

    #[test]
    fn apply_char_transform_capitalize_word_start() {
        assert_eq!(
            apply_char_transform('h', TextTransform::Capitalize, true),
            'H'
        );
        assert_eq!(
            apply_char_transform('h', TextTransform::Capitalize, false),
            'h'
        );
    }

    // ── flex-grow in column direction ──

    #[test]
    fn column_flex_grow_single_child_fills_container() {
        // A column container with fixed height 300pt and a single child with flex_grow: 1.
        // The child should expand to fill the entire 300pt.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                ..Default::default()
            },
            vec![make_text("Short", 12.0)],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page.elements.iter().find(|e| !e.children.is_empty());
        assert!(container_el.is_some());
        let container_el = container_el.unwrap();
        assert!(
            (container_el.height - 300.0).abs() < 1.0,
            "Container should be 300pt, got {}",
            container_el.height
        );

        let child_el = &container_el.children[0];
        assert!(
            (child_el.height - 300.0).abs() < 1.0,
            "flex-grow child should expand to 300pt, got {}",
            child_el.height
        );
    }

    #[test]
    fn column_flex_grow_two_children_proportional() {
        // Two children: one with flex_grow: 1, one with flex_grow: 2.
        // They should share remaining space proportionally (1:2).
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let child1 = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                ..Default::default()
            },
            vec![make_text("A", 12.0)],
        );
        let child2 = make_styled_view(
            Style {
                flex_grow: Some(2.0),
                ..Default::default()
            },
            vec![make_text("B", 12.0)],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![child1, child2],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.children.len() == 2)
            .expect("Should find container with two children");

        let c1 = &container_el.children[0];
        let c2 = &container_el.children[1];

        // Both children have the same natural height (one line of text).
        // The slack is split 1:2 between them.
        // So child2 should be roughly twice as much taller than child1's growth.
        let total = c1.height + c2.height;
        assert!(
            (total - 300.0).abs() < 2.0,
            "Children should sum to ~300pt, got {}",
            total
        );

        // child2.height should be roughly 2x child1.height
        // (not exact because natural heights are equal, but growth is 1:2)
        let ratio = c2.height / c1.height;
        assert!(
            ratio > 1.3 && ratio < 2.5,
            "child2/child1 ratio should be between 1.3 and 2.5, got {}",
            ratio
        );
    }

    #[test]
    fn column_flex_grow_mixed_grow_and_fixed() {
        // One fixed child (no flex_grow) and one flex_grow child.
        // The flex_grow child takes all remaining space.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let fixed_child = make_styled_view(
            Style {
                height: Some(Dimension::Pt(50.0)),
                ..Default::default()
            },
            vec![make_text("Fixed", 12.0)],
        );
        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                ..Default::default()
            },
            vec![make_text("Grow", 12.0)],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![fixed_child, grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.children.len() == 2)
            .expect("Should find container with two children");

        let fixed_el = &container_el.children[0];
        let grow_el = &container_el.children[1];

        // Fixed child stays at 50pt
        assert!(
            (fixed_el.height - 50.0).abs() < 1.0,
            "Fixed child should stay at 50pt, got {}",
            fixed_el.height
        );

        // Grow child takes remaining ~250pt
        assert!(
            (grow_el.height - 250.0).abs() < 2.0,
            "Grow child should expand to ~250pt, got {}",
            grow_el.height
        );
    }

    #[test]
    fn column_flex_grow_page_level() {
        // flex_grow: 1 on a direct Page child should fill the page content area.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                ..Default::default()
            },
            vec![make_text("Fill page", 12.0)],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![grow_child],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        // The child should fill the page content height
        assert!(
            !page.elements.is_empty(),
            "Page should have at least one element"
        );

        let content_height = page.height - page.config.margin.top - page.config.margin.bottom;
        let el = &page.elements[0];
        assert!(
            (el.height - content_height).abs() < 2.0,
            "Page-level flex-grow child should fill content height ({}), got {}",
            content_height,
            el.height
        );
    }

    #[test]
    fn column_flex_grow_with_justify_content() {
        // flex-grow and justify-content: center should work together.
        // A fixed child + a grow child + justify-content: center.
        // After grow fills the space, there's no slack left for justify, so positions stay as-is.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let fixed_child = make_styled_view(
            Style {
                height: Some(Dimension::Pt(50.0)),
                ..Default::default()
            },
            vec![make_text("Top", 12.0)],
        );
        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                ..Default::default()
            },
            vec![make_text("Fill", 12.0)],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                justify_content: Some(JustifyContent::Center),
                ..Default::default()
            },
            vec![fixed_child, grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.children.len() == 2)
            .expect("Should find container");

        // After flex-grow absorbs all slack, justify-content has nothing to distribute.
        // First child should be at the top of the container.
        let first_child = &container_el.children[0];
        assert!(
            (first_child.y - container_el.y).abs() < 1.0,
            "First child should be at top of container"
        );

        // Children should still sum to container height
        let total = container_el.children[0].height + container_el.children[1].height;
        assert!(
            (total - 300.0).abs() < 2.0,
            "Children should fill container, got {}",
            total
        );
    }

    #[test]
    fn column_flex_grow_child_justify_content_center() {
        // A flex-grow child with justify-content: center should vertically center its content.
        // This is the cover-page bug: the inner View grows via flex but its children stay at top.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        // Inner content: a small fixed-height box
        let inner_box = make_styled_view(
            Style {
                height: Some(Dimension::Pt(40.0)),
                ..Default::default()
            },
            vec![make_text("Centered", 12.0)],
        );

        // The grow child: flex: 1, justify-content: center
        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                flex_direction: Some(FlexDirection::Column),
                justify_content: Some(JustifyContent::Center),
                ..Default::default()
            },
            vec![inner_box],
        );

        // Outer column container with fixed height
        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(400.0)),
                ..Default::default()
            },
            vec![grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        // Find the container (has 1 child = the grow child)
        let container_el = page
            .elements
            .iter()
            .find(|e| e.height > 350.0 && e.children.len() == 1)
            .expect("Should find outer container");

        let grow_el = &container_el.children[0];
        assert!(
            (grow_el.height - 400.0).abs() < 2.0,
            "Grow child should expand to 400, got {}",
            grow_el.height
        );

        // The inner box should be vertically centered within the grow child
        let inner_el = &grow_el.children[0];
        let expected_center = grow_el.y + grow_el.height / 2.0;
        let actual_center = inner_el.y + inner_el.height / 2.0;
        assert!(
            (actual_center - expected_center).abs() < 2.0,
            "Inner box should be vertically centered. Expected center ~{}, got ~{}",
            expected_center,
            actual_center
        );
    }

    #[test]
    fn column_flex_grow_child_justify_content_flex_end() {
        // A flex-grow child with justify-content: flex-end should push content to the bottom.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let inner_box = make_styled_view(
            Style {
                height: Some(Dimension::Pt(30.0)),
                ..Default::default()
            },
            vec![make_text("Bottom", 12.0)],
        );

        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                flex_direction: Some(FlexDirection::Column),
                justify_content: Some(JustifyContent::FlexEnd),
                ..Default::default()
            },
            vec![inner_box],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.height > 250.0 && e.children.len() == 1)
            .expect("Should find outer container");

        let grow_el = &container_el.children[0];
        let inner_el = &grow_el.children[0];

        // Inner box should be near the bottom of the grow child
        let inner_bottom = inner_el.y + inner_el.height;
        let grow_bottom = grow_el.y + grow_el.height;
        assert!(
            (inner_bottom - grow_bottom).abs() < 2.0,
            "Inner box bottom ({}) should align with grow child bottom ({})",
            inner_bottom,
            grow_bottom
        );
    }

    #[test]
    fn column_flex_grow_child_no_justify_unchanged() {
        // Regression: flex-grow with default FlexStart should keep content at top.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let inner_box = make_styled_view(
            Style {
                height: Some(Dimension::Pt(50.0)),
                ..Default::default()
            },
            vec![make_text("Top", 12.0)],
        );

        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                flex_direction: Some(FlexDirection::Column),
                // No justify-content set — defaults to FlexStart
                ..Default::default()
            },
            vec![inner_box],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.height > 250.0 && e.children.len() == 1)
            .expect("Should find outer container");

        let grow_el = &container_el.children[0];
        let inner_el = &grow_el.children[0];

        // Inner box should stay at the top of the grow child
        assert!(
            (inner_el.y - grow_el.y).abs() < 2.0,
            "Inner box ({}) should be at top of grow child ({})",
            inner_el.y,
            grow_el.y
        );
    }

    #[test]
    fn column_flex_grow_child_align_items_center() {
        // A flex-grown View with align_items: Center should horizontally center its Text child.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        let text = make_text("Hello", 12.0);

        let grow_child = make_styled_view(
            Style {
                flex_grow: Some(1.0),
                flex_direction: Some(FlexDirection::Column),
                align_items: Some(AlignItems::Center),
                ..Default::default()
            },
            vec![text],
        );

        let container = make_styled_view(
            Style {
                flex_direction: Some(FlexDirection::Column),
                height: Some(Dimension::Pt(300.0)),
                ..Default::default()
            },
            vec![grow_child],
        );

        let doc = Document {
            children: vec![Node::page(
                PageConfig::default(),
                Style::default(),
                vec![container],
            )],
            metadata: Default::default(),
            default_page: PageConfig::default(),
            first_page: None,
            left_page: None,
            right_page: None,
            fonts: vec![],
            tagged: false,
            pdfa: None,
            default_style: None,
            embedded_data: None,
            flatten_forms: false,
            pdf_ua: false,
            certification: None,
        };

        let pages = engine.layout(&doc, &font_context);
        let page = &pages[0];

        let container_el = page
            .elements
            .iter()
            .find(|e| e.height > 250.0 && e.children.len() == 1)
            .expect("Should find outer container");

        let grow_el = &container_el.children[0];
        assert!(
            !grow_el.children.is_empty(),
            "Grow child should have text child"
        );

        let text_el = &grow_el.children[0];
        let text_center = text_el.x + text_el.width / 2.0;
        let grow_center = grow_el.x + grow_el.width / 2.0;
        assert!(
            (text_center - grow_center).abs() < 2.0,
            "Text center ({}) should be near grow child center ({})",
            text_center,
            grow_center
        );
    }

    #[test]
    fn image_intrinsic_width_respects_height_constraint() {
        // An Image with only a height prop should compute intrinsic width from
        // aspect ratio, not return the raw pixel width. This ensures align-items:
        // center can correctly center images.
        let engine = LayoutEngine::new();
        let font_context = FontContext::new();

        // Use a 1x1 PNG data URI (known dimensions: 1x1 pixels)
        let one_px_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

        let image_node = Node {
            kind: NodeKind::Image {
                src: one_px_png.to_string(),
                width: None,
                height: Some(36.0),
            },
            style: Style::default(),
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        };

        let resolved = image_node.style.resolve(None, 0.0);
        let intrinsic = engine.measure_intrinsic_width(&image_node, &resolved, &font_context);

        // 1x1 pixel image with height: 36 should give width = 36 / (1/1) = 36
        assert!(
            (intrinsic - 36.0).abs() < 1.0,
            "Intrinsic width should be ~36 for 1:1 aspect image with height 36, got {}",
            intrinsic
        );
    }
}
