//! `forme-html` — render an HTML file to PDF from the command line.
//!
//! ```text
//! forme-html invoice.html                     # writes invoice.pdf
//! forme-html invoice.html -o out.pdf
//! forme-html invoice.html --css print.css --page-size Letter --margin 36
//! ```

use forme_pdf_html::{render_html, FontSpec, HtmlOptions, PageSize};
use std::process::ExitCode;

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
