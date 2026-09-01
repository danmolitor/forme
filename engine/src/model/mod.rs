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
use serde::{Deserialize, Deserializer, Serialize};

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

    /// Page configuration for the FIRST page only, when it differs from
    /// `default_page` (CSS `@page :first`). Margins and background may
    /// vary; size should match `default_page` — flowing content bakes
    /// widths at layout time, so per-page size changes are not supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page: Option<PageConfig>,

    /// Custom fonts to register before layout. Each entry contains
    /// the font family name, base64-encoded font data, weight, and style.
    #[serde(default)]
    pub fonts: Vec<FontEntry>,

    /// Default style applied to the root of the document tree.
    /// Useful for setting a global `font_family`, `font_size`, `color`, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_style: Option<crate::style::Style>,

    /// Whether to produce a tagged (accessible) PDF with structure tree.
    ///
    /// Defaults to `true`: every render emits a structure tree unless the
    /// caller explicitly sets `tagged: false`. Tagging is layout-neutral —
    /// the tag tree is built after layout, so geometry and visual output are
    /// byte-for-byte unchanged; only the structural PDF objects differ.
    #[serde(default = "default_true")]
    pub tagged: bool,

    /// PDF/A conformance level. When set, forces `tagged = true` for "2a".
    #[serde(default)]
    pub pdfa: Option<PdfAConformance>,

    /// When true, the PDF claims PDF/UA-1 conformance. Forces `tagged = true`.
    #[serde(default)]
    pub pdf_ua: bool,

    /// Optional JSON string to embed as an attached file in the PDF.
    /// Enables round-tripping structured data through PDF files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_data: Option<String>,

    /// When true, form field values are rendered as static content and no
    /// interactive AcroForm widgets are emitted. The resulting PDF has no
    /// fillable fields.
    #[serde(default)]
    pub flatten_forms: bool,

    /// Digital certification configuration. When set, the rendered PDF is certified
    /// with the specified X.509 certificate and RSA private key.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "signature")]
    pub certification: Option<CertificationConfig>,
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
    /// PDF/A-2u: 2b plus a Unicode mapping for all text.
    #[serde(rename = "2u")]
    A2u,
}

/// A rectangular region to redact in an existing PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRegion {
    /// 0-indexed page number.
    pub page: usize,
    /// X coordinate in points from the left edge of the page.
    pub x: f64,
    /// Y coordinate in points from the top edge (web/screen coordinates).
    /// The engine converts to PDF bottom-origin internally — do NOT flip before calling.
    pub y: f64,
    /// Width of the redaction rectangle in points.
    pub width: f64,
    /// Height of the redaction rectangle in points.
    pub height: f64,
    /// Fill color as hex string (e.g. "#000000"). Defaults to black.
    #[serde(default)]
    pub color: Option<String>,
}

/// How to interpret a text pattern for redaction search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    /// Exact string match (case-insensitive).
    Literal,
    /// Regular expression pattern.
    Regex,
}

/// A text pattern to search for in a PDF for redaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionPattern {
    /// The search string (literal text or regex pattern).
    pub pattern: String,
    /// Whether to interpret `pattern` as literal text or a regex.
    pub pattern_type: PatternType,
    /// Restrict search to a specific page (0-indexed). None = all pages.
    #[serde(default)]
    pub page: Option<usize>,
    /// Fill color for the redaction overlay. Defaults to black.
    #[serde(default)]
    pub color: Option<String>,
}

