//! Style computation: UA defaults ⊕ inline style → concrete point values.
//!
//! The em-resolution order here is the correctness-critical part (the plan
//! calls it out explicitly): per CSS, `em` in `font-size` resolves against
//! the PARENT's font size, while `em` in every other property resolves
//! against the element's OWN computed font size. So `resolve` computes
//! font-size first, then resolves everything else against it. Getting this
//! wrong silently halves h1's UA margins and would make the spike's
//! margin-collapse assertion pass for the wrong reason.

use crate::css::{
    BreakInsideVal, BreakVal, ClearVal, CssDisplay, CssGridLine, CssStyle, CssTrack, CssTrackBound,
    FloatVal, Length, LineHeight,
};
use forme::style::{
    AlignItems, BorderStyle, Color, Dimension, FlexDirection, GridTrackSize, JustifyContent,
    TextAlign, TextDecoration, TextTransform, VerticalAlign,
};

/// CSS default `medium` (16px) in points. Matches the engine's own root
/// default of 12pt, which keeps the two systems agreeing about unstyled text.
pub const ROOT_FONT_SIZE: f64 = 12.0;

/// A margin edge value after resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarginV {
    Pt(f64),
    Auto,
}

/// Fully resolved style for one element: all lengths in points. Fields that
/// stay `None` were never specified (by UA or inline) and are left to the
/// engine's own inheritance during `Style::resolve`.
#[derive(Debug, Clone)]
pub struct Computed {
    /// Always resolved — the em context for this element's descendants.
    pub font_size: f64,
    /// Whether font-size was explicitly set (and should be emitted).
    pub font_size_explicit: bool,

    /// [top, right, bottom, left]
    pub margin: [MarginV; 4],
    pub padding: [f64; 4],
    pub border_width: [f64; 4],
    pub border_color: Option<Color>,
    pub border_style: [BorderStyle; 4],
    pub border_radius: Option<f64>,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,

    pub font_family: Option<String>,
    pub font_weight: Option<u32>,
    pub italic: Option<bool>,
    /// Multiplier of font size (CSS lengths are converted).
    pub line_height: Option<f64>,
    pub text_align: Option<TextAlign>,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub text_decoration: Option<TextDecoration>,
    pub text_transform: Option<TextTransform>,
    /// Resolved to points (em against the element's own font size).
    pub letter_spacing: Option<f64>,

    pub display: CssDisplay,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f64>,
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    /// Grid tracks resolved to engine sizes (em against this element's
    /// font size). Non-inherited, like all grid properties.
    pub grid_template_columns: Option<Vec<GridTrackSize>>,
    pub grid_template_rows: Option<Vec<GridTrackSize>>,
    pub grid_auto_rows: Option<GridTrackSize>,
    pub grid_auto_columns: Option<GridTrackSize>,
    /// Raw placement for this element AS a grid item (combined into the
    /// engine's GridPlacement in the mapper).
    pub grid_column: Option<CssGridLine>,
    pub grid_row: Option<CssGridLine>,

    pub border_collapse: Option<bool>,
    pub vertical_align: Option<VerticalAlign>,
    pub max_width: Option<Dimension>,
    pub min_width: Option<Dimension>,
    pub min_height: Option<Dimension>,
    pub position_absolute: bool,
    pub position_relative: bool,
    /// `position: running()` — the element is out of normal flow; the
    /// mapper drops it (warned at parse time).
    pub position_running: bool,
    /// `overflow-x: hidden` computed on this element. Honored on `body`
    /// (page-level clip); warned anywhere else.
    pub overflow_x_hidden: bool,
    /// Offsets in points, meaningful with `position_absolute` or
    /// `position_relative`.
    pub offsets: [Option<f64>; 4],
    pub break_before: Option<BreakVal>,
    pub break_after: Option<BreakVal>,
    pub break_inside: Option<BreakInsideVal>,
    pub orphans: Option<u32>,
    pub widows: Option<u32>,
    /// CSS Paged Media `page: <name>` (non-inherited; from the element's
    /// own declarations only).
    pub page: Option<String>,
    /// `float` (non-inherited). Drives the mapper's float-row transform.
    pub float: Option<FloatVal>,
    /// `clear` (non-inherited). Terminates a float run.
    pub clear: Option<ClearVal>,
}

