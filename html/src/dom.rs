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

/// Parse an HTML string and return the `<body>` element.
///
/// html5ever builds the full html/head/body scaffolding even for fragments,
/// so a body element always exists.
pub fn parse_html(html: &str) -> Element {
    let dom: RcDom = parse_document(RcDom::default(), Default::default()).one(html);
    let root = convert(&dom.document);
    find_body(&root).unwrap_or(Element {
        tag: "body".to_string(),
        attrs: vec![],
        children: root.children,
    })
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
                }));
            }
            NodeData::Text { contents } => {
                out.push(DomNode::Text(contents.borrow().to_string()));
            }
            // Comments, doctype, PI: not content.
            _ => {}
        }
    }
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
