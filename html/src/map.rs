//! Box mapping: styled DOM → `forme` document tree.
//!
//! This is the spike's core novel work. Three normalization passes happen
//! here, in this order:
//!
//! 1. **Whitespace collapsing** (CSS Text §8.1, `white-space: normal`):
//!    done inside `InlineFlattener` while flattening inline content into
//!    `TextRun`s, so collapsing state naturally spans inline-element
//!    boundaries (`</span> <span>` → one space).
//! 2. **Anonymous-box normalization**: consecutive inline children of a
//!    block group into one `Text` node; block children split the groups
//!    (HTML's anonymous block box rule).
//! 3. **Margin pre-collapse**: the engine's margins are additive by design,
//!    so CSS margin collapsing is resolved here — adjacent siblings and
//!    parent/first-last-child collapse-through. Flex containers are
//!    excluded (flex formatting contexts never collapse margins).

use crate::css::{parse_style_attr, CssDisplay};
use crate::dom::{DomNode, Element};
use crate::sheet::{ElemKey, MarginBox, MarginBoxPos, Rule, Stylesheet};
use crate::style::{resolve, Computed, MarginV, ROOT_FONT_SIZE};
use crate::ua::ua_style;
use forme::model::{
    ColumnDef, ColumnWidth, EdgeValue, Edges, FixedPosition, ListMarkerType, MarginEdges, Metadata,
    PageConfig,
};
use forme::style::{
    Color, CornerValues, Dimension, Display, EdgeValues, FlexDirection, FontStyle, GridPlacement,
    TextDecoration,
};
use forme::{Document, Node, NodeKind, Style, TextRun};

pub struct Mapper {
    pub warnings: Vec<String>,
    /// Cascade input: document `<style>` blocks + any caller-provided CSS,
    /// already concatenated in origin order.
    sheet: Stylesheet,
    /// Ancestor identities (root → parent) for selector matching. Pushed
    /// and popped around every recursion into an element's children —
    /// including inline elements, which stylesheet rules also target.
    stack: Vec<ElemKey>,
    /// Families already attributed in a fallback warning (once each).
    warned_fonts: Vec<String>,
    /// Set when a mapped element carried `break-after: page`; consumed by
    /// the containing `map_children` loop, which turns it into
    /// `break_before` on the NEXT sibling (the engine has no break-after).
    /// A set flag surviving to the end of a container deliberately leaks
    /// to the container's own next sibling — that matches CSS break
    /// propagation from last children to the parent's after-edge.
    pending_break_after: bool,
    /// The page name currently in force (CSS `page` property), so nested
    /// blocks repeating an ancestor's name emit no redundant markers and
    /// closing markers restore the OUTER name.
    current_page_name: Option<String>,
    /// Whether ANY rule sets `page` — when false, the per-block peek
    /// (a second cascade pass) is skipped entirely.
    uses_page_names: bool,
    /// Whether ANY rule or inline style floats — the same zero-cost gate
    /// for the float-run transform.
    uses_floats: bool,
}

/// Does any inline style in the tree mention float? (Cheap substring
/// scan — a false positive only costs the peek, never correctness.)
fn dom_mentions_float(el: &Element) -> bool {
    if el.attr("style").is_some_and(|s| s.contains("float")) {
        return true;
    }
    el.children.iter().any(|c| match c {
        DomNode::Element(e) => dom_mentions_float(e),
        _ => false,
    })
}

/// Map a parsed `<body>` element to a complete engine document.
pub fn map_html(body: &Element, sheet: Stylesheet, page: PageConfig) -> (Document, Vec<String>) {
    let mut mapper = Mapper {
        warnings: Vec::new(),
        sheet,
        stack: Vec::new(),
        warned_fonts: Vec::new(),
        pending_break_after: false,
        current_page_name: None,
        uses_page_names: false,
        uses_floats: false,
    };
    mapper.uses_page_names = mapper
        .sheet
        .rules
        .iter()
        .any(|r| r.block.normal.page.is_some() || r.block.important.page.is_some());
    mapper.uses_floats = mapper
        .sheet
        .rules
        .iter()
        .any(|r| r.block.normal.float.is_some() || r.block.important.float.is_some())
        || dom_mentions_float(body);
    let children = match mapper.map_block_element(body, ROOT_FONT_SIZE) {
        Some(mut node) => {
            // body { width: 21cm; height: 29.7cm } is the mPDF-era idiom
            // for "I am the page" — page geometry, not content sizing.
            // Honoring it inside the content box cuts off the right edge
            // and forces a blank first page (template-compat 05/06).
            // Clamp to the content box and name the remedy.
            let (page_w, page_h) = page.size.dimensions();
            let content_w = page_w - page.margin.left - page.margin.right;
            let content_h = page_h - page.margin.top - page.margin.bottom;
            if let Some(Dimension::Pt(w)) = node.style.width {
                if w > content_w + 0.5 {
                    mapper.warnings.push(format!(
                        "body width {w:.0}pt exceeds the {content_w:.0}pt content box and was clamped — page geometry belongs in @page (size, margin)"
                    ));
                    node.style.width = None;
                }
            }
            if let Some(Dimension::Pt(h)) = node.style.height {
                if h > content_h + 0.5 {
                    mapper.warnings.push(format!(
                        "body height {h:.0}pt exceeds the {content_h:.0}pt content box and was clamped — page geometry belongs in @page (size, margin)"
                    ));
                    node.style.height = None;
                }
            }
            vec![node]
        }
        None => vec![],
    };
    let doc = Document {
        children,
        metadata: Metadata::default(),
        default_page: page,
        first_page: None,
        left_page: None,
        named_pages: Default::default(),
        attachments: vec![],
        zugferd: None,
        right_page: None,
        fonts: vec![],
        default_style: None,
        tagged: false,
        pdfa: None,
        pdf_ua: false,
        embedded_data: None,
        flatten_forms: false,
        certification: None,
    };
    (doc, mapper.warnings)
}

/// Elements that generate inline boxes (flattened into TextRuns).
fn is_inline(tag: &str) -> bool {
    matches!(
        tag,
        "span"
            | "b"
            | "strong"
            | "i"
            | "em"
            | "u"
            | "s"
            | "strike"
            | "del"
            | "a"
            | "small"
            | "code"
            | "abbr"
            | "label"
            | "sub"
            | "sup"
            | "mark"
            | "time"
    )
}

/// Elements that produce no boxes at all.
fn is_skip(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "head" | "title" | "meta" | "link" | "template" | "noscript"
    )
}

// ── Inline flattening (pass 1 + run construction) ─────────────────────

/// The effective inline style at a point in the flattening walk. Only
/// deltas from the containing block are tracked — the engine resolves each
/// run against the Text node's style, so unset fields inherit correctly.
#[derive(Debug, Clone, Default)]
struct RunStyle {
    font_family: Option<String>,
    font_size: Option<f64>,
    font_weight: Option<u32>,
    italic: Option<bool>,
    color: Option<Color>,
    text_decoration: Option<TextDecoration>,
    text_transform: Option<forme::style::TextTransform>,
    letter_spacing: Option<f64>,
    href: Option<String>,
}