/// Resolve a parsed CSS track to an engine track size. `em` resolves
/// against the element's own font size, matching every other property.
fn resolve_track(t: &CssTrack, font_size: f64, warnings: &mut Vec<String>) -> GridTrackSize {
    let len_pt = |l: &Length| -> f64 {
        match l {
            Length::Pt(v) => *v,
            Length::Em(e) => e * font_size,
            Length::Rem(r) => r * ROOT_FONT_SIZE,
            // Percent is rejected with a named warning at parse time.
            Length::Percent(_) | Length::Auto => 0.0,
        }
    };
    match t {
        CssTrack::Fr(f) => GridTrackSize::Fr(*f),
        CssTrack::Len(Length::Auto) => GridTrackSize::Auto,
        CssTrack::Len(l) => GridTrackSize::Pt(len_pt(l)),
        CssTrack::MinMax(min, max) => match (min, max) {
            // Tailwind's grid-cols-N emits repeat(N, minmax(0, 1fr)). The
            // engine's MinMax track never joins fr distribution (it
            // content-clamps), so mapping it literally would silently
            // content-size the columns. With a zero minimum, minmax(0, Xfr)
            // is exactly a plain Xfr track — normalize.
            (CssTrackBound::Len(l), CssTrackBound::Fr(f)) => {
                let min_pt = len_pt(l);
                if min_pt != 0.0 {
                    warnings.push(
                        "minmax(<length>, <fr>) with a nonzero minimum is not supported (the \
                         engine cannot flex a track with a floor); treated as a plain fr track"
                            .to_string(),
                    );
                }
                GridTrackSize::Fr(*f)
            }
            (CssTrackBound::Fr(_), _) => {
                warnings.push(
                    "fr is not valid as a minmax() minimum; the maximum bound is used".to_string(),
                );
                match max {
                    CssTrackBound::Fr(f) => GridTrackSize::Fr(*f),
                    CssTrackBound::Len(Length::Auto) => GridTrackSize::Auto,
                    CssTrackBound::Len(l) => GridTrackSize::Pt(len_pt(l)),
                }
            }
            (CssTrackBound::Len(a), CssTrackBound::Len(b)) => GridTrackSize::MinMax(
                Box::new(match a {
                    Length::Auto => GridTrackSize::Auto,
                    l => GridTrackSize::Pt(len_pt(l)),
                }),
                Box::new(match b {
                    Length::Auto => GridTrackSize::Auto,
                    l => GridTrackSize::Pt(len_pt(l)),
                }),
            ),
        },
    }
}

fn resolve_tracks(
    ts: &[CssTrack],
    font_size: f64,
    warnings: &mut Vec<String>,
) -> Vec<GridTrackSize> {
    ts.iter()
        .map(|t| resolve_track(t, font_size, warnings))
        .collect()
}

