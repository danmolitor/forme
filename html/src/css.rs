//! CSS declaration parsing via cssparser: property values, shorthand
//! expansion, and the `!important` split. Shared by inline `style=""`
//! attributes and stylesheet rule bodies (see `sheet.rs` for selectors
//! and the cascade). Unknown properties are collected into a warnings
//! list rather than silently dropped — the documented-subset contract.

use cssparser::{Delimiter, ParseError, Parser, ParserInput, Token};
use forme::style::{
    AlignItems, Color, FlexDirection, JustifyContent, TextAlign, TextDecoration, TextTransform,
};

/// A CSS length as written. Absolute units are normalized to points at parse
/// time; font-relative and percentage units survive until style resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Pt(f64),
    Em(f64),
    Rem(f64),
    Percent(f64),
    Auto,
}

/// `line-height` keeps its two CSS shapes: a bare number is a multiplier of
/// the element's own font size; a length is absolute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Number(f64),
    Length(Length),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssDisplay {
    Block,
    Flex,
    None,
}

/// `break-before` / `break-after` values the engine can honor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakVal {
    Auto,
    Page,
}

/// `break-inside` values the engine can honor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakInsideVal {
    Auto,
    Avoid,
}

/// The declaration bag for one element: UA defaults and the inline style
/// both produce one of these; `merge` layers them. Edge arrays are
/// [top, right, bottom, left].
#[derive(Debug, Clone, Default)]
pub struct CssStyle {
    pub margin: [Option<Length>; 4],
    pub padding: [Option<Length>; 4],
    pub border_width: [Option<f64>; 4],
    pub border_color: Option<Color>,
    pub border_radius: Option<f64>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub font_family: Option<String>,
    pub font_size: Option<Length>,
    pub font_weight: Option<u32>,
    pub italic: Option<bool>,
    pub line_height: Option<LineHeight>,
    pub text_align: Option<TextAlign>,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub display: Option<CssDisplay>,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f64>,
    pub text_decoration: Option<TextDecoration>,
    pub text_transform: Option<TextTransform>,
    pub letter_spacing: Option<Length>,
    pub border_collapse: Option<bool>,
    pub break_before: Option<BreakVal>,
    pub break_after: Option<BreakVal>,
    pub break_inside: Option<BreakInsideVal>,
    pub orphans: Option<u32>,
    pub widows: Option<u32>,
}

impl CssStyle {
    /// Layer `over` on top of `self`: any property `over` sets wins.
    pub fn merge(&self, over: &CssStyle) -> CssStyle {
        let mut out = self.clone();
        for i in 0..4 {
            if over.margin[i].is_some() {
                out.margin[i] = over.margin[i];
            }
            if over.padding[i].is_some() {
                out.padding[i] = over.padding[i];
            }
            if over.border_width[i].is_some() {
                out.border_width[i] = over.border_width[i];
            }
        }
        macro_rules! take {
            ($($field:ident),*) => {
                $( if over.$field.is_some() { out.$field = over.$field.clone(); } )*
            };
        }
        take!(
            border_color,
            border_radius,
            width,
            height,
            font_family,
            font_size,
            font_weight,
            italic,
            line_height,
            text_align,
            color,
            background_color,
            display,
            flex_direction,
            justify_content,
            align_items,
            gap,
            text_decoration,
            text_transform,
            letter_spacing,
            border_collapse,
            break_before,
            break_after,
            break_inside,
            orphans,
            widows
        );
        out
    }
}

/// A parsed declaration block, split by importance. `!important`
/// declarations cascade in a higher bucket than everything normal.
#[derive(Debug, Clone, Default)]
pub struct DeclBlock {
    pub normal: CssStyle,
    pub important: CssStyle,
}

/// Parse a `style=""` attribute value. Malformed declarations are skipped
/// (per CSS error recovery); unknown-but-well-formed properties land in
/// `warnings`.
pub fn parse_style_attr(input: &str, warnings: &mut Vec<String>) -> DeclBlock {
    let mut block = DeclBlock::default();
    let mut pin = ParserInput::new(input);
    let mut parser = Parser::new(&mut pin);
    parse_declarations(&mut parser, &mut block, warnings);
    block
}