impl RunStyle {
    /// Layer an inline element's computed style over this one.
    fn apply(&self, c: &Computed, href: Option<&str>) -> RunStyle {
        RunStyle {
            font_family: c.font_family.clone().or_else(|| self.font_family.clone()),
            font_size: if c.font_size_explicit {
                Some(c.font_size)
            } else {
                self.font_size
            },
            font_weight: c.font_weight.or(self.font_weight),
            italic: c.italic.or(self.italic),
            color: c.color.or(self.color),
            text_decoration: c.text_decoration.or(self.text_decoration),
            text_transform: c.text_transform.or(self.text_transform),
            letter_spacing: c.letter_spacing.or(self.letter_spacing),
            href: href.map(str::to_string).or_else(|| self.href.clone()),
        }
    }

    /// Field-by-field equality. Manual because the engine's Color and
    /// TextDecoration don't derive PartialEq.
    fn same(&self, other: &RunStyle) -> bool {
        fn color_eq(a: Option<Color>, b: Option<Color>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a,
                _ => false,
            }
        }
        fn deco_eq(a: Option<TextDecoration>, b: Option<TextDecoration>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => std::mem::discriminant(&a) == std::mem::discriminant(&b),
                _ => false,
            }
        }
        self.font_family == other.font_family
            && self.font_size == other.font_size
            && self.font_weight == other.font_weight
            && self.italic == other.italic
            && color_eq(self.color, other.color)
            && deco_eq(self.text_decoration, other.text_decoration)
            && self.text_transform.map(|t| t as u8) == other.text_transform.map(|t| t as u8)
            && self.letter_spacing == other.letter_spacing
            && self.href == other.href
    }

    fn to_engine(&self) -> Style {
        Style {
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            font_weight: self.font_weight,
            font_style: self.italic.map(|i| {
                if i {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                }
            }),
            color: self.color,
            text_decoration: self.text_decoration,
            text_transform: self.text_transform,
            letter_spacing: self.letter_spacing,
            ..Default::default()
        }
    }
}

/// Streaming whitespace collapser + run builder. Collapsing state spans the
/// whole inline group, so it works across element boundaries.
struct InlineFlattener {
    runs: Vec<TextRun>,
    current: String,
    current_style: RunStyle,
    /// Whitespace seen but not yet emitted (may be dropped at boundaries).
    pending_space: bool,
    /// Whether any non-whitespace character has been emitted since the last
    /// hard boundary (group start or <br>). Suppresses leading spaces.
    emitted_any: bool,
}

impl InlineFlattener {
    fn new() -> Self {
        InlineFlattener {
            runs: Vec::new(),
            current: String::new(),
            current_style: RunStyle::default(),
            pending_space: false,
            emitted_any: false,
        }
    }

    fn text(&mut self, text: &str, style: &RunStyle) {
        for ch in text.chars() {
            // U+00A0 (nbsp) is deliberately NOT whitespace here — it must
            // survive collapsing.
            if ch.is_ascii_whitespace() {
                self.pending_space = true;
            } else {
                if self.pending_space && self.emitted_any {
                    self.push_char(' ', style);
                }
                self.pending_space = false;
                self.push_char(ch, style);
                self.emitted_any = true;
            }
        }
    }

    fn push_char(&mut self, ch: char, style: &RunStyle) {
        if !style.same(&self.current_style) {
            self.flush();
            self.current_style = style.clone();
        }
        self.current.push(ch);
    }

    /// <br>: a hard line break. Spaces before it are dropped; spaces after
    /// it are leading spaces of the new line and dropped too.
    fn hard_break(&mut self) {
        self.pending_space = false;
        // The newline belongs to whatever run is open; if none is open yet,
        // it opens the current style's run.
        self.current.push('\n');
        self.emitted_any = false;
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.runs.push(TextRun {
                content: std::mem::take(&mut self.current),
                style: self.current_style.to_engine(),
                href: self.current_style.href.clone(),
            });
        }
    }

    fn finish(mut self) -> Vec<TextRun> {
        self.flush();
        // A trailing bare-newline run (e.g. <br> at the very end) renders
        // as an empty extra line; drop pure-newline tails.
        while let Some(last) = self.runs.last() {
            if last.content.chars().all(|c| c == '\n') {
                self.runs.pop();
            } else {
                break;
            }
        }
        self.runs
    }
}

// ── The mapper ────────────────────────────────────────────────────────

