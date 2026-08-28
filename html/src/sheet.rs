//! Stylesheet parsing, the selector subset, and specificity.
//!
//! The v0 selector subset (documented in the README): type, `.class`,
//! `#id`, `*`, compounds (`td.amount`), descendant and child combinators,
//! and grouping (`h1, h2`). Everything else — attribute selectors,
//! pseudo-classes/elements, sibling combinators — is reported in the
//! warnings list and that selector is skipped (the rest of the group and
//! the rule body still apply; a friendlier recovery than CSS's
//! drop-the-whole-rule, and documented as such).
//!
//! Matching is hand-rolled right-to-left against an ancestor stack rather
//! than via Servo's `selectors` crate: the crate's `Element` trait needs
//! parent/sibling pointers our owned DOM doesn't have, and the subset
//! above needs ~150 lines total. Swappable later if the subset grows.

use crate::css::{parse_declarations, DeclBlock, Length};
use cssparser::{Parser, ParserInput, Token};

/// Page geometry from `@page` rules: size in points, margins as written
/// (em resolves against the root font size at application time).
/// Multiple `@page` blocks merge, later winning per-field.
#[derive(Debug, Clone, Default)]
pub struct PageRule {
    pub size: Option<(f64, f64)>,
    pub margin: [Option<Length>; 4],
}

impl PageRule {
    fn merge_from(&mut self, other: PageRule) {
        if other.size.is_some() {
            self.size = other.size;
        }
        for i in 0..4 {
            if other.margin[i].is_some() {
                self.margin[i] = other.margin[i];
            }
        }
    }
}

/// One compound selector: everything between combinators.
#[derive(Debug, Clone, Default)]
pub struct Compound {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// An explicit `*`. Matching-wise it's a no-op (an empty compound
    /// matches everything anyway); this flag only marks the compound as
    /// deliberately present so `* { ... }` isn't dropped as empty.
    pub universal: bool,
}

impl Compound {
    fn is_empty(&self) -> bool {
        !self.universal && self.tag.is_none() && self.id.is_none() && self.classes.is_empty()
    }

    fn matches(&self, key: &ElemKey) -> bool {
        if let Some(tag) = &self.tag {
            if *tag != key.tag {
                return false;
            }
        }
        if let Some(id) = &self.id {
            if key.id.as_deref() != Some(id.as_str()) {
                return false;
            }
        }
        self.classes.iter().all(|c| key.classes.contains(c))
    }
}

/// How a compound relates to the one on its left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

/// A complete selector: compounds left-to-right, with `combinators[i]`
/// relating `compounds[i]` (left) to `compounds[i+1]` (right).
#[derive(Debug, Clone)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<Combinator>,
    pub specificity: u32,
}

/// The identity of an element for selector matching.
#[derive(Debug, Clone)]
pub struct ElemKey {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

impl Selector {
    fn compute_specificity(compounds: &[Compound]) -> u32 {
        let mut ids = 0u32;
        let mut classes = 0u32;
        let mut types = 0u32;
        for c in compounds {
            if c.id.is_some() {
                ids += 1;
            }
            classes += c.classes.len() as u32;
            if c.tag.is_some() {
                types += 1;
            }
        }
        ids * 1_000_000 + classes * 1_000 + types
    }

    /// Match right-to-left against the element and its ancestor stack
    /// (`ancestors` runs root → parent). Backtracking on descendant
    /// combinators keeps `a b c` correct when multiple ancestors match `b`.
    pub fn matches(&self, key: &ElemKey, ancestors: &[ElemKey]) -> bool {
        let last = match self.compounds.last() {
            Some(c) => c,
            None => return false,
        };
        if !last.matches(key) {
            return false;
        }
        self.match_ancestors(self.compounds.len() - 1, ancestors.len(), ancestors)
    }

