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
    /// Margin boxes from the base `@page` rule (`@top-center`, ...).
    pub margin_boxes: Vec<MarginBox>,
    /// The `@page :first` variant, when present.
    pub first: Option<FirstPageRule>,
}

/// The `@page :first` subset: margin overrides plus suppression of the
/// base rule's margin boxes (`content: none` inside a box at-rule).
#[derive(Debug, Clone, Default)]
pub struct FirstPageRule {
    pub margin: [Option<Length>; 4],
    /// Box positions suppressed on the first page via `content: none`.
    pub suppress: Vec<MarginBoxPos>,
}

/// The six supported margin boxes (corner boxes are out of subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginBoxPos {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl MarginBoxPos {
    pub fn is_top(self) -> bool {
        matches!(
            self,
            MarginBoxPos::TopLeft | MarginBoxPos::TopCenter | MarginBoxPos::TopRight
        )
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "top-left" => Some(MarginBoxPos::TopLeft),
            "top-center" => Some(MarginBoxPos::TopCenter),
            "top-right" => Some(MarginBoxPos::TopRight),
            "bottom-left" => Some(MarginBoxPos::BottomLeft),
            "bottom-center" => Some(MarginBoxPos::BottomCenter),
            "bottom-right" => Some(MarginBoxPos::BottomRight),
            _ => None,
        }
    }
}

/// One margin box: where it goes, its resolved `content()` template
/// (counters already rewritten to the engine's `{{pageNumber}}` /
/// `{{totalPages}}` placeholders), and any styling declared in its body.
#[derive(Debug, Clone)]
pub struct MarginBox {
    pub position: MarginBoxPos,
    pub content: String,
    pub style: DeclBlock,
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
        for mb in other.margin_boxes {
            self.margin_boxes.retain(|b| b.position != mb.position);
            self.margin_boxes.push(mb);
        }
        if let Some(first) = other.first {
            let slot = self.first.get_or_insert_with(FirstPageRule::default);
            for i in 0..4 {
                if first.margin[i].is_some() {
                    slot.margin[i] = first.margin[i];
                }
            }
            for s in first.suppress {
                if !slot.suppress.contains(&s) {
                    slot.suppress.push(s);
                }
            }
        }
    }
}

/// The structural pseudo-classes in subset: the zebra-stripe family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // the CSS names ARE :first-child/:last-child/:nth-child
pub enum Pseudo {
    FirstChild,
    LastChild,
    /// `:nth-child(an+b)` — `even` is (2,0), `odd` is (2,1).
    NthChild {
        a: i32,
        b: i32,
    },
}

/// One compound selector: everything between combinators.
#[derive(Debug, Clone, Default)]
pub struct Compound {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub pseudos: Vec<Pseudo>,
    /// An explicit `*`. Matching-wise it's a no-op (an empty compound
    /// matches everything anyway); this flag only marks the compound as
    /// deliberately present so `* { ... }` isn't dropped as empty.
    pub universal: bool,
}