/// Build a selector-matching identity from an element's tag + attributes.
fn elem_key(el: &Element) -> ElemKey {
    ElemKey {
        tag: el.tag.clone(),
        id: el.attr("id").map(str::to_string),
        classes: el
            .attr("class")
            .map(|c| c.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default(),
        index: el.index,
        count: el.sibling_count,
        type_index: el.type_index,
        type_count: el.type_count,
    }
}

impl Mapper {
    /// Compute an element's style through the full cascade:
    /// UA defaults → matching stylesheet rules ascending by (specificity,
    /// source order) → inline style → `!important` rules in the same
    /// order → inline `!important`. Resolved against the parent font size.
    fn computed_for(&mut self, el: &Element, parent_font_size: f64) -> Computed {
        let key = elem_key(el);
        let mut matched: Vec<&Rule> = self
            .sheet
            .rules
            .iter()
            .filter(|r| r.selector.matches(&key, &self.stack))
            .collect();
        matched.sort_by_key(|r| (r.selector.specificity, r.order));

        let inline = parse_style_attr(el.attr("style").unwrap_or(""), &mut self.warnings);

        let mut merged = ua_style(&el.tag);
        for r in &matched {
            merged = merged.merge(&r.block.normal);
        }
        merged = merged.merge(&inline.normal);
        for r in &matched {
            merged = merged.merge(&r.block.important);
        }
        merged = merged.merge(&inline.important);
        let mut computed = resolve(&merged, parent_font_size, &mut self.warnings);
        if let Some(families) = computed.font_family.take() {
            computed.font_family = Some(self.resolve_families(&families));
        }
        computed
    }

    /// Post-process a font-family chain: name the specific family that
    /// will fall back because its @font-face was skipped, and map the CSS
    /// generic families onto the engine's built-in standard fonts.
    fn resolve_families(&mut self, families: &str) -> String {
        let mut out: Vec<String> = Vec::new();
        for raw in families.split(',') {
            let name = raw.trim().trim_matches(['"', '\'']).to_string();
            if self
                .sheet
                .skipped_font_families
                .iter()
                .any(|f| f.eq_ignore_ascii_case(&name))
                && !self
                    .warned_fonts
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&name))
            {
                self.warned_fonts.push(name.clone());
                self.warnings.push(format!(
                    "font-family '{name}' references a skipped @font-face — text using it falls back; provide the font via options.fonts / --font"
                ));
            }
            out.push(match name.to_ascii_lowercase().as_str() {
                "sans-serif" | "system-ui" | "ui-sans-serif" => "Helvetica".to_string(),
                "serif" | "ui-serif" => "Times".to_string(),
                "monospace" | "ui-monospace" => "Courier".to_string(),
                _ => name,
            });
        }
        out.join(", ")
    }

    /// Map a block-level element to an engine node.
    pub fn map_block_element(&mut self, el: &Element, parent_font_size: f64) -> Option<Node> {
        if is_skip(&el.tag) {
            return None;
        }
        let mut computed = self.computed_for(el, parent_font_size);
        if computed.display == CssDisplay::None {
            return None;
        }
        let computed_break_after = computed.break_after;

        // The element becomes an ancestor for everything mapped inside it.
        self.stack.push(elem_key(el));
        let node = match el.tag.as_str() {
            "table" => self.map_table(el, &computed),
            "ul" | "ol" => self.map_list(el, &computed),
            "img" => self.map_img(el, &computed),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = el.tag.as_bytes()[1] - b'0';
                self.map_paragraph_like(el, computed, Some(level))
            }
            "p" => self.map_paragraph_like(el, computed, None),
            _ => {
                // Generic block container (div, section, header, ...).
                let mut children = self.map_children(&el.children, &computed);
                if computed.display == CssDisplay::Block {
                    collapse_sibling_margins(&mut children);
                    collapse_into_parent(&mut computed, &mut children);
                }
                Some(make_node(
                    NodeKind::View,
                    to_engine_style(&computed),
                    children,
                ))
            }
        };
        self.stack.pop();
        // Set AFTER the children recursion: the flag targets this element's
        // next sibling, and setting it earlier would hand it to our own
        // first block child instead.
        if matches!(computed_break_after, Some(crate::css::BreakVal::Page)) {
            self.pending_break_after = true;
        }
        node
    }

    /// Map the children of a block container, grouping consecutive inline
    /// content into anonymous Text nodes (pass 2).
    fn map_children(&mut self, children: &[DomNode], parent: &Computed) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        let mut inline_buf: Vec<&DomNode> = Vec::new();
        // Consecutive floated block siblings collect here and flush as
        // one flex row. CSS ignores float on flex items, so the whole
        // mechanism is off inside display:flex parents.
        let mut float_run: Vec<(Node, crate::css::FloatVal)> = Vec::new();
        let floats_active = self.uses_floats && parent.display != CssDisplay::Flex;

        for child in children {
            let is_inline_item = match child {
                DomNode::Text(_) => true,
                DomNode::Element(e) => is_inline(&e.tag) || e.tag == "br",
            };
            if is_inline_item {
                // Whitespace between floats is structural noise; real
                // inline content beside floats is the unsupported
                // text-wrap case — flush the row and say so.
                if !float_run.is_empty() {
                    let significant = match child {
                        DomNode::Text(t) => !t.trim().is_empty(),
                        DomNode::Element(_) => true,
                    };
                    if significant {
                        self.flush_float_run(&mut float_run, &mut out, true);
                    }
                }
                inline_buf.push(child);
            } else if let DomNode::Element(e) = child {
                self.flush_inline_group(&mut inline_buf, parent, &mut out);
                if !is_skip(&e.tag) {
                    // A pending break-after from the previous sibling
                    // becomes break_before here (the engine's only break
                    // primitive). If this element maps to nothing
                    // (display:none), the pending break carries forward.
                    let pending = std::mem::take(&mut self.pending_break_after);
                    // CSS Paged Media `page: <name>`: a block whose name
                    // differs from the one in force gets PageName markers
                    // around it — the engine breaks between differently
                    // named boxes and selects the named config. The
                    // closing marker restores the OUTER name (stack
                    // discipline handles nesting).
                    let peek = self.uses_page_names
                        || e.attr("style").is_some_and(|st| st.contains("page"));
                    // One cascade peek serves both the page-name switch
                    // and the float-run routing.
                    let peeked = if peek || floats_active {
                        Some(self.computed_for(e, parent.font_size))
                    } else {
                        None
                    };
                    let (block_float, block_clear) = match (&peeked, floats_active) {
                        (Some(c), true) => (c.float, c.clear),
                        _ => (None, None),
                    };
                    // `clear` (on floated or non-floated elements alike)
                    // terminates the current run: following content
                    // starts below, per CSS.
                    if block_clear.is_some() && !float_run.is_empty() {
                        self.flush_float_run(&mut float_run, &mut out, false);
                    }
                    let block_page = if peek {
                        peeked
                            .as_ref()
                            .and_then(|c| c.page.clone())
                            .or_else(|| self.current_page_name.clone())
                    } else {
                        self.current_page_name.clone()
                    };
                    let switches = block_page != self.current_page_name;
                    let saved = if switches {
                        out.push(make_node(
                            NodeKind::PageName {
                                name: block_page.clone(),
                            },
                            Style::default(),
                            vec![],
                        ));
                        std::mem::replace(&mut self.current_page_name, block_page)
                    } else {
                        self.current_page_name.clone()
                    };
                    match self.map_block_element(e, parent.font_size) {
                        Some(mut node) => {
                            if pending {
                                node.style.break_before = Some(true);
                            }
                            if let Some(side) = block_float {
                                float_run.push((node, side));
                            } else {
                                // A non-floated block after an uncleared
                                // run is the text-wrap case: warn, place
                                // below.
                                if !float_run.is_empty() {
                                    self.flush_float_run(&mut float_run, &mut out, true);
                                }
                                out.push(node);
                            }
                        }
                        None => {
                            self.pending_break_after = self.pending_break_after || pending;
                        }
                    }
                    if switches {
                        out.push(make_node(
                            NodeKind::PageName {
                                name: saved.clone(),
                            },
                            Style::default(),
                            vec![],
                        ));
                        self.current_page_name = saved;
                    }
                }
            }
        }
        if !float_run.is_empty() {
            self.flush_float_run(&mut float_run, &mut out, false);
        }
        self.flush_inline_group(&mut inline_buf, parent, &mut out);
        out
    }

    /// Flush a run of consecutive floated siblings as one flex row:
    /// left floats in markup order, a flex-grow spacer, then right
    /// floats REVERSED (successive float:right stack right-to-left per
    /// CSS). flex-wrap gives float-line semantics: an over-wide run
    /// wraps exactly as floats drop. `warn` marks the unsupported
    /// text-wrap termination (non-floated content after an uncleared
    /// run) — that residual warning must never go silent.
    fn flush_float_run(
        &mut self,
        run: &mut Vec<(Node, crate::css::FloatVal)>,
        out: &mut Vec<Node>,
        warn: bool,
    ) {
        use crate::css::FloatVal;
        if run.is_empty() {
            return;
        }
        if warn {
            self.warnings.push(
                "text wrapping alongside floats is not supported; floated siblings are laid out as columns"
                    .to_string(),
            );
        }
        let items = std::mem::take(run);
        let has_right = items.iter().any(|(_, s)| *s == FloatVal::Right);
        let mut children: Vec<Node> = Vec::new();
        for (node, side) in &items {
            if *side == FloatVal::Left {
                children.push(node.clone());
            }
        }
        if has_right {
            children.push(make_node(
                NodeKind::View,
                Style {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                vec![],
            ));
            for (node, side) in items.iter().rev() {
                if *side == FloatVal::Right {
                    children.push(node.clone());
                }
            }
        }
        out.push(make_node(
            NodeKind::View,
            Style {
                flex_direction: Some(forme::style::FlexDirection::Row),
                flex_wrap: Some(forme::style::FlexWrap::Wrap),
                align_items: Some(forme::style::AlignItems::FlexStart),
                ..Default::default()
            },
            children,
        ));
    }

    /// Flatten a pending inline group into an anonymous Text node. Groups
    /// that collapse to nothing (inter-block whitespace) produce no node.
    fn flush_inline_group(
        &mut self,
        buf: &mut Vec<&DomNode>,
        parent: &Computed,
        out: &mut Vec<Node>,
    ) {
        if buf.is_empty() {
            return;
        }
        let items = std::mem::take(buf);
        let mut flattener = InlineFlattener::new();
        let base = RunStyle::default();
        for item in items {
            self.flatten_item(item, &base, parent.font_size, &mut flattener);
        }
        let runs = flattener.finish();
        if runs.is_empty() {
            return;
        }
        let mut style = Style::default();
        if std::mem::take(&mut self.pending_break_after) {
            style.break_before = Some(true);
        }
        out.push(text_node_from_runs(runs, style, None));
    }

    /// Recursive inline flattening (pass 1 lives in the flattener's state).
    fn flatten_item(
        &mut self,
        item: &DomNode,
        style: &RunStyle,
        font_size: f64,
        flattener: &mut InlineFlattener,
    ) {
        match item {
            DomNode::Text(t) => flattener.text(t, style),
            DomNode::Element(e) if e.tag == "br" => flattener.hard_break(),
            DomNode::Element(e) if is_skip(&e.tag) => {}
            DomNode::Element(e) if is_inline(&e.tag) => {
                let computed = self.computed_for(e, font_size);
                let href = if e.tag == "a" { e.attr("href") } else { None };
                let inner = style.apply(&computed, href);
                self.stack.push(elem_key(e));
                for child in &e.children {
                    self.flatten_item(child, &inner, computed.font_size, flattener);
                }
                self.stack.pop();
            }
            DomNode::Element(e) => {
                // A block (or replaced) element inside inline flow — the
                // spike doesn't support it. Loudly recorded, not silent.
                self.warnings.push(format!(
                    "<{}> inside inline flow is unsupported in the spike (skipped)",
                    e.tag
                ));
            }
        }
    }

    /// `<p>` / `<h#>`: all-inline children become a single Text/Heading
    /// node carrying the element's own style. Box props (background,
    /// border, padding) get a wrapping View, since the engine treats text
    /// nodes as pure text containers.
    fn map_paragraph_like(
        &mut self,
        el: &Element,
        mut computed: Computed,
        level: Option<u8>,
    ) -> Option<Node> {
        let has_block_child = el.children.iter().any(|c| {
            matches!(c, DomNode::Element(e) if !is_inline(&e.tag) && !is_skip(&e.tag) && e.tag != "br")
        });
        if has_block_child {
            // Invalid-but-real HTML (block inside <p>). Fall back to a
            // generic container.
            let mut children = self.map_children(&el.children, &computed);
            collapse_sibling_margins(&mut children);
            collapse_into_parent(&mut computed, &mut children);
            return Some(make_node(
                NodeKind::View,
                to_engine_style(&computed),
                children,
            ));
        }

        let mut flattener = InlineFlattener::new();
        let base = RunStyle::default();
        for child in &el.children {
            self.flatten_item(child, &base, computed.font_size, &mut flattener);
        }
        let runs = flattener.finish();
        let needs_box_wrapper = computed.background_color.is_some()
            || computed.border_width.iter().any(|w| *w > 0.0)
            || computed.padding.iter().any(|p| *p > 0.0);

        if runs.is_empty() {
            // An empty <p>/<h#> with no paint contributes nothing and is
            // dropped (its margin collapse-through is out of scope). But when
            // it carries a background, border, or padding, a browser still
            // paints the box — dropping it would silently lose a styled
            // element, which the render contract forbids. Emit the empty box.
            if needs_box_wrapper {
                let (box_style, _text_style) = split_box_and_text_style(&computed);
                return Some(make_node(NodeKind::View, box_style, vec![]));
            }
            return None;
        }

        if needs_box_wrapper {
            let (box_style, text_style) = split_box_and_text_style(&computed);
            let text = text_node_from_runs_kind(runs, text_style, level);
            Some(make_node(NodeKind::View, box_style, vec![text]))
        } else {
            Some(text_node_from_runs_kind(
                runs,
                to_engine_style(&computed),
                level,
            ))
        }
    }

    fn map_list(&mut self, el: &Element, computed: &Computed) -> Option<Node> {
        let ordered = el.tag == "ol";
        let start = el
            .attr("start")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let mut items = Vec::new();
        for child in &el.children {
            match child {
                DomNode::Element(e) if e.tag == "li" => {
                    let li_computed = self.computed_for(e, computed.font_size);
                    self.stack.push(elem_key(e));
                    let li_children = self.map_children(&e.children, &li_computed);
                    self.stack.pop();
                    items.push(make_node(
                        NodeKind::ListItem,
                        to_engine_style(&li_computed),
                        li_children,
                    ));
                }
                DomNode::Text(t) if t.trim().is_empty() => {}
                other => {
                    self.warnings
                        .push(format!("unexpected list child ignored: {other:?}"));
                }
            }
        }
        Some(make_node(
            NodeKind::List {
                ordered,
                marker_type: if ordered {
                    ListMarkerType::Decimal
                } else {
                    ListMarkerType::Disc
                },
                start,
            },
            to_engine_style(computed),
            items,
        ))
    }

    fn map_img(&mut self, el: &Element, computed: &Computed) -> Option<Node> {
        let src = el.attr("src")?.to_string();
        if src.starts_with("http://") || src.starts_with("https://") {
            // Constitution: no external resource fetching.
            self.warnings.push(format!(
                "external image not fetched (provide data URIs or local files): {src}"
            ));
            return None;
        }
        let attr_dim = |name: &str| -> Option<f64> {
            el.attr(name)
                .and_then(|v| v.parse::<f64>().ok())
                .map(|px| px * 0.75)
        };
        let width = match computed.width {
            Some(Dimension::Pt(v)) => Some(v),
            _ => attr_dim("width"),
        };
        let height = match computed.height {
            Some(Dimension::Pt(v)) => Some(v),
            _ => attr_dim("height"),
        };
        let mut node = make_node(
            NodeKind::Image { src, width, height },
            to_engine_style(computed),
            vec![],
        );
        // <img alt> → Figure /Alt (PDF/UA 7.3-1). Absent alt stays None; a
        // pdf_ua render warns about it (see html_to_document).
        node.alt = el.attr("alt").map(|s| s.to_string());
        Some(node)
    }

    fn map_table(&mut self, el: &Element, computed: &Computed) -> Option<Node> {
        // border-collapse: collapse — emulated over the engine's
        // separate-borders cells. Each interior edge is drawn by exactly
        // one owner: cells keep right+bottom; the first row/column keep
        // top/left; when the table has its own border, the outer edges
        // belong to the table wrapper and the outermost cell edges are
        // suppressed too. Row borders (tr { border-bottom } — the zebra
        // separator pattern) are redistributed onto the row's cells,
        // since the engine paints only backgrounds at row level.
        let collapse = computed.border_collapse == Some(true);
        let mut computed = computed.clone();
        if collapse && computed.border_radius.is_some() {
            // Per CSS (and Chrome), border-radius does not apply in the
            // collapsed-borders model.
            computed.border_radius = None;
        }
        let computed = &computed;
        let table_info = TableInfo {
            collapse,
            table_has_border: computed.border_width.iter().any(|w| *w > 0.0),
            total_rows: count_rows(&el.children),
        };
        let mut rows: Vec<Node> = Vec::new();
        let mut next_row = 0usize;
        self.collect_rows(
            &el.children,
            computed.font_size,
            false,
            &mut rows,
            &table_info,
            &mut next_row,
        );

        // Column definitions from the first row's cell widths. Mixed
        // specified/unspecified widths become Auto for the gaps; if nothing
        // is specified, the engine distributes evenly.
        let columns = self.columns_from_first_row(el, computed.font_size);

        Some(make_node(
            NodeKind::Table { columns },
            to_engine_style(computed),
            rows,
        ))
    }

    fn collect_rows(
        &mut self,
        children: &[DomNode],
        font_size: f64,
        in_thead: bool,
        rows: &mut Vec<Node>,
        table_info: &TableInfo,
        next_row: &mut usize,
    ) {
        for child in children {
            match child {
                DomNode::Element(e) if e.tag == "tr" => {
                    let row_pos = RowPos {
                        first: *next_row == 0,
                        last: *next_row + 1 == table_info.total_rows,
                    };
                    *next_row += 1;
                    rows.push(self.map_tr(e, in_thead, font_size, table_info, row_pos));
                }
                DomNode::Element(e) if matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot") => {
                    // Section-level break-inside can't map to an engine
                    // node (sections aren't boxes here) — say so rather
                    // than silently accepting it. Row-level break-inside
                    // IS honored: rows are atomic by engine design.
                    let section_computed = self.computed_for(e, font_size);
                    if section_computed.break_inside.is_some() {
                        self.warnings.push(format!(
                            "break-inside on <{}> is pending (rows are already atomic; put break-inside: avoid on the table to keep the whole table together)",
                            e.tag
                        ));
                    }
                    // The section element is a selector ancestor
                    // (`tbody tr:nth-child(even)` — the zebra idiom).
                    self.stack.push(elem_key(e));
                    self.collect_rows(
                        &e.children,
                        font_size,
                        e.tag == "thead",
                        rows,
                        table_info,
                        next_row,
                    );
                    self.stack.pop();
                }
                DomNode::Text(t) if t.trim().is_empty() => {}
                other => {
                    self.warnings
                        .push(format!("unexpected table child ignored: {other:?}"));
                }
            }
        }
    }

    fn map_tr(
        &mut self,
        el: &Element,
        is_header: bool,
        font_size: f64,
        table_info: &TableInfo,
        row_pos: RowPos,
    ) -> Node {
        let row_computed = self.computed_for(el, font_size);
        self.stack.push(elem_key(el));
        let cell_count = el
            .children
            .iter()
            .filter(|c| matches!(c, DomNode::Element(e) if matches!(e.tag.as_str(), "td" | "th")))
            .count();
        let mut cell_idx = 0usize;
        let mut cells = Vec::new();
        for child in &el.children {
            match child {
                DomNode::Element(e) if matches!(e.tag.as_str(), "td" | "th") => {
                    let mut cell_computed = self.computed_for(e, row_computed.font_size);
                    // Legacy HTML valign attribute (the wkhtmltopdf-era
                    // dialect): a presentational hint below CSS — the
                    // cell's own attr, then the row's, apply only when no
                    // CSS vertical-align matched.
                    if cell_computed.vertical_align.is_none() {
                        let attr_valign =
                            e.attr("valign")
                                .or_else(|| el.attr("valign"))
                                .and_then(|v| match v.to_ascii_lowercase().as_str() {
                                    "top" => Some(forme::style::VerticalAlign::Top),
                                    "middle" | "center" => {
                                        Some(forme::style::VerticalAlign::Middle)
                                    }
                                    "bottom" => Some(forme::style::VerticalAlign::Bottom),
                                    "baseline" => Some(forme::style::VerticalAlign::Baseline),
                                    _ => None,
                                });
                        cell_computed.vertical_align = attr_valign;
                    }
                    if table_info.collapse {
                        apply_collapsed_borders(
                            &mut cell_computed,
                            &row_computed,
                            table_info,
                            row_pos,
                            cell_idx == 0,
                            cell_idx + 1 == cell_count,
                        );
                    }
                    cell_idx += 1;
                    let span = |name: &str| -> u32 {
                        e.attr(name)
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(1)
                            .max(1)
                    };
                    self.stack.push(elem_key(e));
                    let content = self.map_children(&e.children, &cell_computed);
                    self.stack.pop();
                    cells.push(make_node(
                        NodeKind::TableCell {
                            col_span: span("colspan"),
                            row_span: span("rowspan"),
                        },
                        to_engine_style(&cell_computed),
                        content,
                    ));
                }
                DomNode::Text(t) if t.trim().is_empty() => {}
                other => {
                    self.warnings
                        .push(format!("unexpected row child ignored: {other:?}"));
                }
            }
        }
        self.stack.pop();
        make_node(
            NodeKind::TableRow { is_header },
            to_engine_style(&row_computed),
            cells,
        )
    }

    fn columns_from_first_row(&mut self, table: &Element, font_size: f64) -> Vec<ColumnDef> {
        // Harvest widths from the first row WITHOUT colspans. The old
        // first-row-only rule discarded every width the moment a table
        // opened with a full-width banner row — which is how most real
        // invoices open (template-compat/REPORT.md).
        let Some(first_row) = find_first_plain_tr(&table.children) else {
            return vec![];
        };
        let mut defs = Vec::new();
        let mut any_specified = false;
        for child in &first_row.children {
            if let DomNode::Element(e) = child {
                if !matches!(e.tag.as_str(), "td" | "th") {
                    continue;
                }
                let computed = self.computed_for(e, font_size);
                let def = match computed.width {
                    Some(Dimension::Percent(p)) => {
                        any_specified = true;
                        ColumnDef {
                            width: ColumnWidth::Fraction(p / 100.0),
                        }
                    }
                    Some(Dimension::Pt(v)) => {
                        any_specified = true;
                        ColumnDef {
                            width: ColumnWidth::Fixed(v),
                        }
                    }
                    _ => ColumnDef {
                        width: ColumnWidth::Auto,
                    },
                };
                defs.push(def);
            }
        }
        if any_specified {
            defs
        } else {
            vec![]
        }
    }
}

