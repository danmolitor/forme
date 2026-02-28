//! # Document Model
//!
//! The input representation for the rendering engine. A document is a tree of
//! nodes, each with a type, style properties, and children. This is designed
//! to be easily produced by a React reconciler, an HTML parser, or direct
//! JSON construction.
//!
//! The model is intentionally close to the DOM/React mental model: you have
//! containers (View), text (Text), images (Image), and tables (Table). But
//! there is one critical addition: **Page** is a first-class node type.

use crate::style::Style;
use serde::{Deserialize, Serialize};

/// A complete document ready for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// The root nodes of the document. Typically one or more Page nodes,
    /// but can also be content nodes that get auto-wrapped in pages.
    pub children: Vec<Node>,

    /// Document metadata (title, author, etc.)
    #[serde(default)]
    pub metadata: Metadata,

    /// Default page configuration used when content overflows or when
    /// nodes aren't explicitly wrapped in Page nodes.
    #[serde(default)]
    pub default_page: PageConfig,

    /// Custom fonts to register before layout. Each entry contains
    /// the font family name, base64-encoded font data, weight, and style.
    #[serde(default)]
    pub fonts: Vec<FontEntry>,

    /// Whether to produce a tagged (accessible) PDF with structure tree.
    #[serde(default)]
    pub tagged: bool,

    /// PDF/A conformance level. When set, forces `tagged = true` for "2a".
    #[serde(default)]
    pub pdfa: Option<PdfAConformance>,
}

/// PDF/A conformance level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PdfAConformance {
    /// PDF/A-2a: full accessibility (requires tagging).
    #[serde(rename = "2a")]
    A2a,
    /// PDF/A-2b: basic compliance (visual appearance only).
    #[serde(rename = "2b")]
    A2b,
}

/// A custom font to register with the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontEntry {
    /// Font family name (e.g. "Inter", "Roboto").
    pub family: String,
    /// Base64-encoded font data, or a data URI (e.g. "data:font/ttf;base64,...").
    pub src: String,
    /// Font weight (100-900). Defaults to 400.
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Whether this is an italic variant.
    #[serde(default)]
    pub italic: bool,
}

fn default_weight() -> u32 {
    400
}

/// Document metadata embedded in the PDF.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    /// Document language (BCP 47 tag, e.g. "en-US"). Emitted as /Lang in the PDF Catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Configuration for a page: size, margins, orientation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageConfig {
    /// Page size. Defaults to A4.
    #[serde(default = "PageSize::default")]
    pub size: PageSize,

    /// Page margins in points (1/72 inch).
    #[serde(default)]
    pub margin: Edges,

    /// Whether this page auto-wraps content that overflows.
    #[serde(default = "default_true")]
    pub wrap: bool,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            size: PageSize::A4,
            margin: Edges::uniform(54.0), // ~0.75 inch
            wrap: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Standard page sizes in points.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum PageSize {
    #[default]
    A4,
    A3,
    A5,
    Letter,
    Legal,
    Tabloid,
    Custom {
        width: f64,
        height: f64,
    },
}

impl PageSize {
    /// Returns (width, height) in points.
    pub fn dimensions(&self) -> (f64, f64) {
        match self {
            PageSize::A4 => (595.28, 841.89),
            PageSize::A3 => (841.89, 1190.55),
            PageSize::A5 => (419.53, 595.28),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Tabloid => (792.0, 1224.0),
            PageSize::Custom { width, height } => (*width, *height),
        }
    }
}

/// Edge values (top, right, bottom, left) used for margin and padding.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Edges {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Edges {
    pub fn uniform(v: f64) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub fn symmetric(vertical: f64, horizontal: f64) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }
}

