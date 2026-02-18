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
pub mod page_break;

use serde::Serialize;

use crate::font::FontContext;
use crate::model::*;
use crate::style::*;
use crate::text::{StyledChar, TextLayout};

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

/// Serializable subset of ResolvedStyle for the inspector panel.
/// Excludes SizeConstraint (which doesn't derive Serialize).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyleInfo {
    pub margin: Edges,
    pub padding: Edges,
    pub border_width: Edges,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_wrap: FlexWrap,
    pub gap: f64,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: u32,
    pub font_style: FontStyle,
    pub line_height: f64,
    pub text_align: TextAlign,
    pub color: Color,
    pub background_color: Option<Color>,
    pub border_color: EdgeValues<Color>,
    pub border_radius: CornerValues,
    pub opacity: f64,
}

impl ElementStyleInfo {
    fn from_resolved(style: &ResolvedStyle) -> Self {
        ElementStyleInfo {
            margin: style.margin,
            padding: style.padding,
            border_width: style.border_width,
            flex_direction: style.flex_direction,
            justify_content: style.justify_content,
            align_items: style.align_items,
            flex_wrap: style.flex_wrap,
            gap: style.gap,
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            font_weight: style.font_weight,
            font_style: style.font_style,
            line_height: style.line_height,
            text_align: style.text_align,
            color: style.color,
            background_color: style.background_color,
            border_color: style.border_color,
            border_radius: style.border_radius,
            opacity: style.opacity,
        }
    }
}

impl Default for ElementStyleInfo {
    fn default() -> Self {
        ElementStyleInfo {
            margin: Edges::default(),
            padding: Edges::default(),
            border_width: Edges::default(),
            flex_direction: FlexDirection::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            flex_wrap: FlexWrap::default(),
            gap: 0.0,
            font_family: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: 400,
            font_style: FontStyle::default(),
            line_height: 1.4,
            text_align: TextAlign::default(),
            color: Color::BLACK,
            background_color: None,
            border_color: EdgeValues::uniform(Color::BLACK),
            border_radius: CornerValues::uniform(0.0),
            opacity: 1.0,
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
                };
                let text_content = match &elem.draw {
                    DrawCommand::Text { lines, .. } => {
                        let text: String = lines
                            .iter()
                            .flat_map(|line| line.glyphs.iter().map(|g| g.char_value))
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
}

/// Return a human-readable name for a NodeKind variant.
fn node_kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::View => "View",
        NodeKind::Text { .. } => "Text",
        NodeKind::Image { .. } => "Image",
        NodeKind::Table { .. } => "Table",
        NodeKind::TableRow { .. } => "TableRow",
        NodeKind::TableCell { .. } => "TableCell",
        NodeKind::Fixed {
            position: FixedPosition::Header,
        } => "FixedHeader",
        NodeKind::Fixed {
            position: FixedPosition::Footer,
        } => "FixedFooter",
        NodeKind::Page { .. } => "Page",
        NodeKind::PageBreak => "PageBreak",
        NodeKind::Svg { .. } => "Svg",
    }
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
        border_radius: CornerValues,
    },
    /// Draw text.
    Text {
        lines: Vec<TextLine>,
        color: Color,
        text_decoration: TextDecoration,
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
        width: f64,
        height: f64,
    },
}

#[derive(Debug, Clone)]
pub struct TextLine {
    pub x: f64,
    pub y: f64,
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    pub glyph_id: u16,
    pub x_offset: f64,
    pub font_size: f64,
    pub font_family: String,
    pub font_weight: u32,
    pub font_style: FontStyle,
    pub char_value: char,
    /// Per-glyph color (for text runs with different colors).
    pub color: Option<Color>,
    /// Per-glyph href (for inline links within runs).
    pub href: Option<String>,
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

/// The main layout engine.
pub struct LayoutEngine {
    text_layout: TextLayout,
}

/// Tracks where we are on the current page during layout.
#[derive(Debug, Clone)]
struct PageCursor {
    config: PageConfig,
    content_width: f64,
    content_height: f64,
    y: f64,
    elements: Vec<LayoutElement>,
    fixed_header: Vec<(Node, f64)>,
    fixed_footer: Vec<(Node, f64)>,
    content_x: f64,
    content_y: f64,
}

impl PageCursor {
    fn new(config: &PageConfig) -> Self {
        let (page_w, page_h) = config.size.dimensions();
        let content_width = page_w - config.margin.horizontal();
        let content_height = page_h - config.margin.vertical();

        Self {
            config: config.clone(),
            content_width,
            content_height,
            y: 0.0,
            elements: Vec::new(),
            fixed_header: Vec::new(),
            fixed_footer: Vec::new(),
            content_x: config.margin.left,
            content_y: config.margin.top,
        }
    }

