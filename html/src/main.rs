//! `forme-html` — render an HTML file to PDF from the command line.
//!
//! ```text
//! forme-html invoice.html                     # writes invoice.pdf
//! forme-html invoice.html -o out.pdf
//! forme-html invoice.html --css print.css --page-size Letter --margin 36
//! ```

use forme_pdf_html::{render_html, FontSpec, HtmlOptions, PageSize};
use std::path::Path;
use std::process::ExitCode;

/// Host-layer `<link rel="stylesheet">` resolution. The library never fetches
/// anything; the CLI, which has the filesystem and the input's location, reads
/// local stylesheets and inlines them **in place** as `<style>` blocks so they
/// cascade at the `<link>`'s source position (a `<link>` before a `<style>`
/// stays earlier in source order — which appending to `options.css` could not
/// preserve). Absolute `http(s)://` (and protocol-relative `//`) hrefs are left
/// untouched, so the library still emits its "never fetched" warning for them.
/// A missing local file is a hard error naming the resolved path, never a
/// silent skip.
fn inline_stylesheet_links(html: &str, base_dir: &Path) -> Result<String, String> {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    // Case-insensitive search for `<link` tag starts.
    let lower = html.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel_pos) = lower[search..].find("<link") {
        let start = search + rel_pos;
        // Must be a tag boundary: next char is whitespace, '>' or '/'.
        let after = bytes.get(start + 5).copied();
        if !matches!(after, Some(b) if b.is_ascii_whitespace() || b == b'>' || b == b'/') {
            search = start + 5;
            continue;
        }
        let Some(gt_rel) = html[start..].find('>') else {
            break; // unterminated tag — leave the rest verbatim
        };
        let end = start + gt_rel + 1;
        let tag = &html[start..end];
        search = end;

        let rel = tag_attr(tag, "rel");
        let href = tag_attr(tag, "href");
        let is_stylesheet = rel
            .as_deref()
            .map(|r| {
                r.split_ascii_whitespace()
                    .any(|t| t.eq_ignore_ascii_case("stylesheet"))
            })
            .unwrap_or(false);
        let Some(href) = href else { continue };
        if !is_stylesheet || href.is_empty() {
            continue;
        }
        let lower_href = href.to_ascii_lowercase();
        if lower_href.starts_with("http://")
            || lower_href.starts_with("https://")
            || href.starts_with("//")
        {
            continue; // absolute — leave for the library to warn about
        }

        let resolved = base_dir.join(&href);
        let css = std::fs::read_to_string(&resolved).map_err(|e| {
            format!(
                "cannot read stylesheet '{}' (from <link href=\"{}\">): {e}",
                resolved.display(),
                href
            )
        })?;
        // Emit everything up to the tag, then the inlined <style>.
        out.push_str(&html[cursor..start]);
        out.push_str("<style>\n");
        out.push_str(&css);
        out.push_str("\n</style>");
        cursor = end;
    }
    out.push_str(&html[cursor..]);
    Ok(out)
}

/// Extract a quoted attribute value from a single tag string. Minimal, tolerant
/// of single/double quotes; good enough for `<link>` in the host layer.
fn tag_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0usize;
    loop {
        let idx = lower[from..].find(name)? + from;
        // Attribute name must be preceded by whitespace and followed by '='.
        let pre_ok = tag[..idx]
            .chars()
            .last()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
        let rest = tag[idx + name.len()..].trim_start();
        if pre_ok && rest.starts_with('=') {
            let after_eq = rest[1..].trim_start();
            let quote = after_eq.chars().next()?;
            if quote == '"' || quote == '\'' {
                let val = &after_eq[1..];
                let close = val.find(quote)?;
                return Some(val[..close].to_string());
            }
        }
        from = idx + name.len();
    }
}

const USAGE: &str = "\
forme-html — HTML + print-CSS to PDF, no browser

USAGE:
    forme-html <input.html> [OPTIONS]

