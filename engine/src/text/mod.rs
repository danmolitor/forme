//! # Text Layout
//!
//! Line breaking, text measurement, and glyph positioning.
//!
//! Uses real font metrics from the FontContext for accurate character widths.

use crate::font::FontContext;
use crate::style::{Color, FontStyle, TextDecoration};

/// A line of text after line-breaking.
#[derive(Debug, Clone)]
pub struct BrokenLine {
    /// The characters on this line.
    pub chars: Vec<char>,
    /// The text as a string.
    pub text: String,
    /// X position of each character relative to line start.
    pub char_positions: Vec<f64>,
    /// Total width of the line.
    pub width: f64,
}

/// A styled character for multi-style line breaking.
#[derive(Debug, Clone)]
pub struct StyledChar {
    pub ch: char,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: u32,
    pub font_style: FontStyle,
    pub color: Color,
    pub href: Option<String>,
    pub text_decoration: TextDecoration,
    pub letter_spacing: f64,
}

/// A line of text from multi-style (runs) line breaking.
#[derive(Debug, Clone)]
pub struct RunBrokenLine {
    pub chars: Vec<StyledChar>,
    pub char_positions: Vec<f64>,
    pub width: f64,
}

pub struct TextLayout;

impl Default for TextLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl TextLayout {
    pub fn new() -> Self {
        Self
    }

    /// Break a string into lines that fit within `max_width`.
    ///
    /// Uses a greedy line-breaking algorithm. The Knuth-Plass algorithm
    /// would produce better results (fewer rivers, more even spacing)
    /// but is significantly more complex. We'll upgrade later.
    #[allow(clippy::too_many_arguments)]
    pub fn break_into_lines(
        &self,
        font_context: &FontContext,
        text: &str,
        max_width: f64,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        letter_spacing: f64,
    ) -> Vec<BrokenLine> {
        if text.is_empty() {
            return vec![BrokenLine {
                chars: vec![],
                text: String::new(),
                char_positions: vec![],
                width: 0.0,
            }];
        }

        let char_widths = self.measure_chars(
            font_context,
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            letter_spacing,
        );

        let mut lines = Vec::new();
        let mut line_start = 0;
        let mut line_width = 0.0;
        let mut last_break_point = None;
        let mut _last_break_width = 0.0;

        let chars: Vec<char> = text.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            let char_width = char_widths[i];

            // Track potential break points (after spaces, hyphens)
            if ch == ' ' || ch == '-' || ch == '\t' {
                last_break_point = Some(i);
                _last_break_width = line_width + char_width;
            }

            // Explicit newline
            if ch == '\n' {
                let line_chars: Vec<char> = chars[line_start..i].to_vec();
                lines.push(self.make_line(&line_chars, &char_widths[line_start..i]));
                line_start = i + 1;
                line_width = 0.0;
                last_break_point = None;
                continue;
            }

            if line_width + char_width > max_width && line_start < i {
                // Line overflow — break at the last break point if possible
                if let Some(bp) = last_break_point {
                    if bp >= line_start {
                        let break_at = if chars[bp] == ' ' { bp } else { bp + 1 };
                        let line_chars: Vec<char> = chars[line_start..break_at].to_vec();
                        lines.push(self.make_line(&line_chars, &char_widths[line_start..break_at]));

                        line_start = bp + 1;
                        line_width = char_widths[line_start..=i].iter().sum();
                        last_break_point = None;
                        continue;
                    }
                }

                // No good break point — force break at current position
                let line_chars: Vec<char> = chars[line_start..i].to_vec();
                lines.push(self.make_line(&line_chars, &char_widths[line_start..i]));
                line_start = i;
                line_width = char_width;
                last_break_point = None;
                continue;
            }

            line_width += char_width;
        }

        // Last line
        if line_start < chars.len() {
            let line_chars: Vec<char> = chars[line_start..].to_vec();
            lines.push(self.make_line(&line_chars, &char_widths[line_start..]));
        }