    fn remaining_height(&self) -> f64 {
        let footer_height: f64 = self.fixed_footer.iter().map(|(_, h)| *h).sum();
        (self.content_height - self.y - footer_height).max(0.0)
    }

    fn finalize(&self) -> LayoutPage {
        let (page_w, page_h) = self.config.size.dimensions();
        LayoutPage {
            width: page_w,
            height: page_h,
            elements: self.elements.clone(),
            fixed_header: self.fixed_header.clone(),
            fixed_footer: self.fixed_footer.clone(),
            config: self.config.clone(),
        }
    }

    fn new_page(&self) -> Self {
        let mut cursor = PageCursor::new(&self.config);
        cursor.fixed_header = self.fixed_header.clone();
        cursor.fixed_footer = self.fixed_footer.clone();

        let header_height: f64 = cursor.fixed_header.iter().map(|(_, h)| *h).sum();
        cursor.y = header_height;

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
        }
    }

    /// Main entry point: lay out a document into pages.
    pub fn layout(&self, document: &Document, font_context: &FontContext) -> Vec<LayoutPage> {
        let mut pages: Vec<LayoutPage> = Vec::new();
        let mut cursor = PageCursor::new(&document.default_page);

        for node in &document.children {
            match &node.kind {
                NodeKind::Page { config } => {
                    if !cursor.elements.is_empty() || cursor.y > 0.0 {
                        pages.push(cursor.finalize());
                    }
                    cursor = PageCursor::new(config);

                    let cx = cursor.content_x;
                    let cw = cursor.content_width;
                    self.layout_children(
                        &node.children,
                        &node.style,
                        &mut cursor,
                        &mut pages,
                        cx,
                        cw,
                        None,
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
                    self.layout_node(node, &mut cursor, &mut pages, cx, cw, None, font_context);
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
    ) {
        let style = node.style.resolve(parent_style, available_width);

        if style.break_before {
            pages.push(cursor.finalize());
            *cursor = cursor.new_page();
        }

        match &node.kind {
            NodeKind::PageBreak => {
                pages.push(cursor.finalize());
                *cursor = cursor.new_page();
            }

            NodeKind::Fixed { position } => {
                let height = self.measure_node_height(node, available_width, &style, font_context);
                match position {
                    FixedPosition::Header => {
                        cursor.fixed_header.push((node.clone(), height));
                        cursor.y += height;
                    }
                    FixedPosition::Footer => {
                        cursor.fixed_footer.push((node.clone(), height));
                    }
                }
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
        let margin = &style.margin;
        let border = &style.border_width;

        let outer_width = match style.width {
            SizeConstraint::Fixed(w) => w,
            SizeConstraint::Auto => available_width - margin.horizontal(),
        };
        let inner_width = outer_width - padding.horizontal() - border.horizontal();

        let children_height =
            self.measure_children_height(&node.children, inner_width, style, font_context);
        let total_height = children_height + padding.vertical() + border.vertical();

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

            let saved_y = cursor.y;
            cursor.y += margin.top + padding.top + border.top;

            let children_x = node_x + padding.left + border.left;
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
                    border_radius: style.border_radius,
                },
                children: child_elements,
                node_type: Some(node_kind_name(&node.kind).to_string()),
                resolved_style: Some(style.clone()),
                source_location: node.source_location.clone(),
                href: None,
                bookmark: node.bookmark.clone(),
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
        _outer_width: f64,
        inner_width: f64,
        font_context: &FontContext,
    ) {
        let padding = &style.padding;
        let border = &style.border_width;
        let margin = &style.margin;

        cursor.y += margin.top + padding.top + border.top;

        // Emit a zero-height marker element so the bookmark gets into the PDF outline
        if node.bookmark.is_some() {
            cursor.elements.push(LayoutElement {
                x: node_x,
                y: cursor.content_y + cursor.y,
                width: 0.0,
                height: 0.0,
                draw: DrawCommand::None,
                children: vec![],
                node_type: None,
                resolved_style: None,
                source_location: None,
                href: None,
                bookmark: node.bookmark.clone(),
            });
        }

        let children_x = node_x + padding.left + border.left;
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

                for (i, child) in items.iter().enumerate() {
                    if i > 0 {
                        cursor.y += row_gap;
                    }
                    self.layout_node(
                        child,
                        cursor,
                        pages,
                        content_x,
                        available_width,
                        parent_style,
                        font_context,
                    );
                }
            }

            FlexDirection::Row | FlexDirection::RowReverse => {
                let flow_owned: Vec<Node> = flow_children.into_iter().cloned().collect();
                self.layout_flex_row(
                    &flow_owned,
                    cursor,
                    pages,
                    available_width,
                    parent_style,
                    column_gap,
                    row_gap,
                    font_context,
                );
            }
        }

        // Second pass: absolute children
        for abs_child in &abs_children {
            let abs_style = abs_child.style.resolve(parent_style, available_width);

            // Measure intrinsic size
            let child_width = match abs_style.width {
                SizeConstraint::Fixed(w) => w,
                SizeConstraint::Auto => {
                    // If both left and right are set, stretch width
                    if let (Some(l), Some(r)) = (abs_style.left, abs_style.right) {
                        (available_width - l - r).max(0.0)
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

            // Determine position relative to parent content box
            let abs_x = if let Some(l) = abs_style.left {
                content_x + l
            } else if let Some(r) = abs_style.right {
                content_x + available_width - r - child_width
            } else {
                content_x
            };

            let parent_content_y = cursor.content_y;
            let abs_y = if let Some(t) = abs_style.top {
                parent_content_y + t
            } else if let Some(b) = abs_style.bottom {
                // Bottom relative to the page content area
                parent_content_y + cursor.content_height - b - child_height
            } else {
                parent_content_y
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
            );

            // Add absolute elements to the current cursor (renders on top)
            cursor.elements.extend(abs_cursor.elements);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_flex_row(
        &self,
        children: &[Node],
        cursor: &mut PageCursor,
        pages: &mut Vec<LayoutPage>,
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
        let items: Vec<FlexItem> = children
            .iter()
            .map(|child| {
                let style = child.style.resolve(parent_style, available_width);
                let base_width = match style.width {
                    SizeConstraint::Fixed(w) => w,
                    SizeConstraint::Auto => {
                        self.measure_intrinsic_width(child, &style, font_context)
                    }
                };
                FlexItem {
                    node: child,
                    style,
                    base_width,
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
                        final_widths[line.start + j] = w.max(item.style.min_width);
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

            // Page break check for this line
            if line_height > cursor.remaining_height() {
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

            let mut x = cursor.content_x + start_offset;

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

                let y_offset = match align {
                    AlignItems::FlexStart => 0.0,
                    AlignItems::FlexEnd => line_height - item_height - item.style.margin.vertical(),
                    AlignItems::Center => {
                        (line_height - item_height - item.style.margin.vertical()) / 2.0
                    }
                    AlignItems::Stretch => 0.0,
                    AlignItems::Baseline => 0.0,
                };

                let saved_y = cursor.y;
                cursor.y = row_start_y + y_offset;

                self.layout_node(item.node, cursor, pages, x, fw, parent_style, font_context);

                cursor.y = saved_y;
                x += fw;
            }

            cursor.y = row_start_y + line_height;
        }
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
        let margin = &style.margin;
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

        cursor.y += margin.top + padding.top + border.top;

        let cell_x_start = table_x + padding.left + border.left;
        for header_row in &header_rows {
            self.layout_table_row(
                header_row,
                &col_widths,
                style,
                cursor,
                cell_x_start,
                font_context,
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
            );
        }

        cursor.y += padding.bottom + border.bottom + margin.bottom;
    }

    fn layout_table_row(
        &self,
        row: &Node,
        col_widths: &[f64],
        parent_style: &ResolvedStyle,
        cursor: &mut PageCursor,
        start_x: f64,
        font_context: &FontContext,
    ) {
        let row_style = row
            .style
            .resolve(Some(parent_style), col_widths.iter().sum());

        let row_height = self.measure_table_row_height(row, col_widths, parent_style, font_context);
        let row_y = cursor.content_y + cursor.y;
        let total_width: f64 = col_widths.iter().sum();

        // Snapshot before laying out cells — we'll collect them as row children
        let row_snapshot = cursor.elements.len();

        let mut cell_x = start_x;
        for (i, cell) in row.children.iter().enumerate() {
            let col_width = col_widths.get(i).copied().unwrap_or(0.0);

            let cell_style = cell.style.resolve(Some(&row_style), col_width);

            // Snapshot before cell content — we'll collect as cell children
            let cell_snapshot = cursor.elements.len();

            let inner_width =
                col_width - cell_style.padding.horizontal() - cell_style.border_width.horizontal();

            let content_x = cell_x + cell_style.padding.left + cell_style.border_width.left;
            let saved_y = cursor.y;
            cursor.y += cell_style.padding.top + cell_style.border_width.top;

            for child in &cell.children {
                self.layout_node(
                    child,
                    cursor,
                    &mut Vec::new(),
                    content_x,
                    inner_width,
                    Some(&cell_style),
                    font_context,
                );
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
                        border_radius: cell_style.border_radius,
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
                    border_radius: CornerValues::uniform(0.0),
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
        });

        cursor.y += row_height;
    }

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
    ) {
        let margin = &style.margin;
        let text_x = x + margin.left;
        let text_width = available_width - margin.horizontal();

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
            );
            cursor.y += margin.bottom;
            return;
        }

        let lines = self.text_layout.break_into_lines(
            font_context,
            content,
            text_width,
            style.font_size,
            &style.font_family,
            style.font_weight,
            style.font_style,
            style.letter_spacing,
        );

        let line_height = style.font_size * style.line_height;

        // Snapshot-and-collect: accumulate line elements, wrap in parent
        let mut snapshot = cursor.elements.len();
        let mut container_start_y = cursor.content_y + cursor.y;
        let mut is_first_element = true;

        for line in &lines {
            if line_height > cursor.remaining_height() {
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
                        node_type: Some("Text".to_string()),
                        resolved_style: Some(style.clone()),
                        source_location: source_location.cloned(),
                        href: href.map(|s| s.to_string()),
                        bookmark: if is_first_element {
                            bookmark.map(|s| s.to_string())
                        } else {
                            None
                        },
                    });
                    is_first_element = false;
                }

                pages.push(cursor.finalize());
                *cursor = cursor.new_page();

                // Reset snapshot for new page
                snapshot = cursor.elements.len();
                container_start_y = cursor.content_y + cursor.y;
            }

            let line_x = match style.text_align {
                TextAlign::Left => text_x,
                TextAlign::Right => text_x + text_width - line.width,
                TextAlign::Center => text_x + (text_width - line.width) / 2.0,
                TextAlign::Justify => text_x,
            };

            let glyphs: Vec<PositionedGlyph> = line
                .chars
                .iter()
                .enumerate()
                .map(|(j, ch)| {
                    let glyph_x = line.char_positions.get(j).copied().unwrap_or(0.0);
                    PositionedGlyph {
                        glyph_id: *ch as u16,
                        x_offset: glyph_x,
                        font_size: style.font_size,
                        font_family: style.font_family.clone(),
                        font_weight: style.font_weight,
                        font_style: style.font_style,
                        char_value: *ch,
                        color: Some(style.color),
                        href: href.map(|s| s.to_string()),
                    }
                })
                .collect();

            let text_line = TextLine {
                x: line_x,
                y: cursor.content_y + cursor.y + style.font_size,
                glyphs,
                width: line.width,
                height: line_height,
            };

            cursor.elements.push(LayoutElement {
                x: line_x,
                y: cursor.content_y + cursor.y,
                width: line.width,
                height: line_height,
                draw: DrawCommand::Text {
                    lines: vec![text_line],
                    color: style.color,
                    text_decoration: style.text_decoration,
                },
                children: vec![],
                node_type: Some("TextLine".to_string()),
                resolved_style: Some(style.clone()),
                source_location: None,
                href: href.map(|s| s.to_string()),
                bookmark: None,
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
                node_type: Some("Text".to_string()),
                resolved_style: Some(style.clone()),
                source_location: source_location.cloned(),
                href: href.map(|s| s.to_string()),
                bookmark: if is_first_element {
                    bookmark.map(|s| s.to_string())
                } else {
                    None
                },
            });
        }

        cursor.y += margin.bottom;
    }

    /// Layout text runs with per-run styling.
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
    ) {
        // Build StyledChar list from runs
        let mut styled_chars: Vec<StyledChar> = Vec::new();
        for run in runs {
            let run_style = run.style.resolve(Some(style), text_width);
            let run_href = run.href.as_deref().or(parent_href);
            for ch in run.content.chars() {
                styled_chars.push(StyledChar {
                    ch,
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
        let broken_lines =
            self.text_layout
                .break_runs_into_lines(font_context, &styled_chars, text_width);

        let line_height = style.font_size * style.line_height;

        let mut snapshot = cursor.elements.len();
        let mut container_start_y = cursor.content_y + cursor.y;
        let mut is_first_element = true;

        for run_line in &broken_lines {
            if line_height > cursor.remaining_height() {
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
                        node_type: Some("Text".to_string()),
                        resolved_style: Some(style.clone()),
                        source_location: source_location.cloned(),
                        href: parent_href.map(|s| s.to_string()),
                        bookmark: if is_first_element {
                            bookmark.map(|s| s.to_string())
                        } else {
                            None
                        },
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

            let glyphs: Vec<PositionedGlyph> = run_line
                .chars
                .iter()
                .enumerate()
                .map(|(j, sc)| PositionedGlyph {
                    glyph_id: sc.ch as u16,
                    x_offset: run_line.char_positions.get(j).copied().unwrap_or(0.0),
                    font_size: sc.font_size,
                    font_family: sc.font_family.clone(),
                    font_weight: sc.font_weight,
                    font_style: sc.font_style,
                    char_value: sc.ch,
                    color: Some(sc.color),
                    href: sc.href.clone(),
                })
                .collect();

            let text_line = TextLine {
                x: line_x,
                y: cursor.content_y + cursor.y + style.font_size,
                glyphs,
                width: run_line.width,
                height: line_height,
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
                width: run_line.width,
                height: line_height,
                draw: DrawCommand::Text {
                    lines: vec![text_line],
                    color: style.color,
                    text_decoration: text_dec,
                },
                children: vec![],
                node_type: Some("TextLine".to_string()),
                resolved_style: Some(style.clone()),
                source_location: None,
                href: parent_href.map(|s| s.to_string()),
                bookmark: None,
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
                node_type: Some("Text".to_string()),
                resolved_style: Some(style.clone()),
                source_location: source_location.cloned(),
                href: parent_href.map(|s| s.to_string()),
                bookmark: if is_first_element {
                    bookmark.map(|s| s.to_string())
                } else {
                    None
                },
            });
        }
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
        let margin = &style.margin;

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
            href: None,
            bookmark: node.bookmark.clone(),
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
        let margin = &style.margin;
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
            },
            children: vec![],
            node_type: Some("Svg".to_string()),
            resolved_style: Some(style.clone()),
            source_location: node.source_location.clone(),
            href: None,
            bookmark: node.bookmark.clone(),
        });

        cursor.y += svg_height + margin.bottom;
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
            NodeKind::Text { content, runs, .. } => {
                let measure_width = available_width - style.margin.horizontal();
                if !runs.is_empty() {
                    // Measure runs
                    let mut styled_chars: Vec<StyledChar> = Vec::new();
                    for run in runs {
                        let run_style = run.style.resolve(Some(style), measure_width);
                        for ch in run.content.chars() {
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
                    );
                    let line_height = style.font_size * style.line_height;
                    (broken_lines.len() as f64) * line_height + style.padding.vertical()
                } else {
                    let lines = self.text_layout.break_into_lines(
                        font_context,
                        content,
                        measure_width,
                        style.font_size,
                        &style.font_family,
                        style.font_weight,
                        style.font_style,
                        style.letter_spacing,
                    );
                    let line_height = style.font_size * style.line_height;
                    (lines.len() as f64) * line_height + style.padding.vertical()
                }
            }
            NodeKind::Image { height, .. } => {
                height.unwrap_or(available_width * 0.75) + style.padding.vertical()
            }
            NodeKind::Svg { height, .. } => *height + style.margin.vertical(),
            _ => {
                // Match layout_view: when width is Auto, margin reduces the outer width
                let outer_width = match style.width {
                    SizeConstraint::Fixed(w) => w,
                    SizeConstraint::Auto => available_width - style.margin.horizontal(),
                };
                let inner_width =
                    outer_width - style.padding.horizontal() - style.border_width.horizontal();
                let children_height =
                    self.measure_children_height(&node.children, inner_width, style, font_context);
                children_height + style.padding.vertical() + style.border_width.vertical()
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
        let direction = parent_style.flex_direction;
        let row_gap = parent_style.row_gap;
        let column_gap = parent_style.column_gap;

        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                // Measure base widths for all children
                let base_widths: Vec<f64> = children
                    .iter()
                    .map(|child| {
                        let child_style = child.style.resolve(Some(parent_style), available_width);
                        match child_style.width {
                            SizeConstraint::Fixed(w) => w,
                            SizeConstraint::Auto => {
                                self.measure_intrinsic_width(child, &child_style, font_context)
                            }
                        }
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

                let mut total = 0.0;
                for (i, line) in lines.iter().enumerate() {
                    let line_height: f64 = children[line.start..line.end]
                        .iter()
                        .zip(&base_widths[line.start..line.end])
                        .map(|(child, &bw)| {
                            let child_style = child.style.resolve(Some(parent_style), bw);
                            self.measure_node_height(child, bw, &child_style, font_context)
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
            NodeKind::Text { content, .. } => {
                let italic = matches!(style.font_style, FontStyle::Italic | FontStyle::Oblique);
                let text_width = font_context.measure_string(
                    content,
                    &style.font_family,
                    style.font_weight,
                    italic,
                    style.font_size,
                    style.letter_spacing,
                );
                text_width + style.padding.horizontal() + style.margin.horizontal()
            }
            NodeKind::Image { width, .. } => {
                width.unwrap_or(100.0) + style.padding.horizontal() + style.margin.horizontal()
            }
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

        for (i, cell) in row.children.iter().enumerate() {
            let col_width = col_widths.get(i).copied().unwrap_or(0.0);
            let cell_style = cell.style.resolve(Some(&row_style), col_width);
            let inner_width =
                col_width - cell_style.padding.horizontal() - cell_style.border_width.horizontal();

            let mut cell_content_height = 0.0;
            for child in &cell.children {
                let child_style = child.style.resolve(Some(&cell_style), inner_width);
                cell_content_height +=
                    self.measure_node_height(child, inner_width, &child_style, font_context);
            }

            let total = cell_content_height
                + cell_style.padding.vertical()
                + cell_style.border_width.vertical();
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
        for page in pages.iter_mut() {
            if page.fixed_header.is_empty() && page.fixed_footer.is_empty() {
                continue;
            }

            // Lay out headers at top of content area
            if !page.fixed_header.is_empty() {
                let mut hdr_cursor = PageCursor::new(&page.config);
                for (node, _h) in &page.fixed_header {
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
                let total_ftr: f64 = page.fixed_footer.iter().map(|(_, h)| *h).sum();
                let target_y = ftr_cursor.content_height - total_ftr;
                // Layout from y=0
                for (node, _h) in &page.fixed_footer {
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
}

struct FlexItem<'a> {
    node: &'a Node,
    style: ResolvedStyle,
    base_width: f64,
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
        }
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
}
