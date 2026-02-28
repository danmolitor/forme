//! # OpenType Shaping
//!
//! Wraps rustybuzz to perform OpenType shaping (GSUB/GPOS) on text.
//! This produces real glyph IDs, kerning offsets, and ligature substitutions
//! instead of the naive char-as-u16 glyph IDs used before.
//!
//! Standard PDF fonts (Helvetica, Times, Courier) bypass shaping entirely —
//! they use WinAnsi encoding and don't have GSUB/GPOS tables.

/// A single glyph produced by OpenType shaping.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// Real glyph ID from GSUB (not a Unicode codepoint).
    pub glyph_id: u16,
    /// Index of the first character in the input text that maps to this glyph.
    /// Multiple chars may map to one glyph (ligatures), or one char may
    /// produce multiple glyphs (decomposition).
    pub cluster: u32,
    /// Horizontal advance in font units.
    pub x_advance: i32,
    /// Vertical advance in font units (usually 0 for horizontal text).
    pub y_advance: i32,
    /// Horizontal offset from GPOS (kerning, mark positioning).
    pub x_offset: i32,
    /// Vertical offset from GPOS.
    pub y_offset: i32,
}

/// Shape text using the given font data.
///
/// Returns `None` if the font data can't be parsed. For standard fonts
/// (no font data), callers should skip shaping entirely.
pub fn shape_text(text: &str, font_data: &[u8]) -> Option<Vec<ShapedGlyph>> {
    shape_text_with_direction(text, font_data, false)
}

/// Shape text with explicit direction control.
///
/// When `is_rtl` is true, the shaper applies RTL contextual forms (e.g.,
/// Arabic initial/medial/final forms) and produces glyphs in visual order.
pub fn shape_text_with_direction(
    text: &str,
    font_data: &[u8],
    is_rtl: bool,
) -> Option<Vec<ShapedGlyph>> {
    let face = rustybuzz::Face::from_slice(font_data, 0)?;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    if is_rtl {
        buffer.set_direction(rustybuzz::Direction::RightToLeft);
    }

    let output = rustybuzz::shape(&face, &[], buffer);

    let infos = output.glyph_infos();
    let positions = output.glyph_positions();

    let glyphs = infos
        .iter()
        .zip(positions.iter())
        .map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            cluster: info.cluster,
            x_advance: pos.x_advance,
            y_advance: pos.y_advance,
            x_offset: pos.x_offset,
            y_offset: pos.y_offset,
        })
        .collect();

    Some(glyphs)
}

/// Shape a segment of text for a specific font, returning shaped glyphs
/// and a mapping from glyph index to character range.
///
/// `char_offset` is added to cluster values so they reference positions
/// in a larger text buffer (useful for multi-run shaping).
pub fn shape_text_with_offset(
    text: &str,
    font_data: &[u8],
    char_offset: u32,
) -> Option<Vec<ShapedGlyph>> {
    let mut glyphs = shape_text(text, font_data)?;
    for g in &mut glyphs {
        g.cluster += char_offset;
    }
    Some(glyphs)
}

/// Compute the total advance width of shaped glyphs in points.
pub fn shaped_width(glyphs: &[ShapedGlyph], units_per_em: u16, font_size: f64) -> f64 {
    let scale = font_size / units_per_em as f64;
    glyphs.iter().map(|g| g.x_advance as f64 * scale).sum()
}

