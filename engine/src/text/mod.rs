//! # Text Layout
//!
//! Line breaking, text measurement, and glyph positioning.
//!
//! Uses real font metrics from the FontContext for accurate character widths.

pub mod knuth_plass;

use crate::font::FontContext;
use crate::style::{Color, FontStyle, Hyphens, TextDecoration};
use unicode_linebreak::{linebreaks, BreakOpportunity};

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

/// Compute UAX#14 break opportunities indexed by char position.
///
/// Returns a vec of length `text.chars().count()`. Each entry is the break
/// opportunity *before* that character position (i.e. "can we break before
/// char[i]?"). Index 0 is always `None` (no break before the first char).
fn compute_break_opportunities(text: &str) -> Vec<Option<BreakOpportunity>> {
    let char_count = text.chars().count();
    let mut result = vec![None; char_count];

    // linebreaks() yields (byte_offset, opportunity) where byte_offset is the
    // position AFTER the break — i.e. the start of the next segment.
    // We need to convert byte offsets to char indices.
    let byte_to_char: Vec<usize> = {
        let mut map = vec![0usize; text.len() + 1];
        let mut char_idx = 0;
        for (byte_idx, _) in text.char_indices() {
            map[byte_idx] = char_idx;
            char_idx += 1;
        }
        map[text.len()] = char_idx;
        map
    };

    for (byte_offset, opp) in linebreaks(text) {
        let char_idx = byte_to_char[byte_offset];
        if char_idx < char_count {
            result[char_idx] = Some(opp);
        }
        // byte_offset == text.len() means "break at end" — we ignore that
    }

    result
}