/// Configuration for digitally certifying a PDF with an X.509 certificate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificationConfig {
    /// PEM-encoded X.509 certificate.
    pub certificate_pem: String,
    /// PEM-encoded RSA private key (PKCS#8).
    pub private_key_pem: String,
    /// Reason for signing (e.g. "Approved").
    #[serde(default)]
    pub reason: Option<String>,
    /// Location of signing (e.g. "New York, NY").
    #[serde(default)]
    pub location: Option<String>,
    /// Contact info for the signer.
    #[serde(default)]
    pub contact: Option<String>,
    /// Whether to show a visible signature annotation on the page.
    #[serde(default)]
    pub visible: bool,
    /// X coordinate in points for visible signature.
    #[serde(default)]
    pub x: Option<f64>,
    /// Y coordinate in points for visible signature.
    #[serde(default)]
    pub y: Option<f64>,
    /// Width in points for visible signature.
    #[serde(default)]
    pub width: Option<f64>,
    /// Height in points for visible signature.
    #[serde(default)]
    pub height: Option<f64>,
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
#[serde(rename_all = "camelCase")]
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

    /// Optional background image painted behind the page's content.
    /// URL, file path, or `data:image/...;base64,` URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image: Option<String>,

    /// Opacity for the background image (0.0–1.0). Defaults to 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<f64>,

    /// How the background image is sized within the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_size: Option<BackgroundSize>,

    /// Where the background image is positioned within the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_position: Option<BackgroundPosition>,
}

/// How a background image is scaled to fit a page.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundSize {
    /// Stretch the image to the page's exact dimensions (default).
    #[default]
    Fill,
    /// Scale to fully cover the page; crops if aspect ratio differs.
    Cover,
    /// Scale to fit within the page; letterboxes if aspect ratio differs.
    Contain,
}

/// Where a background image is positioned on a page (relevant for
/// `cover` / `contain` when the image doesn't fill the page exactly).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundPosition {
    Center,
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            size: PageSize::A4,
            margin: Edges::uniform(54.0), // ~0.75 inch
            wrap: true,
            background_image: None,
            background_opacity: None,
            background_size: None,
            background_position: None,
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

/// Edge values (top, right, bottom, left) used for padding and page margins.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Edges {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// A margin edge value — either a fixed point value or auto.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum EdgeValue {
    Pt(f64),
    Auto,
}

impl Default for EdgeValue {
    fn default() -> Self {
        EdgeValue::Pt(0.0)
    }
}

impl EdgeValue {
    /// Resolve to a concrete value, treating Auto as 0.
    pub fn resolve(&self) -> f64 {
        match self {
            EdgeValue::Pt(v) => *v,
            EdgeValue::Auto => 0.0,
        }
    }

    /// Whether this edge is auto.
    pub fn is_auto(&self) -> bool {
        matches!(self, EdgeValue::Auto)
    }
}

