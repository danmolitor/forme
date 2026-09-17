//! Print a rendered document's layout tree, for diagnosing box sizing.
//!
//!   cargo run --example dump_layout                    # tests/fixtures/invoice.html
//!   cargo run --example dump_layout -- page.html       # any file
//!   cargo run --example dump_layout -- page.html --json  # machine-readable
//!
//! The indented form is for reading; `--json` is for querying widths across
//! a large tree, which is how the auto-width block collapse was found.
use forme_pdf_html::{render_html_with_layout, HtmlOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_out = args.iter().any(|a| a == "--json");
    let path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "tests/fixtures/invoice.html".to_string());

    let html = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let out = render_html_with_layout(&html, &HtmlOptions::default()).unwrap();

    if json_out {
        println!("{}", serde_json::to_string(&out.layout).unwrap());
        return;
    }

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
