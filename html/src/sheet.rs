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

use crate::css::{parse_declarations, DeclBlock};
use cssparser::{Parser, ParserInput, Token};

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
            Token::AtKeyword(name) => {
                warnings.push(format!(
                    "@{name} rules are unsupported in v0 (skipped){}",
                    if name.eq_ignore_ascii_case("page") {
                        " — @page support is planned"
                    } else {
                        ""
                    }
                ));
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
    fn at_page_warning_mentions_plan() {
        let (_, w) = sheet("@page { margin: 1in }");
        assert!(w.iter().any(|m| m.contains("@page support is planned")));
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
