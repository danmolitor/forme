//! # Font Management
//!
//! Loading, parsing, and subsetting fonts for PDF embedding.
//!
//! For v1, we support the 14 standard PDF fonts (Helvetica, Times, Courier, etc.)
//! which don't require embedding. Custom font support via ttf-parser comes next.

pub mod metrics;
pub mod subset;

pub use metrics::StandardFontMetrics;
use std::collections::HashMap;

/// A font registry that maps font family + weight + style to font data.
pub struct FontRegistry {
    fonts: HashMap<FontKey, FontData>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FontKey {
    pub family: String,
    pub weight: u32,
    pub italic: bool,
}

#[derive(Debug, Clone)]
pub enum FontData {
    /// One of the 14 standard PDF fonts. No embedding needed.
    Standard(StandardFont),
    /// A TrueType/OpenType font that needs to be embedded.
    Custom {
        data: Vec<u8>,
        /// Glyph IDs that are actually used (for subsetting).
        used_glyphs: Vec<u16>,
        /// Parsed metrics from ttf-parser, if available.
        metrics: Option<CustomFontMetrics>,
    },
}

/// Parsed metrics from a TrueType/OpenType font via ttf-parser.
#[derive(Debug, Clone)]
pub struct CustomFontMetrics {
    pub units_per_em: u16,
    pub advance_widths: HashMap<char, u16>,
    pub default_advance: u16,
    pub ascender: i16,
    pub descender: i16,
    /// Maps characters to their glyph IDs in the original font.
    pub glyph_ids: HashMap<char, u16>,
}

impl CustomFontMetrics {
    /// Get the advance width of a character in points.
    pub fn char_width(&self, ch: char, font_size: f64) -> f64 {
        let w = self
            .advance_widths
            .get(&ch)
            .copied()
            .unwrap_or(self.default_advance);
        (w as f64 / self.units_per_em as f64) * font_size
    }

    /// Parse metrics from font data using ttf-parser.
    pub fn from_font_data(data: &[u8]) -> Option<Self> {
        let face = ttf_parser::Face::parse(data, 0).ok()?;
        let units_per_em = face.units_per_em();
        let ascender = face.ascender();
        let descender = face.descender();

        let mut advance_widths = HashMap::new();
        let mut glyph_ids = HashMap::new();
        let mut default_advance = 0u16;

        // Sample common characters to build width and glyph ID maps
        for code in 32u32..=0xFFFF {
            if let Some(ch) = char::from_u32(code) {
                if let Some(glyph_id) = face.glyph_index(ch) {
                    let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0);
                    advance_widths.insert(ch, advance);
                    glyph_ids.insert(ch, glyph_id.0);
                    if ch == ' ' {
                        default_advance = advance;
                    }
                }
            }
        }

        if default_advance == 0 {
            default_advance = units_per_em / 2;
        }

        Some(CustomFontMetrics {
            units_per_em,
            advance_widths,
            default_advance,
            ascender,
            descender,
            glyph_ids,
        })
    }
}

/// The 14 standard PDF fonts.
#[derive(Debug, Clone, Copy)]
pub enum StandardFont {
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    TimesRoman,
    TimesBold,
    TimesItalic,
    TimesBoldItalic,
    Courier,
    CourierBold,
    CourierOblique,
    CourierBoldOblique,
    Symbol,
    ZapfDingbats,
}

impl StandardFont {
    /// The PDF name for this font.
    pub fn pdf_name(&self) -> &'static str {
        match self {
            Self::Helvetica => "Helvetica",
            Self::HelveticaBold => "Helvetica-Bold",
            Self::HelveticaOblique => "Helvetica-Oblique",
            Self::HelveticaBoldOblique => "Helvetica-BoldOblique",
            Self::TimesRoman => "Times-Roman",
            Self::TimesBold => "Times-Bold",
            Self::TimesItalic => "Times-Italic",
            Self::TimesBoldItalic => "Times-BoldItalic",
            Self::Courier => "Courier",
            Self::CourierBold => "Courier-Bold",
            Self::CourierOblique => "Courier-Oblique",
            Self::CourierBoldOblique => "Courier-BoldOblique",
            Self::Symbol => "Symbol",
            Self::ZapfDingbats => "ZapfDingbats",
        }
    }
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FontRegistry {
    pub fn new() -> Self {
        let mut fonts = HashMap::new();

        let standard_mappings = vec![
            (("Helvetica", 400, false), StandardFont::Helvetica),
            (("Helvetica", 700, false), StandardFont::HelveticaBold),
            (("Helvetica", 400, true), StandardFont::HelveticaOblique),
            (("Helvetica", 700, true), StandardFont::HelveticaBoldOblique),
            (("Times", 400, false), StandardFont::TimesRoman),
            (("Times", 700, false), StandardFont::TimesBold),
            (("Times", 400, true), StandardFont::TimesItalic),
            (("Times", 700, true), StandardFont::TimesBoldItalic),
            (("Courier", 400, false), StandardFont::Courier),
            (("Courier", 700, false), StandardFont::CourierBold),
            (("Courier", 400, true), StandardFont::CourierOblique),
            (("Courier", 700, true), StandardFont::CourierBoldOblique),
        ];

        for ((family, weight, italic), font) in standard_mappings {
            fonts.insert(
                FontKey {
                    family: family.to_string(),
                    weight,
                    italic,
                },
                FontData::Standard(font),
            );
        }

        Self { fonts }
    }

    /// Look up a font, falling back to Helvetica if not found.
    pub fn resolve(&self, family: &str, weight: u32, italic: bool) -> &FontData {
        let key = FontKey {
            family: family.to_string(),
            weight,
            italic,
        };
        if let Some(font) = self.fonts.get(&key) {
            return font;
        }

        // Try with normalized weight (snap to 400 or 700)
        let snapped_weight = if weight >= 600 { 700 } else { 400 };
        let key = FontKey {
            family: family.to_string(),
            weight: snapped_weight,
            italic,
        };
        if let Some(font) = self.fonts.get(&key) {
            return font;
        }

        // Fallback to Helvetica
        let key = FontKey {
            family: "Helvetica".to_string(),
            weight: snapped_weight,
            italic,
        };
        self.fonts.get(&key).unwrap_or_else(|| {
            self.fonts
                .get(&FontKey {
                    family: "Helvetica".to_string(),
                    weight: 400,
                    italic: false,
                })
                .expect("Helvetica must be registered")
        })
    }

    /// Register a custom font.
    pub fn register(&mut self, family: &str, weight: u32, italic: bool, data: Vec<u8>) {
        let metrics = CustomFontMetrics::from_font_data(&data);
        self.fonts.insert(
            FontKey {
                family: family.to_string(),
                weight,
                italic,
            },
            FontData::Custom {
                data,
                used_glyphs: Vec::new(),
                metrics,
            },
        );
    }

    /// Iterate over all registered fonts.
    pub fn iter(&self) -> impl Iterator<Item = (&FontKey, &FontData)> {
        self.fonts.iter()
    }
}