impl Compound {
    fn is_empty(&self) -> bool {
        !self.universal
            && self.tag.is_none()
            && self.id.is_none()
            && self.classes.is_empty()
            && self.pseudos.is_empty()
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
        if !self.classes.iter().all(|c| key.classes.contains(c)) {
            return false;
        }
        self.pseudos.iter().all(|p| match p {
            Pseudo::FirstChild => key.index == 0,
            Pseudo::LastChild => key.index + 1 == key.count,
            Pseudo::NthChild { a, b } => {
                let i = key.index as i32 + 1; // :nth-child is 1-based
                if *a == 0 {
                    i == *b
                } else {
                    let d = i - b;
                    d % a == 0 && d / a >= 0
                }
            }
        })
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
    /// 0-based position among the parent's element children.
    pub index: usize,
    /// Total element children in the parent.
    pub count: usize,
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
            classes += (c.classes.len() + c.pseudos.len()) as u32;
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
            Token::Function(name) => {
                // Block-start token: its arguments MUST be consumed here.
                let fname = name.to_ascii_lowercase();
                let args: Vec<Token> = parser
                    .parse_nested_block(|p| -> Result<Vec<Token>, cssparser::ParseError<'_, ()>> {
                        let mut toks = Vec::new();
                        while let Ok(t) = p.next_including_whitespace() {
                            toks.push(t.clone());
                        }
                        Ok(toks)
                    })
                    .unwrap_or_default();
                prelude.push_function(&fname, parse_nth_args(&args));
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

/// Parse an `@page` rule: prelude (base or a pseudo-page variant), then a
/// body of page descriptors and margin-box at-rules. `:first` supports
/// margin overrides and `content: none` box suppression; `:left`/`:right`
/// wait for demand (flowing-element x positions bake at layout time, so
/// mirrored margins would misplace page-crossing fragments).
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
                    pseudo = Some(id.to_ascii_lowercase());
                }
            }
            Ok(_) => continue,
            Err(_) => return, // EOF before a block: nothing to do
        }
    }
    let is_first = pseudo.as_deref() == Some("first");
    if let Some(p) = &pseudo {
        if !is_first {
            warnings.push(format!(
                "@page :{p} variants are not supported (rule skipped){}",
                if matches!(p.as_str(), "left" | "right") {
                    " — mirrored margins wait for re-layout-per-page"
                } else {
                    ""
                }
            ));
        }
    }

    let mut rule = PageRule::default();
    let mut first = FirstPageRule::default();
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
                    let box_name = name.to_ascii_lowercase();
                    let pos = MarginBoxPos::from_name(&box_name);
                    if pos.is_none() {
                        local_warnings.push(format!(
                            "@page box @{box_name} is unsupported (only the six main margin boxes; skipped)"
                        ));
                    }
                    // Find and parse the box body either way (to consume it).
                    let mut body: Option<(Option<String>, DeclBlock)> = None;
                    loop {
                        match p.next_including_whitespace() {
                            Ok(Token::CurlyBracketBlock) => {
                                let parsed = p.parse_nested_block(
                                    |q| -> Result<(Option<String>, DeclBlock), cssparser::ParseError<'_, ()>> {
                                        Ok(parse_margin_box_body(q, &mut local_warnings))
                                    },
                                );
                                body = parsed.ok();
                                break;
                            }
                            Ok(Token::Semicolon) | Err(_) => break,
                            Ok(_) => continue,
                        }
                    }
                    if let (Some(pos), Some((content, style))) = (pos, body) {
                        match content {
                            // `content: none`
                            None => {
                                if is_first {
                                    first.suppress.push(pos);
                                } else {
                                    rule.margin_boxes.retain(|b| b.position != pos);
                                }
                            }
                            Some(content) => {
                                if is_first {
                                    local_warnings.push(format!(
                                        "@page :first with its own @{box_name} content is unsupported (only `content: none` suppression; skipped)"
                                    ));
                                } else {
                                    rule.margin_boxes.retain(|b| b.position != pos);
                                    rule.margin_boxes.push(MarginBox {
                                        position: pos,
                                        content,
                                        style,
                                    });
                                }
                            }
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
                            if is_first {
                                apply_first_page_descriptor(&name, v, &mut first, &mut local_warnings);
                            } else {
                                apply_page_descriptor(&name, v, &mut rule, &mut local_warnings);
                            }
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

    match pseudo {
        None => {
            sheet
                .page
                .get_or_insert_with(PageRule::default)
                .merge_from(rule);
        }
        Some(_) if is_first => {
            let delta = PageRule {
                first: Some(first),
                ..Default::default()
            };
            sheet
                .page
                .get_or_insert_with(PageRule::default)
                .merge_from(delta);
        }
        Some(_) => {} // unsupported variant, already warned
    }
}

/// A margin-box body: a `content` descriptor plus optional styling.
/// Returns (content template, style). `content: none` → content = None.
fn parse_margin_box_body(
    p: &mut Parser<'_, '_>,
    warnings: &mut Vec<String>,
) -> (Option<String>, DeclBlock) {
    let mut content: Option<String> = None;
    let mut saw_content_none = false;
    let mut style = DeclBlock::default();
    while !p.is_exhausted() {
        let _ = p.parse_until_after(
            cssparser::Delimiter::Semicolon,
            |d| -> Result<(), cssparser::ParseError<'_, ()>> {
                let name = d.expect_ident()?.to_ascii_lowercase();
                d.expect_colon()?;
                if name == "content" {
                    let (text, none) = parse_content_value(d, warnings);
                    if none {
                        saw_content_none = true;
                    } else {
                        content = Some(text);
                    }
                } else {
                    // Regular styling for the box (color, font-size, ...).
                    let mut decl = crate::css::CssStyle::default();
                    let _ = d.parse_until_before(
                        cssparser::Delimiter::Bang,
                        |v| -> Result<(), cssparser::ParseError<'_, ()>> {
                            crate::css::apply_declaration(&name, v, &mut decl, warnings);
                            Ok(())
                        },
                    );
                    style.normal = style.normal.merge(&decl);
                }
                Ok(())
            },
        );
    }
    if saw_content_none {
        (None, style)
    } else {
        (Some(content.unwrap_or_default()), style)
    }
}

