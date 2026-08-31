//! # Standard Font Metrics
//!
//! Glyph advance widths for the 14 standard PDF fonts.
//! Data sourced from Adobe AFM files with correct WinAnsiEncoding order.
//! Widths are in units of 1/1000 em (the standard AFM unit).
//!
//! Each array covers WinAnsiEncoding code points 32..=255 (224 entries).
//! Index 0 = code 32 (space), index 223 = code 255 (ydieresis).

use super::StandardFont;

/// Map a Unicode codepoint to a WinAnsiEncoding byte value.
///
/// WinAnsiEncoding is based on Windows-1252. Most codepoints in
/// 0x20..=0x7E and 0xA0..=0xFF map directly. The 0x80..=0x9F range
/// contains special mappings for smart quotes, bullets, dashes, etc.
pub fn unicode_to_winansi(ch: char) -> Option<u8> {
    let cp = ch as u32;
    // ASCII printable range maps directly
    if (0x20..=0x7E).contains(&cp) || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    // Windows-1252 special mappings (0x80-0x9F)
    match cp {
        0x20AC => Some(0x80), // Euro sign
        0x201A => Some(0x82), // Single low-9 quotation mark
        0x0192 => Some(0x83), // Latin small letter f with hook
        0x201E => Some(0x84), // Double low-9 quotation mark
        0x2026 => Some(0x85), // Horizontal ellipsis
        0x2020 => Some(0x86), // Dagger
        0x2021 => Some(0x87), // Double dagger
        0x02C6 => Some(0x88), // Modifier letter circumflex accent
        0x2030 => Some(0x89), // Per mille sign
        0x0160 => Some(0x8A), // Latin capital letter S with caron
        0x2039 => Some(0x8B), // Single left-pointing angle quotation
        0x0152 => Some(0x8C), // Latin capital ligature OE
        0x017D => Some(0x8E), // Latin capital letter Z with caron
        0x2018 => Some(0x91), // Left single quotation mark
        0x2019 => Some(0x92), // Right single quotation mark
        0x201C => Some(0x93), // Left double quotation mark
        0x201D => Some(0x94), // Right double quotation mark
        0x2022 => Some(0x95), // Bullet
        0x2013 => Some(0x96), // En dash
        0x2014 => Some(0x97), // Em dash
        0x02DC => Some(0x98), // Small tilde
        0x2122 => Some(0x99), // Trade mark sign
        0x0161 => Some(0x9A), // Latin small letter s with caron
        0x203A => Some(0x9B), // Single right-pointing angle quotation
        0x0153 => Some(0x9C), // Latin small ligature oe
        0x017E => Some(0x9E), // Latin small letter z with caron
        0x0178 => Some(0x9F), // Latin capital letter Y with diaeresis
        _ => None,
    }
}

/// Reverse of `unicode_to_winansi`: the Unicode scalar a WinAnsiEncoding code
/// maps to. Used when embedding a metric-compatible substitute to look up the
/// substitute's own advance for a code (the PDF/A width carve-out).
pub fn winansi_to_char(code: u8) -> Option<char> {
    let cp = code as u32;
    if (0x20..=0x7E).contains(&cp) || (0xA0..=0xFF).contains(&cp) {
        return char::from_u32(cp);
    }
    let uni: u32 = match code {
        0x80 => 0x20AC,
        0x82 => 0x201A,
        0x83 => 0x0192,
        0x84 => 0x201E,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02C6,
        0x89 => 0x2030,
        0x8A => 0x0160,
        0x8B => 0x2039,
        0x8C => 0x0152,
        0x8E => 0x017D,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201C,
        0x94 => 0x201D,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02DC,
        0x99 => 0x2122,
        0x9A => 0x0161,
        0x9B => 0x203A,
        0x9C => 0x0153,
        0x9E => 0x017E,
        0x9F => 0x0178,
        _ => return None,
    };
    char::from_u32(uni)
}

/// Glyph widths for a standard PDF font.
/// Indexed by WinAnsiEncoding code point: index 0 = code 32, index 223 = code 255.
/// Values are advance widths in units of 1/1000 em.
pub struct StandardFontMetrics {
    pub widths: &'static [u16; 224],
    pub default_width: u16,
}