/// First `<tr>` whose cells all have colspan=1 — the row that can supply
/// unambiguous per-column widths. `None` when every row spans.
fn find_first_plain_tr(children: &[DomNode]) -> Option<&Element> {
    fn is_plain(tr: &Element) -> bool {
        tr.children.iter().all(|c| match c {
            DomNode::Element(e) if matches!(e.tag.as_str(), "td" | "th") => {
                e.attr("colspan")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(1)
                    <= 1
            }
            _ => true,
        })
    }
    fn walk(children: &[DomNode]) -> Option<&Element> {
        for child in children {
            if let DomNode::Element(e) = child {
                if e.tag == "tr" {
                    if is_plain(e) {
                        return Some(e);
                    }
                    continue; // spanned row: keep looking further down
                }
                if matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot") {
                    if let Some(tr) = walk(&e.children) {
                        return Some(tr);
                    }
                }
            }
        }
        None
    }
    walk(children)
}

// ── Node/style construction ───────────────────────────────────────────

fn make_node(kind: NodeKind, style: Style, children: Vec<Node>) -> Node {
    Node {
        kind,
        style,
        children,
        id: None,
        source_location: None,
        bookmark: None,
        href: None,
        alt: None,
    }
}

fn text_node_from_runs(runs: Vec<TextRun>, style: Style, href: Option<String>) -> Node {
    make_node(
        NodeKind::Text {
            content: String::new(),
            href,
            runs,
        },
        style,
        vec![],
    )
}

