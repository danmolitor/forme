//! CSS declaration parsing via cssparser: property values, shorthand
//! expansion, and the `!important` split. Shared by inline `style=""`
//! attributes and stylesheet rule bodies (see `sheet.rs` for selectors
//! and the cascade). Unknown properties are collected into a warnings
//! list rather than silently dropped — the documented-subset contract.

use cssparser::{Delimiter, ParseError, Parser, ParserInput, Token};
use forme::style::{
    AlignItems, BorderStyle, Color, FlexDirection, JustifyContent, TextAlign, TextDecoration,
    TextTransform, VerticalAlign,
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
    Grid,
    None,
}

/// A parsed grid track size. Lengths stay unresolved (`em` needs the
/// element's computed font size, which only style.rs has).
#[derive(Debug, Clone, PartialEq)]
pub enum CssTrack {
    /// Fixed length or `auto` (`Length::Auto`).
    Len(Length),
    /// Fractional unit.
    Fr(f64),
    /// `minmax(min, max)`.
    MinMax(CssTrackBound, CssTrackBound),
}

/// A bound inside `minmax()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssTrackBound {
    Len(Length),
    Fr(f64),
}

/// Parsed `grid-column` / `grid-row` placement for one axis.
/// Line numbers are 1-based and positive — negative lines warn at parse
/// (the engine clamps them to line 1, which is silently wrong vs CSS's
/// count-from-the-end semantics).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CssGridLine {
    pub start: Option<i32>,
    pub end: Option<i32>,
    pub span: Option<u32>,
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
    /// Per-side border line style (top, right, bottom, left).
    pub border_style: [Option<BorderStyle>; 4],
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
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    pub grid_template_columns: Option<Vec<CssTrack>>,
    pub grid_template_rows: Option<Vec<CssTrack>>,
    pub grid_auto_rows: Option<CssTrack>,
    pub grid_auto_columns: Option<CssTrack>,
    pub grid_column: Option<CssGridLine>,
    pub grid_row: Option<CssGridLine>,
    pub text_decoration: Option<TextDecoration>,
    pub text_transform: Option<TextTransform>,
    pub letter_spacing: Option<Length>,
    pub vertical_align: Option<VerticalAlign>,
    pub max_width: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    pub position_absolute: Option<bool>,
    pub position_relative: Option<bool>,
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub border_collapse: Option<bool>,
    pub break_before: Option<BreakVal>,
    pub break_after: Option<BreakVal>,
    pub break_inside: Option<BreakInsideVal>,
    pub orphans: Option<u32>,
    pub widows: Option<u32>,
    /// CSS Paged Media `page: <name>` — assigns the element to a named
    /// page (forces breaks between differently named boxes).
    pub page: Option<String>,
    /// `float: left | right` — consecutive floated siblings are laid out
    /// as a row by the mapper (the document subset; text never wraps
    /// AROUND a float).
    pub float: Option<FloatVal>,
    /// `clear` — terminates a float run; following content starts below.
    pub clear: Option<ClearVal>,
}

/// The supported `float` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatVal {
    Left,
    Right,
}