    /// `compound_idx` is the index of the compound already matched; try to
    /// match everything to its left against ancestors below `upper` (exclusive).
    fn match_ancestors(&self, compound_idx: usize, upper: usize, ancestors: &[ElemKey]) -> bool {
        if compound_idx == 0 {
            return true;
        }
        let next = &self.compounds[compound_idx - 1];
        match self.combinators[compound_idx - 1] {
            Combinator::Child => {
                if upper == 0 {
                    return false;
                }
                next.matches(&ancestors[upper - 1])
                    && self.match_ancestors(compound_idx - 1, upper - 1, ancestors)
            }
            Combinator::Descendant => {
                for j in (0..upper).rev() {
                    if next.matches(&ancestors[j])
                        && self.match_ancestors(compound_idx - 1, j, ancestors)
                    {
                        return true;
                    }
                }
                false
            }
        }
    }
}

/// One rule flattened per selector: `h1, h2 { ... }` becomes two rules
/// sharing the block, with source order preserved for cascade ties.
#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub block: DeclBlock,
    pub order: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    /// Merged geometry from base `@page` rules ( `:first`/`:left`/`:right`
    /// variants are reported as pending and not yet applied).
    pub page: Option<PageRule>,
}

impl Stylesheet {
    /// Append another stylesheet's rules after this one's (later origin
    /// wins cascade ties at equal specificity).
    pub fn append(&mut self, mut other: Stylesheet) {
        let base = self.rules.len();
        for r in &mut other.rules {
            r.order += base;
        }
        self.rules.extend(other.rules);
        if let Some(pr) = other.page {
            self.page
                .get_or_insert_with(PageRule::default)
                .merge_from(pr);
        }
    }
}

/// Parse a stylesheet string. At-rules (`@media`, `@page`, `@import`, ...)
/// are skipped with a warning — `@page` is the Phase 2 feature.
pub fn parse_stylesheet(css: &str, warnings: &mut Vec<String>) -> Stylesheet {
    let mut sheet = Stylesheet::default();
    let mut pin = ParserInput::new(css);
    let mut parser = Parser::new(&mut pin);

    let mut prelude = SelectorPrelude::new();
    loop {
        let tok = match parser.next_including_whitespace() {
            Ok(t) => t.clone(),
            Err(_) => break,
        };
        match &tok {
            Token::AtKeyword(name) if name.eq_ignore_ascii_case("page") => {
                parse_page_rule(&mut parser, &mut sheet, warnings);
                prelude = SelectorPrelude::new();
            }
            Token::AtKeyword(name) => {
                warnings.push(format!("@{name} rules are unsupported (skipped)"));
                // Consume the at-rule: everything up to a `;` (statement
                // form) or through its `{}` block.
                loop {
                    match parser.next_including_whitespace() {
                        Ok(Token::Semicolon) => break,
                        Ok(Token::CurlyBracketBlock) => {
                            let _ = parser.parse_nested_block(
                                |p| -> Result<(), cssparser::ParseError<'_, ()>> {
                                    while p.next_including_whitespace().is_ok() {}
                                    Ok(())
                                },
                            );
                            break;
                        }
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
                prelude = SelectorPrelude::new();
            }
            Token::CurlyBracketBlock => {
                let selectors = prelude.finish(warnings);
                let mut block = DeclBlock::default();
                let _ =
                    parser.parse_nested_block(|p| -> Result<(), cssparser::ParseError<'_, ()>> {
                        parse_declarations(p, &mut block, warnings);
                        Ok(())
                    });
                for selector in selectors {
                    let order = sheet.rules.len();
                    sheet.rules.push(Rule {
                        selector,
                        block: block.clone(),
                        order,
                    });
                }
                prelude = SelectorPrelude::new();
            }
            other => prelude.push_token(other),
        }
    }
    sheet
}