impl<'de> Deserialize<'de> for EdgeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de;

        struct EdgeValueVisitor;

        impl<'de> de::Visitor<'de> for EdgeValueVisitor {
            type Value = EdgeValue;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a number or the string \"auto\"")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<EdgeValue, E> {
                Ok(EdgeValue::Pt(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<EdgeValue, E> {
                Ok(EdgeValue::Pt(v as f64))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<EdgeValue, E> {
                Ok(EdgeValue::Pt(v as f64))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<EdgeValue, E> {
                if v == "auto" {
                    Ok(EdgeValue::Auto)
                } else {
                    Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
                }
            }
        }

        deserializer.deserialize_any(EdgeValueVisitor)
    }
}

/// Margin edges that support auto values.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MarginEdges {
    pub top: EdgeValue,
    pub right: EdgeValue,
    pub bottom: EdgeValue,
    pub left: EdgeValue,
}

impl MarginEdges {
    /// Sum of resolved (non-auto) horizontal margins.
    pub fn horizontal(&self) -> f64 {
        self.left.resolve() + self.right.resolve()
    }

    /// Sum of resolved (non-auto) vertical margins.
    pub fn vertical(&self) -> f64 {
        self.top.resolve() + self.bottom.resolve()
    }

    /// Whether any horizontal margin is auto.
    pub fn has_auto_horizontal(&self) -> bool {
        self.left.is_auto() || self.right.is_auto()
    }

    /// Whether any vertical margin is auto.
    pub fn has_auto_vertical(&self) -> bool {
        self.top.is_auto() || self.bottom.is_auto()
    }

    /// Convert from plain Edges (all Pt values).
    pub fn from_edges(e: Edges) -> Self {
        MarginEdges {
            top: EdgeValue::Pt(e.top),
            right: EdgeValue::Pt(e.right),
            bottom: EdgeValue::Pt(e.bottom),
            left: EdgeValue::Pt(e.left),
        }
    }

    /// Convert to plain Edges, resolving auto to 0.
    pub fn to_edges(&self) -> Edges {
        Edges {
            top: self.top.resolve(),
            right: self.right.resolve(),
            bottom: self.bottom.resolve(),
            left: self.left.resolve(),
        }
    }
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

    /// A semantic heading (H1-H6). Lays out as text but carries a level
    /// so the tagged-PDF builder can emit the right `/H1`...`/H6`
    /// structure element. The React layer provides sensible default
    /// styles per level; users can override via `style`.
    Heading {
        level: u8,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        href: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        runs: Vec<TextRun>,
    },

    /// An ordered or unordered list. Children should be `ListItem` nodes.
    /// Marker numbering continues across page breaks.
    List {
        /// Whether items are numbered (true) or use a bullet glyph (false).
        ordered: bool,
        /// Which marker style to render.
        marker_type: ListMarkerType,
        /// Starting index for ordered lists (default 1). Ignored when
        /// `ordered = false`.
        #[serde(default = "default_list_start")]
        start: u32,
    },

    /// One item inside a `List`. Children are the item content.
    ListItem,

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

    /// A fixed element that repeats on pages (headers, footers, page numbers).
    Fixed {
        /// Where to place this element on the page.
        position: FixedPosition,
        /// Which pages this element appears on (CSS `@page :first`
        /// suppression maps to `NotFirst`). Defaults to all pages.
        #[serde(default)]
        pages: FixedPageFilter,
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

    /// A canvas drawing primitive with arbitrary vector operations.
    Canvas {
        /// Display width in points.
        width: f64,
        /// Display height in points.
        height: f64,
        /// Drawing operations to execute.
        operations: Vec<CanvasOp>,
    },

    /// A 1D barcode rendered as vector rectangles.
    Barcode {
        /// The data to encode.
        data: String,
        /// Barcode format (Code128, Code39, EAN13, EAN8, Codabar). Default: Code128.
        #[serde(default)]
        format: crate::barcode::BarcodeFormat,
        /// Width in points. Defaults to available width.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
        /// Height in points. Default: 60.
        #[serde(default = "default_barcode_height")]
        height: f64,
    },

    /// A QR code rendered as vector rectangles.
    QrCode {
        /// The data to encode (URL, text, etc.).
        data: String,
        /// Display size in points (QR codes are always square).
        /// Defaults to available width if omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<f64>,
    },

    /// A bar chart rendered as native vector graphics.
    BarChart {
        /// Data points with labels and values.
        data: Vec<ChartDataPoint>,
        /// Chart width in points.
        width: f64,
        /// Chart height in points.
        height: f64,
        /// Bar color (hex string). Defaults to "#1a365d".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        /// Show X-axis labels below bars.
        #[serde(default = "default_true")]
        show_labels: bool,
        /// Show value labels above bars.
        #[serde(default)]
        show_values: bool,
        /// Show horizontal grid lines.
        #[serde(default)]
        show_grid: bool,
        /// Optional chart title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },

    /// A line chart rendered as native vector graphics.
    LineChart {
        /// Data series (each with name, data points, optional color).
        series: Vec<ChartSeries>,
        /// X-axis labels.
        labels: Vec<String>,
        /// Chart width in points.
        width: f64,
        /// Chart height in points.
        height: f64,
        /// Show dots at data points.
        #[serde(default)]
        show_points: bool,
        /// Show horizontal grid lines.
        #[serde(default)]
        show_grid: bool,
        /// Optional chart title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },

    /// A pie chart rendered as native vector graphics.
    PieChart {
        /// Data points with labels, values, and optional colors.
        data: Vec<ChartDataPoint>,
        /// Chart width in points.
        width: f64,
        /// Chart height in points.
        height: f64,
        /// Whether to render as donut (hollow center).
        #[serde(default)]
        donut: bool,
        /// Show legend.
        #[serde(default)]
        show_legend: bool,
        /// Optional chart title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },

    /// An area chart rendered as native vector graphics.
    AreaChart {
        /// Data series (each with name, data points, optional color).
        series: Vec<ChartSeries>,
        /// X-axis labels.
        labels: Vec<String>,
        /// Chart width in points.
        width: f64,
        /// Chart height in points.
        height: f64,
        /// Show horizontal grid lines.
        #[serde(default)]
        show_grid: bool,
        /// Optional chart title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },

    /// A dot plot (scatter plot) rendered as native vector graphics.
    DotPlot {
        /// Groups of data points.
        groups: Vec<DotPlotGroup>,
        /// Chart width in points.
        width: f64,
        /// Chart height in points.
        height: f64,
        /// Minimum X value. Auto-computed if not set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x_min: Option<f64>,
        /// Maximum X value. Auto-computed if not set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x_max: Option<f64>,
        /// Minimum Y value. Auto-computed if not set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y_min: Option<f64>,
        /// Maximum Y value. Auto-computed if not set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y_max: Option<f64>,
        /// X-axis label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x_label: Option<String>,
        /// Y-axis label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y_label: Option<String>,
        /// Show legend.
        #[serde(default)]
        show_legend: bool,
        /// Dot radius in points.
        #[serde(default = "default_dot_size")]
        dot_size: f64,
    },

    /// A watermark rendered as rotated text behind page content.
    Watermark {
        /// The watermark text (e.g. "DRAFT", "CONFIDENTIAL").
        text: String,
        /// Font size in points. Default: 60.
        #[serde(default = "default_watermark_font_size")]
        font_size: f64,
        /// Rotation angle in degrees (negative = counterclockwise). Default: -45.
        #[serde(default = "default_watermark_angle")]
        angle: f64,
    },

    /// An interactive text input field (PDF AcroForm widget).
    TextField {
        /// Field name, used for data extraction.
        name: String,
        /// Default/current value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// Placeholder text displayed when empty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
        /// Field width in points.
        width: f64,
        /// Field height in points. Default: 24.
        #[serde(default = "default_form_field_height")]
        height: f64,
        /// Allow multiple lines of input.
        #[serde(default)]
        multiline: bool,
        /// Mask input as password dots.
        #[serde(default)]
        password: bool,
        /// Prevent editing.
        #[serde(default)]
        read_only: bool,
        /// Maximum number of characters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<u32>,
        /// Font size in points. Default: 12.
        #[serde(default = "default_form_font_size")]
        font_size: f64,
    },

    /// An interactive checkbox (PDF AcroForm widget).
    Checkbox {
        /// Field name, used for data extraction.
        name: String,
        /// Default checked state.
        #[serde(default)]
        checked: bool,
        /// Checkbox width in points. Default: 14.
        #[serde(default = "default_checkbox_size")]
        width: f64,
        /// Checkbox height in points. Default: 14.
        #[serde(default = "default_checkbox_size")]
        height: f64,
        /// Prevent editing.
        #[serde(default)]
        read_only: bool,
    },

    /// An interactive dropdown/combo box (PDF AcroForm widget).
    Dropdown {
        /// Field name, used for data extraction.
        name: String,
        /// Available options.
        options: Vec<String>,
        /// Default selected value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// Field width in points.
        width: f64,
        /// Field height in points. Default: 24.
        #[serde(default = "default_form_field_height")]
        height: f64,
        /// Prevent editing.
        #[serde(default)]
        read_only: bool,
        /// Font size in points. Default: 12.
        #[serde(default = "default_form_font_size")]
        font_size: f64,
    },

    /// An interactive radio button (PDF AcroForm widget).
    /// Multiple RadioButtons with the same `name` form a mutually exclusive group.
    RadioButton {
        /// Group name shared by all buttons in the group.
        name: String,
        /// This button's export value.
        value: String,
        /// Default selected state.
        #[serde(default)]
        checked: bool,
        /// Button width in points. Default: 14.
        #[serde(default = "default_checkbox_size")]
        width: f64,
        /// Button height in points. Default: 14.
        #[serde(default = "default_checkbox_size")]
        height: f64,
        /// Prevent editing.
        #[serde(default)]
        read_only: bool,
    },
}

/// A data point for bar charts and pie charts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A data series for line charts and area charts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    pub data: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// A group of data points for dot plots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotPlotGroup {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub data: Vec<(f64, f64)>,
}

/// A canvas drawing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum CanvasOp {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    BezierCurveTo {
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    },
    QuadraticCurveTo {
        cpx: f64,
        cpy: f64,
        x: f64,
        y: f64,
    },
    ClosePath,
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
    },
    Arc {
        cx: f64,
        cy: f64,
        r: f64,
        start_angle: f64,
        end_angle: f64,
        #[serde(default)]
        counterclockwise: bool,
    },
    Stroke,
    Fill,
    FillAndStroke,
    SetFillColor {
        r: f64,
        g: f64,
        b: f64,
    },
    SetStrokeColor {
        r: f64,
        g: f64,
        b: f64,
    },
    SetLineWidth {
        width: f64,
    },
    SetLineCap {
        cap: u32,
    },
    SetLineJoin {
        join: u32,
    },
    Save,
    Restore,
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

fn default_barcode_height() -> f64 {
    60.0
}

fn default_dot_size() -> f64 {
    4.0
}

fn default_watermark_font_size() -> f64 {
    60.0
}

fn default_watermark_angle() -> f64 {
    -45.0
}

fn default_form_field_height() -> f64 {
    24.0
}

fn default_form_font_size() -> f64 {
    12.0
}

fn default_checkbox_size() -> f64 {
    14.0
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

/// Marker style for a `List`. Maps to CSS `list-style-type`:
///   - `Disc` / `Circle` / `Square` / `None` for unordered lists
///   - `Decimal` / `LowerAlpha` / `UpperAlpha` / `LowerRoman` / `UpperRoman`
///     for ordered lists
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListMarkerType {
    Disc,
    Circle,
    Square,
    None,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

fn default_list_start() -> u32 {
    1
}

/// Where a fixed element is placed on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixedPosition {
    /// Top of the content area (below margin).
    Header,
    /// Bottom of the content area (above margin).
    Footer,
}

/// Which pages a fixed element repeats on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixedPageFilter {
    /// Every page (the default).
    #[default]
    All,
    /// The first page only.
    First,
    /// Every page except the first.
    NotFirst,
}

impl FixedPageFilter {
    /// Does a fixed element with this filter appear on `page_index`
    /// (0-based)?
    pub fn applies(self, page_index: usize) -> bool {
        match self {
            FixedPageFilter::All => true,
            FixedPageFilter::First => page_index == 0,
            FixedPageFilter::NotFirst => page_index > 0,
        }
    }
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
            NodeKind::View
            | NodeKind::Table { .. }
            | NodeKind::Text { .. }
            | NodeKind::Heading { .. }
            | NodeKind::List { .. }
            | NodeKind::ListItem => self.style.wrap.unwrap_or(true),
            NodeKind::TableRow { .. } => true,
            NodeKind::Image { .. } => false,
            NodeKind::Svg { .. } => false,
            NodeKind::Canvas { .. } => false,
            NodeKind::Barcode { .. } => false,
            NodeKind::QrCode { .. } => false,
            NodeKind::BarChart { .. } => false,
            NodeKind::LineChart { .. } => false,
            NodeKind::PieChart { .. } => false,
            NodeKind::AreaChart { .. } => false,
            NodeKind::DotPlot { .. } => false,
            NodeKind::Watermark { .. } => false,
            NodeKind::TextField { .. } => false,
            NodeKind::Checkbox { .. } => false,
            NodeKind::Dropdown { .. } => false,
            NodeKind::RadioButton { .. } => false,
            NodeKind::PageBreak => false,
            NodeKind::Fixed { .. } => false,
            NodeKind::Page { .. } => true,
            NodeKind::TableCell { .. } => true,
        }
    }
}