impl StandardFontMetrics {
    /// Get the advance width of a character in points.
    pub fn char_width(&self, ch: char, font_size: f64) -> f64 {
        let code = ch as u32;
        let w = if (32..=255).contains(&code) {
            let idx = (code - 32) as usize;
            let w = self.widths[idx];
            if w > 0 {
                w
            } else {
                self.default_width
            }
        } else if let Some(winansi) = unicode_to_winansi(ch) {
            if winansi >= 32 {
                let idx = (winansi as u32 - 32) as usize;
                let w = self.widths[idx];
                if w > 0 {
                    w
                } else {
                    self.default_width
                }
            } else {
                self.default_width
            }
        } else {
            self.default_width
        };
        (w as f64 / 1000.0) * font_size
    }

    /// Measure the width of a string in points.
    pub fn measure_string(&self, text: &str, font_size: f64, letter_spacing: f64) -> f64 {
        let mut width = 0.0;
        for ch in text.chars() {
            width += self.char_width(ch, font_size) + letter_spacing;
        }
        width
    }
}

impl StandardFont {
    /// Get the metrics for this standard font.
    pub fn metrics(&self) -> StandardFontMetrics {
        match self {
            Self::Helvetica => StandardFontMetrics {
                widths: &HELVETICA_WIDTHS,
                default_width: 278,
            },
            Self::HelveticaBold => StandardFontMetrics {
                widths: &HELVETICA_BOLD_WIDTHS,
                default_width: 278,
            },
            Self::HelveticaOblique => StandardFontMetrics {
                widths: &HELVETICA_OBLIQUE_WIDTHS,
                default_width: 278,
            },
            Self::HelveticaBoldOblique => StandardFontMetrics {
                widths: &HELVETICA_BOLD_OBLIQUE_WIDTHS,
                default_width: 278,
            },
            Self::TimesRoman => StandardFontMetrics {
                widths: &TIMES_ROMAN_WIDTHS,
                default_width: 250,
            },
            Self::TimesBold => StandardFontMetrics {
                widths: &TIMES_BOLD_WIDTHS,
                default_width: 250,
            },
            Self::TimesItalic => StandardFontMetrics {
                widths: &TIMES_ITALIC_WIDTHS,
                default_width: 250,
            },
            Self::TimesBoldItalic => StandardFontMetrics {
                widths: &TIMES_BOLD_ITALIC_WIDTHS,
                default_width: 250,
            },
            Self::Courier => StandardFontMetrics {
                widths: &COURIER_WIDTHS,
                default_width: 600,
            },
            Self::CourierBold => StandardFontMetrics {
                widths: &COURIER_BOLD_WIDTHS,
                default_width: 600,
            },
            Self::CourierOblique => StandardFontMetrics {
                widths: &COURIER_OBLIQUE_WIDTHS,
                default_width: 600,
            },
            Self::CourierBoldOblique => StandardFontMetrics {
                widths: &COURIER_BOLD_OBLIQUE_WIDTHS,
                default_width: 600,
            },
            Self::Symbol => StandardFontMetrics {
                widths: &SYMBOL_WIDTHS,
                default_width: 250,
            },
            Self::ZapfDingbats => StandardFontMetrics {
                widths: &ZAPF_DINGBATS_WIDTHS,
                default_width: 278,
            },
        }
    }
}

// ─── Helvetica ───────────────────────────────────────────────────
// AFM: Helvetica (Adobe Standard), WinAnsiEncoding order

static HELVETICA_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    278, 278, 355, 556, 556, 889, 667, 222, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 222, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 222, 222, 333,
    333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556,
    278, 278, 278, 278, 556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556,
    500,
];

static HELVETICA_BOLD_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    278, 333, 474, 556, 556, 889, 722, 278, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 278, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    556, 0, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 278, 278, 500,
    500, 350, 556, 1000, 333, 1000, 556, 333, 944, 0, 500, 667,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    278, 333, 556, 556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 611, 556, 278, 333, 333, 365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556,
    278, 278, 278, 278, 611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611,
    556,
];

// Helvetica-Oblique has identical widths to Helvetica
static HELVETICA_OBLIQUE_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    278, 278, 355, 556, 556, 889, 667, 222, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 222, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 222, 222, 333,
    333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556,
    278, 278, 278, 278, 556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556,
    500,
];

