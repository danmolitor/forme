use forme_pdf_html::{render_html_with_layout, HtmlOptions};

fn main() {
    let html = std::fs::read_to_string("tests/fixtures/invoice.html").unwrap();
    let out = render_html_with_layout(&html, &HtmlOptions::default()).unwrap();
    println!("pages: {}", out.layout.pages.len());
    println!("warnings: {:#?}", out.warnings);
    for (i, page) in out.layout.pages.iter().enumerate() {
        println!("--- page {} ---", i);
        dump(&page.elements, 0);
    }
}

fn dump(els: &[forme::layout::ElementInfo], depth: usize) {
    for el in els {
        let t = el.text_content.as_deref().unwrap_or("");
        let t: String = t.chars().take(40).collect();
        println!(
            "{}{} [{}] x={:.1} y={:.1} w={:.1} h={:.1} {}",
            "  ".repeat(depth),
            el.node_type,
            el.kind,
            el.x,
            el.y,
            el.width,
            el.height,
            t
        );
        dump(&el.children, depth + 1);
    }
}