/// Map a BCP 47 language tag to a `hypher::Lang` for hyphenation.
///
/// Returns `Some(lang)` for supported languages, `None` for unsupported ones
/// (which disables algorithmic hyphenation). Defaults to English when no tag
/// is provided, for backward compatibility.
fn resolve_hypher_lang(lang: Option<&str>) -> Option<hypher::Lang> {
    let tag = match lang {
        Some(t) => t,
        None => return Some(hypher::Lang::English),
    };
    let primary = tag.split('-').next().unwrap_or(tag).to_lowercase();
    match primary.as_str() {
        "af" => Some(hypher::Lang::Afrikaans),
        "sq" => Some(hypher::Lang::Albanian),
        "be" => Some(hypher::Lang::Belarusian),
        "bg" => Some(hypher::Lang::Bulgarian),
        "ca" => Some(hypher::Lang::Catalan),
        "hr" => Some(hypher::Lang::Croatian),
        "cs" => Some(hypher::Lang::Czech),
        "da" => Some(hypher::Lang::Danish),
        "nl" => Some(hypher::Lang::Dutch),
        "en" => Some(hypher::Lang::English),
        "et" => Some(hypher::Lang::Estonian),
        "fi" => Some(hypher::Lang::Finnish),
        "fr" => Some(hypher::Lang::French),
        "ka" => Some(hypher::Lang::Georgian),
        "de" => Some(hypher::Lang::German),
        "el" => Some(hypher::Lang::Greek),
        "hu" => Some(hypher::Lang::Hungarian),
        "is" => Some(hypher::Lang::Icelandic),
        "it" => Some(hypher::Lang::Italian),
        "ku" => Some(hypher::Lang::Kurmanji),
        "la" => Some(hypher::Lang::Latin),
        "lt" => Some(hypher::Lang::Lithuanian),
        "mn" => Some(hypher::Lang::Mongolian),
        "nb" | "nn" | "no" => Some(hypher::Lang::Norwegian),
        "pl" => Some(hypher::Lang::Polish),
        "pt" => Some(hypher::Lang::Portuguese),
        "ru" => Some(hypher::Lang::Russian),
        "sr" => Some(hypher::Lang::Serbian),
        "sk" => Some(hypher::Lang::Slovak),
        "sl" => Some(hypher::Lang::Slovenian),
        "es" => Some(hypher::Lang::Spanish),
        "sv" => Some(hypher::Lang::Swedish),
        "tr" => Some(hypher::Lang::Turkish),
        "tk" => Some(hypher::Lang::Turkmen),
        "uk" => Some(hypher::Lang::Ukrainian),
        _ => None,
    }
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
    /// Uses a greedy line-breaking algorithm with optional hyphenation.
    /// When `hyphens` is `Auto`, long words that don't fit are split at
    /// syllable boundaries using the Knuth-Liang algorithm. When `Manual`,
    /// only soft hyphens (U+00AD) in the text are used as break points.
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
        hyphens: Hyphens,
        lang: Option<&str>,
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

        let hyphen_width = font_context.char_width(
            '-',
            font_family,
            font_weight,
            matches!(font_style, FontStyle::Italic | FontStyle::Oblique),
            font_size,
        ) + letter_spacing;

        let mut lines = Vec::new();
        let mut line_start = 0;
        let mut line_width = 0.0;
        let mut last_break_point = None;
        let mut _last_break_width = 0.0;

        let chars: Vec<char> = text.chars().collect();
        let break_opps = compute_break_opportunities(text);

        for (i, &ch) in chars.iter().enumerate() {
            let char_width = char_widths[i];

            // UAX#14 break opportunities: a break *before* char[i] means
            // we can end the previous line at char[i-1].
            // We record break points at i-1 (the char before the break).
            if i > 0 {
                if let Some(opp) = break_opps[i] {
                    match opp {
                        BreakOpportunity::Mandatory => {
                            // Mandatory break: flush the current line
                            let end = if chars[i - 1] == '\n'
                                || chars[i - 1] == '\r'
                                || chars[i - 1] == '\u{2028}'
                                || chars[i - 1] == '\u{2029}'
                            {
                                i - 1
                            } else {
                                i
                            };
                            let line_chars = self.filter_soft_hyphens(&chars[line_start..end]);
                            let line_widths = self.filter_soft_hyphen_widths(
                                &chars[line_start..end],
                                &char_widths[line_start..end],
                            );
                            lines.push(self.make_line(&line_chars, &line_widths));
                            line_start = i;
                            line_width = 0.0;
                            last_break_point = None;
                            // Don't skip — still need to process char[i] width below
                        }
                        BreakOpportunity::Allowed => {
                            // Record the char BEFORE this position as a break point
                            last_break_point = Some(i - 1);
                            _last_break_width = line_width;
                        }
                    }
                }
            }

            // Soft hyphens are additional break points for Manual and Auto modes
            if ch == '\u{00AD}' && hyphens != Hyphens::None {
                last_break_point = Some(i);
                _last_break_width = line_width;
            }

            // Soft hyphens are zero-width when not at a break
            if ch == '\u{00AD}' {
                continue;
            }

            // Skip newline/CR chars (already handled by mandatory break above)
            if ch == '\n' || ch == '\r' || ch == '\u{2028}' || ch == '\u{2029}' {
                continue;
            }

            if line_width + char_width > max_width && line_start < i {
                // Line overflow — break at the last break point if possible
                if let Some(bp) = last_break_point {
                    if bp >= line_start {
                        if chars[bp] == '\u{00AD}' {
                            // Break at soft hyphen: render visible hyphen
                            let mut line_chars = self.filter_soft_hyphens(&chars[line_start..bp]);
                            let mut line_widths = self.filter_soft_hyphen_widths(
                                &chars[line_start..bp],
                                &char_widths[line_start..bp],
                            );
                            line_chars.push('-');
                            line_widths.push(hyphen_width);
                            lines.push(self.make_line(&line_chars, &line_widths));
                        } else {
                            // bp is the last char on this line (UAX#14 break is *after* bp)
                            let break_at = bp + 1;
                            let line_chars = self.filter_soft_hyphens(&chars[line_start..break_at]);
                            let line_widths = self.filter_soft_hyphen_widths(
                                &chars[line_start..break_at],
                                &char_widths[line_start..break_at],
                            );
                            lines.push(self.make_line(&line_chars, &line_widths));
                        }

                        line_start = bp + 1;
                        // Recalculate width excluding soft hyphens
                        line_width = chars[line_start..=i]
                            .iter()
                            .zip(char_widths[line_start..=i].iter())
                            .filter(|(c, _)| **c != '\u{00AD}')
                            .map(|(_, w)| w)
                            .sum();
                        last_break_point = None;
                        continue;
                    }
                }

                // No space/hyphen break point — try algorithmic hyphenation
                if hyphens == Hyphens::Auto {
                    if let Some((hyphen_line_chars, hyphen_line_widths, new_start)) = self
                        .try_hyphenate_word(
                            &chars,
                            &char_widths,
                            line_start,
                            i,
                            line_width,
                            max_width,
                            hyphen_width,
                            lang,
                        )
                    {
                        lines.push(self.make_line(&hyphen_line_chars, &hyphen_line_widths));
                        line_start = new_start;
                        line_width = chars[line_start..=i]
                            .iter()
                            .zip(char_widths[line_start..=i].iter())
                            .filter(|(c, _)| **c != '\u{00AD}')
                            .map(|(_, w)| w)
                            .sum();
                        last_break_point = None;
                        continue;
                    }
                }

                // No good break point — force break at current position
                let line_chars = self.filter_soft_hyphens(&chars[line_start..i]);
                let line_widths = self
                    .filter_soft_hyphen_widths(&chars[line_start..i], &char_widths[line_start..i]);
                lines.push(self.make_line(&line_chars, &line_widths));
                line_start = i;
                line_width = char_width;
                last_break_point = None;
                continue;
            }

            line_width += char_width;
        }

        // Last line
        if line_start < chars.len() {
            let line_chars = self.filter_soft_hyphens(&chars[line_start..]);
            let line_widths =
                self.filter_soft_hyphen_widths(&chars[line_start..], &char_widths[line_start..]);
            lines.push(self.make_line(&line_chars, &line_widths));
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

    /// Filter out soft hyphens from a char slice.
    fn filter_soft_hyphens(&self, chars: &[char]) -> Vec<char> {
        chars.iter().copied().filter(|c| *c != '\u{00AD}').collect()
    }

    /// Filter out widths corresponding to soft hyphens.
    fn filter_soft_hyphen_widths(&self, chars: &[char], widths: &[f64]) -> Vec<f64> {
        chars
            .iter()
            .zip(widths.iter())
            .filter(|(c, _)| **c != '\u{00AD}')
            .map(|(_, w)| *w)
            .collect()
    }

    /// Try to hyphenate the current word at a syllable boundary that fits.
    ///
    /// Looks backward from the overflow point to find word boundaries, then
    /// uses `hypher` to find syllable breaks within the word. Returns the
    /// rightmost break that fits (with hyphen char appended).
    ///
    /// Returns `Some((line_chars, line_widths, new_line_start))` on success.
    #[allow(clippy::too_many_arguments)]
    fn try_hyphenate_word(
        &self,
        chars: &[char],
        char_widths: &[f64],
        line_start: usize,
        overflow_at: usize,
        _line_width: f64,
        max_width: f64,
        hyphen_width: f64,
        lang: Option<&str>,
    ) -> Option<(Vec<char>, Vec<f64>, usize)> {
        // Find the start of the current word (scan backward from overflow)
        let mut word_start = overflow_at;
        while word_start > line_start && !chars[word_start - 1].is_whitespace() {
            word_start -= 1;
        }

        // Collect the word chars (up to and including overflow_at - 1)
        let word_end = overflow_at; // exclusive — the char at overflow_at triggered overflow
        if word_end <= word_start {
            return None;
        }

        let word: String = chars[word_start..word_end].iter().collect();
        let hypher_lang = resolve_hypher_lang(lang)?;
        let syllables = hypher::hyphenate(&word, hypher_lang);

        let syllables: Vec<&str> = syllables.collect();
        if syllables.len() < 2 {
            return None;
        }

        // Width of content before the word on this line
        let prefix_width: f64 = chars[line_start..word_start]
            .iter()
            .zip(char_widths[line_start..word_start].iter())
            .filter(|(c, _)| **c != '\u{00AD}')
            .map(|(_, w)| w)
            .sum();

        // Find the rightmost syllable boundary that fits
        let mut best_break: Option<usize> = None; // index into chars[] to break AFTER
        let mut syllable_offset = word_start;
        for (si, syllable) in syllables.iter().enumerate() {
            if si == syllables.len() - 1 {
                break; // don't break after the last syllable
            }
            syllable_offset += syllable.chars().count();

            // Width of word chars from word_start..syllable_offset
            let word_part_width: f64 = chars[word_start..syllable_offset]
                .iter()
                .zip(char_widths[word_start..syllable_offset].iter())
                .filter(|(c, _)| **c != '\u{00AD}')
                .map(|(_, w)| w)
                .sum();

            if prefix_width + word_part_width + hyphen_width <= max_width {
                best_break = Some(syllable_offset);
            }
        }

        let break_at = best_break?;

        let mut line_chars = self.filter_soft_hyphens(&chars[line_start..break_at]);
        let mut line_widths = self.filter_soft_hyphen_widths(
            &chars[line_start..break_at],
            &char_widths[line_start..break_at],
        );
        line_chars.push('-');
        line_widths.push(hyphen_width);

        Some((line_chars, line_widths, break_at))
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
        hyphens: Hyphens,
        lang: Option<&str>,
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

        // Build plain text for UAX#14 break analysis
        let plain_text: String = chars.iter().map(|sc| sc.ch).collect();
        let break_opps = compute_break_opportunities(&plain_text);

        for (i, sc) in chars.iter().enumerate() {
            let char_width = char_widths[i];

            // UAX#14 break opportunities
            if i > 0 {
                if let Some(opp) = break_opps[i] {
                    match opp {
                        BreakOpportunity::Mandatory => {
                            let end = if chars[i - 1].ch == '\n'
                                || chars[i - 1].ch == '\r'
                                || chars[i - 1].ch == '\u{2028}'
                                || chars[i - 1].ch == '\u{2029}'
                            {
                                i - 1
                            } else {
                                i
                            };
                            let filtered = self.filter_soft_hyphens_runs(&chars[line_start..end]);
                            let filtered_widths = self.filter_soft_hyphen_widths_runs(
                                &chars[line_start..end],
                                &char_widths[line_start..end],
                            );
                            lines.push(self.make_run_line(&filtered, &filtered_widths));
                            line_start = i;
                            line_width = 0.0;
                            last_break_point = None;
                        }
                        BreakOpportunity::Allowed => {
                            last_break_point = Some(i - 1);
                        }
                    }
                }
            }

            // Soft hyphens are additional break points
            if sc.ch == '\u{00AD}' && hyphens != Hyphens::None {
                last_break_point = Some(i);
            }

            // Soft hyphens are zero-width when not at a break
            if sc.ch == '\u{00AD}' {
                continue;
            }

            // Skip newline/CR chars (already handled by mandatory break above)
            if sc.ch == '\n' || sc.ch == '\r' || sc.ch == '\u{2028}' || sc.ch == '\u{2029}' {
                continue;
            }

            if line_width + char_width > max_width && line_start < i {
                if let Some(bp) = last_break_point {
                    if bp >= line_start {
                        if chars[bp].ch == '\u{00AD}' {
                            // Break at soft hyphen: render visible hyphen
                            let mut filtered =
                                self.filter_soft_hyphens_runs(&chars[line_start..bp]);
                            let mut filtered_widths = self.filter_soft_hyphen_widths_runs(
                                &chars[line_start..bp],
                                &char_widths[line_start..bp],
                            );
                            // Add a visible hyphen with the style of the char before the soft hyphen
                            let hyphen_style = if bp > 0 {
                                chars[bp - 1].clone()
                            } else {
                                chars[bp].clone()
                            };
                            let italic = matches!(
                                hyphen_style.font_style,
                                FontStyle::Italic | FontStyle::Oblique
                            );
                            let hw = font_context.char_width(
                                '-',
                                &hyphen_style.font_family,
                                hyphen_style.font_weight,
                                italic,
                                hyphen_style.font_size,
                            ) + hyphen_style.letter_spacing;
                            let mut hyphen_sc = hyphen_style;
                            hyphen_sc.ch = '-';
                            filtered.push(hyphen_sc);
                            filtered_widths.push(hw);
                            lines.push(self.make_run_line(&filtered, &filtered_widths));
                        } else {
                            // bp is the last char on this line (UAX#14 break after bp)
                            let break_at = bp + 1;
                            let filtered =
                                self.filter_soft_hyphens_runs(&chars[line_start..break_at]);
                            let filtered_widths = self.filter_soft_hyphen_widths_runs(
                                &chars[line_start..break_at],
                                &char_widths[line_start..break_at],
                            );
                            lines.push(self.make_run_line(&filtered, &filtered_widths));
                        }

                        line_start = bp + 1;
                        line_width = chars[line_start..=i]
                            .iter()
                            .zip(char_widths[line_start..=i].iter())
                            .filter(|(sc, _)| sc.ch != '\u{00AD}')
                            .map(|(_, w)| w)
                            .sum();
                        last_break_point = None;
                        continue;
                    }
                }

                // Try algorithmic hyphenation
                if hyphens == Hyphens::Auto {
                    let plain_chars: Vec<char> = chars.iter().map(|sc| sc.ch).collect();
                    let italic = if !chars.is_empty() {
                        matches!(
                            chars[line_start].font_style,
                            FontStyle::Italic | FontStyle::Oblique
                        )
                    } else {
                        false
                    };
                    let hyphen_width = if !chars.is_empty() {
                        font_context.char_width(
                            '-',
                            &chars[line_start].font_family,
                            chars[line_start].font_weight,
                            italic,
                            chars[line_start].font_size,
                        ) + chars[line_start].letter_spacing
                    } else {
                        0.0
                    };

                    if let Some((_, _, new_start)) = self.try_hyphenate_word(
                        &plain_chars,
                        &char_widths,
                        line_start,
                        i,
                        line_width,
                        max_width,
                        hyphen_width,
                        lang,
                    ) {
                        // Build the run line with hyphen
                        let mut filtered =
                            self.filter_soft_hyphens_runs(&chars[line_start..new_start]);
                        let mut filtered_widths = self.filter_soft_hyphen_widths_runs(
                            &chars[line_start..new_start],
                            &char_widths[line_start..new_start],
                        );
                        let hyphen_style_ref = if new_start > 0 {
                            &chars[new_start - 1]
                        } else {
                            &chars[0]
                        };
                        let mut hyphen_sc = hyphen_style_ref.clone();
                        hyphen_sc.ch = '-';
                        filtered.push(hyphen_sc);
                        filtered_widths.push(hyphen_width);
                        lines.push(self.make_run_line(&filtered, &filtered_widths));

                        line_start = new_start;
                        line_width = chars[line_start..=i]
                            .iter()
                            .zip(char_widths[line_start..=i].iter())
                            .filter(|(sc, _)| sc.ch != '\u{00AD}')
                            .map(|(_, w)| w)
                            .sum();
                        last_break_point = None;
                        continue;
                    }
                }

                let filtered = self.filter_soft_hyphens_runs(&chars[line_start..i]);
                let filtered_widths = self.filter_soft_hyphen_widths_runs(
                    &chars[line_start..i],
                    &char_widths[line_start..i],
                );
                lines.push(self.make_run_line(&filtered, &filtered_widths));
                line_start = i;
                line_width = char_width;
                last_break_point = None;
                continue;
            }

            line_width += char_width;
        }

        if line_start < chars.len() {
            let filtered = self.filter_soft_hyphens_runs(&chars[line_start..]);
            let filtered_widths = self
                .filter_soft_hyphen_widths_runs(&chars[line_start..], &char_widths[line_start..]);
            lines.push(self.make_run_line(&filtered, &filtered_widths));
        }

        lines
    }

    /// Filter out soft hyphens from styled char slices.
    fn filter_soft_hyphens_runs(&self, chars: &[StyledChar]) -> Vec<StyledChar> {
        chars
            .iter()
            .filter(|sc| sc.ch != '\u{00AD}')
            .cloned()
            .collect()
    }

    /// Filter out widths corresponding to soft hyphens in styled char slices.
    fn filter_soft_hyphen_widths_runs(&self, chars: &[StyledChar], widths: &[f64]) -> Vec<f64> {
        chars
            .iter()
            .zip(widths.iter())
            .filter(|(sc, _)| sc.ch != '\u{00AD}')
            .map(|(_, w)| *w)
            .collect()
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

    /// Measure the widest single word in a string (min-content width).
    ///
    /// When `hyphens` is `Auto`, returns the widest *syllable* width instead
    /// of the widest word, since hyphenation allows breaking within words.
    #[allow(clippy::too_many_arguments)]
    pub fn measure_widest_word(
        &self,
        font_context: &FontContext,
        text: &str,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        letter_spacing: f64,
        hyphens: Hyphens,
        lang: Option<&str>,
    ) -> f64 {
        if hyphens == Hyphens::Auto {
            if let Some(hypher_lang) = resolve_hypher_lang(lang) {
                // With auto hyphenation, min-content is the widest syllable
                return text
                    .split_whitespace()
                    .flat_map(|word| {
                        let syllables = hypher::hyphenate(word, hypher_lang);
                        syllables
                            .into_iter()
                            .map(|s| {
                                self.measure_width(
                                    font_context,
                                    s,
                                    font_size,
                                    font_family,
                                    font_weight,
                                    font_style,
                                    letter_spacing,
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .fold(0.0f64, f64::max);
            }
            // Unsupported language — fall through to word-level measurement
        }
        text.split_whitespace()
            .map(|word| {
                self.measure_width(
                    font_context,
                    word,
                    font_size,
                    font_family,
                    font_weight,
                    font_style,
                    letter_spacing,
                )
            })
            .fold(0.0f64, f64::max)
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

    /// Break text into lines using the Knuth-Plass optimal algorithm.
    ///
    /// Falls back to greedy breaking if KP finds no feasible solution.
    #[allow(clippy::too_many_arguments)]
    pub fn break_into_lines_optimal(
        &self,
        font_context: &FontContext,
        text: &str,
        max_width: f64,
        font_size: f64,
        font_family: &str,
        font_weight: u32,
        font_style: FontStyle,
        letter_spacing: f64,
        hyphens: Hyphens,
        lang: Option<&str>,
        justify: bool,
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

        let hyphen_width = font_context.char_width(
            '-',
            font_family,
            font_weight,
            matches!(font_style, FontStyle::Italic | FontStyle::Oblique),
            font_size,
        ) + letter_spacing;

        let chars: Vec<char> = text.chars().collect();
        let break_opps = compute_break_opportunities(text);

        // Check for mandatory breaks — if present, handle each segment separately
        let mut segments = Vec::new();
        let mut seg_start = 0;
        for (i, opp) in break_opps.iter().enumerate() {
            if let Some(BreakOpportunity::Mandatory) = opp {
                // End of previous segment is just before this char
                // But the mandatory break could be at \n, so the end is i-1 or earlier
                let end = if i > 0
                    && (chars[i - 1] == '\n'
                        || chars[i - 1] == '\r'
                        || chars[i - 1] == '\u{2028}'
                        || chars[i - 1] == '\u{2029}')
                {
                    i - 1
                } else {
                    i
                };
                segments.push(seg_start..end);
                seg_start = i;
            }
        }
        segments.push(seg_start..chars.len());

        if segments.len() > 1 {
            // Multiple mandatory-break segments: run KP on each
            let mut all_lines = Vec::new();
            for seg in &segments {
                if seg.is_empty() {
                    all_lines.push(BrokenLine {
                        chars: vec![],
                        text: String::new(),
                        char_positions: vec![],
                        width: 0.0,
                    });
                    continue;
                }
                let seg_chars: Vec<char> = chars[seg.clone()]
                    .iter()
                    .copied()
                    .filter(|c| *c != '\n' && *c != '\r' && *c != '\u{2028}' && *c != '\u{2029}')
                    .collect();
                if seg_chars.is_empty() {
                    continue;
                }
                let seg_text: String = seg_chars.iter().collect();
                let seg_lines = self.break_into_lines_optimal(
                    font_context,
                    &seg_text,
                    max_width,
                    font_size,
                    font_family,
                    font_weight,
                    font_style,
                    letter_spacing,
                    hyphens,
                    lang,
                    justify,
                );
                all_lines.extend(seg_lines);
            }
            return all_lines;
        }

        // Single segment — run KP
        let items = knuth_plass::build_items(
            &chars,
            &char_widths,
            hyphen_width,
            hyphens,
            &break_opps,
            lang,
        );
        let config = knuth_plass::Config {
            line_width: max_width,
            ..Default::default()
        };

        if let Some(solutions) = knuth_plass::find_breaks(&items, &config) {
            knuth_plass::reconstruct_lines(
                &solutions,
                &items,
                &chars,
                &char_widths,
                max_width,
                justify,
            )
        } else {
            // Fallback to greedy
            self.break_into_lines(
                font_context,
                text,
                max_width,
                font_size,
                font_family,
                font_weight,
                font_style,
                letter_spacing,
                hyphens,
                lang,
            )
        }
    }

    /// Break multi-style text into lines using the Knuth-Plass optimal algorithm.
    ///
    /// Falls back to greedy breaking if KP finds no feasible solution.
    pub fn break_runs_into_lines_optimal(
        &self,
        font_context: &FontContext,
        chars: &[StyledChar],
        max_width: f64,
        hyphens: Hyphens,
        lang: Option<&str>,
        justify: bool,
    ) -> Vec<RunBrokenLine> {
        if chars.is_empty() {
            return vec![RunBrokenLine {
                chars: vec![],
                char_positions: vec![],
                width: 0.0,
            }];
        }

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

        // Use the first char's style for hyphen width
        let hyphen_width = if !chars.is_empty() {
            let sc = &chars[0];
            let italic = matches!(sc.font_style, FontStyle::Italic | FontStyle::Oblique);
            font_context.char_width('-', &sc.font_family, sc.font_weight, italic, sc.font_size)
                + sc.letter_spacing
        } else {
            0.0
        };

        let plain_text: String = chars.iter().map(|sc| sc.ch).collect();
        let break_opps = compute_break_opportunities(&plain_text);

        // Handle mandatory breaks by splitting into segments
        let plain_chars: Vec<char> = chars.iter().map(|sc| sc.ch).collect();
        let has_mandatory = break_opps
            .iter()
            .any(|o| matches!(o, Some(BreakOpportunity::Mandatory)));

        if has_mandatory {
            let mut all_lines = Vec::new();
            let mut seg_start = 0;

            for (i, opp) in break_opps.iter().enumerate() {
                if let Some(BreakOpportunity::Mandatory) = opp {
                    let end = if i > 0
                        && (plain_chars[i - 1] == '\n'
                            || plain_chars[i - 1] == '\r'
                            || plain_chars[i - 1] == '\u{2028}'
                            || plain_chars[i - 1] == '\u{2029}')
                    {
                        i - 1
                    } else {
                        i
                    };
                    let seg_chars: Vec<StyledChar> = chars[seg_start..end]
                        .iter()
                        .filter(|sc| {
                            sc.ch != '\n'
                                && sc.ch != '\r'
                                && sc.ch != '\u{2028}'
                                && sc.ch != '\u{2029}'
                        })
                        .cloned()
                        .collect();
                    let seg_lines = self.break_runs_into_lines_optimal(
                        font_context,
                        &seg_chars,
                        max_width,
                        hyphens,
                        lang,
                        justify,
                    );
                    all_lines.extend(seg_lines);
                    seg_start = i;
                }
            }
            // Last segment
            let seg_chars: Vec<StyledChar> = chars[seg_start..]
                .iter()
                .filter(|sc| {
                    sc.ch != '\n' && sc.ch != '\r' && sc.ch != '\u{2028}' && sc.ch != '\u{2029}'
                })
                .cloned()
                .collect();
            if !seg_chars.is_empty() {
                let seg_lines = self.break_runs_into_lines_optimal(
                    font_context,
                    &seg_chars,
                    max_width,
                    hyphens,
                    lang,
                    justify,
                );
                all_lines.extend(seg_lines);
            }
            return all_lines;
        }

        let items = knuth_plass::build_items_styled(
            chars,
            &char_widths,
            hyphen_width,
            hyphens,
            &break_opps,
            lang,
        );
        let config = knuth_plass::Config {
            line_width: max_width,
            ..Default::default()
        };

        if let Some(solutions) = knuth_plass::find_breaks(&items, &config) {
            knuth_plass::reconstruct_run_lines(
                &solutions,
                &items,
                chars,
                &char_widths,
                max_width,
                justify,
            )
        } else {
            // Fallback to greedy
            self.break_runs_into_lines(font_context, chars, max_width, hyphens, lang)
        }
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
            Hyphens::Manual,
            None,
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
            Hyphens::Manual,
            None,
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
            Hyphens::Manual,
            None,
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
            Hyphens::Manual,
            None,
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

    #[test]
    fn test_hyphenation_auto_breaks_long_word() {
        let tl = TextLayout::new();
        let fc = ctx();
        // "extraordinary" is long enough to need hyphenation in a narrow column
        let lines = tl.break_into_lines(
            &fc,
            "extraordinary",
            50.0, // very narrow
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Auto,
            None,
        );
        // Should break into multiple lines with hyphens
        assert!(
            lines.len() >= 2,
            "Auto hyphenation should break 'extraordinary' into multiple lines, got {}",
            lines.len()
        );
        // First line should end with a hyphen
        assert!(
            lines[0].text.ends_with('-'),
            "First line should end with hyphen, got: '{}'",
            lines[0].text
        );
    }

    #[test]
    fn test_hyphenation_none_forces_break() {
        let tl = TextLayout::new();
        let fc = ctx();
        let lines = tl.break_into_lines(
            &fc,
            "extraordinary",
            50.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::None,
            None,
        );
        // Should still break (force break), but NO hyphens inserted
        assert!(lines.len() >= 2);
        // No line should end with '-' from hyphenation
        assert!(
            !lines[0].text.ends_with('-'),
            "hyphens:none should not insert hyphens, got: '{}'",
            lines[0].text
        );
    }

    #[test]
    fn test_hyphenation_manual_uses_soft_hyphens() {
        let tl = TextLayout::new();
        let fc = ctx();
        // "extra\u{00AD}ordinary" — soft hyphen between "extra" and "ordinary"
        let lines = tl.break_into_lines(
            &fc,
            "extra\u{00AD}ordinary",
            40.0, // narrow enough to trigger break
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Manual,
            None,
        );
        assert!(
            lines.len() >= 2,
            "Should break at soft hyphen, got {} lines",
            lines.len()
        );
        // First line should end with visible hyphen
        assert!(
            lines[0].text.ends_with('-'),
            "Should render visible hyphen at soft-hyphen break, got: '{}'",
            lines[0].text
        );
        // The soft hyphen itself should not appear in output
        for line in &lines {
            assert!(
                !line.text.contains('\u{00AD}'),
                "Soft hyphens should be filtered from output"
            );
        }
    }

    #[test]
    fn test_hyphenation_prefers_space_over_hyphen() {
        let tl = TextLayout::new();
        let fc = ctx();
        // "Hello extraordinary" — should break at space first
        let lines = tl.break_into_lines(
            &fc,
            "Hello extraordinary",
            60.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Auto,
            None,
        );
        assert!(lines.len() >= 2);
        // First line should break at the space, not hyphenate "Hello"
        assert!(
            lines[0].text.starts_with("Hello"),
            "Should break at space first, got: '{}'",
            lines[0].text
        );
    }

    #[test]
    fn test_min_content_width_with_hyphenation() {
        let tl = TextLayout::new();
        let fc = ctx();
        let auto_width = tl.measure_widest_word(
            &fc,
            "extraordinary",
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Auto,
            None,
        );
        let manual_width = tl.measure_widest_word(
            &fc,
            "extraordinary",
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Manual,
            None,
        );
        assert!(
            auto_width < manual_width,
            "Auto hyphenation min-content ({auto_width}) should be less than manual ({manual_width})"
        );
    }

    #[test]
    fn test_cjk_break_opportunities() {
        // UAX#14 should identify break opportunities between CJK chars
        let opps = compute_break_opportunities("\u{4F60}\u{597D}\u{4E16}\u{754C}"); // 你好世界
                                                                                    // Between CJK ideographs, UAX#14 should allow breaks
        let allowed_count = opps
            .iter()
            .filter(|o| matches!(o, Some(BreakOpportunity::Allowed)))
            .count();
        assert!(
            allowed_count >= 2,
            "Should have at least 2 break opportunities between 4 CJK chars, got {}",
            allowed_count
        );
    }

    #[test]
    fn test_hyphenation_german() {
        let tl = TextLayout::new();
        let fc = ctx();
        // German compound word — should hyphenate with lang "de"
        let lines = tl.break_into_lines(
            &fc,
            "Donaudampfschifffahrt",
            60.0,
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Auto,
            Some("de"),
        );
        assert!(
            lines.len() >= 2,
            "German word should hyphenate with lang='de', got {} lines",
            lines.len()
        );
        assert!(
            lines[0].text.ends_with('-'),
            "First line should end with hyphen, got: '{}'",
            lines[0].text
        );
    }

    #[test]
    fn test_hyphenation_unsupported_lang() {
        // Unknown lang disables algorithmic hyphenation
        let lang = resolve_hypher_lang(Some("xx-unknown"));
        assert!(lang.is_none(), "Unsupported language should return None");
    }

    #[test]
    fn test_resolve_hypher_lang_mapping() {
        assert!(matches!(
            resolve_hypher_lang(None),
            Some(hypher::Lang::English)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("en")),
            Some(hypher::Lang::English)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("en-US")),
            Some(hypher::Lang::English)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("de")),
            Some(hypher::Lang::German)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("fr")),
            Some(hypher::Lang::French)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("es")),
            Some(hypher::Lang::Spanish)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("nb")),
            Some(hypher::Lang::Norwegian)
        ));
        assert!(matches!(
            resolve_hypher_lang(Some("nn")),
            Some(hypher::Lang::Norwegian)
        ));
        assert!(resolve_hypher_lang(Some("zz")).is_none());
    }

    #[test]
    fn test_knuth_plass_fallback_to_greedy() {
        let tl = TextLayout::new();
        let fc = ctx();
        // Very narrow width — KP may fail, should fall back to greedy
        let lines = tl.break_into_lines_optimal(
            &fc,
            "Hello World",
            1.0, // impossibly narrow
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Manual,
            None,
            false,
        );
        assert!(
            !lines.is_empty(),
            "Should still produce lines via greedy fallback"
        );
    }

    #[test]
    fn test_min_content_width_without_hyphenation() {
        let tl = TextLayout::new();
        let fc = ctx();
        let manual_width = tl.measure_widest_word(
            &fc,
            "extraordinary",
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
            Hyphens::Manual,
            None,
        );
        let full_width = tl.measure_width(
            &fc,
            "extraordinary",
            12.0,
            "Helvetica",
            400,
            FontStyle::Normal,
            0.0,
        );
        assert!(
            (manual_width - full_width).abs() < 0.01,
            "Manual min-content ({manual_width}) should equal full word width ({full_width})"
        );
    }
}