/// Build a Text or Heading node. A single unstyled run collapses to plain
/// `content` (the common case, and what the React serializer emits too).
fn text_node_from_runs_kind(mut runs: Vec<TextRun>, style: Style, level: Option<u8>) -> Node {
    let single_plain =
        runs.len() == 1 && runs[0].href.is_none() && is_default_style(&runs[0].style);
    let (content, runs) = if single_plain {
        (runs.remove(0).content, vec![])
    } else {
        (String::new(), runs)
    };
    let kind = match level {
        Some(level) => NodeKind::Heading {
            level,
            content,
            href: None,
            runs,
        },
        None => NodeKind::Text {
            content,
            href: None,
            runs,
        },
    };
    make_node(kind, style, vec![])
}

fn is_default_style(s: &Style) -> bool {
    s.font_family.is_none()
        && s.font_size.is_none()
        && s.font_weight.is_none()
        && s.font_style.is_none()
        && s.color.is_none()
        && s.text_decoration.is_none()
}

/// Convert a resolved style to the engine's Style. Margins are always
/// emitted explicitly (the collapse pass edits them); other fields only
/// when set, so the engine's own inheritance does the rest.
fn to_engine_style(c: &Computed) -> Style {
    let mut s = Style::default();

    let ev = |m: MarginV| -> EdgeValue {
        match m {
            MarginV::Pt(v) => EdgeValue::Pt(v),
            MarginV::Auto => EdgeValue::Auto,
        }
    };
    s.margin = Some(MarginEdges {
        top: ev(c.margin[0]),
        right: ev(c.margin[1]),
        bottom: ev(c.margin[2]),
        left: ev(c.margin[3]),
    });

    if c.padding.iter().any(|p| *p > 0.0) {
        s.padding = Some(Edges {
            top: c.padding[0],
            right: c.padding[1],
            bottom: c.padding[2],
            left: c.padding[3],
        });
    }
    if c.border_width.iter().any(|w| *w > 0.0) {
        s.border_width = Some(EdgeValues {
            top: c.border_width[0],
            right: c.border_width[1],
            bottom: c.border_width[2],
            left: c.border_width[3],
        });
        s.border_color = Some(EdgeValues::uniform(c.border_color.unwrap_or(Color::BLACK)));
        s.border_style = Some(EdgeValues {
            top: c.border_style[0],
            right: c.border_style[1],
            bottom: c.border_style[2],
            left: c.border_style[3],
        });
    }
    if let Some(r) = c.border_radius {
        s.border_radius = Some(CornerValues::uniform(r));
    }

    s.width = c.width;
    s.height = c.height;
    s.max_width = c.max_width;
    s.min_width = c.min_width;
    s.min_height = c.min_height;
    s.vertical_align = c.vertical_align;

    s.font_family = c.font_family.clone();
    if c.font_size_explicit {
        s.font_size = Some(c.font_size);
    }
    s.font_weight = c.font_weight;
    s.font_style = c.italic.map(|i| {
        if i {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }
    });
    s.line_height = c.line_height;
    s.text_align = c.text_align;
    s.color = c.color;
    s.background_color = c.background_color;
    s.text_decoration = c.text_decoration;
    s.text_transform = c.text_transform;
    s.letter_spacing = c.letter_spacing;

    if c.position_absolute {
        s.position = Some(forme::model::Position::Absolute);
        s.top = c.offsets[0];
        s.right = c.offsets[1];
        s.bottom = c.offsets[2];
        s.left = c.offsets[3];
    } else if c.position_relative {
        // Relative stays in normal flow (space preserved); the engine paints
        // it offset by these values.
        s.position = Some(forme::model::Position::Relative);
        s.top = c.offsets[0];
        s.right = c.offsets[1];
        s.bottom = c.offsets[2];
        s.left = c.offsets[3];
    }

    if matches!(c.break_before, Some(crate::css::BreakVal::Page)) {
        s.break_before = Some(true);
    }
    if matches!(c.break_inside, Some(crate::css::BreakInsideVal::Avoid)) {
        // The engine's wrap=false means "keep together; move to the next
        // page if it doesn't fit" — exactly break-inside: avoid.
        s.wrap = Some(false);
    }
    s.min_orphan_lines = c.orphans;
    s.min_widow_lines = c.widows;

    if c.display == CssDisplay::Flex {
        // CSS flex defaults to row; the engine's View defaults to column.
        s.flex_direction = Some(c.flex_direction.unwrap_or(FlexDirection::Row));
    } else {
        s.flex_direction = c.flex_direction;
    }
    s.justify_content = c.justify_content;
    s.align_items = c.align_items;
    s.gap = c.gap;
    s.row_gap = c.row_gap;
    s.column_gap = c.column_gap;

    // Grid container. style.rs already downgraded template-less grids to
    // block (with a named warning), so Grid here always has columns.
    if c.display == CssDisplay::Grid {
        s.display = Some(Display::Grid);
        s.grid_template_columns = c.grid_template_columns.clone();
        s.grid_template_rows = c.grid_template_rows.clone();
        s.grid_auto_rows = c.grid_auto_rows.clone();
        s.grid_auto_columns = c.grid_auto_columns.clone();
    }

    // Grid item placement (this element inside a grid parent). `span` on
    // its own or alongside a start line — both engine-native.
    if c.grid_column.is_some() || c.grid_row.is_some() {
        let mut gp = GridPlacement::default();
        if let Some(col) = c.grid_column {
            gp.column_start = col.start;
            gp.column_end = col.end;
            gp.column_span = col.span;
        }
        if let Some(row) = c.grid_row {
            gp.row_start = row.start;
            gp.row_end = row.end;
            gp.row_span = row.span;
        }
        s.grid_placement = Some(gp);
    }

    s
}

