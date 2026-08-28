//! Style computation: UA defaults ⊕ inline style → concrete point values.
//!
//! The em-resolution order here is the correctness-critical part (the plan
//! calls it out explicitly): per CSS, `em` in `font-size` resolves against
//! the PARENT's font size, while `em` in every other property resolves
//! against the element's OWN computed font size. So `resolve` computes
//! font-size first, then resolves everything else against it. Getting this
//! wrong silently halves h1's UA margins and would make the spike's
//! margin-collapse assertion pass for the wrong reason.

use crate::css::{CssDisplay, CssStyle, Length, LineHeight};
use forme::style::{
    AlignItems, Color, Dimension, FlexDirection, JustifyContent, TextAlign, TextDecoration,
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

    pub display: CssDisplay,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f64>,
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
        display: css.display.unwrap_or(CssDisplay::Block),
        flex_direction: css.flex_direction,
        justify_content: css.justify_content,
        align_items: css.align_items,
        gap: css.gap,
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