/// The shared declaration-list loop: used for `style=""` attributes and
/// for rule bodies inside stylesheets.
pub(crate) fn parse_declarations(
    parser: &mut Parser<'_, '_>,
    block: &mut DeclBlock,
    warnings: &mut Vec<String>,
) {
    while !parser.is_exhausted() {
        let _ = parser.parse_until_after(
            Delimiter::Semicolon,
            |p| -> Result<(), ParseError<'_, ()>> {
                let name = p.expect_ident()?.to_ascii_lowercase();
                p.expect_colon()?;
                // Bound the value at `!` so multi-token value parsers
                // (margin shorthand, border, font-family) can't swallow a
                // trailing `!important`.
                let mut decl = CssStyle::default();
                let _ =
                    p.parse_until_before(Delimiter::Bang, |p| -> Result<(), ParseError<'_, ()>> {
                        apply_declaration(&name, p, &mut decl, warnings);
                        Ok(())
                    });
                let important = p
                    .try_parse(|p| -> Result<(), ParseError<'_, ()>> {
                        p.expect_delim('!')?;
                        let id = p.expect_ident()?;
                        if id.eq_ignore_ascii_case("important") {
                            Ok(())
                        } else {
                            Err(p.new_custom_error(()))
                        }
                    })
                    .is_ok();
                if important {
                    block.important = block.important.merge(&decl);
                } else {
                    block.normal = block.normal.merge(&decl);
                }
                Ok(())
            },
        );
    }
}