/// A node in the document tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// What kind of node this is.
    pub kind: NodeKind,

    /// Style properties for this node.
    #[serde(default)]
    pub style: Style,

    /// Child nodes.
    #[serde(default)]
    pub children: Vec<Node>,

    /// A unique identifier for this node (optional, useful for debugging).
    #[serde(default)]
    pub id: Option<String>,

    /// Source code location for click-to-source in the dev inspector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,

    /// Bookmark title for this node (creates a PDF outline entry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    /// Optional hyperlink URL for this node (creates a PDF link annotation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,

    /// Optional alt text for images and SVGs (accessibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// The different kinds of nodes in the document tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodeKind {
    /// A page boundary. Content inside flows according to page config.
    Page {
        #[serde(default)]
        config: PageConfig,
    },

    /// A generic container, analogous to a <div> or React <View>.
    View,

    /// A text node with string content.
    Text {
        content: String,
        /// Optional hyperlink URL.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        href: Option<String>,
        /// Inline styled runs. When non-empty, `content` is ignored.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        runs: Vec<TextRun>,
    },

    /// An image node.
    Image {
        /// Base64-encoded image data, or a file path.
        src: String,
        /// Image width in points (optional, will use intrinsic if not set).
        width: Option<f64>,
        /// Image height in points (optional, will use intrinsic if not set).
        height: Option<f64>,
    },

    /// A table container. Children should be TableRow nodes.
    Table {
        /// Column width definitions. If omitted, columns distribute evenly.
        #[serde(default)]
        columns: Vec<ColumnDef>,
    },

    /// A row inside a Table.
    TableRow {
        /// If true, this row repeats at the top of each page when the table
        /// breaks across pages. This is the killer feature.
        #[serde(default)]
        is_header: bool,
    },

    /// A cell inside a TableRow.
    TableCell {
        /// Column span.
        #[serde(default = "default_one")]
        col_span: u32,
        /// Row span.
        #[serde(default = "default_one")]
        row_span: u32,
    },

    /// A fixed element that repeats on every page (headers, footers, page numbers).
    Fixed {
        /// Where to place this element on the page.
        position: FixedPosition,
    },

    /// An explicit page break.
    PageBreak,

    /// An SVG element rendered as vector graphics.
    Svg {
        /// Display width in points.
        width: f64,
        /// Display height in points.
        height: f64,
        /// Optional viewBox (e.g. "0 0 100 100").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        view_box: Option<String>,
        /// SVG markup content (the inner XML).
        content: String,
    },
}

/// An inline styled run within a Text node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub content: String,
    #[serde(default)]
    pub style: crate::style::Style,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// Positioning mode for a node.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

fn default_one() -> u32 {
    1
}

/// Column definition for tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Width as a fraction (0.0-1.0) of available table width, or fixed points.
    pub width: ColumnWidth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnWidth {
    /// Fraction of available width (0.0-1.0).
    Fraction(f64),
    /// Fixed width in points.
    Fixed(f64),
    /// Distribute remaining space evenly among Auto columns.
    Auto,
}

/// Where a fixed element is placed on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixedPosition {
    /// Top of the content area (below margin).
    Header,
    /// Bottom of the content area (above margin).
    Footer,
}

/// Source code location for click-to-source in the dev server inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl Node {
    /// Create a View node with children.
    pub fn view(style: Style, children: Vec<Node>) -> Self {
        Self {
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

    /// Create a Text node.
    pub fn text(content: &str, style: Style) -> Self {
        Self {
            kind: NodeKind::Text {
                content: content.to_string(),
                href: None,
                runs: vec![],
            },
            style,
            children: vec![],
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    /// Create a Page node.
    pub fn page(config: PageConfig, style: Style, children: Vec<Node>) -> Self {
        Self {
            kind: NodeKind::Page { config },
            style,
            children,
            id: None,
            source_location: None,
            bookmark: None,
            href: None,
            alt: None,
        }
    }

    /// Is this node breakable across pages?
    pub fn is_breakable(&self) -> bool {
        match &self.kind {
            NodeKind::View | NodeKind::Table { .. } | NodeKind::Text { .. } => {
                self.style.wrap.unwrap_or(true)
            }
            NodeKind::TableRow { .. } => true,
            NodeKind::Image { .. } => false,
            NodeKind::Svg { .. } => false,
            NodeKind::PageBreak => false,
            NodeKind::Fixed { .. } => false,
            NodeKind::Page { .. } => true,
            NodeKind::TableCell { .. } => true,
        }
    }
}