/// `content: "Page " counter(page) " of " counter(pages)` → the engine's
/// placeholder template. Unsupported content values (attr(), url(),
/// string(), named counters) are reported.
fn parse_content_value(p: &mut Parser<'_, '_>, warnings: &mut Vec<String>) -> (String, bool) {
    let mut out = String::new();
    let mut none = false;
    while let Ok(tok) = p.next() {
        let tok = tok.clone();
        match &tok {
            Token::QuotedString(s) => out.push_str(s.as_ref()),
            Token::Ident(id) if id.eq_ignore_ascii_case("none") => none = true,
            Token::Function(f) if f.eq_ignore_ascii_case("counter") => {
                let counter =
                    p.parse_nested_block(|q| -> Result<String, cssparser::ParseError<'_, ()>> {
                        let mut name = String::new();
                        while let Ok(t) = q.next() {
                            if let Token::Ident(id) = t {
                                name = id.to_ascii_lowercase();
                            }
                        }
                        Ok(name)
                    });
                match counter.as_deref() {
                    Ok("page") => out.push_str("{{pageNumber}}"),
                    Ok("pages") => out.push_str("{{totalPages}}"),
                    Ok(other) => {
                        warnings.push(format!("unsupported counter '{other}' in content()"));
                    }
                    Err(_) => {}
                }
            }
            other => {
                warnings.push(format!("unsupported content() component: {other:?}"));
            }
        }
    }
    (out, none)
}

