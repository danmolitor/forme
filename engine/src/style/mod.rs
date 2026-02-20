//! # Style System
//!
//! A CSS-like style model for document nodes. This is intentionally a subset
//! of CSS that covers the properties needed for document layout: flexbox,
//! box model, typography, color, borders.
//!
//! We don't try to implement all of CSS. We implement the parts that matter
//! for PDF documents, and we implement them correctly.

use crate::model::{Edges, Position};
use serde::{Deserialize, Serialize};

/// The complete set of style properties for a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    // ── Box Model ──────────────────────────────────────────────
    /// Explicit width in points.
    pub width: Option<Dimension>,
    /// Explicit height in points.
    pub height: Option<Dimension>,
    /// Minimum width.
    pub min_width: Option<Dimension>,
    /// Minimum height.
    pub min_height: Option<Dimension>,
    /// Maximum width.
    pub max_width: Option<Dimension>,
    /// Maximum height.
    pub max_height: Option<Dimension>,

    /// Padding inside the border.
    #[serde(default)]
    pub padding: Option<Edges>,
    /// Margin outside the border.
    #[serde(default)]
    pub margin: Option<Edges>,

    // ── Flexbox Layout ─────────────────────────────────────────
    /// Direction of the main axis.
    #[serde(default)]
    pub flex_direction: Option<FlexDirection>,
    /// How to distribute space along the main axis.
    #[serde(default)]
    pub justify_content: Option<JustifyContent>,
    /// How to align items along the cross axis.
    #[serde(default)]
    pub align_items: Option<AlignItems>,
    /// Override align-items for this specific child.
    #[serde(default)]
    pub align_self: Option<AlignItems>,
    /// Whether flex items wrap to new lines.
    #[serde(default)]
    pub flex_wrap: Option<FlexWrap>,
    /// How to distribute space between flex lines on the cross axis.
    pub align_content: Option<AlignContent>,
    /// Flex grow factor.
    pub flex_grow: Option<f64>,
    /// Flex shrink factor.
    pub flex_shrink: Option<f64>,
    /// Flex basis (initial main size).
    pub flex_basis: Option<Dimension>,
    /// Gap between flex items.
    pub gap: Option<f64>,
    /// Row gap (overrides gap for rows).
    pub row_gap: Option<f64>,
    /// Column gap (overrides gap for columns).
    pub column_gap: Option<f64>,

    // ── Typography ─────────────────────────────────────────────
    /// Font family name.
    pub font_family: Option<String>,
    /// Font size in points.
    pub font_size: Option<f64>,
    /// Font weight (100-900).
    pub font_weight: Option<u32>,
    /// Font style.
    pub font_style: Option<FontStyle>,
    /// Line height as a multiplier of font size.
    pub line_height: Option<f64>,
    /// Text alignment within the text block.
    pub text_align: Option<TextAlign>,
    /// Letter spacing in points.
    pub letter_spacing: Option<f64>,
    /// Text decoration.
    pub text_decoration: Option<TextDecoration>,
    /// Text transform.
    pub text_transform: Option<TextTransform>,

    // ── Color & Background ─────────────────────────────────────
    /// Text color.
    pub color: Option<Color>,
    /// Background color.
    pub background_color: Option<Color>,
    /// Opacity (0.0 - 1.0).
    pub opacity: Option<f64>,

    // ── Border ─────────────────────────────────────────────────
    /// Border width for all sides.
    pub border_width: Option<EdgeValues<f64>>,
    /// Border color for all sides.
    pub border_color: Option<EdgeValues<Color>>,
    /// Border radius (uniform or per-corner).
    pub border_radius: Option<CornerValues>,

    // ── Positioning ─────────────────────────────────────────────
    /// Positioning mode (relative or absolute).
    pub position: Option<Position>,
    /// Top offset (for absolute positioning).
    pub top: Option<f64>,
    /// Right offset (for absolute positioning).
    pub right: Option<f64>,
    /// Bottom offset (for absolute positioning).
    pub bottom: Option<f64>,
    /// Left offset (for absolute positioning).
    pub left: Option<f64>,

    // ── Page Behavior ──────────────────────────────────────────
    /// Whether this node can be broken across pages.
    /// `true` = breakable (default for View, Text, Table).
    /// `false` = keep on one page; if it doesn't fit, move to next page.
    pub wrap: Option<bool>,

    /// Force a page break before this node.
    pub break_before: Option<bool>,

    /// Minimum number of lines to keep at the bottom of a page before
    /// breaking (widow control). Default: 2.
    pub min_widow_lines: Option<u32>,

    /// Minimum number of lines to keep at the top of a new page after
    /// breaking (orphan control). Default: 2.
    pub min_orphan_lines: Option<u32>,
}

