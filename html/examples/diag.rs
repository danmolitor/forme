//! Layout-tree diagnostic: render an HTML file and print the element
//! tree with y/height, flagging large vertical gaps between siblings and
//! containers far taller than their children — the shapes the
//! measure/layout agreement check (`FORME_MEASURE_CHECK=1`) exists to
//! catch. Usage: `cargo run --example diag <file.html> [max_depth]`.

use forme::layout::ElementInfo;
use forme_pdf_html::{render_html_with_layout, HtmlOptions};

fn children_extent(el: &ElementInfo) -> f64 {
    el.children
        .iter()
        .map(|c| c.y + c.height)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn dump(els: &[ElementInfo], depth: usize, max_depth: usize) {
    let mut prev_bottom: Option<f64> = None;
    for el in els {
        let indent = "  ".repeat(depth);
        let text = el
            .text_content
            .as_deref()
            .map(|t| {
                let t = t.trim();
                let cut = t.char_indices().nth(28).map(|(i, _)| i).unwrap_or(t.len());
                format!(" {:?}", &t[..cut])
            })
            .unwrap_or_default();
        let mut flags = String::new();
        if let Some(pb) = prev_bottom {
            let gap = el.y - pb;
            if gap > 40.0 {
                flags.push_str(&format!("  <<< GAP {gap:.0}pt above"));
            }
        }
        if !el.children.is_empty() {
            let slack = (el.y + el.height) - children_extent(el);
            if slack > 40.0 {
                flags.push_str(&format!("  <<< {slack:.0}pt taller than content"));
            }
        }
        println!(
            "{indent}{} y={:.0} h={:.0} w={:.0} x={:.0}{}{}",
            el.node_type, el.y, el.height, el.width, el.x, text, flags
        );
        if depth < max_depth {
            dump(&el.children, depth + 1, max_depth);
        }
        prev_bottom = Some(el.y + el.height);
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: diag <file.html> [max_depth]");
    let max_depth: usize = std::env::args()
        .nth(2)
        .and_then(|d| d.parse().ok())
        .unwrap_or(3);
    let html = std::fs::read_to_string(&path).expect("read file");
    let out = render_html_with_layout(&html, &HtmlOptions::default()).expect("render");
    for w in &out.warnings {
        eprintln!("warning: {w}");
    }
    for (i, page) in out.layout.pages.iter().enumerate() {
        println!("──── page {} ────", i + 1);
        dump(&page.elements, 0, max_depth);
    }
}