/// Split a paragraph's style into box props (for a wrapping View) and text
/// props (for the inner Text node). Margins go on the View so the collapse
/// pass sees them.
fn split_box_and_text_style(c: &Computed) -> (Style, Style) {
    let full = to_engine_style(c);
    let box_style = Style {
        margin: full.margin,
        padding: full.padding,
        border_width: full.border_width,
        border_color: full.border_color,
        border_radius: full.border_radius,
        background_color: full.background_color,
        width: full.width,
        height: full.height,
        ..Default::default()
    };
    let text_style = Style {
        font_family: full.font_family,
        font_size: full.font_size,
        font_weight: full.font_weight,
        font_style: full.font_style,
        line_height: full.line_height,
        text_align: full.text_align,
        color: full.color,
        text_decoration: full.text_decoration,
        ..Default::default()
    };
    (box_style, text_style)
}

// ── Table border collapsing ───────────────────────────────────────────

/// Per-table context for collapsed-border emulation.
struct TableInfo {
    collapse: bool,
    table_has_border: bool,
    total_rows: usize,
}

/// A row's position within its table (thead + tbody flattened).
#[derive(Clone, Copy)]
struct RowPos {
    first: bool,
    last: bool,
}

fn count_rows(children: &[DomNode]) -> usize {
    let mut n = 0;
    for child in children {
        if let DomNode::Element(e) = child {
            match e.tag.as_str() {
                "tr" => n += 1,
                "thead" | "tbody" | "tfoot" => n += count_rows(&e.children),
                _ => {}
            }
        }
    }
    n
}