/// A dimension that can be points, percentage, or auto.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Dimension {
    /// Fixed size in points (1/72 inch).
    Pt(f64),
    /// Percentage of parent's corresponding dimension.
    Percent(f64),
    /// Size determined by content.
    Auto,
}

impl Dimension {
    /// Resolve this dimension given a parent size.
    /// Returns None for Auto.
    pub fn resolve(&self, parent_size: f64) -> Option<f64> {
        match self {
            Dimension::Pt(v) => Some(*v),
            Dimension::Percent(p) => Some(parent_size * p / 100.0),
            Dimension::Auto => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum FlexDirection {
    #[default]
    Column,
    Row,
    ColumnReverse,
    RowReverse,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    #[default]
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum AlignContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

/// An RGBA color.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color {
    pub r: f64, // 0.0 - 1.0
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                (r, g, b)
            }
            _ => (0, 0, 0),
        };
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: 1.0,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

/// Values for each edge (top, right, bottom, left).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EdgeValues<T: Copy> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> EdgeValues<T> {
    pub fn uniform(v: T) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// Values for each corner (top-left, top-right, bottom-right, bottom-left).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CornerValues {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_right: f64,
    pub bottom_left: f64,
}

impl CornerValues {
    pub fn uniform(v: f64) -> Self {
        Self {
            top_left: v,
            top_right: v,
            bottom_right: v,
            bottom_left: v,
        }
    }
}

/// Resolved style: all values are concrete (no Option, no Auto for computed values).
/// This is what the layout engine works with after style resolution.
#[derive(Debug, Clone)]
pub struct ResolvedStyle {
    // Box model
    pub width: SizeConstraint,
    pub height: SizeConstraint,
    pub min_width: f64,
    pub min_height: f64,
    pub max_width: f64,
    pub max_height: f64,
    pub padding: Edges,
    pub margin: Edges,

    // Flex
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: Option<AlignItems>,
    pub flex_wrap: FlexWrap,
    pub align_content: AlignContent,
    pub flex_grow: f64,
    pub flex_shrink: f64,
    pub flex_basis: SizeConstraint,
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
    pub opacity: f64,
    pub border_width: Edges,
    pub border_color: EdgeValues<Color>,
    pub border_radius: CornerValues,

    // Positioning
    pub position: Position,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
    pub left: Option<f64>,

