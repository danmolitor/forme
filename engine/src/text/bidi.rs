//! # BiDi Text Support
//!
//! Implements UAX#9 (Unicode Bidirectional Algorithm) for mixed LTR/RTL text.
//! Uses `unicode-bidi` for analysis and `unicode-script` for script detection.
//!
//! The pipeline:
//! 1. Analyze paragraph direction → split into directional runs
//! 2. Each run is shaped independently with the correct direction
//! 3. After line breaking, runs on each line are visually reordered
//! 4. Glyphs within RTL runs are reversed

use crate::layout::PositionedGlyph;
use crate::style::Direction;
use unicode_bidi::{BidiInfo, Level};

/// A contiguous run of text with a single BiDi direction.
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// Start index in chars of the original text.
    pub char_start: usize,
    /// End index (exclusive) in chars.
    pub char_end: usize,
    /// BiDi embedding level (even = LTR, odd = RTL).
    pub level: Level,
    /// Convenience: true if this run is right-to-left.
    pub is_rtl: bool,
}

/// Analyze text for BiDi runs using the Unicode Bidirectional Algorithm.
///
/// Returns a list of `BidiRun` covering the entire text. For pure LTR text,
/// returns a single run. `direction` controls the paragraph-level direction:
/// - `Ltr` → paragraph is LTR
/// - `Rtl` → paragraph is RTL
/// - `Auto` → detect from first strong character
pub fn analyze_bidi(text: &str, direction: Direction) -> Vec<BidiRun> {
    if text.is_empty() {
        return vec![];
    }

    let para_level = match direction {
        Direction::Ltr => Some(Level::ltr()),
        Direction::Rtl => Some(Level::rtl()),
        Direction::Auto => None, // BidiInfo will auto-detect
    };

    let bidi_info = BidiInfo::new(text, para_level);

    // BidiInfo may contain multiple paragraphs (split by \n), but for our
    // text layout each paragraph is already a separate text node. We only
    // process the first paragraph.
    if bidi_info.paragraphs.is_empty() {
        return vec![];
    }

    let paragraph = &bidi_info.paragraphs[0];
    let levels = &bidi_info.levels;

    // Build runs from contiguous same-level chars
    let chars: Vec<char> = text.chars().collect();
    let mut runs = Vec::new();
    let mut run_start = 0;

    // The levels array is indexed by byte position in the paragraph range
    let para_start = paragraph.range.start;
    let para_end = paragraph.range.end;

    // Extract per-char levels
    let mut char_levels = Vec::with_capacity(chars.len());
    for (byte_idx, _ch) in text.char_indices() {
        if byte_idx >= para_start && byte_idx < para_end {
            char_levels.push(levels[byte_idx]);
        }
    }

    if char_levels.is_empty() {
        return vec![];
    }

    for i in 1..char_levels.len() {
        if char_levels[i] != char_levels[run_start] {
            runs.push(BidiRun {
                char_start: run_start,
                char_end: i,
                level: char_levels[run_start],
                is_rtl: char_levels[run_start].is_rtl(),
            });
            run_start = i;
        }
    }
    // Final run
    runs.push(BidiRun {
        char_start: run_start,
        char_end: char_levels.len(),
        level: char_levels[run_start],
        is_rtl: char_levels[run_start].is_rtl(),
    });

    runs
}

/// Check if text is purely LTR (no RTL characters at all).
/// This is a fast path to skip BiDi processing for the common case.
pub fn is_pure_ltr(text: &str, direction: Direction) -> bool {
    if matches!(direction, Direction::Rtl) {
        return false;
    }

    // Quick scan: if no char has RTL BiDi class, it's pure LTR
    !text.chars().any(is_rtl_char)
}

/// Check if a character has an RTL BiDi class (R, AL, or AN).
fn is_rtl_char(ch: char) -> bool {
    // Unicode BiDi character types: R (Right-to-Left), AL (Arabic Letter),
    // AN (Arabic Number). We check common RTL ranges.
    matches!(ch,
        '\u{0590}'..='\u{05FF}' |  // Hebrew
        '\u{0600}'..='\u{06FF}' |  // Arabic
        '\u{0700}'..='\u{074F}' |  // Syriac
        '\u{0750}'..='\u{077F}' |  // Arabic Supplement
        '\u{0780}'..='\u{07BF}' |  // Thaana
        '\u{07C0}'..='\u{07FF}' |  // NKo
        '\u{0800}'..='\u{083F}' |  // Samaritan
        '\u{0840}'..='\u{085F}' |  // Mandaic
        '\u{08A0}'..='\u{08FF}' |  // Arabic Extended-A
        '\u{FB1D}'..='\u{FB4F}' |  // Hebrew Presentation Forms
        '\u{FB50}'..='\u{FDFF}' |  // Arabic Presentation Forms-A
        '\u{FE70}'..='\u{FEFF}' |  // Arabic Presentation Forms-B
        '\u{10800}'..='\u{10FFF}' | // Various RTL scripts
        '\u{1E800}'..='\u{1EEFF}' | // More RTL
        '\u{200F}' |               // RTL Mark
        '\u{202B}' |               // RTL Embedding
        '\u{202E}' |               // RTL Override
        '\u{2067}'                  // RTL Isolate
    )
}

