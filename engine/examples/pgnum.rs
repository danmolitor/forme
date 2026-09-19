fn main() {
    let filler: String = (1..=120).map(|i| format!(
        r#"{{ "kind": {{ "type": "Text", "content": "line {i}" }}, "style": {{}}, "children": [] }},"#)).collect();
    for (label, fixed) in [("fixed footer", true), ("inline text", false)] {
        let node = if fixed {
            r#"{ "kind": { "type": "Fixed", "position": "Footer" }, "style": {},
                 "children": [ { "kind": { "type": "Text", "content": "Page {{pageNumber}} of {{totalPages}}" }, "style": {}, "children": [] } ] }"#
        } else {
            r#"{ "kind": { "type": "Text", "content": "Page {{pageNumber}} of {{totalPages}}" }, "style": {}, "children": [] }"#
        };
        let json = format!(
            r#"{{ "children": [ {node}, {} ], "metadata": {{}} }}"#,
            filler.trim_end_matches(',')
        );
        match forme::render_json_with_layout(&json) {
            Ok((pdf, l, _w)) => {
                std::fs::write("/tmp/pg.pdf", &pdf).unwrap();
                let out = std::process::Command::new("pdftotext")
                    .args(["/tmp/pg.pdf", "-"])
                    .output()
                    .unwrap();
                let s = String::from_utf8_lossy(&out.stdout);
                let found: Vec<&str> = s
                    .lines()
                    .filter(|l| l.contains("Page ") && l.contains(" of "))
                    .collect();
                println!("{label}: {} pages -> {:?}", l.pages.len(), found);
            }
            Err(e) => println!("{label}: ERROR {e:?}"),
        }
    }
}