/// Parse an `@page` rule: prelude (base or a pseudo-page variant we don't
/// support yet), then a body of page descriptors. `size` and `margin*` are
/// applied; margin-box at-rules (`@top-center`, ...) and other descriptors
/// (`bleed`, `marks`, ...) are reported.
fn parse_page_rule(
    parser: &mut Parser<'_, '_>,
    sheet: &mut Stylesheet,
    warnings: &mut Vec<String>,
) {
    // Prelude: collect any pseudo-page selector before the block.
    let mut pseudo: Option<String> = None;
    loop {
        match parser.next_including_whitespace() {
            Ok(Token::CurlyBracketBlock) => break,
            Ok(Token::Colon) => {
                if let Ok(Token::Ident(id)) = parser.next_including_whitespace() {
                    pseudo = Some(id.as_ref().to_string());
                }
            }
            Ok(_) => continue,
            Err(_) => return, // EOF before a block: nothing to do
        }
    }

    let mut rule = PageRule::default();
    let mut local_warnings: Vec<String> = Vec::new();
    let _ = parser.parse_nested_block(|p| -> Result<(), cssparser::ParseError<'_, ()>> {
        loop {
            let tok = match p.next_including_whitespace() {
                Ok(t) => t.clone(),
                Err(_) => break,
            };
            match &tok {
                Token::WhiteSpace(_) | Token::Semicolon => continue,
                Token::AtKeyword(name) => {
                    // Margin boxes: @top-left ... @bottom-right.
                    local_warnings.push(format!(
                        "@page margin box @{name} is not supported yet (skipped)"
                    ));
                    // Skip its block.
                    loop {
                        match p.next_including_whitespace() {
                            Ok(Token::CurlyBracketBlock) => {
                                let _ = p.parse_nested_block(
                                    |q| -> Result<(), cssparser::ParseError<'_, ()>> {
                                        while q.next_including_whitespace().is_ok() {}
                                        Ok(())
                                    },
                                );
                                break;
                            }
                            Ok(Token::Semicolon) | Err(_) => break,
                            Ok(_) => continue,
                        }
                    }
                }
                Token::Ident(name) => {
                    let name = name.to_ascii_lowercase();
                    if p.expect_colon().is_err() {
                        continue;
                    }
                    let _ = p.parse_until_after(
                        cssparser::Delimiter::Semicolon,
                        |v| -> Result<(), cssparser::ParseError<'_, ()>> {
                            apply_page_descriptor(&name, v, &mut rule, &mut local_warnings);
                            Ok(())
                        },
                    );
                }
                _ => continue,
            }
        }
        Ok(())
    });
    warnings.append(&mut local_warnings);

    if let Some(pseudo) = pseudo {
        warnings.push(format!(
            "@page :{pseudo} variants are not supported yet (rule skipped)"
        ));
        return;
    }
    sheet
        .page
        .get_or_insert_with(PageRule::default)
        .merge_from(rule);
}

/// One descriptor inside an `@page` body.
fn apply_page_descriptor(
    name: &str,
    p: &mut Parser<'_, '_>,
    rule: &mut PageRule,
    warnings: &mut Vec<String>,
) {
    match name {
        "size" => {
            if let Some(size) = parse_page_size(p, warnings) {
                rule.size = Some(size);
            }
        }
        "margin" => {
            let mut vals = Vec::new();
            while let Ok(tok) = p.next() {
                let tok = tok.clone();
                match crate::css::token_to_length(&tok) {
                    Some(l) => vals.push(l),
                    None => break,
                }
            }
            let expanded: Option<[Length; 4]> = match vals.as_slice() {
                [a] => Some([*a; 4]),
                [v, h] => Some([*v, *h, *v, *h]),
                [t, h, b] => Some([*t, *h, *b, *h]),
                [t, r, b, l] => Some([*t, *r, *b, *l]),
                _ => None,
            };
            if let Some(m) = expanded {
                rule.margin = m.map(Some);
            }
        }
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
            let idx = match name {
                "margin-top" => 0,
                "margin-right" => 1,
                "margin-bottom" => 2,
                _ => 3,
            };
            if let Ok(tok) = p.next() {
                let tok = tok.clone();
                rule.margin[idx] = crate::css::token_to_length(&tok);
            }
        }
        other => {
            warnings.push(format!(
                "@page descriptor '{other}' is unsupported (skipped)"
            ));
        }
    }
}

