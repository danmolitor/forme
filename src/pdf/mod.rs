//! # PDF Serializer
//!
//! Takes the laid-out pages from the layout engine and writes a valid PDF file.
//!
//! This is a from-scratch PDF 1.7 writer. We write the raw bytes ourselves
//! because it gives us full control over the output and makes the engine
//! self-contained. The PDF spec is verbose but the subset we need for
//! document rendering is manageable.
//!
//! ## PDF Structure (simplified)
//!
//! ```text
//! %PDF-1.7            <- header
//! 1 0 obj ... endobj  <- objects (fonts, pages, content streams, etc.)
//! 2 0 obj ... endobj
//! ...
//! xref                <- cross-reference table (byte offsets of each object)
//! trailer             <- points to the root object
//! %%EOF
//! ```

use std::fmt::Write as FmtWrite; // for write! on String
use std::io::Write as IoWrite;   // for write! on Vec<u8>

use crate::layout::*;
use crate::model::*;
use crate::style::{Color, FontStyle};
use crate::font::{FontContext, FontData, FontKey};
use miniz_oxide::deflate::compress_to_vec_zlib;

pub struct PdfWriter;

/// Tracks allocated PDF objects during writing.
struct PdfBuilder {
    objects: Vec<PdfObject>,
    /// Maps (family, weight, italic) -> (object_id, index)
    font_objects: Vec<(FontKey, usize)>,
}

struct PdfObject {
    #[allow(dead_code)]
    id: usize,
    data: Vec<u8>,
}

impl PdfWriter {
    pub fn new() -> Self {
        Self
    }

    /// Write laid-out pages to a PDF byte vector.
    pub fn write(&self, pages: &[LayoutPage], metadata: &Metadata, font_context: &FontContext) -> Vec<u8> {
        let mut builder = PdfBuilder {
            objects: Vec::new(),
            font_objects: Vec::new(),
        };

        // Reserve object IDs:
        // 0 = placeholder (PDF objects are 1-indexed)
        // 1 = Catalog
        // 2 = Pages (page tree root)
        // 3+ = fonts, then page objects, then content streams
        builder.objects.push(PdfObject { id: 0, data: vec![] });
        builder.objects.push(PdfObject { id: 1, data: vec![] });
        builder.objects.push(PdfObject { id: 2, data: vec![] });

        // Register the fonts actually used across all pages
        self.register_fonts(&mut builder, pages, font_context);

        // Build page objects and content streams
        let mut page_obj_ids: Vec<usize> = Vec::new();

        for page in pages {
            let content = self.build_content_stream(page, &builder.font_objects);
            let compressed = compress_to_vec_zlib(content.as_bytes(), 6);

            let content_obj_id = builder.objects.len();
            let mut content_data: Vec<u8> = Vec::new();
            let _ = write!(
                content_data,
                "<< /Length {} /Filter /FlateDecode >>\nstream\n",
                compressed.len()
            );
            content_data.extend_from_slice(&compressed);
            content_data.extend_from_slice(b"\nendstream");
            builder.objects.push(PdfObject {
                id: content_obj_id,
                data: content_data,
            });

            let page_obj_id = builder.objects.len();
            let font_resources = self.build_font_resource_dict(&builder.font_objects);
            let page_dict = format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Contents {} 0 R /Resources << /Font << {} >> >> >>",
                page.width, page.height, content_obj_id, font_resources
            );
            builder.objects.push(PdfObject {
                id: page_obj_id,
                data: page_dict.into_bytes(),
            });
            page_obj_ids.push(page_obj_id);
        }

        // Write Catalog (object 1)
        builder.objects[1].data = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();