    // Page behavior
    pub breakable: bool,
    pub break_before: bool,
    pub min_widow_lines: u32,
    pub min_orphan_lines: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum SizeConstraint {
    Fixed(f64),
    Auto,
}

impl Style {
    /// Resolve this style against a parent's resolved style and available dimensions.
    pub fn resolve(&self, parent: Option<&ResolvedStyle>, available_width: f64) -> ResolvedStyle {
        let parent_font_size = parent.map(|p| p.font_size).unwrap_or(12.0);
        let parent_color = parent.map(|p| p.color).unwrap_or(Color::BLACK);
        let parent_font_family = parent
            .map(|p| p.font_family.clone())
            .unwrap_or_else(|| "Helvetica".to_string());

        let font_size = self.font_size.unwrap_or(parent_font_size);

        ResolvedStyle {
            width: self
                .width
                .map(|d| match d {
                    Dimension::Pt(v) => SizeConstraint::Fixed(v),
                    Dimension::Percent(p) => SizeConstraint::Fixed(available_width * p / 100.0),
                    Dimension::Auto => SizeConstraint::Auto,
                })
                .unwrap_or(SizeConstraint::Auto),

            height: self
                .height
                .map(|d| match d {
                    Dimension::Pt(v) => SizeConstraint::Fixed(v),
                    Dimension::Percent(p) => SizeConstraint::Fixed(p), // height % is complex, simplified
                    Dimension::Auto => SizeConstraint::Auto,
                })
                .unwrap_or(SizeConstraint::Auto),

            min_width: self
                .min_width
                .and_then(|d| d.resolve(available_width))
                .unwrap_or(0.0),
            min_height: self.min_height.and_then(|d| d.resolve(0.0)).unwrap_or(0.0),
            max_width: self
                .max_width
                .and_then(|d| d.resolve(available_width))
                .unwrap_or(f64::INFINITY),
            max_height: self
                .max_height
                .and_then(|d| d.resolve(0.0))
                .unwrap_or(f64::INFINITY),

            padding: self.padding.unwrap_or_default(),
            margin: self.margin.unwrap_or_default(),

            flex_direction: self.flex_direction.unwrap_or_default(),
            justify_content: self.justify_content.unwrap_or_default(),
            align_items: self.align_items.unwrap_or_default(),
            align_self: self.align_self,
            flex_wrap: self.flex_wrap.unwrap_or_default(),
            align_content: self.align_content.unwrap_or_default(),
            flex_grow: self.flex_grow.unwrap_or(0.0),
            flex_shrink: self.flex_shrink.unwrap_or(1.0),
            flex_basis: self
                .flex_basis
                .map(|d| match d {
                    Dimension::Pt(v) => SizeConstraint::Fixed(v),
                    Dimension::Percent(p) => SizeConstraint::Fixed(available_width * p / 100.0),
                    Dimension::Auto => SizeConstraint::Auto,
                })
                .unwrap_or(SizeConstraint::Auto),
            gap: self.gap.unwrap_or(0.0),
            row_gap: self.row_gap.or(self.gap).unwrap_or(0.0),
            column_gap: self.column_gap.or(self.gap).unwrap_or(0.0),

            font_family: self.font_family.clone().unwrap_or(parent_font_family),
            font_size,
            font_weight: self
                .font_weight
                .unwrap_or(parent.map(|p| p.font_weight).unwrap_or(400)),
            font_style: self
                .font_style
                .unwrap_or(parent.map(|p| p.font_style).unwrap_or_default()),
            line_height: self
                .line_height
                .unwrap_or(parent.map(|p| p.line_height).unwrap_or(1.4)),
            text_align: self
                .text_align
                .unwrap_or(parent.map(|p| p.text_align).unwrap_or_default()),
            letter_spacing: self.letter_spacing.unwrap_or(0.0),
            text_decoration: self
                .text_decoration
                .unwrap_or(parent.map(|p| p.text_decoration).unwrap_or_default()),
            text_transform: self
                .text_transform
                .unwrap_or(parent.map(|p| p.text_transform).unwrap_or_default()),

            color: self.color.unwrap_or(parent_color),
            background_color: self.background_color,
            opacity: self.opacity.unwrap_or(1.0),

            border_width: self
                .border_width
                .map(|e| Edges {
                    top: e.top,
                    right: e.right,
                    bottom: e.bottom,
                    left: e.left,
                })
                .unwrap_or_default(),

            border_color: self
                .border_color
                .unwrap_or(EdgeValues::uniform(Color::BLACK)),
            border_radius: self.border_radius.unwrap_or(CornerValues::uniform(0.0)),

            position: self.position.unwrap_or_default(),
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            left: self.left,

            breakable: self.wrap.unwrap_or(true),
            break_before: self.break_before.unwrap_or(false),
            min_widow_lines: self.min_widow_lines.unwrap_or(2),
            min_orphan_lines: self.min_orphan_lines.unwrap_or(2),
        }
    }
}