/// Dispatch one declaration into the style bag.
pub(crate) fn apply_declaration(
    name: &str,
    p: &mut Parser<'_, '_>,
    style: &mut CssStyle,
    warnings: &mut Vec<String>,
) {
    match name {
        "margin" => {
            if let Some(vals) = parse_lengths(p) {
                style.margin = expand4(&vals).map(Some);
            }
        }
        "margin-top" => style.margin[0] = parse_length(p),
        "margin-right" => style.margin[1] = parse_length(p),
        "margin-bottom" => style.margin[2] = parse_length(p),
        "margin-left" => style.margin[3] = parse_length(p),

        "padding" => {
            if let Some(vals) = parse_lengths(p) {
                style.padding = expand4(&vals).map(Some);
            }
        }
        "padding-top" => style.padding[0] = parse_length(p),
        "padding-right" => style.padding[1] = parse_length(p),
        "padding-bottom" => style.padding[2] = parse_length(p),
        "padding-left" => style.padding[3] = parse_length(p),

        "border" => {
            let (width, color) = parse_border_shorthand(p);
            style.border_width = [width; 4].map(Some);
            if color.is_some() {
                style.border_color = color;
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let idx = match name {
                "border-top" => 0,
                "border-right" => 1,
                "border-bottom" => 2,
                _ => 3,
            };
            let (width, color) = parse_border_shorthand(p);
            style.border_width[idx] = Some(width);
            if color.is_some() {
                style.border_color = color;
            }
        }
        "border-width" => {
            if let Some(w) = parse_border_width_token(p) {
                style.border_width = [w; 4].map(Some);
            }
        }
        "border-color" => style.border_color = parse_color(p),
        "border-radius" => {
            if let Some(Length::Pt(v)) = parse_length(p) {
                style.border_radius = Some(v);
            }
        }

        "width" => style.width = parse_length(p),
        "height" => style.height = parse_length(p),

        "font-family" => style.font_family = parse_font_family(p),
        "font-size" => style.font_size = parse_length(p),
        "font-weight" => style.font_weight = parse_font_weight(p, warnings),
        "font-style" => {
            if let Ok(id) = p.expect_ident() {
                style.italic = Some(id.eq_ignore_ascii_case("italic"));
            }
        }
        "line-height" => style.line_height = parse_line_height(p),
        "text-align" => {
            if let Ok(id) = p.expect_ident() {
                style.text_align = match id.to_ascii_lowercase().as_str() {
                    "left" => Some(TextAlign::Left),
                    "right" => Some(TextAlign::Right),
                    "center" => Some(TextAlign::Center),
                    "justify" => Some(TextAlign::Justify),
                    _ => None,
                };
            }
        }
        "color" => style.color = parse_color(p),
        "background-color" | "background" => {
            // `background` shorthand: spike accepts a bare color, anything
            // fancier (gradients, images) is a warning.
            match parse_color(p) {
                Some(c) => style.background_color = Some(c),
                None => warnings.push(format!("unsupported value for '{name}'")),
            }
        }

        "display" => {
            if let Ok(id) = p.expect_ident() {
                style.display = match id.to_ascii_lowercase().as_str() {
                    "block" => Some(CssDisplay::Block),
                    "flex" => Some(CssDisplay::Flex),
                    "none" => Some(CssDisplay::None),
                    other => {
                        warnings.push(format!(
                            "unsupported display value '{other}' (treated as block)"
                        ));
                        Some(CssDisplay::Block)
                    }
                };
            }
        }
        "flex-direction" => {
            if let Ok(id) = p.expect_ident() {
                style.flex_direction = match id.to_ascii_lowercase().as_str() {
                    "row" => Some(FlexDirection::Row),
                    "column" => Some(FlexDirection::Column),
                    "row-reverse" => Some(FlexDirection::RowReverse),
                    "column-reverse" => Some(FlexDirection::ColumnReverse),
                    _ => None,
                };
            }
        }
        "justify-content" => {
            if let Ok(id) = p.expect_ident() {
                style.justify_content = match id.to_ascii_lowercase().as_str() {
                    "flex-start" | "start" => Some(JustifyContent::FlexStart),
                    "flex-end" | "end" => Some(JustifyContent::FlexEnd),
                    "center" => Some(JustifyContent::Center),
                    "space-between" => Some(JustifyContent::SpaceBetween),
                    "space-around" => Some(JustifyContent::SpaceAround),
                    "space-evenly" => Some(JustifyContent::SpaceEvenly),
                    _ => None,
                };
            }
        }
        "align-items" => {
            if let Ok(id) = p.expect_ident() {
                style.align_items = match id.to_ascii_lowercase().as_str() {
                    "flex-start" | "start" => Some(AlignItems::FlexStart),
                    "flex-end" | "end" => Some(AlignItems::FlexEnd),
                    "center" => Some(AlignItems::Center),
                    "stretch" => Some(AlignItems::Stretch),
                    "baseline" => Some(AlignItems::Baseline),
                    _ => None,
                };
            }
        }
        "gap" => {
            if let Some(Length::Pt(v)) = parse_length(p) {
                style.gap = Some(v);
            }
        }
        "text-decoration" | "text-decoration-line" => {
            if let Ok(id) = p.expect_ident() {
                style.text_decoration = match id.to_ascii_lowercase().as_str() {
                    "underline" => Some(TextDecoration::Underline),
                    "line-through" => Some(TextDecoration::LineThrough),
                    "none" => Some(TextDecoration::None),
                    _ => None,
                };
            }
        }

        "text-transform" => {
            if let Ok(id) = p.expect_ident() {
                style.text_transform = match id.to_ascii_lowercase().as_str() {
                    "uppercase" => Some(TextTransform::Uppercase),
                    "lowercase" => Some(TextTransform::Lowercase),
                    "capitalize" => Some(TextTransform::Capitalize),
                    "none" => Some(TextTransform::None),
                    other => {
                        warnings.push(format!("unsupported text-transform value '{other}'"));
                        None
                    }
                };
            }
        }
        "letter-spacing" => {
            let tok = p.next().ok().cloned();
            style.letter_spacing = match tok.as_ref() {
                Some(Token::Ident(id)) if id.eq_ignore_ascii_case("normal") => {
                    Some(Length::Pt(0.0))
                }
                Some(t) => token_to_length(t),
                None => None,
            };
        }
        "border-collapse" => {
            if let Ok(id) = p.expect_ident() {
                style.border_collapse = match id.to_ascii_lowercase().as_str() {
                    "collapse" => Some(true),
                    "separate" => Some(false),
                    _ => None,
                };
            }
        }

        // Modern names and the wkhtmltopdf-era `page-break-*` legacy
        // aliases the migration audience's templates actually use.
        "break-before" | "page-break-before" => {
            style.break_before = parse_break_value(name, p, warnings);
        }
        "break-after" | "page-break-after" => {
            style.break_after = parse_break_value(name, p, warnings);
        }
        "break-inside" | "page-break-inside" => {
            if let Ok(id) = p.expect_ident() {
                style.break_inside = match id.to_ascii_lowercase().as_str() {
                    "avoid" | "avoid-page" => Some(BreakInsideVal::Avoid),
                    "auto" => Some(BreakInsideVal::Auto),
                    other => {
                        warnings.push(format!("unsupported {name} value '{other}'"));
                        None
                    }
                };
            }
        }
        "orphans" => style.orphans = parse_count(p),
        "widows" => style.widows = parse_count(p),

        other => {
            warnings.push(format!("unsupported property: {other}"));
        }
    }
}

/// `page` / `always` force a break; `left`/`right` need the `:left`/`:right`
/// page machinery and are reported; `avoid` (breaks) is not supported.
fn parse_break_value(
    name: &str,
    p: &mut Parser<'_, '_>,
    warnings: &mut Vec<String>,
) -> Option<BreakVal> {
    let id = p.next().ok()?.clone();
    let cssparser::Token::Ident(id) = id else {
        return None;
    };
    match id.to_ascii_lowercase().as_str() {
        "page" | "always" => Some(BreakVal::Page),
        "auto" => Some(BreakVal::Auto),
        other @ ("left" | "right" | "recto" | "verso") => {
            warnings.push(format!(
                "{name}: {other} needs :left/:right page support (treated as 'page')"
            ));
            Some(BreakVal::Page)
        }
        other => {
            warnings.push(format!("unsupported {name} value '{other}'"));
            None
        }
    }
}

/// A small positive integer (orphans/widows).
fn parse_count(p: &mut Parser<'_, '_>) -> Option<u32> {
    match p.next().ok()? {
        Token::Number { value, .. } if *value >= 1.0 => Some(*value as u32),
        _ => None,
    }
}

/// Convert an absolute CSS unit to points. Returns None for unknown units.
fn unit_to_length(value: f64, unit: &str) -> Option<Length> {
    match unit.to_ascii_lowercase().as_str() {
        "px" => Some(Length::Pt(value * 0.75)),
        "pt" => Some(Length::Pt(value)),
        "in" => Some(Length::Pt(value * 72.0)),
        "cm" => Some(Length::Pt(value * 72.0 / 2.54)),
        "mm" => Some(Length::Pt(value * 72.0 / 25.4)),
        "em" => Some(Length::Em(value)),
        "rem" => Some(Length::Rem(value)),
        _ => None,
    }
}

fn parse_length(p: &mut Parser<'_, '_>) -> Option<Length> {
    let tok = p.next().ok()?.clone();
    token_to_length(&tok)
}

pub(crate) fn token_to_length(tok: &Token) -> Option<Length> {
    match tok {
        Token::Dimension { value, unit, .. } => unit_to_length(*value as f64, unit),
        Token::Percentage { unit_value, .. } => Some(Length::Percent(*unit_value as f64 * 100.0)),
        Token::Number { value, .. } if *value == 0.0 => Some(Length::Pt(0.0)),
        Token::Ident(id) if id.eq_ignore_ascii_case("auto") => Some(Length::Auto),
        _ => None,
    }
}

/// Parse a run of 1-4 lengths (for margin/padding shorthand).
fn parse_lengths(p: &mut Parser<'_, '_>) -> Option<Vec<Length>> {
    let mut out = Vec::new();
    while let Ok(tok) = p.next() {
        let tok = tok.clone();
        out.push(token_to_length(&tok)?);
    }
    if out.is_empty() || out.len() > 4 {
        None
    } else {
        Some(out)
    }
}

/// CSS 1-4 value shorthand expansion → [top, right, bottom, left].
fn expand4(vals: &[Length]) -> [Length; 4] {
    match vals {
        [a] => [*a, *a, *a, *a],
        [v, h] => [*v, *h, *v, *h],
        [t, h, b] => [*t, *h, *b, *h],
        [t, r, b, l] => [*t, *r, *b, *l],
        _ => unreachable!("parse_lengths bounds the count"),
    }
}

/// `border: 1px solid #000` — width, style keyword (recognized, ignored),
/// and color in any order. `none`/`hidden` zero the width.
fn parse_border_shorthand(p: &mut Parser<'_, '_>) -> (f64, Option<Color>) {
    let mut width = 3.0 * 0.75; // CSS `medium`
    let mut color = None;
    while let Ok(tok) = p.next() {
        let tok = tok.clone();
        match &tok {
            Token::Dimension { value, unit, .. } => {
                if let Some(Length::Pt(v)) = unit_to_length(*value as f64, unit) {
                    width = v;
                }
            }
            Token::Number { value, .. } if *value == 0.0 => width = 0.0,
            Token::Ident(id) => match id.to_ascii_lowercase().as_str() {
                "thin" => width = 0.75,
                "medium" => width = 2.25,
                "thick" => width = 3.75,
                "none" | "hidden" => width = 0.0,
                "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset"
                | "outset" => {}
                name => {
                    if let Some(c) = named_color(name) {
                        color = Some(c);
                    }
                }
            },
            Token::Hash(s) | Token::IDHash(s) => color = Some(Color::hex(s)),
            Token::Function(f) => {
                if let Some(c) = parse_color_function(f.as_ref().to_ascii_lowercase(), p) {
                    color = Some(c);
                }
            }
            _ => {}
        }
    }
    (width, color)
}

fn parse_border_width_token(p: &mut Parser<'_, '_>) -> Option<f64> {
    let tok = p.next().ok()?.clone();
    match &tok {
        Token::Dimension { value, unit, .. } => match unit_to_length(*value as f64, unit) {
            Some(Length::Pt(v)) => Some(v),
            _ => None,
        },
        Token::Number { value, .. } if *value == 0.0 => Some(0.0),
        Token::Ident(id) => match id.to_ascii_lowercase().as_str() {
            "thin" => Some(0.75),
            "medium" => Some(2.25),
            "thick" => Some(3.75),
            _ => None,
        },
        _ => None,
    }
}

fn parse_font_family(p: &mut Parser<'_, '_>) -> Option<String> {
    let mut families = Vec::new();
    let mut current = Vec::new();
    while let Ok(tok) = p.next() {
        let tok = tok.clone();
        match &tok {
            Token::Ident(id) => current.push(id.as_ref().to_string()),
            Token::QuotedString(s) => current.push(s.as_ref().to_string()),
            Token::Comma if !current.is_empty() => {
                families.push(current.join(" "));
                current.clear();
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        families.push(current.join(" "));
    }
    if families.is_empty() {
        None
    } else {
        // The engine's FontRegistry already understands comma-separated
        // fallback chains and falls back to Helvetica at the end.
        Some(families.join(", "))
    }
}

fn parse_font_weight(p: &mut Parser<'_, '_>, warnings: &mut Vec<String>) -> Option<u32> {
    let tok = p.next().ok()?.clone();
    match &tok {
        Token::Number { value, .. } => Some((*value as u32).clamp(100, 900)),
        Token::Ident(id) => match id.to_ascii_lowercase().as_str() {
            "bold" => Some(700),
            "normal" => Some(400),
            other => {
                warnings.push(format!("unsupported font-weight '{other}'"));
                None
            }
        },
        _ => None,
    }
}

fn parse_line_height(p: &mut Parser<'_, '_>) -> Option<LineHeight> {
    let tok = p.next().ok()?.clone();
    match &tok {
        Token::Number { value, .. } => Some(LineHeight::Number(*value as f64)),
        _ => token_to_length(&tok).map(LineHeight::Length),
    }
}

fn parse_color(p: &mut Parser<'_, '_>) -> Option<Color> {
    let tok = p.next().ok()?.clone();
    match &tok {
        Token::Hash(s) | Token::IDHash(s) => Some(Color::hex(s)),
        Token::Ident(id) => named_color(&id.to_ascii_lowercase()),
        Token::Function(f) => parse_color_function(f.as_ref().to_ascii_lowercase(), p),
        _ => None,
    }
}

/// rgb()/rgba() with comma or space syntax. Percentages scale to 255.
fn parse_color_function(name: String, p: &mut Parser<'_, '_>) -> Option<Color> {
    if name != "rgb" && name != "rgba" {
        return None;
    }
    let vals: Vec<f64> = p
        .parse_nested_block(|p| -> Result<Vec<f64>, ParseError<'_, ()>> {
            let mut vals = Vec::new();
            while let Ok(tok) = p.next() {
                match tok {
                    Token::Number { value, .. } => vals.push(*value as f64),
                    Token::Percentage { unit_value, .. } => vals.push(*unit_value as f64 * 255.0),
                    Token::Comma | Token::Delim('/') => {}
                    _ => {}
                }
            }
            Ok(vals)
        })
        .ok()?;
    if vals.len() < 3 {
        return None;
    }
    Some(Color {
        r: (vals[0] / 255.0).clamp(0.0, 1.0),
        g: (vals[1] / 255.0).clamp(0.0, 1.0),
        b: (vals[2] / 255.0).clamp(0.0, 1.0),
        // rgba's alpha is already 0-1.
        a: vals.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0),
    })
}

fn named_color(name: &str) -> Option<Color> {
    let hex = match name {
        "black" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "green" => "#008000",
        "blue" => "#0000ff",
        "yellow" => "#ffff00",
        "orange" => "#ffa500",
        "purple" => "#800080",
        "gray" | "grey" => "#808080",
        "silver" => "#c0c0c0",
        "lightgray" | "lightgrey" => "#d3d3d3",
        "darkgray" | "darkgrey" => "#a9a9a9",
        "navy" => "#000080",
        "teal" => "#008080",
        "maroon" => "#800000",
        "transparent" => return Some(Color::TRANSPARENT),
        _ => return None,
    };
    Some(Color::hex(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> (CssStyle, Vec<String>) {
        let mut warnings = Vec::new();
        let block = parse_style_attr(s, &mut warnings);
        (block.normal, warnings)
    }

    #[test]
    fn important_lands_in_its_own_bucket() {
        let mut w = Vec::new();
        let block = parse_style_attr("color: red !important; font-weight: bold", &mut w);
        assert!(block.important.color.is_some());
        assert!(block.normal.color.is_none());
        assert_eq!(block.normal.font_weight, Some(700));
    }

    #[test]
    fn important_after_shorthand_value() {
        // The Bang delimiter must stop the multi-token margin parser.
        let mut w = Vec::new();
        let block = parse_style_attr("margin: 8px 16px !important", &mut w);
        assert_eq!(block.important.margin[0], Some(Length::Pt(6.0)));
        assert!(block.normal.margin[0].is_none());
    }

    #[test]
    fn margin_shorthand_two_values() {
        let (s, w) = parse("margin: 8px 16px");
        assert_eq!(s.margin[0], Some(Length::Pt(6.0)));
        assert_eq!(s.margin[1], Some(Length::Pt(12.0)));
        assert_eq!(s.margin[2], Some(Length::Pt(6.0)));
        assert_eq!(s.margin[3], Some(Length::Pt(12.0)));
        assert!(w.is_empty());
    }

    #[test]
    fn border_shorthand() {
        let (s, _) = parse("border: 1px solid #333");
        assert_eq!(s.border_width[0], Some(0.75));
        assert!(s.border_color.is_some());
    }

    #[test]
    fn unknown_property_warns() {
        let (_, w) = parse("transform: rotate(3deg); color: red");
        assert!(w.iter().any(|m| m.contains("transform")));
    }

    #[test]
    fn unknown_property_does_not_eat_next_declaration() {
        let (s, _) = parse("transform: rotate(3deg); color: red");
        assert!(s.color.is_some());
    }

    #[test]
    fn em_and_percent_survive_parsing() {
        let (s, _) = parse("font-size: 1.5em; width: 50%");
        assert_eq!(s.font_size, Some(Length::Em(1.5)));
        assert_eq!(s.width, Some(Length::Percent(50.0)));
    }

    #[test]
    fn rgb_function_color() {
        let (s, _) = parse("color: rgb(255, 0, 0)");
        let c = s.color.unwrap();
        assert!((c.r - 1.0).abs() < 1e-9 && c.g == 0.0);
    }

    #[test]
    fn malformed_declaration_recovers() {
        let (s, _) = parse("color:; font-weight: bold");
        assert_eq!(s.font_weight, Some(700));
    }
}