/// Reorder positioned glyphs on a line for visual display.
///
/// Takes glyphs in logical order with their BiDi levels and produces
/// visual order. RTL runs are reversed so they display correctly.
pub fn reorder_line_glyphs(
    mut glyphs: Vec<PositionedGlyph>,
    levels: &[Level],
) -> Vec<PositionedGlyph> {
    if glyphs.is_empty() || levels.is_empty() {
        return glyphs;
    }

    // Use the standard BiDi reordering algorithm (L2):
    // Find the highest level, then reverse all runs at that level and above,
    // working down to the lowest odd level.
    let min_level = levels.iter().copied().min().unwrap_or(Level::ltr());
    let max_level = levels.iter().copied().max().unwrap_or(Level::ltr());

    // Only reorder if there's actually an RTL level
    if !max_level.is_rtl() {
        return glyphs;
    }

    // L2: For each level from max down to the lowest odd level,
    // reverse any contiguous run of glyphs at that level or higher
    let min_odd = if min_level.is_rtl() {
        min_level
    } else {
        Level::rtl() // level 1
    };

    let mut current_level = max_level;
    while current_level >= min_odd {
        let mut i = 0;
        while i < glyphs.len() {
            if levels.get(i).copied().unwrap_or(Level::ltr()) >= current_level {
                // Find the end of this run at >= current_level
                let start = i;
                while i < glyphs.len()
                    && levels.get(i).copied().unwrap_or(Level::ltr()) >= current_level
                {
                    i += 1;
                }
                // Reverse the run
                glyphs[start..i].reverse();
            } else {
                i += 1;
            }
        }
        // Move to next lower level
        if current_level.number() == 0 {
            break;
        }
        current_level = Level::new(current_level.number() - 1).unwrap_or(Level::ltr());
    }

    glyphs
}

/// Reposition glyphs after visual reordering.
///
/// After `reorder_line_glyphs`, x_offsets still reflect logical order.
/// This recalculates x positions from left to right based on advance widths.
pub fn reposition_after_reorder(glyphs: &mut [PositionedGlyph], start_x: f64) {
    let mut x = start_x;
    for g in glyphs.iter_mut() {
        g.x_offset = x;
        x += g.x_advance;
    }
}

/// Build a byte-offset → char-index map for a string.
#[allow(dead_code)]
fn build_byte_to_char_map(text: &str) -> Vec<usize> {
    let mut map = vec![0usize; text.len() + 1];
    let mut char_idx = 0;
    for (byte_idx, _) in text.char_indices() {
        map[byte_idx] = char_idx;
        char_idx += 1;
    }
    map[text.len()] = char_idx;
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_ltr() {
        assert!(is_pure_ltr("Hello World", Direction::Ltr));
        assert!(is_pure_ltr("Hello World", Direction::Auto));
        assert!(!is_pure_ltr("Hello World", Direction::Rtl));
    }

    #[test]
    fn test_rtl_detection() {
        assert!(!is_pure_ltr("مرحبا", Direction::Ltr));
        assert!(!is_pure_ltr("שלום", Direction::Ltr));
    }

    #[test]
    fn test_analyze_bidi_pure_ltr() {
        let runs = analyze_bidi("Hello World", Direction::Ltr);
        assert_eq!(runs.len(), 1);
        assert!(!runs[0].is_rtl);
        assert_eq!(runs[0].char_start, 0);
        assert_eq!(runs[0].char_end, 11);
    }

    #[test]
    fn test_analyze_bidi_pure_rtl() {
        let runs = analyze_bidi("مرحبا", Direction::Rtl);
        assert_eq!(runs.len(), 1);
        assert!(runs[0].is_rtl);
    }

    #[test]
    fn test_analyze_bidi_mixed() {
        // "Hello مرحبا World" — should produce 3 runs: LTR, RTL, LTR
        let runs = analyze_bidi("Hello مرحبا World", Direction::Ltr);
        assert!(
            runs.len() >= 2,
            "Expected at least 2 runs, got {}",
            runs.len()
        );
        // The first run should be LTR (Hello + space)
        assert!(!runs[0].is_rtl);
        // There should be an RTL run somewhere
        assert!(runs.iter().any(|r| r.is_rtl), "Should have an RTL run");
    }

    #[test]
    fn test_analyze_bidi_empty() {
        let runs = analyze_bidi("", Direction::Ltr);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_rtl_direction_defaults_right_align() {
        // This tests the style system integration
        use crate::style::{Style, TextAlign};
        let style = Style {
            direction: Some(Direction::Rtl),
            ..Default::default()
        };
        let resolved = style.resolve(None, 500.0);
        assert!(matches!(resolved.text_align, TextAlign::Right));
    }

    #[test]
    fn test_ltr_direction_defaults_left_align() {
        use crate::style::{Style, TextAlign};
        let style = Style {
            direction: Some(Direction::Ltr),
            ..Default::default()
        };
        let resolved = style.resolve(None, 500.0);
        assert!(matches!(resolved.text_align, TextAlign::Left));
    }
}