        lines
    }

    /// Create a BrokenLine from characters and their widths.
    fn make_line(&self, chars: &[char], widths: &[f64]) -> BrokenLine {
        let mut positions = Vec::with_capacity(chars.len());
        let mut x = 0.0;
        for &w in widths {
            positions.push(x);
            x += w;
        }

        // Trim trailing spaces from width calculation
        let mut effective_width = x;
        let mut i = chars.len();
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
            effective_width -= widths[i];
        }

        BrokenLine {
            text: chars.iter().collect(),
            chars: chars.to_vec(),
            char_positions: positions,
            width: effective_width,
        }
    }

    /// Measure individual character widths using real font metrics.
    #[allow(clippy::too_many_arguments)]
    fn measure_chars(
        &self,
        font_context: &FontContext,
        text: &str,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        letter_spacing: f64,
    ) -> Vec<f64> {
        let italic = matches!(font_style, FontStyle::Italic | FontStyle::Oblique);
        text.chars()
            .map(|ch| {
                font_context.char_width(ch, font_family, font_weight, italic, font_size)
                    + letter_spacing
            })
            .collect()
    }

    /// Break multi-style text (runs) into lines that fit within `max_width`.
    pub fn break_runs_into_lines(
        &self,
        font_context: &FontContext,
        chars: &[StyledChar],
        max_width: f64,
    ) -> Vec<RunBrokenLine> {
        if chars.is_empty() {
            return vec![RunBrokenLine {
                chars: vec![],
                char_positions: vec![],
                width: 0.0,
            }];
        }

        // Measure each character width using its own font/style
        let char_widths: Vec<f64> = chars
            .iter()
            .map(|sc| {
                let italic = matches!(sc.font_style, FontStyle::Italic | FontStyle::Oblique);
                font_context.char_width(
                    sc.ch,
                    &sc.font_family,
                    sc.font_weight,
                    italic,
                    sc.font_size,
                ) + sc.letter_spacing
            })
            .collect();

        let mut lines = Vec::new();
        let mut line_start = 0;
        let mut line_width = 0.0;
        let mut last_break_point: Option<usize> = None;

        for (i, sc) in chars.iter().enumerate() {
            let char_width = char_widths[i];

            if sc.ch == ' ' || sc.ch == '-' || sc.ch == '\t' {
                last_break_point = Some(i);
            }

            if sc.ch == '\n' {
                lines.push(self.make_run_line(&chars[line_start..i], &char_widths[line_start..i]));
                line_start = i + 1;
                line_width = 0.0;
                last_break_point = None;
                continue;
            }

            if line_width + char_width > max_width && line_start < i {
                if let Some(bp) = last_break_point {
                    if bp >= line_start {
                        let break_at = if chars[bp].ch == ' ' { bp } else { bp + 1 };
                        lines.push(self.make_run_line(
                            &chars[line_start..break_at],
                            &char_widths[line_start..break_at],
                        ));
                        line_start = bp + 1;
                        line_width = char_widths[line_start..=i].iter().sum();
                        last_break_point = None;
                        continue;
                    }
                }

                lines.push(self.make_run_line(&chars[line_start..i], &char_widths[line_start..i]));
                line_start = i;
                line_width = char_width;
                last_break_point = None;
                continue;
            }

            line_width += char_width;
        }

        if line_start < chars.len() {
            lines.push(self.make_run_line(&chars[line_start..], &char_widths[line_start..]));
        }

        lines
    }

    fn make_run_line(&self, chars: &[StyledChar], widths: &[f64]) -> RunBrokenLine {
        let mut positions = Vec::with_capacity(chars.len());
        let mut x = 0.0;
        for &w in widths {
            positions.push(x);
            x += w;
        }

        // Trim trailing spaces from width calculation
        let mut effective_width = x;
        let mut i = chars.len();
        while i > 0 && chars[i - 1].ch == ' ' {
            i -= 1;
            effective_width -= widths[i];
        }

        RunBrokenLine {
            chars: chars.to_vec(),
            char_positions: positions,
            width: effective_width,
        }
    }

    /// Measure the width of a string on a single line.
    #[allow(clippy::too_many_arguments)]
    pub fn measure_width(
        &self,
        font_context: &FontContext,
        text: &str,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        letter_spacing: f64,
    ) -> f64 {
        self.measure_chars(
            font_context,
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            letter_spacing,
        )
        .iter()
        .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> FontContext {
        FontContext::new()
    }

    #[test]
    fn test_single_line() {
        let tl = TextLayout::new();
        let fc = ctx();
        let lines = tl.break_into_lines(
            &fc,
            "Hello",
            200.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
    }

    #[test]
    fn test_line_break_at_space() {
        let tl = TextLayout::new();
        let fc = ctx();
        let lines = tl.break_into_lines(
            &fc,
            "Hello World",
            40.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_explicit_newline() {
        let tl = TextLayout::new();
        let fc = ctx();
        let lines = tl.break_into_lines(
            &fc,
            "Hello\nWorld",
            200.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[1].text, "World");
    }

    #[test]
    fn test_empty_string() {
        let tl = TextLayout::new();
        let fc = ctx();
        let lines = tl.break_into_lines(
            &fc,
            "",
            200.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].width, 0.0);
    }

    #[test]
    fn test_bold_text_wider() {
        let tl = TextLayout::new();
        let fc = ctx();
        let regular = tl.measure_width(
            &fc,
            "ABCDEFG",
            32.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        let bold = tl.measure_width(
            &fc,
            "ABCDEFG",
            32.0,
            "Helvetica",
            700,
            FontStyle::Normal,
            0.0,
        );
        assert!(
            bold > regular,
            "Bold text should be wider: bold={bold}, regular={regular}"
        );
    }
}