/// Shared font context used by layout and PDF serialization.
/// Provides text measurement with real glyph metrics.
pub struct FontContext {
    registry: FontRegistry,
}

impl Default for FontContext {
    fn default() -> Self {
        Self::new()
    }
}

impl FontContext {
    pub fn new() -> Self {
        Self {
            registry: FontRegistry::new(),
        }
    }

    /// Get the advance width of a single character in points.
    pub fn char_width(
        &self,
        ch: char,
        family: &str,
        weight: u32,
        italic: bool,
        font_size: f64,
    ) -> f64 {
        let font_data = self.registry.resolve(family, weight, italic);
        match font_data {
            FontData::Standard(std_font) => std_font.metrics().char_width(ch, font_size),
            FontData::Custom {
                metrics: Some(m), ..
            } => m.char_width(ch, font_size),
            FontData::Custom { metrics: None, .. } => {
                StandardFont::Helvetica.metrics().char_width(ch, font_size)
            }
        }
    }

    /// Measure the width of a string in points.
    pub fn measure_string(
        &self,
        text: &str,
        family: &str,
        weight: u32,
        italic: bool,
        font_size: f64,
        letter_spacing: f64,
    ) -> f64 {
        let font_data = self.registry.resolve(family, weight, italic);
        match font_data {
            FontData::Standard(std_font) => {
                std_font
                    .metrics()
                    .measure_string(text, font_size, letter_spacing)
            }
            FontData::Custom {
                metrics: Some(m), ..
            } => {
                let mut width = 0.0;
                for ch in text.chars() {
                    width += m.char_width(ch, font_size) + letter_spacing;
                }
                width
            }
            FontData::Custom { metrics: None, .. } => StandardFont::Helvetica
                .metrics()
                .measure_string(text, font_size, letter_spacing),
        }
    }

    /// Resolve a font key to its font data.
    pub fn resolve(&self, family: &str, weight: u32, italic: bool) -> &FontData {
        self.registry.resolve(family, weight, italic)
    }

    /// Access the underlying font registry.
    pub fn registry(&self) -> &FontRegistry {
        &self.registry
    }

    /// Access the underlying font registry mutably.
    pub fn registry_mut(&mut self) -> &mut FontRegistry {
        &mut self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_context_helvetica() {
        let ctx = FontContext::new();
        let w = ctx.char_width(' ', "Helvetica", 400, false, 12.0);
        assert!((w - 3.336).abs() < 0.001);
    }

    #[test]
    fn test_font_context_bold_wider() {
        let ctx = FontContext::new();
        let regular = ctx.char_width('A', "Helvetica", 400, false, 12.0);
        let bold = ctx.char_width('A', "Helvetica", 700, false, 12.0);
        assert!(bold > regular, "Bold A should be wider than regular A");
    }

    #[test]
    fn test_font_context_measure_string() {
        let ctx = FontContext::new();
        let w = ctx.measure_string("Hello", "Helvetica", 400, false, 12.0, 0.0);
        assert!(w > 0.0);
    }

    #[test]
    fn test_font_context_fallback() {
        let ctx = FontContext::new();
        let w1 = ctx.char_width('A', "Helvetica", 400, false, 12.0);
        let w2 = ctx.char_width('A', "UnknownFont", 400, false, 12.0);
        assert!((w1 - w2).abs() < 0.001);
    }

    #[test]
    fn test_font_context_weight_resolution() {
        let ctx = FontContext::new();
        let w700 = ctx.char_width('A', "Helvetica", 700, false, 12.0);
        let w800 = ctx.char_width('A', "Helvetica", 800, false, 12.0);
        assert!((w700 - w800).abs() < 0.001);
    }
}