OPTIONS:
    -o, --output <file>     Output path (default: input with .pdf extension)
        --css <file>        Extra stylesheet applied after the document's own
        --page-size <size>  A4, A3, A5, Letter, Legal, Tabloid
                            (overrides the document's @page rule; default A4)
        --margin <pt>       Uniform page margin in points
                            (overrides @page margins; default 54)
        --font <spec>       Register a TTF: 'Family=path.ttf'. Repeatable.
                            Variants: 'Family:700=..', 'Family:bold:italic=..
        --tagged            Emit a tagged PDF (structure tree)
        --pdf-ua            Emit a PDF/UA-1 conforming file (implies --tagged;
                            register embeddable fonts via --font and set --lang)
        --lang <lang>       Document language for PDF/UA (e.g. en, en-US)
    -q, --quiet             Suppress unsupported-CSS warnings
    -h, --help              Show this help
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return if args.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }

    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut css_path: Option<String> = None;
    let mut options = HtmlOptions::default();
    let mut quiet = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let take_value = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("{arg} requires a value"))
        };
        let result: Result<(), String> = (|| {
            match arg.as_str() {
                "-o" | "--output" => output = Some(take_value(&mut i)?),
                "--css" => css_path = Some(take_value(&mut i)?),
                "--page-size" => {
                    let v = take_value(&mut i)?;
                    options.page_size = Some(match v.to_ascii_lowercase().as_str() {
                        "a4" => PageSize::A4,
                        "a3" => PageSize::A3,
                        "a5" => PageSize::A5,
                        "letter" => PageSize::Letter,
                        "legal" => PageSize::Legal,
                        "tabloid" => PageSize::Tabloid,
                        other => return Err(format!("unknown page size '{other}'")),
                    });
                }
                "--margin" => {
                    let v = take_value(&mut i)?;
                    options.page_margin =
                        Some(v.parse().map_err(|_| format!("invalid margin '{v}'"))?);
                }
                "--font" => {
                    let spec = take_value(&mut i)?;
                    let (head, path) = spec
                        .split_once('=')
                        .ok_or_else(|| format!("--font expects 'Family=path', got '{spec}'"))?;
                    let mut parts = head.split(':');
                    let family = parts.next().unwrap_or_default().to_string();
                    let mut weight = 400u32;
                    let mut italic = false;
                    for part in parts {
                        match part.to_ascii_lowercase().as_str() {
                            "bold" => weight = 700,
                            "italic" => italic = true,
                            n => {
                                weight = n
                                    .parse()
                                    .map_err(|_| format!("bad font variant '{part}'"))?
                            }
                        }
                    }
                    let data =
                        std::fs::read(path).map_err(|e| format!("cannot read font {path}: {e}"))?;
                    options.fonts.push(FontSpec {
                        family,
                        data,
                        weight,
                        italic,
                    });
                }
                "--tagged" => options.tagged = true,
                "--pdf-ua" => options.pdf_ua = true,
                "--lang" => options.lang = Some(take_value(&mut i)?),
                "-q" | "--quiet" => quiet = true,
                other if other.starts_with('-') => {
                    return Err(format!("unknown option '{other}'"));
                }
                _ => {
                    if input.is_some() {
                        return Err("multiple input files given".to_string());
                    }
                    input = Some(arg.clone());
                }
            }
            Ok(())
        })();
        if let Err(msg) = result {
            eprintln!("error: {msg}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
        i += 1;
    }

    let Some(input) = input else {
        eprintln!("error: no input file\n\n{USAGE}");
        return ExitCode::FAILURE;
    };

    let html = match std::fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Resolve local <link rel="stylesheet"> against the input file's directory.
    let base_dir = Path::new(&input).parent().unwrap_or_else(|| Path::new("."));
    let html = match inline_stylesheet_links(&html, base_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(path) = css_path {
        match std::fs::read_to_string(&path) {
            Ok(css) => options.css = Some(css),
            Err(e) => {
                eprintln!("error: cannot read {path}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    let out_path = output.unwrap_or_else(|| {
        let stem = input.strip_suffix(".html").or(input.strip_suffix(".htm"));
        format!("{}.pdf", stem.unwrap_or(&input))
    });

    match render_html(&html, &options) {
        Ok(out) => {
            if !quiet {
                for w in &out.warnings {
                    eprintln!("warning: {w}");
                }
            }
            if let Err(e) = std::fs::write(&out_path, &out.pdf) {
                eprintln!("error: cannot write {out_path}: {e}");
                return ExitCode::FAILURE;
            }
            eprintln!("{out_path} ({} bytes)", out.pdf.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: render failed: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("forme-link-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn inlines_local_link_in_source_order() {
        let dir = scratch("order");
        fs::write(dir.join("brand.css"), "h1{color:#ff0000}").unwrap();
        let html = r#"<head><link rel="stylesheet" href="brand.css"><style>h1{color:#00ff00}</style></head>"#;
        let out = inline_stylesheet_links(html, &dir).unwrap();
        assert!(
            out.contains("<style>\nh1{color:#ff0000}\n</style>"),
            "{out}"
        );
        assert!(!out.contains("<link"), "{out}");
        // The linked stylesheet must precede the document's own <style>.
        assert!(out.find("#ff0000").unwrap() < out.find("#00ff00").unwrap());
    }

    #[test]
    fn missing_local_stylesheet_is_a_named_error() {
        let dir = scratch("missing");
        let err = inline_stylesheet_links(r#"<link rel="stylesheet" href="nope.css">"#, &dir)
            .unwrap_err();
        assert!(err.contains("nope.css"), "{err}");
    }

    #[test]
    fn absolute_href_is_left_for_the_library_to_warn() {
        let dir = scratch("abs");
        let html = r#"<link rel="stylesheet" href="https://cdn/app.css">"#;
        assert_eq!(inline_stylesheet_links(html, &dir).unwrap(), html);
    }

    #[test]
    fn non_stylesheet_link_untouched() {
        let dir = scratch("rel");
        let html = r#"<link rel="icon" href="favicon.ico">"#;
        assert_eq!(inline_stylesheet_links(html, &dir).unwrap(), html);
    }

    #[test]
    fn multiple_links_all_resolved() {
        let dir = scratch("multi");
        fs::write(dir.join("a.css"), "a{color:red}").unwrap();
        fs::write(dir.join("b.css"), "b{color:blue}").unwrap();
        let html = r#"<link rel="stylesheet" href="a.css"><link rel="stylesheet" href="b.css">"#;
        let out = inline_stylesheet_links(html, &dir).unwrap();
        assert!(
            out.contains("a{color:red}") && out.contains("b{color:blue}"),
            "{out}"
        );
        assert!(!out.contains("<link"), "{out}");
    }
}