/// `@page :first` descriptors: margins only.
fn apply_first_page_descriptor(
    name: &str,
    p: &mut Parser<'_, '_>,
    first: &mut FirstPageRule,
    warnings: &mut Vec<String>,
) {
    let mut probe = PageRule::default();
    apply_page_descriptor(name, p, &mut probe, warnings);
    if probe.size.is_some() {
        warnings.push("@page :first size overrides are unsupported (margins only)".to_string());
    }
    for i in 0..4 {
        if probe.margin[i].is_some() {
            first.margin[i] = probe.margin[i];
        }
    }
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

/// Parse `:nth-child()` arguments: `even`, `odd`, and the an+b forms the
/// CSS tokenizer splits in creative ways (`2n+1` → Dimension(2,"n") +
/// Number(+1); `n-2` → Ident("n-2")). Returns None for anything else.
fn parse_nth_args(args: &[Token]) -> Option<(i32, i32)> {
    let toks: Vec<&Token> = args
        .iter()
        .filter(|t| !matches!(t, Token::WhiteSpace(_)))
        .collect();

    // Ident forms: even / odd / n / -n / n-<b>
    fn ident_form(s: &str) -> Option<(i32, Option<i32>)> {
        match s {
            "even" => Some((2, Some(0))),
            "odd" => Some((2, Some(1))),
            "n" => Some((1, None)),
            "-n" => Some((-1, None)),
            _ => {
                let (a, rest) = if let Some(r) = s.strip_prefix("-n") {
                    (-1, r)
                } else if let Some(r) = s.strip_prefix('n') {
                    (1, r)
                } else {
                    return None;
                };
                // rest like "-2"
                rest.parse::<i32>().ok().map(|b| (a, Some(b)))
            }
        }
    }

    match toks.as_slice() {
        [Token::Ident(id)] => {
            let (a, b) = ident_form(&id.to_ascii_lowercase())?;
            Some((a, b.unwrap_or(0)))
        }
        [Token::Ident(id), Token::Number {
            value, has_sign, ..
        }] => {
            let (a, b) = ident_form(&id.to_ascii_lowercase())?;
            if b.is_some() || !has_sign {
                return None;
            }
            Some((a, *value as i32))
        }
        [Token::Number { value, .. }] => Some((0, *value as i32)),
        [Token::Dimension { value, unit, .. }] => {
            let unit = unit.to_ascii_lowercase();
            if unit == "n" {
                Some((*value as i32, 0))
            } else if let Some(rest) = unit.strip_prefix("n-") {
                let b: i32 = rest.parse().ok()?;
                Some((*value as i32, -b))
            } else {
                None
            }
        }
        [Token::Dimension { value, unit, .. }, Token::Number {
            value: b, has_sign, ..
        }] => {
            if unit.to_ascii_lowercase() != "n" || !has_sign {
                return None;
            }
            Some((*value as i32, *b as i32))
        }
        _ => None,
    }
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
    /// A `:` was just seen — the next ident/function names a pseudo-class.
    expecting_pseudo: bool,
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
            expecting_pseudo: false,
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
                if self.expecting_pseudo {
                    self.expecting_pseudo = false;
                    match id.to_ascii_lowercase().as_str() {
                        "first-child" => self.current.pseudos.push(Pseudo::FirstChild),
                        "last-child" => self.current.pseudos.push(Pseudo::LastChild),
                        other => {
                            if self.unsupported.is_none() {
                                self.unsupported = Some(format!("pseudo-class ':{other}'"));
                            }
                        }
                    }
                } else if self.expecting_class {
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
            Token::Colon => {
                if self.expecting_pseudo {
                    // `::` — pseudo-elements are out of subset.
                    if self.unsupported.is_none() {
                        self.unsupported =
                            Some("pseudo-element ('::before', '::after', ...)".to_string());
                    }
                    self.expecting_pseudo = false;
                } else {
                    // Commit a pending combinator so `tbody :first-child`
                    // starts a fresh compound.
                    self.begin_part();
                    self.expecting_pseudo = true;
                }
            }
            other => {
                if self.unsupported.is_none() {
                    let what = match other {
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

    /// A function token in the prelude — only `:nth-child(...)` is in
    /// subset.
    fn push_function(&mut self, name: &str, nth: Option<(i32, i32)>) {
        let was_pseudo = std::mem::take(&mut self.expecting_pseudo);
        if self.unsupported.is_some() {
            return;
        }
        if !was_pseudo || name != "nth-child" {
            self.unsupported = Some(format!("selector function '{name}()'"));
            return;
        }
        match nth {
            Some((a, b)) => self.current.pseudos.push(Pseudo::NthChild { a, b }),
            None => {
                self.unsupported =
                    Some(":nth-child() argument (use even, odd, or an+b)".to_string());
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
            index: 0,
            count: 1,
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
    fn at_page_first_margin_overrides_parse() {
        let (s, w) = sheet("@page :first { margin-top: 90pt } h1 { color: red }");
        let first = s.page.unwrap().first.expect(":first captured");
        assert_eq!(first.margin[0], Some(Length::Pt(90.0)));
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.rules.len(), 1, "following rules still parse");
    }

    #[test]
    fn margin_box_content_counters_rewrite_to_placeholders() {
        let (s, w) = sheet(
            "@page { margin: 54pt; @bottom-center { content: \"Page \" counter(page) \" of \" counter(pages); color: #666 } }",
        );
        let page = s.page.unwrap();
        assert_eq!(page.margin[0], Some(Length::Pt(54.0)));
        assert_eq!(page.margin_boxes.len(), 1);
        let mb = &page.margin_boxes[0];
        assert_eq!(mb.position, MarginBoxPos::BottomCenter);
        assert_eq!(mb.content, "Page {{pageNumber}} of {{totalPages}}");
        assert!(mb.style.normal.color.is_some());
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn first_content_none_suppresses_box() {
        let (s, w) = sheet(
            "@page { margin: 72pt; @top-center { content: \"Running\" } } \
             @page :first { @top-center { content: none } }",
        );
        let page = s.page.unwrap();
        assert_eq!(page.margin_boxes.len(), 1, "base box survives");
        let first = page.first.expect(":first captured");
        assert_eq!(first.suppress, vec![MarginBoxPos::TopCenter]);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn corner_boxes_and_left_right_variants_warn() {
        let (_, w) = sheet("@page { @top-left-corner { content: \"x\" } }");
        assert!(w.iter().any(|m| m.contains("@top-left-corner")));
        let (_, w2) = sheet("@page :left { margin-right: 1in }");
        assert!(w2.iter().any(|m| m.contains(":left")));
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
    fn nth_child_forms_parse_and_match() {
        let (s, w) = sheet(
            "tr:nth-child(even) { color: red } tr:nth-child(odd) { color: red } \
             tr:nth-child(3) { color: red } tr:nth-child(2n+1) { color: red } \
             tr:nth-child(n-1) { color: red }",
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.rules.len(), 5);
        let key = |i: usize| ElemKey {
            tag: "tr".into(),
            id: None,
            classes: vec![],
            index: i,
            count: 6,
        };
        let m = |rule: usize, idx: usize| s.rules[rule].selector.matches(&key(idx), &[]);
        // even: 1-based 2,4,6 → 0-based 1,3,5
        assert!(m(0, 1) && m(0, 3) && !m(0, 0) && !m(0, 2));
        // odd: 0-based 0,2,4
        assert!(m(1, 0) && m(1, 2) && !m(1, 1));
        // exactly 3
        assert!(m(2, 2) && !m(2, 1) && !m(2, 3));
        // 2n+1 == odd
        assert!(m(3, 0) && m(3, 2) && !m(3, 1));
        // n-1: every index (1-based i >= 0... a=1,b=-1 → i-(-1) ≥ 0 always)
        assert!(m(4, 0) && m(4, 5));
    }

    #[test]
    fn first_and_last_child_match_by_position() {
        let (s, w) = sheet("li:first-child { color: red } li:last-child { color: red }");
        assert!(w.is_empty(), "{w:?}");
        let key = |i: usize, n: usize| ElemKey {
            tag: "li".into(),
            id: None,
            classes: vec![],
            index: i,
            count: n,
        };
        assert!(s.rules[0].selector.matches(&key(0, 3), &[]));
        assert!(!s.rules[0].selector.matches(&key(1, 3), &[]));
        assert!(s.rules[1].selector.matches(&key(2, 3), &[]));
        assert!(!s.rules[1].selector.matches(&key(0, 3), &[]));
        // Pseudo-classes carry class-level specificity.
        assert_eq!(s.rules[0].selector.specificity, 1_001);
    }

    #[test]
    fn unsupported_pseudo_forms_warn_by_name() {
        let (s, w) = sheet("tr:nth-of-type(2) { color: red } td::after { content: \"x\" }");
        assert!(s.rules.is_empty());
        assert!(w.iter().any(|m| m.contains("nth-of-type")), "{w:?}");
        assert!(w.iter().any(|m| m.contains("pseudo-element")), "{w:?}");
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