/// Compute per-cluster widths from shaped glyphs.
///
/// Returns a Vec where index i is the width contributed by the glyph(s)
/// whose cluster value is i. This accounts for ligatures (one glyph for
/// multiple chars) and decomposition (multiple glyphs for one char).
///
/// `num_chars` is the total number of characters in the input text.
pub fn cluster_widths(
    glyphs: &[ShapedGlyph],
    num_chars: usize,
    units_per_em: u16,
    font_size: f64,
    letter_spacing: f64,
) -> Vec<f64> {
    let scale = font_size / units_per_em as f64;
    let mut widths = vec![0.0_f64; num_chars];

    for glyph in glyphs {
        let cluster = glyph.cluster as usize;
        if cluster < num_chars {
            widths[cluster] += glyph.x_advance as f64 * scale + letter_spacing;
        }
    }

    // For ligatures, the first char of the cluster gets the full width,
    // and subsequent chars in the same cluster get zero width.
    // We need to identify cluster ranges and zero out non-first chars.
    if !glyphs.is_empty() {
        let mut cluster_starts: Vec<bool> = vec![false; num_chars];
        for glyph in glyphs {
            let c = glyph.cluster as usize;
            if c < num_chars {
                cluster_starts[c] = true;
            }
        }

        // Walk through chars: if a char's cluster_starts is false and the
        // previous char's cluster value would encompass it (ligature), its
        // width should be 0 (the first char of the cluster already has it).
        // We detect this by checking: chars not in cluster_starts that follow
        // a cluster_start should have width 0.
        let mut in_ligature = false;
        for i in 0..num_chars {
            if cluster_starts[i] {
                in_ligature = false;
            } else if i > 0 {
                // This char wasn't the start of any glyph cluster.
                // It's part of a ligature with the previous cluster.
                in_ligature = true;
            }
            if in_ligature {
                widths[i] = 0.0;
            }
        }
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a minimal test font (we can't easily create one in tests,
    // so we test the public functions with None returns for invalid data)
    #[test]
    fn test_shape_text_invalid_font() {
        let result = shape_text("Hello", &[0, 1, 2, 3]);
        assert!(result.is_none());
    }

    #[test]
    fn test_shape_text_empty() {
        let result = shape_text("", &[0, 1, 2, 3]);
        assert!(result.is_none());
    }

    #[test]
    fn test_shaped_width_empty() {
        let width = shaped_width(&[], 1000, 12.0);
        assert_eq!(width, 0.0);
    }

    #[test]
    fn test_cluster_widths_empty() {
        let widths = cluster_widths(&[], 0, 1000, 12.0, 0.0);
        assert!(widths.is_empty());
    }

    #[test]
    fn test_cluster_widths_basic() {
        // Simulate 3 glyphs for 3 chars, each in its own cluster
        let glyphs = vec![
            ShapedGlyph {
                glyph_id: 1,
                cluster: 0,
                x_advance: 500,
                y_advance: 0,
                x_offset: 0,
                y_offset: 0,
            },
            ShapedGlyph {
                glyph_id: 2,
                cluster: 1,
                x_advance: 600,
                y_advance: 0,
                x_offset: 0,
                y_offset: 0,
            },
            ShapedGlyph {
                glyph_id: 3,
                cluster: 2,
                x_advance: 500,
                y_advance: 0,
                x_offset: 0,
                y_offset: 0,
            },
        ];
        let widths = cluster_widths(&glyphs, 3, 1000, 10.0, 0.0);
        assert_eq!(widths.len(), 3);
        assert!((widths[0] - 5.0).abs() < 0.001); // 500/1000 * 10
        assert!((widths[1] - 6.0).abs() < 0.001); // 600/1000 * 10
        assert!((widths[2] - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_cluster_widths_ligature() {
        // Simulate a ligature: 1 glyph for 2 chars (cluster 0 covers chars 0 and 1)
        let glyphs = vec![
            ShapedGlyph {
                glyph_id: 100,
                cluster: 0,
                x_advance: 800,
                y_advance: 0,
                x_offset: 0,
                y_offset: 0,
            },
            ShapedGlyph {
                glyph_id: 3,
                cluster: 2,
                x_advance: 500,
                y_advance: 0,
                x_offset: 0,
                y_offset: 0,
            },
        ];
        let widths = cluster_widths(&glyphs, 3, 1000, 10.0, 0.0);
        assert_eq!(widths.len(), 3);
        assert!((widths[0] - 8.0).abs() < 0.001); // Ligature gets full width
        assert!((widths[1] - 0.0).abs() < 0.001); // Second char of ligature = 0
        assert!((widths[2] - 5.0).abs() < 0.001);
    }
}