// Helvetica-BoldOblique has identical widths to Helvetica-Bold
static HELVETICA_BOLD_OBLIQUE_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    278, 333, 474, 556, 556, 889, 722, 278, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 278, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    556, 0, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 278, 278, 500,
    500, 350, 556, 1000, 333, 1000, 556, 333, 944, 0, 500, 667,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    278, 333, 556, 556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 611, 556, 278, 333, 333, 365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556,
    278, 278, 278, 278, 611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611,
    556,
];

// ─── Times ───────────────────────────────────────────────────────

static TIMES_ROMAN_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    250, 333, 408, 500, 500, 833, 778, 333, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611,
    556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722,
    722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500,
    278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    500, 0, 333, 500, 444, 1000, 500, 500, 333, 1000, 556, 333, 889, 0, 611, 0, 0, 333, 333, 444,
    444, 350, 500, 1000, 333, 980, 389, 333, 722, 0, 444, 722,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    250, 333, 500, 500, 500, 500, 200, 500, 333, 760, 276, 500, 564, 333, 760, 333, 400, 564, 300,
    300, 333, 500, 453, 250, 333, 300, 310, 500, 750, 750, 750, 444, 722, 722, 722, 722, 722, 722,
    889, 667, 611, 611, 611, 611, 333, 333, 333, 333, 722, 722, 722, 722, 722, 722, 722, 564, 722,
    722, 722, 722, 722, 722, 556, 500, 444, 444, 444, 444, 444, 444, 667, 444, 444, 444, 444, 444,
    278, 278, 278, 278, 500, 500, 500, 500, 500, 500, 500, 564, 500, 500, 500, 500, 500, 500, 500,
    500,
];

static TIMES_BOLD_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    250, 333, 555, 500, 500, 1000, 833, 333, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 930, 722, 667, 722, 722, 667,
    611, 778, 778, 389, 500, 778, 667, 944, 722, 778, 611, 778, 722, 556, 667, 722, 722, 1000, 722,
    722, 667, 333, 278, 333, 581, 500, 333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556,
    278, 833, 556, 500, 556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 1000, 0, 667, 0, 0, 333, 333, 500,
    500, 350, 500, 1000, 333, 1000, 389, 333, 722, 0, 444, 722,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    250, 333, 500, 500, 500, 500, 220, 500, 333, 747, 300, 500, 570, 333, 747, 333, 400, 570, 300,
    300, 333, 556, 540, 250, 333, 300, 330, 500, 750, 750, 750, 500, 722, 722, 722, 722, 722, 722,
    1000, 722, 667, 667, 667, 667, 389, 389, 389, 389, 722, 722, 778, 778, 778, 778, 778, 570, 778,
    722, 722, 722, 722, 722, 611, 556, 500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444,
    278, 278, 278, 278, 500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 500, 556,
    500,
];

static TIMES_ITALIC_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    250, 333, 420, 500, 500, 833, 778, 333, 333, 333, 500, 675, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500, 920, 611, 611, 667, 722, 611,
    611, 722, 722, 333, 444, 667, 556, 833, 667, 722, 611, 722, 611, 500, 556, 722, 611, 833, 611,
    556, 556, 389, 278, 389, 422, 500, 333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444,
    278, 722, 500, 500, 500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    500, 0, 333, 500, 556, 889, 500, 500, 333, 1000, 500, 333, 944, 0, 556, 0, 0, 333, 333, 556,
    556, 350, 500, 889, 333, 980, 389, 333, 667, 0, 389, 556,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    250, 389, 500, 500, 500, 500, 275, 500, 333, 760, 276, 500, 675, 333, 760, 333, 400, 675, 300,
    300, 333, 500, 523, 250, 333, 300, 310, 500, 750, 750, 750, 500, 611, 611, 611, 611, 611, 611,
    889, 667, 611, 611, 611, 611, 333, 333, 333, 333, 722, 667, 722, 722, 722, 722, 722, 675, 722,
    722, 722, 722, 722, 556, 611, 500, 500, 500, 500, 500, 500, 500, 667, 444, 444, 444, 444, 444,
    278, 278, 278, 278, 500, 500, 500, 500, 500, 500, 500, 675, 500, 500, 500, 500, 500, 444, 500,
    444,
];