/// `size: <named> | <length>{1,2} | [<named>|<length>{1,2}] [portrait|landscape]`
fn parse_page_size(p: &mut Parser<'_, '_>, warnings: &mut Vec<String>) -> Option<(f64, f64)> {
    let mut dims: Vec<f64> = Vec::new();
    let mut named: Option<(f64, f64)> = None;
    let mut landscape = false;
    while let Ok(tok) = p.next() {
        let tok = tok.clone();
        match &tok {
            Token::Ident(id) => match id.to_ascii_lowercase().as_str() {
                "a3" => named = Some((841.89, 1190.55)),
                "a4" | "auto" => named = Some((595.28, 841.89)),
                "a5" => named = Some((419.53, 595.28)),
                "letter" => named = Some((612.0, 792.0)),
                "legal" => named = Some((612.0, 1008.0)),
                "tabloid" | "ledger" => named = Some((792.0, 1224.0)),
                "landscape" => landscape = true,
                "portrait" => {}
                other => {
                    warnings.push(format!("@page size '{other}' is unsupported"));
                    return None;
                }
            },
            _ => match crate::css::token_to_length(&tok) {
                Some(Length::Pt(v)) => dims.push(v),
                _ => {
                    warnings.push("@page size accepts named sizes or absolute lengths".to_string());
                    return None;
                }
            },
        }
    }
    let (w, h) = match (named, dims.as_slice()) {
        (Some(n), []) => n,
        (None, [s]) => (*s, *s),
        (None, [w, h]) => (*w, *h),
        _ => return None,
    };
    Some(if landscape && h > w { (h, w) } else { (w, h) })
}

/// Incremental selector-prelude builder fed one token at a time.
struct SelectorPrelude {
    /// Finished selectors in the current group (before each `,`).
    done: Vec<(Vec<Compound>, Vec<Combinator>)>,
    compounds: Vec<Compound>,
    combinators: Vec<Combinator>,
    current: Compound,
    /// Whitespace seen since the current compound ended.
    ws_pending: bool,
    /// An explicit `>` seen since the current compound ended.
    child_pending: bool,
    /// The current selector contains something outside the subset.
    unsupported: Option<String>,
    /// Skipped-selector notices accumulated across the group; drained
    /// into the warnings list when the rule finishes.
    skipped: Vec<String>,
    /// After a `.`, the next ident is a class name.
    expecting_class: bool,
}

impl SelectorPrelude {
    fn new() -> Self {
        SelectorPrelude {
            done: Vec::new(),
            compounds: Vec::new(),
            combinators: Vec::new(),
            current: Compound::default(),
            ws_pending: false,
            child_pending: false,
            unsupported: None,
            skipped: Vec::new(),
            expecting_class: false,
        }
    }

    /// Called before adding to a compound: commit a pending combinator.
    fn begin_part(&mut self) {
        if !self.current.is_empty() && (self.child_pending || self.ws_pending) {
            self.compounds.push(std::mem::take(&mut self.current));
            self.combinators.push(if self.child_pending {
                Combinator::Child
            } else {
                Combinator::Descendant
            });
        }
        self.ws_pending = false;
        self.child_pending = false;
    }