/// `clear` values (all treated as run terminators).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearVal {
    Left,
    Right,
    Both,
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
            if over.border_style[i].is_some() {
                out.border_style[i] = over.border_style[i];
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
            row_gap,
            column_gap,
            grid_template_columns,
            grid_template_rows,
            grid_auto_rows,
            grid_auto_columns,
            grid_column,
            grid_row,
            text_decoration,
            text_transform,
            letter_spacing,
            vertical_align,
            max_width,
            min_width,
            min_height,
            position_absolute,
            position_relative,
            top,
            right,
            bottom,
            left,
            border_collapse,
            break_before,
            break_after,
            break_inside,
            orphans,
            widows,
            page,
            float,
            clear
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
            let (width, color, bstyle) = parse_border_shorthand(p);
            style.border_width = [width; 4].map(Some);
            if color.is_some() {
                style.border_color = color;
            }
            if let Some(st) = bstyle {
                style.border_style = [Some(st); 4];
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let idx = match name {
                "border-top" => 0,
                "border-right" => 1,
                "border-bottom" => 2,
                _ => 3,
            };
            let (width, color, bstyle) = parse_border_shorthand(p);
            style.border_width[idx] = Some(width);
            if color.is_some() {
                style.border_color = color;
            }
            if let Some(st) = bstyle {
                style.border_style[idx] = Some(st);
            }
        }
        "border-width" => {
            if let Some(w) = parse_border_width_token(p) {
                style.border_width = [w; 4].map(Some);
            }
        }
        "border-color" => style.border_color = parse_color(p),
        // `border-style: <top> [<right> [<bottom> [<left>]]]` — 1–4 keywords,
        // CSS edge-shorthand expansion.
        "border-style" => {
            let mut vals = Vec::new();
            while let Ok(id) = p.expect_ident() {
                vals.push(border_style_keyword(id.as_ref()).unwrap_or(BorderStyle::Solid));
            }
            let expanded = match vals.as_slice() {
                [a] => [*a, *a, *a, *a],
                [a, b] => [*a, *b, *a, *b],
                [a, b, c] => [*a, *b, *c, *b],
                [a, b, c, d] => [*a, *b, *c, *d],
                _ => [BorderStyle::Solid; 4],
            };
            style.border_style = expanded.map(Some);
        }
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            let idx = match name {
                "border-top-style" => 0,
                "border-right-style" => 1,
                "border-bottom-style" => 2,
                _ => 3,
            };
            if let Ok(id) = p.expect_ident() {
                style.border_style[idx] =
                    Some(border_style_keyword(id.as_ref()).unwrap_or(BorderStyle::Solid));
            }
        }
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
                    "grid" => Some(CssDisplay::Grid),
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
            // 1 value: both axes. 2 values: <row-gap> <column-gap>.
            let first = parse_length(p);
            let second = parse_length(p);
            match (first, second) {
                (Some(Length::Pt(row)), Some(Length::Pt(col))) => {
                    style.row_gap = Some(row);
                    style.column_gap = Some(col);
                    style.gap = Some(row);
                }
                (Some(Length::Pt(v)), None) => {
                    style.gap = Some(v);
                    style.row_gap = Some(v);
                    style.column_gap = Some(v);
                }
                _ => {}
            }
        }
        "row-gap" => {
            if let Some(Length::Pt(v)) = parse_length(p) {
                style.row_gap = Some(v);
            }
        }
        "column-gap" => {
            if let Some(Length::Pt(v)) = parse_length(p) {
                style.column_gap = Some(v);
            }
        }
        "grid-template-columns" => {
            style.grid_template_columns = parse_track_list(p, name, warnings);
        }
        "grid-template-rows" => {
            style.grid_template_rows = parse_track_list(p, name, warnings);
        }
        "grid-auto-rows" => {
            style.grid_auto_rows = parse_track_list(p, name, warnings).and_then(|mut v| {
                if v.len() == 1 {
                    Some(v.remove(0))
                } else {
                    warnings.push(format!("'{name}' supports a single track size"));
                    None
                }
            });
        }
        "grid-auto-columns" => {
            style.grid_auto_columns = parse_track_list(p, name, warnings).and_then(|mut v| {
                if v.len() == 1 {
                    Some(v.remove(0))
                } else {
                    warnings.push(format!("'{name}' supports a single track size"));
                    None
                }
            });
        }
        "grid-column" => {
            style.grid_column = parse_grid_line(p, name, warnings);
        }
        "grid-row" => {
            style.grid_row = parse_grid_line(p, name, warnings);
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

        "position" => {
            // `running(<ident>)` is Paged-Media running elements — out of
            // subset, and worth naming (the generic ident path would
            // silently swallow the function token).
            let tok = p.next().cloned();
            if let Ok(cssparser::Token::Function(f)) = &tok {
                if f.eq_ignore_ascii_case("running") {
                    warnings.push(
                        "position: running() (running elements) is not supported — use @page margin boxes for running headers/footers"
                            .to_string(),
                    );
                } else {
                    warnings.push(format!("unsupported position value '{f}('"));
                }
                return;
            }
            if let Ok(cssparser::Token::Ident(id)) = tok {
                match id.to_ascii_lowercase().as_str() {
                    "absolute" => style.position_absolute = Some(true),
                    // `relative` stays in normal flow but its offsets paint;
                    // `static` is the plain default.
                    "relative" => style.position_relative = Some(true),
                    "static" => style.position_absolute = Some(false),
                    other @ ("fixed" | "sticky") => {
                        warnings.push(format!(
                            "position: {other} is unsupported (use a margin box for running content)"
                        ));
                    }
                    other => warnings.push(format!("unsupported position value '{other}'")),
                }
            }
        }
        "top" => style.top = parse_length(p),
        "right" => style.right = parse_length(p),
        "bottom" => style.bottom = parse_length(p),
        "left" => style.left = parse_length(p),

        // Floats, document subset: consecutive floated siblings lay out
        // as a row (the shape real templates use — Bootstrap columns,
        // left/right pairs). Text wrapping AROUND a float stays out; the
        // mapper warns when that case is actually hit.
        "float" => {
            if let Ok(id) = p.expect_ident() {
                style.float = match id.to_ascii_lowercase().as_str() {
                    "left" => Some(FloatVal::Left),
                    "right" => Some(FloatVal::Right),
                    "none" => None,
                    other => {
                        warnings.push(format!("unsupported float value '{other}'"));
                        None
                    }
                };
            }
        }
        "clear" => {
            if let Ok(id) = p.expect_ident() {
                style.clear = match id.to_ascii_lowercase().as_str() {
                    "left" => Some(ClearVal::Left),
                    "right" => Some(ClearVal::Right),
                    "both" => Some(ClearVal::Both),
                    "none" => None,
                    other => {
                        warnings.push(format!("unsupported clear value '{other}'"));
                        None
                    }
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

        // CSS Paged Media: assign the element to a named page. `auto` is
        // the initial value (no name).
        "page" => {
            if let Ok(id) = p.expect_ident() {
                let id = id.as_ref().to_string();
                if !id.eq_ignore_ascii_case("auto") {
                    style.page = Some(id);
                }
            }
        }

        "vertical-align" => {
            if let Ok(id) = p.expect_ident() {
                style.vertical_align = match id.to_ascii_lowercase().as_str() {
                    "top" => Some(VerticalAlign::Top),
                    "middle" => Some(VerticalAlign::Middle),
                    "bottom" => Some(VerticalAlign::Bottom),
                    "baseline" => Some(VerticalAlign::Baseline),
                    other => {
                        warnings.push(format!("unsupported vertical-align value '{other}'"));
                        None
                    }
                };
            }
        }
        "max-width" => style.max_width = parse_length(p),
        "min-width" => style.min_width = parse_length(p),
        "min-height" => style.min_height = parse_length(p),
        "max-height" => {
            warnings.push("max-height is pending (clipping semantics undecided)".to_string());
        }

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

/// Parse a grid track list (`grid-template-columns` / `-rows`).
///
/// The supported subset is deliberate: lengths, `fr`, `auto`,
/// `minmax(bound, bound)`, and integer `repeat()`. Everything the engine
/// cannot express — named lines, `auto-fill`/`auto-fit`, percentage
/// tracks, content-sized keywords — warns by name and drops the whole
/// declaration (a partially-applied template would mislay out silently).
fn parse_track_list(
    p: &mut Parser<'_, '_>,
    prop: &str,
    warnings: &mut Vec<String>,
) -> Option<Vec<CssTrack>> {
    let mut out = Vec::new();
    loop {
        let tok = match p.next() {
            Ok(t) => t.clone(),
            Err(_) => break,
        };
        match &tok {
            Token::SquareBracketBlock => {
                warnings.push(format!("'{prop}': named grid lines are not supported"));
                // Consume the bracket block so the parser stays sane.
                let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    while p.next().is_ok() {}
                    Ok(())
                });
                return None;
            }
            Token::Function(f) => {
                let fname = f.as_ref().to_ascii_lowercase();
                match fname.as_str() {
                    "repeat" => {
                        let expanded = parse_repeat(p, prop, warnings)?;
                        out.extend(expanded);
                    }
                    "minmax" => {
                        let mm = parse_minmax(p, prop, warnings)?;
                        out.push(mm);
                    }
                    other => {
                        warnings.push(format!("'{prop}': unsupported function '{other}()'"));
                        return None;
                    }
                }
            }
            _ => {
                let track = token_to_track(&tok, prop, warnings)?;
                out.push(track);
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// A single non-function track token → `CssTrack`, or a named warning.
fn token_to_track(tok: &Token, prop: &str, warnings: &mut Vec<String>) -> Option<CssTrack> {
    if let Token::Dimension { value, unit, .. } = tok {
        if unit.eq_ignore_ascii_case("fr") {
            return Some(CssTrack::Fr(*value as f64));
        }
    }
    if let Token::Ident(id) = tok {
        let id = id.to_ascii_lowercase();
        if id == "min-content" || id == "max-content" || id.starts_with("fit-content") {
            warnings.push(format!(
                "'{prop}': content-sized track '{id}' is not supported"
            ));
            return None;
        }
    }
    match token_to_length(tok) {
        Some(Length::Percent(_)) => {
            warnings.push(format!(
                "'{prop}': percentage track sizes are not supported"
            ));
            None
        }
        Some(len) => Some(CssTrack::Len(len)),
        None => {
            warnings.push(format!("'{prop}': unsupported track size"));
            None
        }
    }
}

/// Nested-block result: warnings collected inside the block (the closure
/// cannot borrow the outer warnings vec alongside the parser) + the value.
type TrackParse = (Vec<String>, Option<Vec<CssTrack>>);

/// `repeat(<integer>, <track-list>)`, expanded inline. `auto-fill` /
/// `auto-fit` need viewport-driven track counts the engine doesn't model.
fn parse_repeat(
    p: &mut Parser<'_, '_>,
    prop: &str,
    warnings: &mut Vec<String>,
) -> Option<Vec<CssTrack>> {
    let result = p.parse_nested_block(|p| -> Result<TrackParse, ParseError<'_, ()>> {
        let mut local_warnings = Vec::new();
        let count = match p.next() {
            Ok(Token::Number {
                int_value: Some(n), ..
            }) if *n > 0 => *n as usize,
            Ok(Token::Ident(id)) => {
                let id = id.to_ascii_lowercase();
                local_warnings.push(format!("'{prop}': repeat({id}, …) is not supported"));
                while p.next().is_ok() {}
                return Ok(drain(local_warnings, None));
            }
            _ => {
                local_warnings.push(format!(
                    "'{prop}': repeat() count must be a positive integer"
                ));
                while p.next().is_ok() {}
                return Ok(drain(local_warnings, None));
            }
        };
        let _comma = p.next(); // the comma after the count
        let mut pattern = Vec::new();
        while let Ok(tok) = p.next() {
            let tok = tok.clone();
            match &tok {
                Token::Function(f) if f.as_ref().eq_ignore_ascii_case("minmax") => {
                    match parse_minmax(p, prop, &mut local_warnings) {
                        Some(mm) => pattern.push(mm),
                        None => return Ok(drain(local_warnings, None)),
                    }
                }
                Token::SquareBracketBlock => {
                    local_warnings.push(format!("'{prop}': named grid lines are not supported"));
                    return Ok(drain(local_warnings, None));
                }
                _ => match token_to_track(&tok, prop, &mut local_warnings) {
                    Some(t) => pattern.push(t),
                    None => return Ok(drain(local_warnings, None)),
                },
            }
        }
        if pattern.is_empty() {
            return Ok(drain(local_warnings, None));
        }
        let mut expanded = Vec::with_capacity(count * pattern.len());
        for _ in 0..count {
            expanded.extend(pattern.iter().cloned());
        }
        Ok(drain(local_warnings, Some(expanded)))
    });
    match result {
        Ok((mut w, tracks)) => {
            warnings.append(&mut w);
            tracks
        }
        Err(_) => None,
    }
}

/// Bundle nested-block warnings with the result (the nested closure can't
/// borrow the outer warnings vec mutably alongside the parser).
fn drain<T>(warnings: Vec<String>, value: Option<T>) -> (Vec<String>, Option<T>) {
    (warnings, value)
}

/// `minmax(bound, bound)` — bounds are lengths, `fr`, or `auto`.
fn parse_minmax(
    p: &mut Parser<'_, '_>,
    prop: &str,
    warnings: &mut Vec<String>,
) -> Option<CssTrack> {
    let result = p.parse_nested_block(
        |p| -> Result<(Vec<String>, Option<CssTrack>), ParseError<'_, ()>> {
            let mut w = Vec::new();
            let mut bounds = Vec::new();
            while let Ok(tok) = p.next() {
                let tok = tok.clone();
                match &tok {
                    Token::Comma => {}
                    Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("fr") => {
                        bounds.push(CssTrackBound::Fr(*value as f64));
                    }
                    _ => match token_to_length(&tok) {
                        Some(Length::Percent(_)) => {
                            w.push(format!(
                                "'{prop}': percentage minmax() bounds are not supported"
                            ));
                            return Ok((w, None));
                        }
                        Some(len) => bounds.push(CssTrackBound::Len(len)),
                        None => {
                            w.push(format!("'{prop}': unsupported minmax() bound"));
                            return Ok((w, None));
                        }
                    },
                }
            }
            if bounds.len() != 2 {
                w.push(format!("'{prop}': minmax() takes exactly two bounds"));
                return Ok((w, None));
            }
            Ok((w, Some(CssTrack::MinMax(bounds[0], bounds[1]))))
        },
    );
    match result {
        Ok((mut w, track)) => {
            warnings.append(&mut w);
            track
        }
        Err(_) => None,
    }
}

/// `grid-column` / `grid-row`: `<int>`, `<int> / <int>`, `span <int>`,
/// `<int> / span <int>`. Negative line numbers warn — the engine clamps
/// them to line 1, which silently contradicts CSS's count-from-the-end.
fn parse_grid_line(
    p: &mut Parser<'_, '_>,
    prop: &str,
    warnings: &mut Vec<String>,
) -> Option<CssGridLine> {
    let mut line = CssGridLine::default();
    let mut after_slash = false;
    let mut pending_span = false;
    while let Ok(tok) = p.next() {
        match tok {
            Token::Delim('/') => after_slash = true,
            Token::Ident(id) if id.eq_ignore_ascii_case("span") => pending_span = true,
            Token::Ident(id) if id.eq_ignore_ascii_case("auto") => {}
            Token::Number {
                int_value: Some(n), ..
            } => {
                let n = *n;
                if n < 0 {
                    warnings.push(format!(
                        "'{prop}': negative grid line numbers are not supported (CSS counts from the end; the engine cannot)"
                    ));
                    return None;
                }
                if pending_span {
                    line.span = Some(n as u32);
                    pending_span = false;
                } else if after_slash {
                    line.end = Some(n);
                } else {
                    line.start = Some(n);
                }
            }
            _ => {
                warnings.push(format!("'{prop}': unsupported placement value"));
                return None;
            }
        }
    }
    if line.start.is_none() && line.end.is_none() && line.span.is_none() {
        None
    } else {
        Some(line)
    }
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
fn parse_border_shorthand(p: &mut Parser<'_, '_>) -> (f64, Option<Color>, Option<BorderStyle>) {
    let mut width = 3.0 * 0.75; // CSS `medium`
    let mut color = None;
    let mut style = None;
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
                "solid" => style = Some(BorderStyle::Solid),
                "dashed" => style = Some(BorderStyle::Dashed),
                "dotted" => style = Some(BorderStyle::Dotted),
                // double/groove/ridge/inset/outset are out of subset — they
                // fall back to solid (the default), no warning.
                "double" | "groove" | "ridge" | "inset" | "outset" => {}
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
    (width, color, style)
}

/// A single `border-style` keyword → engine `BorderStyle` (subset).
fn border_style_keyword(id: &str) -> Option<BorderStyle> {
    match id.to_ascii_lowercase().as_str() {
        "solid" | "none" | "hidden" => Some(BorderStyle::Solid),
        "dashed" => Some(BorderStyle::Dashed),
        "dotted" => Some(BorderStyle::Dotted),
        _ => None, // double/groove/... fall back to solid via absence
    }
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