/// Resolve a merged declaration bag against the parent's computed font size.
pub fn resolve(css: &CssStyle, parent_font_size: f64, warnings: &mut Vec<String>) -> Computed {
    // font-size FIRST: em here is relative to the parent.
    let (font_size, font_size_explicit) = match css.font_size {
        None => (parent_font_size, false),
        Some(l) => (
            match l {
                Length::Pt(v) => v,
                Length::Em(e) => e * parent_font_size,
                Length::Rem(r) => r * ROOT_FONT_SIZE,
                Length::Percent(p) => p / 100.0 * parent_font_size,
                Length::Auto => parent_font_size,
            },
            true,
        ),
    };

    // Everything else: em is relative to the element's OWN font size.
    let to_pt = |l: Length, warnings: &mut Vec<String>, prop: &str| -> f64 {
        match l {
            Length::Pt(v) => v,
            Length::Em(e) => e * font_size,
            Length::Rem(r) => r * ROOT_FONT_SIZE,
            Length::Percent(_) => {
                warnings.push(format!("percentage {prop} is unsupported (treated as 0)"));
                0.0
            }
            Length::Auto => 0.0,
        }
    };

    let margin = [0, 1, 2, 3].map(|i| match css.margin[i] {
        None => MarginV::Pt(0.0),
        Some(Length::Auto) => MarginV::Auto,
        Some(l) => MarginV::Pt(to_pt(l, warnings, "margin")),
    });
    let padding = [0, 1, 2, 3].map(|i| match css.padding[i] {
        None => 0.0,
        Some(l) => to_pt(l, warnings, "padding"),
    });
    let border_width = [0, 1, 2, 3].map(|i| css.border_width[i].unwrap_or(0.0));
    let border_style = [0, 1, 2, 3].map(|i| css.border_style[i].unwrap_or(BorderStyle::Solid));

    let dim = |l: Option<Length>| -> Option<Dimension> {
        match l? {
            Length::Pt(v) => Some(Dimension::Pt(v)),
            Length::Em(e) => Some(Dimension::Pt(e * font_size)),
            Length::Rem(r) => Some(Dimension::Pt(r * ROOT_FONT_SIZE)),
            Length::Percent(p) => Some(Dimension::Percent(p)),
            Length::Auto => Some(Dimension::Auto),
        }
    };

    let line_height = css.line_height.map(|lh| match lh {
        LineHeight::Number(n) => n,
        LineHeight::Length(l) => {
            let pt = match l {
                Length::Pt(v) => v,
                Length::Em(e) => e * font_size,
                Length::Rem(r) => r * ROOT_FONT_SIZE,
                Length::Percent(p) => p / 100.0 * font_size,
                Length::Auto => font_size,
            };
            pt / font_size
        }
    });

    Computed {
        font_size,
        font_size_explicit,
        margin,
        padding,
        border_width,
        border_style,
        border_color: css.border_color,
        border_radius: css.border_radius,
        width: dim(css.width),
        height: dim(css.height),
        font_family: css.font_family.clone(),
        font_weight: css.font_weight,
        italic: css.italic,
        line_height,
        text_align: css.text_align,
        color: css.color,
        background_color: css.background_color,
        text_decoration: css.text_decoration,
        text_transform: css.text_transform,
        letter_spacing: css
            .letter_spacing
            .map(|l| to_pt(l, warnings, "letter-spacing")),
        display: {
            let d = css.display.unwrap_or(CssDisplay::Block);
            if d == CssDisplay::Grid && css.grid_template_columns.is_none() {
                warnings.push(
                    "display: grid without grid-template-columns behaves as a block".to_string(),
                );
                CssDisplay::Block
            } else {
                d
            }
        },
        flex_direction: css.flex_direction,
        justify_content: css.justify_content,
        align_items: css.align_items,
        gap: css.gap,
        row_gap: css.row_gap,
        column_gap: css.column_gap,
        grid_template_columns: css
            .grid_template_columns
            .as_ref()
            .map(|ts| resolve_tracks(ts, font_size, warnings)),
        grid_template_rows: css
            .grid_template_rows
            .as_ref()
            .map(|ts| resolve_tracks(ts, font_size, warnings)),
        grid_auto_rows: css
            .grid_auto_rows
            .as_ref()
            .map(|t| resolve_track(t, font_size, warnings)),
        grid_auto_columns: css
            .grid_auto_columns
            .as_ref()
            .map(|t| resolve_track(t, font_size, warnings)),
        grid_column: css.grid_column,
        grid_row: css.grid_row,
        vertical_align: css.vertical_align,
        max_width: dim(css.max_width),
        min_width: dim(css.min_width),
        min_height: dim(css.min_height),
        position_absolute: css.position_absolute == Some(true),
        position_relative: css.position_relative == Some(true),
        position_running: css.position_running == Some(true),
        overflow_x_hidden: css.overflow_x_hidden == Some(true),
        offsets: {
            // Offsets are meaningful for both absolute and relative; on a
            // static box they're inert (warned).
            let positioned =
                css.position_absolute == Some(true) || css.position_relative == Some(true);
            let mut out = [None; 4];
            for (i, l) in [css.top, css.right, css.bottom, css.left]
                .into_iter()
                .enumerate()
            {
                if let Some(l) = l {
                    if positioned {
                        out[i] = Some(to_pt(l, warnings, "offset"));
                    } else {
                        warnings.push(
                            "top/right/bottom/left without position: relative/absolute are unsupported"
                                .to_string(),
                        );
                        break;
                    }
                }
            }
            out
        },
        border_collapse: css.border_collapse,
        break_before: css.break_before,
        break_after: css.break_after,
        break_inside: css.break_inside,
        orphans: css.orphans,
        widows: css.widows,
        page: css.page.clone(),
        float: css.float,
        clear: css.clear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ua::ua_style;

    #[test]
    fn h1_em_margin_resolves_against_own_font_size() {
        // The plan's named trap: h1 { font-size: 2em; margin: 0.67em 0 }
        // at root 12pt must give font-size 24pt and margin 16.08pt — the
        // margin em resolves against h1's OWN 24pt, not the parent's 12pt.
        let mut w = Vec::new();
        let c = resolve(&ua_style("h1"), ROOT_FONT_SIZE, &mut w);
        assert!((c.font_size - 24.0).abs() < 1e-9);
        assert_eq!(c.margin[0], MarginV::Pt(0.67 * 24.0));
        assert_eq!(c.margin[2], MarginV::Pt(0.67 * 24.0));
    }

    #[test]
    fn font_size_em_resolves_against_parent() {
        let mut w = Vec::new();
        let css = CssStyle {
            font_size: Some(Length::Em(1.5)),
            ..Default::default()
        };
        let c = resolve(&css, 20.0, &mut w);
        assert!((c.font_size - 30.0).abs() < 1e-9);
    }

    #[test]
    fn p_margin_at_default_font_size() {
        let mut w = Vec::new();
        let c = resolve(&ua_style("p"), ROOT_FONT_SIZE, &mut w);
        assert_eq!(c.margin[0], MarginV::Pt(12.0));
    }

    #[test]
    fn percent_margin_warns() {
        let mut w = Vec::new();
        let mut css = CssStyle::default();
        css.margin[0] = Some(Length::Percent(10.0));
        resolve(&css, 12.0, &mut w);
        assert!(w.iter().any(|m| m.contains("percentage margin")));
    }
}