    fn push_token(&mut self, tok: &Token) {
        match tok {
            Token::WhiteSpace(_) => {
                self.ws_pending = true;
                self.expecting_class = false;
            }
            Token::Comma => self.end_selector(),
            Token::Ident(id) => {
                if self.expecting_class {
                    self.current.classes.push(id.as_ref().to_string());
                    self.expecting_class = false;
                } else {
                    self.begin_part();
                    self.current.tag = Some(id.to_ascii_lowercase());
                }
            }
            Token::Delim('.') => {
                self.begin_part();
                self.expecting_class = true;
            }
            Token::Delim('*') => {
                self.begin_part();
                self.current.universal = true;
            }
            Token::Delim('>') => {
                self.child_pending = true;
                self.ws_pending = false;
            }
            Token::IDHash(id) | Token::Hash(id) => {
                self.begin_part();
                self.current.id = Some(id.as_ref().to_string());
            }
            other => {
                if self.unsupported.is_none() {
                    let what = match other {
                        Token::Colon => {
                            "pseudo-class/pseudo-element (':hover', '::before', ...)".to_string()
                        }
                        Token::SquareBracketBlock => "attribute selector ('[...]')".to_string(),
                        Token::Delim('+') => "adjacent-sibling combinator ('+')".to_string(),
                        Token::Delim('~') => "general-sibling combinator ('~')".to_string(),
                        other => format!("{other:?}"),
                    };
                    self.unsupported = Some(what);
                }
            }
        }
    }

    fn end_selector(&mut self) {
        let mut compounds = std::mem::take(&mut self.compounds);
        let combinators = std::mem::take(&mut self.combinators);
        if !self.current.is_empty() {
            compounds.push(std::mem::take(&mut self.current));
        }
        // An unsupported marker only poisons the selector it occurred in;
        // record the notice so `finish` can surface it.
        if let Some(what) = self.unsupported.take() {
            self.skipped.push(format!(
                "unsupported selector syntax (selector skipped): {what}"
            ));
        } else if !compounds.is_empty() {
            self.done.push((compounds, combinators));
        }
        self.current = Compound::default();
        self.ws_pending = false;
        self.child_pending = false;
        self.expecting_class = false;
    }