        // Write Pages tree (object 2)
        let kids: String = page_obj_ids
            .iter()
            .map(|id| format!("{} 0 R", id))
            .collect::<Vec<_>>()
            .join(" ");
        builder.objects[2].data = format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids,
            page_obj_ids.len()
        )
        .into_bytes();

        // Info dictionary (metadata)
        let info_obj_id = if metadata.title.is_some() || metadata.author.is_some() {
            let id = builder.objects.len();
            let mut info = String::from("<< ");
            if let Some(ref title) = metadata.title {
                let _ = write!(info, "/Title ({}) ", Self::escape_pdf_string(title));
            }
            if let Some(ref author) = metadata.author {
                let _ = write!(info, "/Author ({}) ", Self::escape_pdf_string(author));
            }
            if let Some(ref subject) = metadata.subject {
                let _ = write!(info, "/Subject ({}) ", Self::escape_pdf_string(subject));
            }
            let _ = write!(info, "/Producer (Forme 0.1) /Creator (Forme) >>");
            builder.objects.push(PdfObject {
                id,
                data: info.into_bytes(),
            });
            Some(id)
        } else {
            None
        };

        self.serialize(&builder, info_obj_id)
    }

    /// Build the PDF content stream for a single page.
    fn build_content_stream(
        &self,
        page: &LayoutPage,
        font_objects: &[(FontKey, usize)],
    ) -> String {
        let mut stream = String::new();
        let page_height = page.height;

        for element in &page.elements {
            self.write_element(&mut stream, element, page_height, font_objects);
        }

        stream
    }

    /// Write a single layout element as PDF operators.
    fn write_element(
        &self,
        stream: &mut String,
        element: &LayoutElement,
        page_height: f64,
        font_objects: &[(FontKey, usize)],
    ) {
        match &element.draw {
            DrawCommand::None => {}

            DrawCommand::Rect {
                background,
                border_width,
                border_color,
                border_radius,
            } => {
                let x = element.x;
                let y = page_height - element.y - element.height;
                let w = element.width;
                let h = element.height;

                if let Some(bg) = background {
                    if bg.a > 0.0 {
                        let _ = write!(stream, "q\n{:.3} {:.3} {:.3} rg\n", bg.r, bg.g, bg.b);

                        if border_radius.top_left > 0.0 {
                            self.write_rounded_rect(stream, x, y, w, h, border_radius);
                        } else {
                            let _ = write!(stream, "{:.2} {:.2} {:.2} {:.2} re\n", x, y, w, h);
                        }

                        let _ = write!(stream, "f\nQ\n");
                    }
                }

                let bw = border_width;
                if bw.top > 0.0 || bw.right > 0.0 || bw.bottom > 0.0 || bw.left > 0.0 {
                    if (bw.top - bw.right).abs() < 0.001
                        && (bw.right - bw.bottom).abs() < 0.001
                        && (bw.bottom - bw.left).abs() < 0.001
                    {
                        let bc = &border_color.top;
                        let _ = write!(
                            stream,
                            "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n",
                            bc.r, bc.g, bc.b, bw.top
                        );

                        if border_radius.top_left > 0.0 {
                            self.write_rounded_rect(stream, x, y, w, h, border_radius);
                        } else {
                            let _ = write!(stream, "{:.2} {:.2} {:.2} {:.2} re\n", x, y, w, h);
                        }

                        let _ = write!(stream, "S\nQ\n");
                    } else {
                        self.write_border_sides(stream, x, y, w, h, bw, border_color);
                    }
                }
            }

            DrawCommand::Text { lines, color } => {
                let _ = write!(
                    stream,
                    "BT\n{:.3} {:.3} {:.3} rg\n",
                    color.r, color.g, color.b
                );

                for line in lines {
                    // Determine font resource name from glyph weight/style
                    let font_name = if !line.glyphs.is_empty() {
                        let g = &line.glyphs[0];
                        let idx = self.font_index(
                            &g.font_family, g.font_weight, g.font_style, font_objects,
                        );
                        format!("F{}", idx)
                    } else {
                        "F0".to_string()
                    };

                    let font_size = if !line.glyphs.is_empty() {
                        line.glyphs[0].font_size
                    } else {
                        12.0
                    };

                    let pdf_y = page_height - line.y;

                    let _ = write!(
                        stream,
                        "/{} {:.1} Tf\n{:.2} {:.2} Td\n",
                        font_name, font_size, line.x, pdf_y
                    );

                    let text: String = line.glyphs.iter().map(|g| g.char_value).collect();
                    let _ = write!(stream, "({}) Tj\n", Self::escape_pdf_string(&text));
                }

                let _ = write!(stream, "ET\n");
            }

            DrawCommand::Image { .. } => {
                let x = element.x;
                let y = page_height - element.y - element.height;
                let _ = write!(
                    stream,
                    "q\n0.9 0.9 0.9 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\nQ\n",
                    x, y, element.width, element.height
                );
            }
        }

        for child in &element.children {
            self.write_element(stream, child, page_height, font_objects);
        }
    }

    fn write_rounded_rect(
        &self,
        stream: &mut String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: &crate::style::CornerValues,
    ) {
        let k = 0.5522847498;

        let tl = r.top_left.min(w / 2.0).min(h / 2.0);
        let tr = r.top_right.min(w / 2.0).min(h / 2.0);
        let br = r.bottom_right.min(w / 2.0).min(h / 2.0);
        let bl = r.bottom_left.min(w / 2.0).min(h / 2.0);

        let _ = write!(stream, "{:.2} {:.2} m\n", x + bl, y);

        let _ = write!(stream, "{:.2} {:.2} l\n", x + w - br, y);
        if br > 0.0 {
            let _ = write!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                x + w - br + br * k, y,
                x + w, y + br - br * k,
                x + w, y + br
            );
        }

        let _ = write!(stream, "{:.2} {:.2} l\n", x + w, y + h - tr);
        if tr > 0.0 {
            let _ = write!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                x + w, y + h - tr + tr * k,
                x + w - tr + tr * k, y + h,
                x + w - tr, y + h
            );
        }

        let _ = write!(stream, "{:.2} {:.2} l\n", x + tl, y + h);
        if tl > 0.0 {
            let _ = write!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                x + tl - tl * k, y + h,
                x, y + h - tl + tl * k,
                x, y + h - tl
            );
        }

        let _ = write!(stream, "{:.2} {:.2} l\n", x, y + bl);
        if bl > 0.0 {
            let _ = write!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                x, y + bl - bl * k,
                x + bl - bl * k, y,
                x + bl, y
            );
        }

        let _ = write!(stream, "h\n");
    }

    fn write_border_sides(
        &self,
        stream: &mut String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        bw: &Edges,
        bc: &crate::style::EdgeValues<Color>,
    ) {
        if bw.top > 0.0 {
            let _ = write!(
                stream,
                "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                bc.top.r, bc.top.g, bc.top.b, bw.top,
                x, y + h, x + w, y + h
            );
        }
        if bw.bottom > 0.0 {
            let _ = write!(
                stream,
                "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                bc.bottom.r, bc.bottom.g, bc.bottom.b, bw.bottom,
                x, y, x + w, y
            );
        }
        if bw.left > 0.0 {
            let _ = write!(
                stream,
                "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                bc.left.r, bc.left.g, bc.left.b, bw.left,
                x, y, x, y + h
            );
        }
        if bw.right > 0.0 {
            let _ = write!(
                stream,
                "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                bc.right.r, bc.right.g, bc.right.b, bw.right,
                x + w, y, x + w, y + h
            );
        }
    }

    /// Register fonts used across all pages — each unique (family, weight, italic)
    /// combination gets its own PDF font object.
    fn register_fonts(
        &self,
        builder: &mut PdfBuilder,
        pages: &[LayoutPage],
        font_context: &FontContext,
    ) {
        let mut keys: Vec<FontKey> = Vec::new();

        for page in pages {
            self.collect_font_keys_from_elements(&page.elements, &mut keys);
        }

        // Sort for deterministic ordering, then dedup
        keys.sort_by(|a, b| {
            a.family.cmp(&b.family)
                .then(a.weight.cmp(&b.weight))
                .then(a.italic.cmp(&b.italic))
        });
        keys.dedup();

        // Always have at least Helvetica
        if keys.is_empty() {
            keys.push(FontKey {
                family: "Helvetica".to_string(),
                weight: 400,
                italic: false,
            });
        }

        for key in &keys {
            let font_data = font_context.resolve(&key.family, key.weight, key.italic);
            let obj_id = builder.objects.len();

            let font_dict = match font_data {
                FontData::Standard(std_font) => {
                    format!(
                        "<< /Type /Font /Subtype /Type1 /BaseFont /{} \
                         /Encoding /WinAnsiEncoding >>",
                        std_font.pdf_name()
                    )
                }
                FontData::Custom { .. } => {
                    // TODO: TrueType font embedding with subsetting
                    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
                     /Encoding /WinAnsiEncoding >>"
                        .to_string()
                }
            };

            builder.objects.push(PdfObject {
                id: obj_id,
                data: font_dict.into_bytes(),
            });
            builder.font_objects.push((key.clone(), obj_id));
        }
    }

    /// Collect unique FontKey tuples from layout elements.
    fn collect_font_keys_from_elements(&self, elements: &[LayoutElement], keys: &mut Vec<FontKey>) {
        for element in elements {
            if let DrawCommand::Text { lines, .. } = &element.draw {
                for line in lines {
                    for glyph in &line.glyphs {
                        let italic = matches!(glyph.font_style, FontStyle::Italic | FontStyle::Oblique);
                        let key = FontKey {
                            family: glyph.font_family.clone(),
                            weight: if glyph.font_weight >= 600 { 700 } else { 400 },
                            italic,
                        };
                        if !keys.contains(&key) {
                            keys.push(key);
                        }
                    }
                }
            }
            self.collect_font_keys_from_elements(&element.children, keys);
        }
    }

    fn build_font_resource_dict(&self, font_objects: &[(FontKey, usize)]) -> String {
        font_objects
            .iter()
            .enumerate()
            .map(|(i, (_, obj_id))| format!("/F{} {} 0 R", i, obj_id))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Look up the font index (/F0, /F1, etc.) for a given family+weight+style.
    fn font_index(
        &self,
        family: &str,
        weight: u32,
        font_style: FontStyle,
        font_objects: &[(FontKey, usize)],
    ) -> usize {
        let italic = matches!(font_style, FontStyle::Italic | FontStyle::Oblique);
        let snapped_weight = if weight >= 600 { 700 } else { 400 };

        // Exact match
        for (i, (key, _)) in font_objects.iter().enumerate() {
            if key.family == family && key.weight == snapped_weight && key.italic == italic {
                return i;
            }
        }

        // Fallback: try Helvetica with same weight/style
        for (i, (key, _)) in font_objects.iter().enumerate() {
            if key.family == "Helvetica" && key.weight == snapped_weight && key.italic == italic {
                return i;
            }
        }

        // Last resort: first font
        0
    }

    /// Escape special characters in a PDF string.
    fn escape_pdf_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    }

    /// Serialize all objects into the final PDF byte stream.
    fn serialize(&self, builder: &PdfBuilder, info_obj_id: Option<usize>) -> Vec<u8> {
        let mut output: Vec<u8> = Vec::new();
        let mut offsets: Vec<usize> = vec![0; builder.objects.len()];

        // Header
        output.extend_from_slice(b"%PDF-1.7\n");
        output.extend_from_slice(b"%\xe2\xe3\xcf\xd3\n");

        for (i, obj) in builder.objects.iter().enumerate().skip(1) {
            offsets[i] = output.len();
            let header = format!("{} 0 obj\n", i);
            output.extend_from_slice(header.as_bytes());
            output.extend_from_slice(&obj.data);
            output.extend_from_slice(b"\nendobj\n\n");
        }

        let xref_offset = output.len();
        let _ = write!(output, "xref\n0 {}\n", builder.objects.len());
        let _ = write!(output, "0000000000 65535 f \n");
        for i in 1..builder.objects.len() {
            let _ = write!(output, "{:010} 00000 n \n", offsets[i]);
        }

        let _ = write!(output, "trailer\n<< /Size {} /Root 1 0 R", builder.objects.len());
        if let Some(info_id) = info_obj_id {
            let _ = write!(output, " /Info {} 0 R", info_id);
        }
        let _ = write!(output, " >>\nstartxref\n{}\n%%EOF\n", xref_offset);

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontContext;

    #[test]
    fn test_escape_pdf_string() {
        assert_eq!(
            PdfWriter::escape_pdf_string("Hello (World)"),
            "Hello \\(World\\)"
        );
        assert_eq!(
            PdfWriter::escape_pdf_string("back\\slash"),
            "back\\\\slash"
        );
    }

    #[test]
    fn test_empty_document_produces_valid_pdf() {
        let writer = PdfWriter::new();
        let font_context = FontContext::new();
        let pages = vec![LayoutPage {
            width: 595.28,
            height: 841.89,
            elements: vec![],
        }];
        let metadata = Metadata::default();
        let bytes = writer.write(&pages, &metadata, &font_context);

        assert!(bytes.starts_with(b"%PDF-1.7"));
        assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
        assert!(bytes.windows(4).any(|w| w == b"xref"));
        assert!(bytes.windows(7).any(|w| w == b"trailer"));
    }

    #[test]
    fn test_metadata_in_pdf() {
        let writer = PdfWriter::new();
        let font_context = FontContext::new();
        let pages = vec![LayoutPage {
            width: 595.28,
            height: 841.89,
            elements: vec![],
        }];
        let metadata = Metadata {
            title: Some("Test Document".to_string()),
            author: Some("Forme".to_string()),
            subject: None,
            creator: None,
        };
        let bytes = writer.write(&pages, &metadata, &font_context);
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("/Title (Test Document)"));
        assert!(text.contains("/Author (Forme)"));
    }

    #[test]
    fn test_bold_font_registered_separately() {
        let writer = PdfWriter::new();
        let font_context = FontContext::new();

        // Create pages with both regular and bold text
        let pages = vec![LayoutPage {
            width: 595.28,
            height: 841.89,
            elements: vec![
                LayoutElement {
                    x: 54.0, y: 54.0, width: 100.0, height: 16.8,
                    draw: DrawCommand::Text {
                        lines: vec![TextLine {
                            x: 54.0, y: 66.0, width: 50.0, height: 16.8,
                            glyphs: vec![PositionedGlyph {
                                glyph_id: 65, x_offset: 0.0, font_size: 12.0,
                                font_family: "Helvetica".to_string(),
                                font_weight: 400, font_style: FontStyle::Normal,
                                char_value: 'A',
                            }],
                        }],
                        color: Color::BLACK,
                    },
                    children: vec![],
                },
                LayoutElement {
                    x: 54.0, y: 74.0, width: 100.0, height: 16.8,
                    draw: DrawCommand::Text {
                        lines: vec![TextLine {
                            x: 54.0, y: 86.0, width: 50.0, height: 16.8,
                            glyphs: vec![PositionedGlyph {
                                glyph_id: 65, x_offset: 0.0, font_size: 12.0,
                                font_family: "Helvetica".to_string(),
                                font_weight: 700, font_style: FontStyle::Normal,
                                char_value: 'A',
                            }],
                        }],
                        color: Color::BLACK,
                    },
                    children: vec![],
                },
            ],
        }];

        let metadata = Metadata::default();
        let bytes = writer.write(&pages, &metadata, &font_context);
        let text = String::from_utf8_lossy(&bytes);

        // Should have both Helvetica and Helvetica-Bold registered
        assert!(text.contains("Helvetica"), "Should contain regular Helvetica");
        assert!(text.contains("Helvetica-Bold"), "Should contain Helvetica-Bold");
    }
}
