//! # Forme CLI
//!
//! Usage:
//!   forme input.json -o output.pdf
//!   echo '{ ... }' | forme -o output.pdf
//!   forme --example > invoice.json

use std::env;
use std::fs;
use std::io::{self, Read};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Handle --example flag
    if args.iter().any(|a| a == "--example") {
        print!("{}", example_invoice_json());
        return;
    }

    // Read input
    let input = if args.len() > 1 && !args[1].starts_with('-') {
        fs::read_to_string(&args[1]).expect("Failed to read input file")
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).expect("Failed to read stdin");
        buf
    };

    // Parse output path
    let output_path = args
        .windows(2)
        .find(|w| w[0] == "-o")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "output.pdf".to_string());

    // Render
    match forme::render_json(&input) {
        Ok(pdf_bytes) => {
            fs::write(&output_path, &pdf_bytes).expect("Failed to write PDF");
            eprintln!(
                "✓ Written {} bytes to {}",
                pdf_bytes.len(),
                output_path
            );
        }
        Err(e) => {
            eprintln!("✗ Failed to parse document: {}", e);
            std::process::exit(1);
        }
    }
}

fn example_invoice_json() -> &'static str {
    r##"{
  "metadata": {
    "title": "Invoice #INV-2026-001",
    "author": "Forme"
  },
  "defaultPage": {
    "size": "A4",
    "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 }
  },
  "children": [
    {
      "kind": { "type": "View" },
      "style": {
        "flexDirection": "Row",
        "justifyContent": "SpaceBetween",
        "padding": { "top": 0, "right": 0, "bottom": 24, "left": 0 }
      },
      "children": [
        {
          "kind": { "type": "Text", "content": "INVOICE" },
          "style": {
            "fontSize": 32,
            "fontWeight": 700,
            "color": { "r": 0.1, "g": 0.1, "b": 0.15, "a": 1.0 }
          }
        },
        {
          "kind": { "type": "View" },
          "style": { "alignItems": "FlexEnd" },
          "children": [
            {
              "kind": { "type": "Text", "content": "Acme Corp" },
              "style": { "fontSize": 14, "fontWeight": 700 }
            },
            {
              "kind": { "type": "Text", "content": "123 Business St, Suite 100" },
              "style": { "fontSize": 10, "color": { "r": 0.4, "g": 0.4, "b": 0.4, "a": 1.0 } }
            },
            {
              "kind": { "type": "Text", "content": "San Francisco, CA 94102" },
              "style": { "fontSize": 10, "color": { "r": 0.4, "g": 0.4, "b": 0.4, "a": 1.0 } }
            }
          ]
        }
      ]
    },
    {
      "kind": { "type": "View" },
      "style": {
        "flexDirection": "Row",
        "gap": 48,
        "padding": { "top": 0, "right": 0, "bottom": 24, "left": 0 }
      },
      "children": [
        {
          "kind": { "type": "View" },
          "children": [
            {
              "kind": { "type": "Text", "content": "Bill To:" },
              "style": { "fontSize": 10, "fontWeight": 700, "color": { "r": 0.4, "g": 0.4, "b": 0.4, "a": 1.0 } }
            },
            {
              "kind": { "type": "Text", "content": "Widget Industries" },
              "style": { "fontSize": 12, "fontWeight": 700 }
            },
            {
              "kind": { "type": "Text", "content": "456 Client Ave\nNew York, NY 10001" },
              "style": { "fontSize": 10 }
            }
          ]
        },
        {
          "kind": { "type": "View" },
          "children": [
            {
              "kind": { "type": "Text", "content": "Invoice #: INV-2026-001" },
              "style": { "fontSize": 10 }
            },
            {
              "kind": { "type": "Text", "content": "Date: February 14, 2026" },
              "style": { "fontSize": 10 }
            },
            {
              "kind": { "type": "Text", "content": "Due: March 1, 2026" },
              "style": { "fontSize": 10 }
            }
          ]
        }
      ]
    },
    {
      "kind": {
        "type": "Table",
        "columns": [
          { "width": { "Fraction": 0.45 } },
          { "width": { "Fraction": 0.15 } },
          { "width": { "Fraction": 0.20 } },
          { "width": { "Fraction": 0.20 } }
        ]
      },
      "style": {
        "padding": { "top": 0, "right": 0, "bottom": 24, "left": 0 }
      },
      "children": [
        {
          "kind": { "type": "TableRow", "is_header": true },
          "style": {
            "backgroundColor": { "r": 0.12, "g": 0.12, "b": 0.18, "a": 1.0 }
          },
          "children": [
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Description" }, "style": { "fontSize": 10, "fontWeight": 700, "color": { "r": 1, "g": 1, "b": 1, "a": 1 } } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Qty" }, "style": { "fontSize": 10, "fontWeight": 700, "color": { "r": 1, "g": 1, "b": 1, "a": 1 } } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Unit Price" }, "style": { "fontSize": 10, "fontWeight": 700, "color": { "r": 1, "g": 1, "b": 1, "a": 1 } } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Total" }, "style": { "fontSize": 10, "fontWeight": 700, "color": { "r": 1, "g": 1, "b": 1, "a": 1 } } }]
            }
          ]
        },
        {
          "kind": { "type": "TableRow", "is_header": false },
          "style": {},
          "children": [
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Web Development Services" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "40" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$150.00" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$6,000.00" }, "style": { "fontSize": 10 } }]
            }
          ]
        },
        {
          "kind": { "type": "TableRow", "is_header": false },
          "style": {
            "backgroundColor": { "r": 0.97, "g": 0.97, "b": 0.98, "a": 1.0 }
          },
          "children": [
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "UI/UX Design" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "20" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$175.00" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$3,500.00" }, "style": { "fontSize": 10 } }]
            }
          ]
        },
        {
          "kind": { "type": "TableRow", "is_header": false },
          "children": [
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "Server Infrastructure Setup" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "1" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$2,500.00" }, "style": { "fontSize": 10 } }]
            },
            {
              "kind": { "type": "TableCell" },
              "style": { "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 } },
              "children": [{ "kind": { "type": "Text", "content": "$2,500.00" }, "style": { "fontSize": 10 } }]
            }
          ]
        }
      ]
    },
    {
      "kind": { "type": "View" },
      "style": {
        "alignItems": "FlexEnd",
        "padding": { "top": 12, "right": 0, "bottom": 0, "left": 0 }
      },
      "children": [
        {
          "kind": { "type": "View" },
          "style": {
            "width": { "Pt": 200 },
            "flexDirection": "Row",
            "justifyContent": "SpaceBetween",
            "padding": { "top": 4, "right": 0, "bottom": 4, "left": 0 }
          },
          "children": [
            { "kind": { "type": "Text", "content": "Subtotal:" }, "style": { "fontSize": 10 } },
            { "kind": { "type": "Text", "content": "$12,000.00" }, "style": { "fontSize": 10 } }
          ]
        },
        {
          "kind": { "type": "View" },
          "style": {
            "width": { "Pt": 200 },
            "flexDirection": "Row",
            "justifyContent": "SpaceBetween",
            "padding": { "top": 4, "right": 0, "bottom": 4, "left": 0 }
          },
          "children": [
            { "kind": { "type": "Text", "content": "Tax (8%):" }, "style": { "fontSize": 10 } },
            { "kind": { "type": "Text", "content": "$960.00" }, "style": { "fontSize": 10 } }
          ]
        },
        {
          "kind": { "type": "View" },
          "style": {
            "width": { "Pt": 200 },
            "flexDirection": "Row",
            "justifyContent": "SpaceBetween",
            "padding": { "top": 8, "right": 0, "bottom": 4, "left": 0 },
            "borderWidth": { "top": 1.5, "right": 0, "bottom": 0, "left": 0 },
            "borderColor": { "top": { "r": 0.12, "g": 0.12, "b": 0.18, "a": 1 }, "right": { "r": 0, "g": 0, "b": 0, "a": 1 }, "bottom": { "r": 0, "g": 0, "b": 0, "a": 1 }, "left": { "r": 0, "g": 0, "b": 0, "a": 1 } }
          },
          "children": [
            { "kind": { "type": "Text", "content": "Total:" }, "style": { "fontSize": 14, "fontWeight": 700 } },
            { "kind": { "type": "Text", "content": "$12,960.00" }, "style": { "fontSize": 14, "fontWeight": 700 } }
          ]
        }
      ]
    },
    {
      "kind": { "type": "View" },
      "style": {
        "padding": { "top": 48, "right": 0, "bottom": 0, "left": 0 }
      },
      "children": [
        {
          "kind": { "type": "Text", "content": "Payment Terms" },
          "style": { "fontSize": 11, "fontWeight": 700, "padding": { "top": 0, "right": 0, "bottom": 4, "left": 0 } }
        },
        {
          "kind": { "type": "Text", "content": "Payment is due within 15 days of invoice date. Please make checks payable to Acme Corp or wire transfer to the account details provided separately." },
          "style": { "fontSize": 9, "color": { "r": 0.35, "g": 0.35, "b": 0.4, "a": 1.0 }, "lineHeight": 1.5 }
        }
      ]
    }
  ]
}"##
}
