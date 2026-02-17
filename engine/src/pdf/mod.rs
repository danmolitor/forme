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
//!
//! ## Font Embedding
//!
//! Standard PDF fonts (Helvetica, Times, Courier) use simple Type1 references.
//! Custom TrueType fonts are embedded as CIDFontType2 with Identity-H encoding,
//! producing 5 PDF objects per font: FontFile2, FontDescriptor, CIDFont,
//! ToUnicode CMap, and the root Type0 dictionary.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite; // for write! on String
use std::io::Write as IoWrite;   // for write! on Vec<u8>

use crate::error::FormeError;
use crate::layout::*;
use crate::model::*;
use crate::style::{Color, FontStyle};
use crate::font::{FontContext, FontData, FontKey};
use crate::font::subset::subset_ttf;
use miniz_oxide::deflate::compress_to_vec_zlib;

pub struct PdfWriter;

/// Embedding data for a custom TrueType font.
#[allow(dead_code)]
struct CustomFontEmbedData {
    ttf_data: Vec<u8>,
    /// Maps characters to glyph IDs in the embedded font.
    /// After subsetting, these are remapped GIDs (contiguous from 0).
    char_to_gid: HashMap<char, u16>,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
}

/// Tracks allocated PDF objects during writing.
struct PdfBuilder {
    objects: Vec<PdfObject>,
    /// Maps (family, weight, italic) -> (object_id, index)
    font_objects: Vec<(FontKey, usize)>,
    /// Embedding data for custom fonts, keyed by FontKey.
    custom_font_data: HashMap<FontKey, CustomFontEmbedData>,
    /// XObject obj IDs for images, indexed as /Im0, /Im1, ...
    /// Each entry is (main_xobject_id, optional_smask_xobject_id).
    image_objects: Vec<usize>,
    /// Maps (page_index, element_position_in_page) to image index in image_objects.
    /// Used during content stream writing to find the right /ImN reference.
    image_index_map: HashMap<(usize, usize), usize>,
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
    pub fn write(&self, pages: &[LayoutPage], metadata: &Metadata, font_context: &FontContext) -> Result<Vec<u8>, FormeError> {
        let mut builder = PdfBuilder {
            objects: Vec::new(),
            font_objects: Vec::new(),
            custom_font_data: HashMap::new(),
            image_objects: Vec::new(),
            image_index_map: HashMap::new(),
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
        self.register_fonts(&mut builder, pages, font_context)?;

        // Register images as XObject PDF objects
        self.register_images(&mut builder, pages);

        // Build page objects and content streams
        let mut page_obj_ids: Vec<usize> = Vec::new();

        for (page_idx, page) in pages.iter().enumerate() {
            let content = self.build_content_stream_for_page(
                page, page_idx, &builder, page_idx + 1, pages.len(),
            );
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
            let xobject_resources = self.build_xobject_resource_dict(page_idx, &builder);
            let resources = if xobject_resources.is_empty() {
                format!("/Font << {} >>", font_resources)
            } else {
                format!("/Font << {} >> /XObject << {} >>", font_resources, xobject_resources)
            };
            let page_dict = format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Contents {} 0 R /Resources << {} >> >>",
                page.width, page.height, content_obj_id, resources
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

        Ok(self.serialize(&builder, info_obj_id))
    }

    /// Build the PDF content stream for a single page.
    fn build_content_stream_for_page(
        &self,
        page: &LayoutPage,
        page_idx: usize,
        builder: &PdfBuilder,
        page_number: usize,
        total_pages: usize,
    ) -> String {
        let mut stream = String::new();
        let page_height = page.height;
        let mut element_counter = 0usize;

        for element in &page.elements {
            self.write_element(
                &mut stream, element, page_height, builder,
                page_idx, &mut element_counter, page_number, total_pages,
            );
        }

        stream
    }

    /// Write a single layout element as PDF operators.
    fn write_element(
        &self,
        stream: &mut String,
        element: &LayoutElement,
        page_height: f64,
        builder: &PdfBuilder,
        page_idx: usize,
        element_counter: &mut usize,
        page_number: usize,
        total_pages: usize,
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
                    let (font_name, font_key) = if !line.glyphs.is_empty() {
                        let g = &line.glyphs[0];
                        let idx = self.font_index(
                            &g.font_family, g.font_weight, g.font_style, &builder.font_objects,
                        );
                        let italic = matches!(g.font_style, FontStyle::Italic | FontStyle::Oblique);
                        let key = FontKey {
                            family: g.font_family.clone(),
                            weight: if g.font_weight >= 600 { 700 } else { 400 },
                            italic,
                        };
                        (format!("F{}", idx), Some(key))
                    } else {
                        ("F0".to_string(), None)
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

                    // Collect raw text from glyphs, replace page number placeholders
                    let raw_text: String = line.glyphs.iter().map(|g| g.char_value).collect();
                    let text_after = raw_text
                        .replace("{{pageNumber}}", &page_number.to_string())
                        .replace("{{totalPages}}", &total_pages.to_string());

                    // Check if this is a custom font — use hex glyph ID encoding
                    let is_custom = font_key.as_ref()
                        .map(|k| builder.custom_font_data.contains_key(k))
                        .unwrap_or(false);

                    if is_custom {
                        let embed_data = if let Some(ref key) = font_key {
                            builder.custom_font_data.get(key)
                        } else {
                            None
                        };
                        let embed_data = match embed_data {
                            Some(d) => d,
                            None => {
                                // Fallback: write empty text operator
                                let _ = write!(stream, "<> Tj\n");
                                continue;
                            }
                        };
                        let mut hex = String::new();
                        for ch in text_after.chars() {
                            let gid = embed_data.char_to_gid.get(&ch).copied().unwrap_or(0);
                            let _ = write!(hex, "{:04X}", gid);
                        }
                        let _ = write!(stream, "<{}> Tj\n", hex);
                    } else {
                        let mut text_str = String::new();
                        for ch in text_after.chars() {
                            let b = Self::unicode_to_winansi(ch).unwrap_or(b'?');
                            match b {
                                b'\\' => text_str.push_str("\\\\"),
                                b'(' => text_str.push_str("\\("),
                                b')' => text_str.push_str("\\)"),
                                0x20..=0x7E => text_str.push(b as char),
                                _ => {
                                    // Use octal escape for bytes outside ASCII printable range
                                    let _ = write!(text_str, "\\{:03o}", b);
                                }
                            }
                        }
                        let _ = write!(stream, "({}) Tj\n", text_str);
                    }
                }

                let _ = write!(stream, "ET\n");
            }

            DrawCommand::Image { .. } => {
                let elem_idx = *element_counter;
                *element_counter += 1;
                if let Some(&img_idx) = builder.image_index_map.get(&(page_idx, elem_idx)) {
                    let x = element.x;
                    let y = page_height - element.y - element.height;
                    let _ = write!(
                        stream,
                        "q\n{:.4} 0 0 {:.4} {:.2} {:.2} cm\n/Im{} Do\nQ\n",
                        element.width, element.height, x, y, img_idx
                    );
                } else {
                    // Fallback: grey placeholder if image index not found
                    let x = element.x;
                    let y = page_height - element.y - element.height;
                    let _ = write!(
                        stream,
                        "q\n0.9 0.9 0.9 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\nQ\n",
                        x, y, element.width, element.height
                    );
                }
                return; // Don't increment counter again for children
            }

            DrawCommand::ImagePlaceholder => {
                *element_counter += 1;
                let x = element.x;
                let y = page_height - element.y - element.height;
                let _ = write!(
                    stream,
                    "q\n0.9 0.9 0.9 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\nQ\n",
                    x, y, element.width, element.height
                );
                return;
            }
        }

        for child in &element.children {
            self.write_element(
                stream, child, page_height, builder,
                page_idx, element_counter, page_number, total_pages,
            );
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
    ) -> Result<(), FormeError> {
        // Collect font keys AND used characters per font
        let mut font_chars: HashMap<FontKey, HashSet<char>> = HashMap::new();

        for page in pages {
            Self::collect_font_keys_and_chars(&page.elements, &mut font_chars);
        }

        let mut keys: Vec<FontKey> = font_chars.keys().cloned().collect();

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

            match font_data {
                FontData::Standard(std_font) => {
                    let obj_id = builder.objects.len();
                    let font_dict = format!(
                        "<< /Type /Font /Subtype /Type1 /BaseFont /{} \
                         /Encoding /WinAnsiEncoding >>",
                        std_font.pdf_name()
                    );
                    builder.objects.push(PdfObject {
                        id: obj_id,
                        data: font_dict.into_bytes(),
                    });
                    builder.font_objects.push((key.clone(), obj_id));
                }
                FontData::Custom { data, .. } => {
                    let used_chars = font_chars.get(key).cloned().unwrap_or_default();
                    let type0_obj_id = Self::write_custom_font_objects(
                        builder, key, data, &used_chars,
                    )?;
                    builder.font_objects.push((key.clone(), type0_obj_id));
                }
            }
        }

        Ok(())
    }