    fn finish(&mut self, warnings: &mut Vec<String>) -> Vec<Selector> {
        self.end_selector();
        warnings.append(&mut self.skipped);
        self.done
            .drain(..)
            .map(|(compounds, combinators)| {
                let specificity = Selector::compute_specificity(&compounds);
                Selector {
                    compounds,
                    combinators,
                    specificity,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(css: &str) -> (Stylesheet, Vec<String>) {
        let mut w = Vec::new();
        let s = parse_stylesheet(css, &mut w);
        (s, w)
    }

    fn key(tag: &str, id: Option<&str>, classes: &[&str]) -> ElemKey {
        ElemKey {
            tag: tag.to_string(),
            id: id.map(str::to_string),
            classes: classes.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn type_class_id_specificity() {
        let (s, w) = sheet("td { color: red } .amount { color: blue } #total { color: green }");
        assert!(w.is_empty());
        assert_eq!(s.rules.len(), 3);
        assert_eq!(s.rules[0].selector.specificity, 1);
        assert_eq!(s.rules[1].selector.specificity, 1_000);
        assert_eq!(s.rules[2].selector.specificity, 1_000_000);
    }

    #[test]
    fn compound_and_grouping() {
        let (s, _) = sheet("td.amount.bold, h1 { color: red }");
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[0].selector.specificity, 2_001);
        let k = key("td", None, &["amount", "bold"]);
        assert!(s.rules[0].selector.matches(&k, &[]));
        assert!(!s.rules[0]
            .selector
            .matches(&key("td", None, &["amount"]), &[]));
    }

    #[test]
    fn descendant_combinator_matches_any_depth() {
        let (s, _) = sheet("table td { padding: 0 }");
        let sel = &s.rules[0].selector;
        let td = key("td", None, &[]);
        let deep = [key("table", None, &[]), key("tr", None, &[])];
        assert!(sel.matches(&td, &deep));
        assert!(!sel.matches(&td, &[key("div", None, &[])]));
    }

    #[test]
    fn child_combinator_requires_direct_parent() {
        let (s, _) = sheet("ul > li { color: red }");
        let sel = &s.rules[0].selector;
        let li = key("li", None, &[]);
        assert!(sel.matches(&li, &[key("ul", None, &[])]));
        assert!(!sel.matches(&li, &[key("ul", None, &[]), key("ol", None, &[])]));
    }

    #[test]
    fn descendant_backtracking() {
        // "a b c" where two ancestors match b at different depths.
        let (s, _) = sheet("div section p { color: red }");
        let sel = &s.rules[0].selector;
        let p = key("p", None, &[]);
        let ancestors = [
            key("section", None, &[]),
            key("div", None, &[]),
            key("section", None, &[]),
        ];
        // div must be ABOVE a section: section(0) < div(1) fails that
        // ordering, but section(2) with div(1) above it succeeds.
        assert!(sel.matches(&p, &ancestors));
        let wrong = [key("section", None, &[]), key("div", None, &[])];
        assert!(!sel.matches(&p, &wrong));
    }

    #[test]
    fn unsupported_selector_skipped_others_kept() {
        let (s, w) = sheet("td:hover, .kept { color: red }");
        assert_eq!(s.rules.len(), 1, "only .kept survives");
        assert_eq!(s.rules[0].selector.specificity, 1_000);
        assert!(w.iter().any(|m| m.contains("unsupported selector")));
    }

    #[test]
    fn at_rules_skipped_with_warning_rest_parses() {
        let (s, w) = sheet("@media print { td { color: red } } h1 { color: blue }");
        assert_eq!(s.rules.len(), 1);
        assert!(s.rules[0].selector.compounds[0].tag.as_deref() == Some("h1"));
        assert!(w.iter().any(|m| m.contains("@media")));
    }

    #[test]
    fn at_page_size_and_margin_parse() {
        let (s, w) = sheet("@page { size: Letter; margin: 1in 0.5in }");
        let page = s.page.expect("page rule");
        assert_eq!(page.size, Some((612.0, 792.0)));
        assert_eq!(page.margin[0], Some(Length::Pt(72.0)));
        assert_eq!(page.margin[1], Some(Length::Pt(36.0)));
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn at_page_landscape_and_dimensions() {
        let (s, _) = sheet("@page { size: A4 landscape }");
        assert_eq!(s.page.unwrap().size, Some((841.89, 595.28)));
        let (s2, _) = sheet("@page { size: 8.5in 11in }");
        assert_eq!(s2.page.unwrap().size, Some((612.0, 792.0)));
    }

    #[test]
    fn at_page_pseudo_variant_warns_pending() {
        let (s, w) = sheet("@page :first { margin-top: 90pt } h1 { color: red }");
        assert!(s.page.is_none(), "variant rule must not apply yet");
        assert!(w.iter().any(|m| m.contains(":first")));
        assert_eq!(s.rules.len(), 1, "following rules still parse");
    }

    #[test]
    fn at_page_margin_box_warns_pending() {
        let (s, w) = sheet("@page { margin: 54pt; @bottom-center { content: counter(page) } }");
        assert_eq!(s.page.unwrap().margin[0], Some(Length::Pt(54.0)));
        assert!(w.iter().any(|m| m.contains("@bottom-center")));
    }

    #[test]
    fn at_page_unknown_descriptor_warns() {
        let (s, w) = sheet("@page { bleed: 3mm; marks: crop; size: Legal }");
        assert_eq!(s.page.unwrap().size, Some((612.0, 1008.0)));
        assert!(w.iter().any(|m| m.contains("'bleed'")));
        assert!(w.iter().any(|m| m.contains("'marks'")));
    }

    #[test]
    fn important_in_rule_body() {
        let (s, _) = sheet("p { color: red !important; margin: 0 }");
        assert!(s.rules[0].block.important.color.is_some());
        assert!(s.rules[0].block.normal.margin[0].is_some());
    }

    #[test]
    fn universal_selector_matches_everything() {
        let (s, _) = sheet("* { color: red }");
        assert!(s.rules[0]
            .selector
            .matches(&key("anything", None, &[]), &[]));
        assert_eq!(s.rules[0].selector.specificity, 0);
    }
}