static TIMES_BOLD_ITALIC_WIDTHS: [u16; 224] = [
    // 32-126: ASCII
    250, 389, 555, 500, 500, 833, 778, 333, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 832, 667, 667, 667, 722, 667,
    667, 722, 778, 389, 500, 667, 611, 889, 722, 722, 611, 722, 667, 556, 611, 722, 667, 889, 667,
    611, 611, 333, 278, 333, 570, 500, 333, 500, 500, 444, 500, 444, 333, 500, 556, 278, 278, 500,
    278, 778, 556, 500, 500, 500, 389, 389, 278, 556, 444, 667, 500, 444, 389, 348, 220, 348, 570,
    // 127: DEL
    0, // 128-159: WinAnsi specials (0x80-0x9F)
    500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 944, 0, 611, 0, 0, 333, 333, 500,
    500, 350, 500, 1000, 333, 1000, 389, 333, 722, 0, 389, 611,
    // 160-255: Latin-1 Supplement (0xA0-0xFF) — WinAnsi order from Adobe AFM
    250, 389, 500, 500, 500, 500, 220, 500, 333, 747, 266, 500, 570, 333, 747, 333, 400, 570, 300,
    300, 333, 576, 500, 250, 333, 300, 300, 500, 750, 750, 750, 500, 667, 667, 667, 667, 667, 667,
    944, 667, 667, 667, 667, 667, 389, 389, 389, 389, 722, 722, 722, 722, 722, 722, 722, 570, 722,
    722, 722, 722, 722, 611, 611, 500, 500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444,
    278, 278, 278, 278, 500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 444, 500,
    444,
];

// ─── Courier (monospaced — all 600) ─────────────────────────────

static COURIER_WIDTHS: [u16; 224] = [
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    0, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 0, 0, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
];

static COURIER_BOLD_WIDTHS: [u16; 224] = [
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    0, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 0, 0, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
];

static COURIER_OBLIQUE_WIDTHS: [u16; 224] = [
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    0, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 0, 0, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
];

static COURIER_BOLD_OBLIQUE_WIDTHS: [u16; 224] = [
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    0, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 0, 0, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 0, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
];

// ─── Symbol & ZapfDingbats (own encodings, unaffected) ──────────

static SYMBOL_WIDTHS: [u16; 224] = [
    250, 333, 713, 500, 549, 833, 778, 439, 333, 333, 500, 549, 250, 549, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 549, 549, 549, 444, 549, 722, 667, 722, 612, 611,
    763, 603, 722, 333, 631, 722, 686, 889, 722, 722, 768, 741, 556, 592, 611, 690, 439, 768, 645,
    795, 611, 333, 863, 333, 658, 500, 500, 631, 549, 549, 494, 439, 521, 411, 603, 329, 603, 549,
    549, 576, 521, 549, 549, 521, 549, 603, 439, 576, 713, 686, 493, 686, 494, 480, 200, 480, 549,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 750, 620, 247, 549, 167, 713, 500, 753, 753, 753, 753, 1042, 987, 603, 987, 603, 400, 549,
    411, 549, 549, 713, 494, 460, 549, 549, 549, 549, 1000, 603, 1000, 658, 823, 686, 795, 987,
    768, 768, 823, 768, 768, 713, 713, 713, 713, 713, 713, 713, 768, 713, 790, 790, 890, 823, 549,
    250, 713, 603, 603, 1042, 987, 603, 987, 603, 494, 329, 790, 790, 786, 713, 384, 384, 384, 384,
    384, 384, 494, 494, 494, 494, 0, 329, 274, 686, 686, 686, 384, 384, 384, 384, 384, 384, 494,
    494, 494, 0,
];

