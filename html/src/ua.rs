//! User-agent default styles, keyed by tag name.
//!
//! No selector engine — the spike is inline-styles-only, and UA defaults
//! only ever match by element name. Values mirror the Chrome UA stylesheet
//! (the reference the fixture is compared against), expressed in em where
//! Chrome uses em so that per-element em-resolution is exercised for real:
//! h1's `margin: 0.67em 0` must resolve against h1's own 2em font-size.

use crate::css::{CssStyle, Length};
use forme::style::{Color, TextAlign};

/// Look up the UA default style for a tag. Returns an empty style for
/// unknown tags.
pub fn ua_style(tag: &str) -> CssStyle {
    let mut s = CssStyle::default();
    match tag {
        "body" => {
            // Chrome: body { margin: 8px }
            s.margin = [Some(Length::Pt(6.0)); 4];
        }
        "h1" => heading(&mut s, 2.0, 0.67),
        "h2" => heading(&mut s, 1.5, 0.83),
        "h3" => heading(&mut s, 1.17, 1.0),
        "h4" => heading(&mut s, 1.0, 1.33),
        "h5" => heading(&mut s, 0.83, 1.67),
        "h6" => heading(&mut s, 0.67, 2.33),
        "p" | "blockquote" => {
            s.margin[0] = Some(Length::Em(1.0));
            s.margin[2] = Some(Length::Em(1.0));
            if tag == "blockquote" {
                s.margin[1] = Some(Length::Pt(30.0)); // 40px
                s.margin[3] = Some(Length::Pt(30.0));
            }
        }
        "ul" | "ol" => {
            s.margin[0] = Some(Length::Em(1.0));
            s.margin[2] = Some(Length::Em(1.0));
            s.padding[3] = Some(Length::Pt(30.0)); // 40px
        }
        "b" | "strong" | "th" => {
            s.font_weight = Some(700);
            if tag == "th" {
                s.text_align = Some(TextAlign::Center);
                s.padding = [Some(Length::Pt(0.75)); 4]; // 1px
            }
        }
        "i" | "em" | "address" => s.italic = Some(true),
        "u" => s.text_decoration = Some(forme::style::TextDecoration::Underline),
        "s" | "strike" | "del" => {
            s.text_decoration = Some(forme::style::TextDecoration::LineThrough)
        }
        "a" => {
            s.color = Some(Color::hex("#0000ee"));
            s.text_decoration = Some(forme::style::TextDecoration::Underline);
        }
        "td" => {
            s.padding = [Some(Length::Pt(0.75)); 4]; // 1px
        }
        "small" => s.font_size = Some(Length::Em(0.83)),
        "code" | "pre" => s.font_family = Some("Courier".to_string()),
        "hr" => {
            s.margin[0] = Some(Length::Em(0.5));
            s.margin[2] = Some(Length::Em(0.5));
            s.border_width = [Some(0.75); 4].map(|w| w.map(|_| 0.0));
            s.border_width[0] = Some(0.75);
            s.border_color = Some(Color::hex("#808080"));
        }
        _ => {}
    }
    s
}

fn heading(s: &mut CssStyle, font_em: f64, margin_em: f64) {
    s.font_size = Some(Length::Em(font_em));
    s.font_weight = Some(700);
    s.margin[0] = Some(Length::Em(margin_em));
    s.margin[2] = Some(Length::Em(margin_em));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h1_margins_are_em_based() {
        let s = ua_style("h1");
        assert_eq!(s.font_size, Some(Length::Em(2.0)));
        assert_eq!(s.margin[0], Some(Length::Em(0.67)));
        assert_eq!(s.font_weight, Some(700));
    }

    #[test]
    fn unknown_tag_is_empty() {
        let s = ua_style("custom-widget");
        assert!(s.margin.iter().all(|m| m.is_none()));
        assert!(s.font_size.is_none());
    }
}
