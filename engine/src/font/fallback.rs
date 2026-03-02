//! # Per-Character Font Fallback
//!
//! Segments text into runs by font coverage. When a font family is a
//! comma-separated chain like "Inter, NotoSansArabic, NotoSansSC", each
//! character is resolved to the first font that has a glyph for it.
//! Consecutive characters using the same font are coalesced into runs
//! to minimize shaping calls.

use super::FontRegistry;

/// A contiguous run of characters that all resolve to the same font.
#[derive(Debug, Clone)]
pub struct FontRun {
    /// Start index in the original char array (inclusive).
    pub start: usize,
    /// End index in the original char array (exclusive).
    pub end: usize,
    /// The resolved single font family name (e.g. "Inter", not "Inter, Noto").
    pub family: String,
}

/// Segment characters into runs by font coverage.
///
/// **Fast path:** when `families` contains no comma, returns a single run
/// covering all characters — zero overhead for single-font text.
///
/// **Slow path:** iterates characters, calling `resolve_for_char` per char,
/// and coalesces consecutive same-font characters into runs.
pub fn segment_by_font(
    chars: &[char],
    families: &str,
    weight: u32,
    italic: bool,
    registry: &FontRegistry,
) -> Vec<FontRun> {
    if chars.is_empty() {
        return vec![];
    }

    // Fast path: no comma means single font family
    if !families.contains(',') {
        let family = families.trim().trim_matches('"').trim_matches('\'');
        return vec![FontRun {
            start: 0,
            end: chars.len(),
            family: family.to_string(),
        }];
    }

    // Slow path: per-character font resolution
    let mut runs = Vec::new();
    let (_, first_family) = registry.resolve_for_char(families, chars[0], weight, italic);
    let mut current_family = first_family;
    let mut run_start = 0;

    for (i, &ch) in chars.iter().enumerate().skip(1) {
        let (_, family) = registry.resolve_for_char(families, ch, weight, italic);
        if family != current_family {
            runs.push(FontRun {
                start: run_start,
                end: i,
                family: current_family,
            });
            current_family = family;
            run_start = i;
        }
    }

    // Push final run
    runs.push(FontRun {
        start: run_start,
        end: chars.len(),
        family: current_family,
    });

    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_font_fast_path() {
        let registry = FontRegistry::new();
        let chars: Vec<char> = "Hello world".chars().collect();
        let runs = segment_by_font(&chars, "Helvetica", 400, false, &registry);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].family, "Helvetica");
        assert_eq!(runs[0].start, 0);
        assert_eq!(runs[0].end, 11);
    }

    #[test]
    fn test_empty_input() {
        let registry = FontRegistry::new();
        let chars: Vec<char> = vec![];
        let runs = segment_by_font(&chars, "Helvetica, Times", 400, false, &registry);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_all_chars_same_font() {
        let registry = FontRegistry::new();
        let chars: Vec<char> = "ABC".chars().collect();
        // Both Helvetica and Times have Latin chars, so first match wins
        let runs = segment_by_font(&chars, "Helvetica, Times", 400, false, &registry);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].family, "Helvetica");
    }
}