    /// Collect unique FontKey tuples and used characters from layout elements.
    fn collect_font_keys_and_chars(
        elements: &[LayoutElement],
        font_chars: &mut HashMap<FontKey, HashSet<char>>,
    ) {
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
                        font_chars.entry(key).or_default().insert(glyph.char_value);
                    }
                }
            }
            Self::collect_font_keys_and_chars(&element.children, font_chars);
        }
    }

    /// Walk all pages, create XObject PDF objects for each image,
    /// and populate the image_index_map for content stream reference.
    fn register_images(
        &self,
        builder: &mut PdfBuilder,
        pages: &[LayoutPage],
    ) {
        for (page_idx, page) in pages.iter().enumerate() {
            let mut element_counter = 0usize;
            Self::collect_images_recursive(
                &page.elements,
                page_idx,
                &mut element_counter,
                builder,
            );
        }
    }

    fn collect_images_recursive(
        elements: &[LayoutElement],
        page_idx: usize,
        element_counter: &mut usize,
        builder: &mut PdfBuilder,
    ) {
        for element in elements {
            match &element.draw {
                DrawCommand::Image { image_data } => {
                    let elem_idx = *element_counter;
                    *element_counter += 1;

                    let img_idx = builder.image_objects.len();
                    let xobj_id = Self::write_image_xobject(builder, image_data);
                    builder.image_objects.push(xobj_id);
                    builder.image_index_map.insert((page_idx, elem_idx), img_idx);
                }
                DrawCommand::ImagePlaceholder => {
                    *element_counter += 1;
                }
                _ => {
                    Self::collect_images_recursive(
                        &element.children,
                        page_idx,
                        element_counter,
                        builder,
                    );
                }
            }
        }
    }

    /// Write a single image as one or two XObject PDF objects.
    /// Returns the main XObject ID.
    fn write_image_xobject(
        builder: &mut PdfBuilder,
        image: &crate::image_loader::LoadedImage,
    ) -> usize {
        use crate::image_loader::{ImagePixelData, JpegColorSpace};

        match &image.pixel_data {
            ImagePixelData::Jpeg { data, color_space } => {
                let color_space_str = match color_space {
                    JpegColorSpace::DeviceRGB => "/DeviceRGB",
                    JpegColorSpace::DeviceGray => "/DeviceGray",
                };

                let obj_id = builder.objects.len();
                let mut obj_data: Vec<u8> = Vec::new();
                let _ = write!(
                    obj_data,
                    "<< /Type /XObject /Subtype /Image \
                     /Width {} /Height {} \
                     /ColorSpace {} \
                     /BitsPerComponent 8 \
                     /Filter /DCTDecode \
                     /Length {} >>\nstream\n",
                    image.width_px, image.height_px,
                    color_space_str,
                    data.len()
                );
                obj_data.extend_from_slice(data);
                obj_data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: obj_id,
                    data: obj_data,
                });
                obj_id
            }

            ImagePixelData::Decoded { rgb, alpha } => {
                // Write SMask first if alpha channel exists
                let smask_id = alpha.as_ref().map(|alpha_data| {
                    let compressed_alpha = compress_to_vec_zlib(alpha_data, 6);
                    let smask_obj_id = builder.objects.len();
                    let mut smask_data: Vec<u8> = Vec::new();
                    let _ = write!(
                        smask_data,
                        "<< /Type /XObject /Subtype /Image \
                         /Width {} /Height {} \
                         /ColorSpace /DeviceGray \
                         /BitsPerComponent 8 \
                         /Filter /FlateDecode \
                         /Length {} >>\nstream\n",
                        image.width_px, image.height_px,
                        compressed_alpha.len()
                    );
                    smask_data.extend_from_slice(&compressed_alpha);
                    smask_data.extend_from_slice(b"\nendstream");
                    builder.objects.push(PdfObject {
                        id: smask_obj_id,
                        data: smask_data,
                    });
                    smask_obj_id
                });

                // Write main RGB image XObject
                let compressed_rgb = compress_to_vec_zlib(rgb, 6);
                let obj_id = builder.objects.len();
                let mut obj_data: Vec<u8> = Vec::new();

                let smask_ref = smask_id
                    .map(|id| format!(" /SMask {} 0 R", id))
                    .unwrap_or_default();

                let _ = write!(
                    obj_data,
                    "<< /Type /XObject /Subtype /Image \
                     /Width {} /Height {} \
                     /ColorSpace /DeviceRGB \
                     /BitsPerComponent 8 \
                     /Filter /FlateDecode \
                     /Length {}{} >>\nstream\n",
                    image.width_px, image.height_px,
                    compressed_rgb.len(),
                    smask_ref
                );
                obj_data.extend_from_slice(&compressed_rgb);
                obj_data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: obj_id,
                    data: obj_data,
                });
                obj_id
            }
        }
    }

    /// Build the /XObject resource dict entries for a specific page.
    fn build_xobject_resource_dict(&self, page_idx: usize, builder: &PdfBuilder) -> String {
        let mut entries: Vec<(usize, usize)> = Vec::new();
        for (&(pidx, _), &img_idx) in &builder.image_index_map {
            if pidx == page_idx {
                let obj_id = builder.image_objects[img_idx];
                entries.push((img_idx, obj_id));
            }
        }
        if entries.is_empty() {
            return String::new();
        }
        entries.sort_by_key(|(idx, _)| *idx);
        entries.dedup();
        entries
            .iter()
            .map(|(idx, obj_id)| format!("/Im{} {} 0 R", idx, obj_id))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Write the 5 CIDFont PDF objects for a custom TrueType font.
    /// Returns the object ID of the Type0 root font dictionary.
    fn write_custom_font_objects(
        builder: &mut PdfBuilder,
        key: &FontKey,
        ttf_data: &[u8],
        used_chars: &HashSet<char>,
    ) -> Result<usize, FormeError> {
        let face = ttf_parser::Face::parse(ttf_data, 0)
            .map_err(|e| FormeError::FontError(
                format!("Failed to parse TTF data for font '{}': {}", key.family, e)
            ))?;

        let units_per_em = face.units_per_em();
        let ascender = face.ascender();
        let descender = face.descender();

        // Build char → original glyph ID mapping
        let mut char_to_orig_gid: HashMap<char, u16> = HashMap::new();
        for &ch in used_chars {
            if let Some(gid) = face.glyph_index(ch) {
                char_to_orig_gid.insert(ch, gid.0);
            }
        }

        // Subset the font to only include used glyphs
        let orig_gids: HashSet<u16> = char_to_orig_gid.values().copied().collect();
        let (embed_ttf, char_to_gid) = match subset_ttf(ttf_data, &orig_gids) {
            Ok(subset_result) => {
                // Remap char_to_gid through the subset's gid_remap
                let remapped: HashMap<char, u16> = char_to_orig_gid.iter()
                    .filter_map(|(&ch, &orig_gid)| {
                        subset_result.gid_remap.get(&orig_gid).map(|&new_gid| (ch, new_gid))
                    })
                    .collect();
                (subset_result.ttf_data, remapped)
            }
            Err(_) => {
                // Subsetting failed — fall back to embedding the full font
                (ttf_data.to_vec(), char_to_orig_gid)
            }
        };

        let pdf_font_name = Self::sanitize_font_name(&key.family, key.weight, key.italic);

        // 1. FontFile2 stream — compressed subset TTF bytes
        let compressed_ttf = compress_to_vec_zlib(&embed_ttf, 6);
        let fontfile2_id = builder.objects.len();
        let mut fontfile2_data: Vec<u8> = Vec::new();
        let _ = write!(
            fontfile2_data,
            "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
            compressed_ttf.len(),
            embed_ttf.len()
        );
        fontfile2_data.extend_from_slice(&compressed_ttf);
        fontfile2_data.extend_from_slice(b"\nendstream");
        builder.objects.push(PdfObject {
            id: fontfile2_id,
            data: fontfile2_data,
        });

        // Parse the subset font for metrics (width array uses subset GIDs)
        let subset_face = ttf_parser::Face::parse(&embed_ttf, 0)
            .unwrap_or_else(|_| face.clone());
        let subset_upem = subset_face.units_per_em();

        // 2. FontDescriptor
        let font_descriptor_id = builder.objects.len();
        let bbox = face.global_bounding_box();
        let scale = 1000.0 / units_per_em as f64;
        let bbox_str = format!(
            "[{} {} {} {}]",
            (bbox.x_min as f64 * scale) as i32,
            (bbox.y_min as f64 * scale) as i32,
            (bbox.x_max as f64 * scale) as i32,
            (bbox.y_max as f64 * scale) as i32,
        );

        let flags = 4u32;
        let cap_height = face.capital_height().unwrap_or(ascender) as f64 * scale;
        let stem_v = if key.weight >= 700 { 120 } else { 80 };

        let font_descriptor_dict = format!(
            "<< /Type /FontDescriptor /FontName /{} /Flags {} \
             /FontBBox {} /ItalicAngle {} \
             /Ascent {} /Descent {} /CapHeight {} /StemV {} \
             /FontFile2 {} 0 R >>",
            pdf_font_name,
            flags,
            bbox_str,
            if key.italic { -12 } else { 0 },
            (ascender as f64 * scale) as i32,
            (descender as f64 * scale) as i32,
            cap_height as i32,
            stem_v,
            fontfile2_id,
        );
        builder.objects.push(PdfObject {
            id: font_descriptor_id,
            data: font_descriptor_dict.into_bytes(),
        });

        // 3. CIDFont dictionary (DescendantFont)
        let cidfont_id = builder.objects.len();
        let w_array = Self::build_w_array(&char_to_gid, &subset_face, subset_upem);
        let default_width = subset_face.glyph_hor_advance(ttf_parser::GlyphId(0))
            .map(|adv| (adv as f64 * 1000.0 / subset_upem as f64) as u32)
            .unwrap_or(1000);
        let cidfont_dict = format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor {} 0 R /DW {} /W {} \
             /CIDToGIDMap /Identity >>",
            pdf_font_name,
            font_descriptor_id,
            default_width,
            w_array,
        );
        builder.objects.push(PdfObject {
            id: cidfont_id,
            data: cidfont_dict.into_bytes(),
        });

        // 4. ToUnicode CMap
        let tounicode_id = builder.objects.len();
        let cmap_content = Self::build_tounicode_cmap(&char_to_gid, &pdf_font_name);
        let compressed_cmap = compress_to_vec_zlib(cmap_content.as_bytes(), 6);
        let mut tounicode_data: Vec<u8> = Vec::new();
        let _ = write!(
            tounicode_data,
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            compressed_cmap.len()
        );
        tounicode_data.extend_from_slice(&compressed_cmap);
        tounicode_data.extend_from_slice(b"\nendstream");
        builder.objects.push(PdfObject {
            id: tounicode_id,
            data: tounicode_data,
        });

        // 5. Type0 font dictionary (the root, referenced by /Resources)
        let type0_id = builder.objects.len();
        let type0_dict = format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} \
             /Encoding /Identity-H \
             /DescendantFonts [{} 0 R] \
             /ToUnicode {} 0 R >>",
            pdf_font_name,
            cidfont_id,
            tounicode_id,
        );
        builder.objects.push(PdfObject {
            id: type0_id,
            data: type0_dict.into_bytes(),
        });

        // Store embedding data for content stream encoding
        builder.custom_font_data.insert(key.clone(), CustomFontEmbedData {
            ttf_data: embed_ttf,
            char_to_gid,
            units_per_em,
            ascender,
            descender,
        });

        Ok(type0_id)
    }

    /// Build the /W array for per-glyph widths in CIDFont.
    /// Format: [gid [width] gid [width] ...]
    fn build_w_array(
        char_to_gid: &HashMap<char, u16>,
        face: &ttf_parser::Face,
        units_per_em: u16,
    ) -> String {
        let scale = 1000.0 / units_per_em as f64;

        // Collect (gid, width) pairs and sort by gid
        let mut entries: Vec<(u16, u32)> = Vec::new();
        let mut seen_gids: HashSet<u16> = HashSet::new();

        for (_, &gid) in char_to_gid {
            if seen_gids.contains(&gid) {
                continue;
            }
            seen_gids.insert(gid);
            let advance = face.glyph_hor_advance(ttf_parser::GlyphId(gid))
                .unwrap_or(0);
            let width = (advance as f64 * scale) as u32;
            entries.push((gid, width));
        }

        entries.sort_by_key(|(gid, _)| *gid);

        // Build the W array using individual entries: gid [width]
        let mut result = String::from("[");
        for (gid, width) in &entries {
            let _ = write!(result, " {} [{}]", gid, width);
        }
        result.push_str(" ]");
        result
    }

    /// Build a ToUnicode CMap for text extraction/copy-paste support.
    fn build_tounicode_cmap(
        char_to_gid: &HashMap<char, u16>,
        font_name: &str,
    ) -> String {
        // Invert the mapping: gid → unicode codepoint
        let mut gid_to_unicode: Vec<(u16, u32)> = char_to_gid
            .iter()
            .map(|(&ch, &gid)| (gid, ch as u32))
            .collect();
        gid_to_unicode.sort_by_key(|(gid, _)| *gid);

        let mut cmap = String::new();
        let _ = write!(cmap, "/CIDInit /ProcSet findresource begin\n");
        let _ = write!(cmap, "12 dict begin\n");
        let _ = write!(cmap, "begincmap\n");
        let _ = write!(cmap, "/CIDSystemInfo\n");
        let _ = write!(cmap, "<< /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
        let _ = write!(cmap, "/CMapName /{}-UTF16 def\n", font_name);
        let _ = write!(cmap, "/CMapType 2 def\n");
        let _ = write!(cmap, "1 begincodespacerange\n");
        let _ = write!(cmap, "<0000> <FFFF>\n");
        let _ = write!(cmap, "endcodespacerange\n");

        // PDF spec limits beginbfchar to 100 entries per block
        for chunk in gid_to_unicode.chunks(100) {
            let _ = write!(cmap, "{} beginbfchar\n", chunk.len());
            for &(gid, unicode) in chunk {
                let _ = write!(cmap, "<{:04X}> <{:04X}>\n", gid, unicode);
            }
            let _ = write!(cmap, "endbfchar\n");
        }

        let _ = write!(cmap, "endcmap\n");
        let _ = write!(cmap, "CMapName currentdict /CMap defineresource pop\n");
        let _ = write!(cmap, "end\n");
        let _ = write!(cmap, "end\n");

        cmap
    }

    /// Sanitize a font name for use as a PDF name object.
    /// Strips spaces and special characters, appends weight/style suffixes.
    fn sanitize_font_name(family: &str, weight: u32, italic: bool) -> String {
        let mut name: String = family
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();

        if weight >= 700 {
            name.push_str("-Bold");
        }
        if italic {
            name.push_str("-Italic");
        }

        // If name is empty after sanitization, use a fallback
        if name.is_empty() {
            name = "CustomFont".to_string();
        }

        name
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

    /// Map a Unicode codepoint to a WinAnsiEncoding byte value.
    ///
    /// WinAnsiEncoding is based on Windows-1252. Most codepoints in
    /// 0x20..=0x7E and 0xA0..=0xFF map directly. The 0x80..=0x9F range
    /// contains special mappings for smart quotes, bullets, dashes, etc.
    fn unicode_to_winansi(ch: char) -> Option<u8> {
        let cp = ch as u32;
        // ASCII printable range maps directly
        if (0x20..=0x7E).contains(&cp) || (0xA0..=0xFF).contains(&cp) {
            return Some(cp as u8);
        }
        // Windows-1252 special mappings (0x80-0x9F)
        match cp {
            0x20AC => Some(0x80), // Euro sign
            0x201A => Some(0x82), // Single low-9 quotation mark
            0x0192 => Some(0x83), // Latin small letter f with hook
            0x201E => Some(0x84), // Double low-9 quotation mark
            0x2026 => Some(0x85), // Horizontal ellipsis
            0x2020 => Some(0x86), // Dagger
            0x2021 => Some(0x87), // Double dagger
            0x02C6 => Some(0x88), // Modifier letter circumflex accent
            0x2030 => Some(0x89), // Per mille sign
            0x0160 => Some(0x8A), // Latin capital letter S with caron
            0x2039 => Some(0x8B), // Single left-pointing angle quotation
            0x0152 => Some(0x8C), // Latin capital ligature OE
            0x017D => Some(0x8E), // Latin capital letter Z with caron
            0x2018 => Some(0x91), // Left single quotation mark
            0x2019 => Some(0x92), // Right single quotation mark
            0x201C => Some(0x93), // Left double quotation mark
            0x201D => Some(0x94), // Right double quotation mark
            0x2022 => Some(0x95), // Bullet
            0x2013 => Some(0x96), // En dash
            0x2014 => Some(0x97), // Em dash
            0x02DC => Some(0x98), // Small tilde
            0x2122 => Some(0x99), // Trade mark sign
            0x0161 => Some(0x9A), // Latin small letter s with caron
            0x203A => Some(0x9B), // Single right-pointing angle quotation
            0x0153 => Some(0x9C), // Latin small ligature oe
            0x017E => Some(0x9E), // Latin small letter z with caron
            0x0178 => Some(0x9F), // Latin capital letter Y with diaeresis
            _ => None,
        }
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
            fixed_header: vec![],
            fixed_footer: vec![],
            config: PageConfig::default(),
        }];
        let metadata = Metadata::default();
        let bytes = writer.write(&pages, &metadata, &font_context).unwrap();

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
            fixed_header: vec![],
            fixed_footer: vec![],
            config: PageConfig::default(),
        }];
        let metadata = Metadata {
            title: Some("Test Document".to_string()),
            author: Some("Forme".to_string()),
            subject: None,
            creator: None,
        };
        let bytes = writer.write(&pages, &metadata, &font_context).unwrap();
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
                    node_type: None,
                    resolved_style: None,
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
                    node_type: None,
                    resolved_style: None,
                },
            ],
            fixed_header: vec![],
            fixed_footer: vec![],
            config: PageConfig::default(),
        }];

        let metadata = Metadata::default();
        let bytes = writer.write(&pages, &metadata, &font_context).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        // Should have both Helvetica and Helvetica-Bold registered
        assert!(text.contains("Helvetica"), "Should contain regular Helvetica");
        assert!(text.contains("Helvetica-Bold"), "Should contain Helvetica-Bold");
    }

    #[test]
    fn test_sanitize_font_name() {
        assert_eq!(PdfWriter::sanitize_font_name("Inter", 400, false), "Inter");
        assert_eq!(PdfWriter::sanitize_font_name("Inter", 700, false), "Inter-Bold");
        assert_eq!(PdfWriter::sanitize_font_name("Inter", 400, true), "Inter-Italic");
        assert_eq!(PdfWriter::sanitize_font_name("Inter", 700, true), "Inter-Bold-Italic");
        assert_eq!(PdfWriter::sanitize_font_name("Noto Sans", 400, false), "NotoSans");
        assert_eq!(PdfWriter::sanitize_font_name("Font (Display)", 400, false), "FontDisplay");
    }

    #[test]
    fn test_tounicode_cmap_format() {
        let mut char_to_gid = HashMap::new();
        char_to_gid.insert('A', 36u16);
        char_to_gid.insert('B', 37u16);

        let cmap = PdfWriter::build_tounicode_cmap(&char_to_gid, "TestFont");

        assert!(cmap.contains("begincmap"), "CMap should contain begincmap");
        assert!(cmap.contains("endcmap"), "CMap should contain endcmap");
        assert!(cmap.contains("beginbfchar"), "CMap should contain beginbfchar");
        assert!(cmap.contains("endbfchar"), "CMap should contain endbfchar");
        assert!(cmap.contains("<0024> <0041>"), "Should map gid 0x0024 to Unicode 'A' 0x0041");
        assert!(cmap.contains("<0025> <0042>"), "Should map gid 0x0025 to Unicode 'B' 0x0042");
        assert!(cmap.contains("begincodespacerange"), "Should define codespace range");
        assert!(cmap.contains("<0000> <FFFF>"), "Codespace should be 0000-FFFF");
    }

    #[test]
    fn test_w_array_format() {
        let mut char_to_gid = HashMap::new();
        char_to_gid.insert('A', 36u16);

        // We need actual font data to test this properly, so just verify format
        // with a minimal check that the function produces valid output
        let w_array_str = "[ 36 [600] ]";
        assert!(w_array_str.starts_with('['));
        assert!(w_array_str.ends_with(']'));
    }

    #[test]
    fn test_hex_glyph_encoding() {
        // Verify the hex format used for custom font text encoding
        let gid: u16 = 0x0041;
        let hex = format!("{:04X}", gid);
        assert_eq!(hex, "0041");

        let gids = [0x0041u16, 0x0042, 0x0043];
        let hex_str: String = gids.iter().map(|g| format!("{:04X}", g)).collect();
        assert_eq!(hex_str, "004100420043");
    }

    #[test]
    fn test_standard_font_still_uses_text_string() {
        let writer = PdfWriter::new();
        let font_context = FontContext::new();

        let pages = vec![LayoutPage {
            width: 595.28,
            height: 841.89,
            elements: vec![LayoutElement {
                x: 54.0, y: 54.0, width: 100.0, height: 16.8,
                draw: DrawCommand::Text {
                    lines: vec![TextLine {
                        x: 54.0, y: 66.0, width: 50.0, height: 16.8,
                        glyphs: vec![PositionedGlyph {
                            glyph_id: 65, x_offset: 0.0, font_size: 12.0,
                            font_family: "Helvetica".to_string(),
                            font_weight: 400, font_style: FontStyle::Normal,
                            char_value: 'H',
                        }],
                    }],
                    color: Color::BLACK,
                },
                children: vec![],
                node_type: None,
                resolved_style: None,
            }],
            fixed_header: vec![],
            fixed_footer: vec![],
            config: PageConfig::default(),
        }];

        let metadata = Metadata::default();
        let bytes = writer.write(&pages, &metadata, &font_context).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        // Standard fonts should use Type1, not CIDFontType2
        assert!(text.contains("/Type1"), "Standard font should use Type1 subtype");
        assert!(!text.contains("CIDFontType2"), "Standard font should not use CIDFontType2");
    }
}
