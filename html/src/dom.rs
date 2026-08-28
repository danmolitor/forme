//! HTML parsing via html5ever into a small owned DOM.
//!
//! We deliberately convert out of rcdom's `Rc<RefCell<...>>` graph into a
//! plain owned tree: the mapper does multiple normalization passes and an
//! owned structure keeps those passes free of borrow gymnastics.

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// An element in the simplified DOM. Tag names are lowercase.
#[derive(Debug, Clone)]
pub struct Element {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<DomNode>,
    /// 0-based position among the parent's ELEMENT children (what
    /// `:nth-child` counts). Assigned once at parse time.
    pub index: usize,
    /// Number of element children in the parent (for `:last-child`).
    pub sibling_count: usize,
    /// 0-based position among same-TAG element siblings (what
    /// `:nth-of-type` counts). Assigned once at parse time.
    pub type_index: usize,
    /// Number of same-tag element siblings (for `:last-of-type`).
    pub type_count: usize,
}

impl Element {
    /// Look up an attribute value by (lowercase) name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A node in the simplified DOM. Comments, doctypes, and processing
/// instructions are dropped during conversion.
#[derive(Debug, Clone)]
pub enum DomNode {
    Element(Element),
    Text(String),
}

/// Parse an HTML string and return the `<body>` element. Test convenience
/// wrapper around [`parse_html_with_styles`].
#[cfg(test)]
pub(crate) fn parse_html(html: &str) -> Element {
    parse_html_with_styles(html).0
}

/// Parse an HTML string, returning the `<body>` element, the text of
/// every `<style>` block, and the href of every stylesheet `<link>` —
/// all collected from the WHOLE document, because they live in `<head>`,
/// which the body-only mapper never sees. The links are collected purely
/// to warn: nothing is fetched, and a silently dropped
/// `<link rel="stylesheet">` would render the most common template shape
/// on earth unstyled with zero explanation.
pub fn parse_html_with_styles(html: &str) -> (Element, Vec<String>, Vec<String>) {
    let dom: RcDom = parse_document(RcDom::default(), Default::default()).one(html);
    let root = convert(&dom.document);
    let mut styles = Vec::new();
    let mut links = Vec::new();
    collect_style_sources(&root, &mut styles, &mut links);
    let body = find_body(&root).unwrap_or(Element {
        tag: "body".to_string(),
        attrs: vec![],
        children: root.children,
        index: 0,
        sibling_count: 1,
        type_index: 0,
        type_count: 1,
    });
    (body, styles, links)
}

fn collect_style_sources(el: &Element, styles: &mut Vec<String>, links: &mut Vec<String>) {
    for child in &el.children {
        if let DomNode::Element(e) = child {
            if e.tag == "style" {
                let text: String = e
                    .children
                    .iter()
                    .filter_map(|c| match c {
                        DomNode::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect();
                styles.push(text);
            } else {
                if e.tag == "link"
                    && e.attr("rel")
                        .is_some_and(|r| r.eq_ignore_ascii_case("stylesheet"))
                {
                    links.push(e.attr("href").unwrap_or("<no href>").to_string());
                }
                collect_style_sources(e, styles, links);
            }
        }
    }
}

/// Convert the rcdom graph into the owned tree. The document node itself
/// becomes a synthetic element so the walk has a uniform shape.
fn convert(handle: &Handle) -> Element {
    let mut children = Vec::new();
    convert_children(handle, &mut children);
    Element {
        tag: "#document".to_string(),
        attrs: vec![],
        children,
        index: 0,
        sibling_count: 1,
        type_index: 0,
        type_count: 1,
    }
}

/// Fill in sibling indices after a children list is complete.
fn assign_sibling_positions(out: &mut [DomNode]) {
    let count = out
        .iter()
        .filter(|n| matches!(n, DomNode::Element(_)))
        .count();
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for node in out.iter() {
        if let DomNode::Element(e) = node {
            *type_counts.entry(e.tag.clone()).or_insert(0) += 1;
        }
    }
    let mut idx = 0;
    let mut type_seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for node in out.iter_mut() {
        if let DomNode::Element(e) = node {
            e.index = idx;
            e.sibling_count = count;
            let seen = type_seen.entry(e.tag.clone()).or_insert(0);
            e.type_index = *seen;
            *seen += 1;
            e.type_count = type_counts[&e.tag];
            idx += 1;
        }
    }
}

fn convert_children(handle: &Handle, out: &mut Vec<DomNode>) {
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.as_ref().to_ascii_lowercase();
                let attrs = attrs
                    .borrow()
                    .iter()
                    .map(|a| {
                        (
                            a.name.local.as_ref().to_ascii_lowercase(),
                            a.value.to_string(),
                        )
                    })
                    .collect();
                let mut grandchildren = Vec::new();
                convert_children(child, &mut grandchildren);
                out.push(DomNode::Element(Element {
                    tag,
                    attrs,
                    children: grandchildren,
                    index: 0,
                    sibling_count: 0,
                    type_index: 0,
                    type_count: 0,
                }));
            }
            NodeData::Text { contents } => {
                out.push(DomNode::Text(contents.borrow().to_string()));
            }
            // Comments, doctype, PI: not content.
            _ => {}
        }
    }
    assign_sibling_positions(out);
}

fn find_body(el: &Element) -> Option<Element> {
    for child in &el.children {
        if let DomNode::Element(e) = child {
            if e.tag == "body" {
                return Some(e.clone());
            }
            if let Some(body) = find_body(e) {
                return Some(body);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sloppy_fragment_to_body() {
        let body = parse_html("<div>\n  <p>hi <b>there</b></p>\n  <!-- c -->\n</div>");
        assert_eq!(body.tag, "body");
        assert_eq!(body.children.len(), 1);
        match &body.children[0] {
            DomNode::Element(div) => {
                assert_eq!(div.tag, "div");
            }
            other => panic!("expected div, got {other:?}"),
        }
    }

    #[test]
    fn drops_comments_keeps_whitespace_text() {
        let body = parse_html("<p>a</p>\n<!-- gone -->\n<p>b</p>");
        // Whitespace text nodes between the <p>s survive parsing — the
        // whitespace-collapsing pass in map.rs is responsible for them.
        let has_ws_text = body
            .children
            .iter()
            .any(|n| matches!(n, DomNode::Text(t) if t.trim().is_empty()));
        assert!(has_ws_text);
    }
}