/// The collapsed-border emulation for one cell: merge row-level borders
/// down (the engine paints only backgrounds at row level), then give each
/// interior edge exactly one owner. Uniform-border approximation — CSS's
/// widest-border-wins conflict resolution is out of subset.
fn apply_collapsed_borders(
    cell: &mut Computed,
    row: &Computed,
    table: &TableInfo,
    row_pos: RowPos,
    first_cell: bool,
    last_cell: bool,
) {
    // Row borders → cells: top/bottom to every cell, left/right to the
    // edge cells. The cell's own border wins where both are set.
    if row.border_width[0] > 0.0 && cell.border_width[0] == 0.0 {
        cell.border_width[0] = row.border_width[0];
        cell.border_color = cell.border_color.or(row.border_color);
    }
    if row.border_width[2] > 0.0 && cell.border_width[2] == 0.0 {
        cell.border_width[2] = row.border_width[2];
        cell.border_color = cell.border_color.or(row.border_color);
    }
    if first_cell && row.border_width[3] > 0.0 && cell.border_width[3] == 0.0 {
        cell.border_width[3] = row.border_width[3];
        cell.border_color = cell.border_color.or(row.border_color);
    }
    if last_cell && row.border_width[1] > 0.0 && cell.border_width[1] == 0.0 {
        cell.border_width[1] = row.border_width[1];
        cell.border_color = cell.border_color.or(row.border_color);
    }

    // border-radius is ignored in the collapsed model (spec + Chrome).
    cell.border_radius = None;

    // One owner per edge. Cells own right+bottom; top/left belong to the
    // previous row/cell (which drew them as ITS bottom/right), except on
    // the table's outer edges, where either the first row/column keeps
    // them or the table's own border does.
    if !row_pos.first || table.table_has_border {
        cell.border_width[0] = 0.0;
    }
    if !first_cell || table.table_has_border {
        cell.border_width[3] = 0.0;
    }
    if last_cell && table.table_has_border {
        cell.border_width[1] = 0.0;
    }
    if row_pos.last && table.table_has_border {
        cell.border_width[2] = 0.0;
    }
}

// ── Margin boxes → Fixed bands ────────────────────────────────────────

/// Build the Fixed node for one margin band (top or bottom).
///
/// The band trick that makes the mapping exact: the engine's page margin
/// on this edge is set to 0 and this Fixed node, with explicit height
/// equal to the declared `@page` margin, occupies precisely the strip CSS
/// calls the margin. Content then starts exactly where `@page` said.
/// Inside: a 3-cell flex row (left/center/right), vertically centered via
/// justify-content on the fixed-height band.
pub(crate) fn build_margin_band(
    boxes: &[&MarginBox],
    band_height: f64,
    top: bool,
    pages: forme::model::FixedPageFilter,
    page_name: Option<String>,
    exclude_page_names: Vec<String>,
    warnings: &mut Vec<String>,
) -> Node {
    let mut cells: Vec<Node> = Vec::new();
    for slot in [0, 1, 2] {
        let want = match (top, slot) {
            (true, 0) => MarginBoxPos::TopLeft,
            (true, 1) => MarginBoxPos::TopCenter,
            (true, _) => MarginBoxPos::TopRight,
            (false, 0) => MarginBoxPos::BottomLeft,
            (false, 1) => MarginBoxPos::BottomCenter,
            (false, _) => MarginBoxPos::BottomRight,
        };
        let align = match slot {
            0 => forme::style::TextAlign::Left,
            1 => forme::style::TextAlign::Center,
            _ => forme::style::TextAlign::Right,
        };
        let cell_style = Style {
            width: Some(Dimension::Percent(100.0 / 3.0)),
            text_align: Some(align),
            ..Default::default()
        };
        let content_node = boxes.iter().find(|b| b.position == want).map(|b| {
            let computed = resolve(&b.style.normal, ROOT_FONT_SIZE, warnings);
            let mut text_style = to_engine_style(&computed);
            // Slot position dictates alignment unless the box set its own.
            if text_style.text_align.is_none() {
                text_style.text_align = Some(align);
            }
            // The band cell has no margins of its own.
            text_style.margin = None;
            make_node(
                NodeKind::Text {
                    content: b.content.clone(),
                    href: None,
                    runs: vec![],
                },
                text_style,
                vec![],
            )
        });
        cells.push(make_node(
            NodeKind::View,
            cell_style,
            content_node.into_iter().collect(),
        ));
    }

    let row = make_node(
        NodeKind::View,
        Style {
            flex_direction: Some(FlexDirection::Row),
            ..Default::default()
        },
        cells,
    );
    let band = make_node(
        NodeKind::View,
        Style {
            height: Some(Dimension::Pt(band_height)),
            justify_content: Some(forme::style::JustifyContent::Center),
            ..Default::default()
        },
        vec![row],
    );
    make_node(
        NodeKind::Fixed {
            position: if top {
                FixedPosition::Header
            } else {
                FixedPosition::Footer
            },
            pages,
            page_name,
            exclude_page_names,
        },
        Style::default(),
        vec![band],
    )
}

// ── Margin collapsing (pass 3) ────────────────────────────────────────

/// Whether a node participates in margin collapsing (block-level, in-flow).
/// Absolutely positioned nodes are out of flow and never collapse.
fn participates(node: &Node) -> bool {
    if matches!(node.style.position, Some(forme::model::Position::Absolute)) {
        return false;
    }
    matches!(
        node.kind,
        NodeKind::View
            | NodeKind::Text { .. }
            | NodeKind::Heading { .. }
            | NodeKind::List { .. }
            | NodeKind::Table { .. }
            | NodeKind::Image { .. }
    )
}