static ZAPF_DINGBATS_WIDTHS: [u16; 224] = [
    278, 974, 961, 974, 980, 719, 789, 790, 791, 690, 960, 939, 549, 855, 911, 933, 911, 945, 974,
    755, 846, 762, 761, 571, 677, 763, 760, 759, 754, 494, 552, 537, 577, 692, 786, 788, 788, 790,
    793, 794, 816, 823, 789, 841, 823, 833, 816, 831, 923, 744, 723, 749, 790, 792, 695, 776, 768,
    792, 759, 707, 708, 682, 701, 826, 815, 789, 789, 707, 687, 696, 689, 786, 787, 713, 791, 785,
    791, 873, 761, 762, 762, 759, 759, 892, 892, 788, 784, 438, 138, 277, 415, 392, 392, 668, 668,
    0, 390, 390, 317, 317, 276, 276, 509, 509, 410, 410, 234, 234, 334, 334, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 732, 544, 544, 910, 667, 760, 760, 776, 595, 694, 626, 788,
    788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788,
    788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788,
    788, 894, 838, 1016, 458, 748, 924, 748, 918, 927, 928, 928, 834, 873, 828, 924, 924, 917, 930,
    931, 463, 883, 836, 836, 867, 867, 696, 696, 874, 0, 874, 760, 946, 771, 865, 771, 888, 967,
    888, 831, 873, 927, 970, 918, 0,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helvetica_known_widths() {
        let m = StandardFont::Helvetica.metrics();
        // space = 278, A = 667, m = 833
        assert_eq!(m.widths[0], 278); // space (code 32)
        assert_eq!(m.widths[33], 667); // A (code 65)
        assert_eq!(m.widths[77], 833); // m (code 109)
    }

    #[test]
    fn test_helvetica_bold_wider() {
        let regular = StandardFont::Helvetica.metrics();
        let bold = StandardFont::HelveticaBold.metrics();
        // Bold A = 722 > regular A = 667
        assert_eq!(bold.widths[33], 722);
        assert!(bold.widths[33] > regular.widths[33]);
    }

    #[test]
    fn test_courier_monospaced() {
        let m = StandardFont::Courier.metrics();
        // All Courier glyphs are 600
        for &w in m.widths.iter() {
            if w > 0 {
                assert_eq!(w, 600, "Courier should be monospaced");
            }
        }
    }

    #[test]
    fn test_char_width_calculation() {
        let m = StandardFont::Helvetica.metrics();
        // space at 12pt: 278/1000 * 12 = 3.336
        let w = m.char_width(' ', 12.0);
        assert!((w - 3.336).abs() < 0.001);
    }

    #[test]
    fn test_measure_string() {
        let m = StandardFont::Helvetica.metrics();
        let w = m.measure_string("A", 12.0, 0.0);
        assert!((w - 8.004).abs() < 0.001); // 667/1000 * 12
    }

    #[test]
    fn test_char_width_em_dash_uses_winansi() {
        let m = StandardFont::Helvetica.metrics();
        // Em dash (U+2014) maps to WinAnsi 0x97 = 151, index 151-32 = 119
        // Helvetica em dash width = 1000
        let w = m.char_width('\u{2014}', 10.0);
        assert!(
            (w - 10.0).abs() < 0.001,
            "em dash should be 1000/1000 * 10 = 10.0, got {}",
            w
        );
    }

    #[test]
    fn test_char_width_en_dash_uses_winansi() {
        let m = StandardFont::Helvetica.metrics();
        // En dash (U+2013) maps to WinAnsi 0x96 = 150, index 150-32 = 118
        // Helvetica en dash width = 556
        let w = m.char_width('\u{2013}', 10.0);
        assert!(
            (w - 5.56).abs() < 0.001,
            "en dash should be 556/1000 * 10 = 5.56, got {}",
            w
        );
    }

    #[test]
    fn test_unicode_to_winansi_mappings() {
        assert_eq!(unicode_to_winansi('\u{2014}'), Some(0x97)); // Em dash
        assert_eq!(unicode_to_winansi('\u{2013}'), Some(0x96)); // En dash
        assert_eq!(unicode_to_winansi('\u{201C}'), Some(0x93)); // Left double quote
        assert_eq!(unicode_to_winansi('\u{2026}'), Some(0x85)); // Ellipsis
        assert_eq!(unicode_to_winansi('A'), Some(0x41)); // ASCII
        assert_eq!(unicode_to_winansi('\u{4E00}'), None); // CJK — not in WinAnsi
    }

    #[test]
    fn test_latin_extended_character_widths() {
        let m = StandardFont::Helvetica.metrics();
        // Accented capitals must have the same width as their base character
        let a_width = m.widths[33]; // A = 667
        assert_eq!(m.widths[0xC0 - 32], a_width, "Agrave should match A"); // À
        assert_eq!(m.widths[0xC1 - 32], a_width, "Aacute should match A"); // Á
        assert_eq!(m.widths[0xC2 - 32], a_width, "Acircumflex should match A"); // Â
        assert_eq!(m.widths[0xC3 - 32], a_width, "Atilde should match A"); // Ã
        assert_eq!(m.widths[0xC4 - 32], a_width, "Adieresis should match A"); // Ä
        assert_eq!(m.widths[0xC5 - 32], a_width, "Aring should match A"); // Å

        // Verify char_width gives correct results for Latin Extended
        let w_a_dieresis = m.char_width('Ä', 10.0);
        let w_a = m.char_width('A', 10.0);
        assert!(
            (w_a_dieresis - w_a).abs() < 0.001,
            "Ä width ({}) should equal A width ({})",
            w_a_dieresis,
            w_a
        );

        let w_a_ring = m.char_width('Å', 10.0);
        assert!(
            (w_a_ring - w_a).abs() < 0.001,
            "Å width ({}) should equal A width ({})",
            w_a_ring,
            w_a
        );

        // O-dieresis should match O
        let o_width = m.widths[0x4F - 32]; // O = 778
        assert_eq!(m.widths[0xD6 - 32], o_width, "Odieresis should match O");

        // AE should be 1000
        assert_eq!(m.widths[0xC6 - 32], 1000, "AE should be 1000");

        // Verify sequential characters don't stack (each x position increases)
        let test_str = "ÅÄÖÉÈÊÑÜÚÙû";
        let mut prev_x = 0.0;
        for (i, ch) in test_str.chars().enumerate() {
            let w = m.char_width(ch, 12.0);
            assert!(
                w > 1.0,
                "Character '{}' (U+{:04X}) has suspiciously small width: {}",
                ch,
                ch as u32,
                w
            );
            if i > 0 {
                assert!(
                    prev_x > 0.0,
                    "Previous character should have non-zero width"
                );
            }
            prev_x += w;
        }
        // Total width should be reasonable (not near-zero from stacking)
        assert!(
            prev_x > 50.0,
            "Total width of '{}' at 12pt should be >50pt, got {}",
            test_str,
            prev_x
        );
    }

    #[test]
    fn test_latin_extended_widths_all_fonts() {
        // Verify accented characters have correct widths across all proportional fonts
        let fonts = [
            StandardFont::Helvetica,
            StandardFont::HelveticaBold,
            StandardFont::HelveticaOblique,
            StandardFont::HelveticaBoldOblique,
            StandardFont::TimesRoman,
            StandardFont::TimesBold,
            StandardFont::TimesItalic,
            StandardFont::TimesBoldItalic,
        ];

        for font in &fonts {
            let m = font.metrics();
            // Ä (0xC4) should have same width as A (0x41)
            let a_width = m.widths[0x41 - 32];
            let a_dieresis_width = m.widths[0xC4 - 32];
            assert_eq!(
                a_dieresis_width, a_width,
                "{:?}: Ä width ({}) != A width ({})",
                font, a_dieresis_width, a_width
            );

            // Å (0xC5) should have same width as A (0x41)
            let a_ring_width = m.widths[0xC5 - 32];
            assert_eq!(
                a_ring_width, a_width,
                "{:?}: Å width ({}) != A width ({})",
                font, a_ring_width, a_width
            );

            // ö (0xF6) should have same width as o (0x6F)
            let o_width = m.widths[0x6F - 32];
            let o_dieresis_width = m.widths[0xF6 - 32];
            assert_eq!(
                o_dieresis_width, o_width,
                "{:?}: ö width ({}) != o width ({})",
                font, o_dieresis_width, o_width
            );
        }
    }

    #[test]
    fn test_page_placeholder_width_difference() {
        let m = StandardFont::Helvetica.metrics();
        let literal = m.measure_string("{{pageNumber}}", 12.0, 0.0);
        let substituted = m.measure_string("00", 12.0, 0.0);
        // "{{pageNumber}}" is 16 chars, "00" is 2 chars — massive difference
        assert!(
            literal > substituted * 3.0,
            "Literal placeholder ({}) is much wider than substituted ({})",
            literal,
            substituted
        );
    }
}