fn margin_of(node: &Node) -> MarginEdges {
    node.style.margin.unwrap_or_default()
}

/// The CSS collapse formula for two adjoining margins:
/// max of the positives plus min of the negatives.
fn collapse2(a: f64, b: f64) -> f64 {
    a.max(b).max(0.0) + a.min(b).min(0.0)
}

/// Collapse adjacent sibling margins in place: A's bottom absorbs the
/// joint value, B's top zeroes (the engine adds margins, so sum == joint).
pub fn collapse_sibling_margins(children: &mut [Node]) {
    for i in 1..children.len() {
        let (head, tail) = children.split_at_mut(i);
        let a = head.last_mut().unwrap();
        let b = &mut tail[0];
        if !participates(a) || !participates(b) {
            continue;
        }
        let (ma, mb) = (margin_of(a), margin_of(b));
        let (EdgeValue::Pt(bottom), EdgeValue::Pt(top)) = (ma.bottom, mb.top) else {
            continue; // auto margins don't collapse in the spike
        };
        let joint = collapse2(bottom, top);
        set_margin(a, |m| m.bottom = EdgeValue::Pt(joint));
        set_margin(b, |m| m.top = EdgeValue::Pt(0.0));
    }
}

/// Parent/first-and-last-child collapse-through: when nothing (border or
/// padding) separates a block parent's edge from its first/last child's
/// margin, the two margins collapse into the parent's.
pub fn collapse_into_parent(parent: &mut Computed, children: &mut [Node]) {
    if children.is_empty() {
        return;
    }
    // Top edge.
    if parent.border_width[0] == 0.0 && parent.padding[0] == 0.0 {
        let first = &mut children[0];
        if participates(first) {
            if let (MarginV::Pt(pm), EdgeValue::Pt(cm)) = (parent.margin[0], margin_of(first).top) {
                parent.margin[0] = MarginV::Pt(collapse2(pm, cm));
                set_margin(first, |m| m.top = EdgeValue::Pt(0.0));
            }
        }
    }
    // Bottom edge — only when the parent's height is auto.
    if parent.border_width[2] == 0.0 && parent.padding[2] == 0.0 && parent.height.is_none() {
        let last = children.last_mut().unwrap();
        if participates(last) {
            if let (MarginV::Pt(pm), EdgeValue::Pt(cm)) = (parent.margin[2], margin_of(last).bottom)
            {
                parent.margin[2] = MarginV::Pt(collapse2(pm, cm));
                set_margin(last, |m| m.bottom = EdgeValue::Pt(0.0));
            }
        }
    }
}

fn set_margin(node: &mut Node, f: impl FnOnce(&mut MarginEdges)) {
    let mut m = node.style.margin.unwrap_or_default();
    f(&mut m);
    node.style.margin = Some(m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse_html;

    fn map(html: &str) -> (Document, Vec<String>) {
        let body = parse_html(html);
        map_html(&body, Stylesheet::default(), PageConfig::default())
    }

    fn body_children(doc: &Document) -> &[Node] {
        &doc.children[0].children
    }

    #[test]
    fn whitespace_collapses_across_inline_boundaries() {
        let (doc, _) = map("<p>Due <strong>net\n   <span>30</span></strong>\n days.</p>");
        let p = &body_children(&doc)[0];
        let NodeKind::Text { runs, .. } = &p.kind else {
            panic!("expected Text, got {:?}", p.kind);
        };
        let full: String = runs.iter().map(|r| r.content.as_str()).collect();
        assert_eq!(full, "Due net 30 days.");
    }

    #[test]
    fn whitespace_only_text_between_blocks_is_dropped() {
        let (doc, _) = map("<div>\n  <p>a</p>\n  <p>b</p>\n</div>");
        let div = &body_children(&doc)[0];
        assert_eq!(div.children.len(), 2);
    }

    #[test]
    fn sibling_margins_collapse_to_max() {
        // h1 (mb = 0.67em × 24pt = 16.08) then p (mt = 1em × 12pt = 12):
        // joint margin must be 16.08, split as h1.bottom=16.08 / p.top=0.
        let (doc, _) = map("<h1>Title</h1><p>Body</p>");
        let kids = body_children(&doc);
        let h1_bottom = match kids[0].style.margin.unwrap().bottom {
            EdgeValue::Pt(v) => v,
            _ => panic!(),
        };
        let p_top = match kids[1].style.margin.unwrap().top {
            EdgeValue::Pt(v) => v,
            _ => panic!(),
        };
        assert!((h1_bottom - 16.08).abs() < 1e-6, "got {h1_bottom}");
        assert_eq!(p_top, 0.0);
    }

    #[test]
    fn h1_margin_collapses_into_body() {
        // body margin-top (6pt) collapses with h1's 16.08 → body carries
        // 16.08 and h1's top zeroes.
        let (doc, _) = map("<h1>Title</h1>");
        let body = &doc.children[0];
        let body_top = match body.style.margin.unwrap().top {
            EdgeValue::Pt(v) => v,
            _ => panic!(),
        };
        assert!((body_top - 16.08).abs() < 1e-6, "got {body_top}");
        let h1_top = match body.children[0].style.margin.unwrap().top {
            EdgeValue::Pt(v) => v,
            _ => panic!(),
        };
        assert_eq!(h1_top, 0.0);
    }

    #[test]
    fn thead_rows_are_headers() {
        let (doc, _) = map(
            "<table><thead><tr><th>A</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>",
        );
        let table = &body_children(&doc)[0];
        let NodeKind::Table { .. } = table.kind else {
            panic!()
        };
        let headers: Vec<bool> = table
            .children
            .iter()
            .map(|r| match r.kind {
                NodeKind::TableRow { is_header } => is_header,
                _ => panic!(),
            })
            .collect();
        assert_eq!(headers, vec![true, false]);
    }

    #[test]
    fn colspan_parses() {
        let (doc, _) = map("<table><tr><td colspan=\"2\">x</td></tr></table>");
        let cell = &body_children(&doc)[0].children[0].children[0];
        match cell.kind {
            NodeKind::TableCell { col_span, .. } => assert_eq!(col_span, 2),
            _ => panic!(),
        }
    }

    #[test]
    fn br_becomes_newline() {
        let (doc, _) = map("<p>line one<br>line two</p>");
        let p = &body_children(&doc)[0];
        let NodeKind::Text { content, runs, .. } = &p.kind else {
            panic!()
        };
        let full = if runs.is_empty() {
            content.clone()
        } else {
            runs.iter().map(|r| r.content.as_str()).collect()
        };
        assert_eq!(full, "line one\nline two");
    }

    #[test]
    fn flex_display_defaults_to_row() {
        let (doc, _) = map("<div style=\"display:flex\"><p>a</p><p>b</p></div>");
        let div = &body_children(&doc)[0];
        assert!(matches!(div.style.flex_direction, Some(FlexDirection::Row)));
    }

    #[test]
    fn external_image_warns_and_drops() {
        let (doc, warnings) = map("<img src=\"https://example.com/x.png\">");
        assert!(body_children(&doc).is_empty());
        assert!(warnings.iter().any(|w| w.contains("external image")));
    }
}
