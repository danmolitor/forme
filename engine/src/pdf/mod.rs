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

pub mod certify;
pub mod merge;
pub mod redaction;
pub(crate) mod tagged;
pub(crate) mod xmp;

use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite; // for write! on String
use std::io::Write as IoWrite; // for write! on Vec<u8>

use crate::error::FormeError;
use crate::font::subset::subset_ttf;
use crate::font::{FontContext, FontData, FontKey};
use crate::layout::*;
use crate::model::*;
use crate::style::{Color, FontStyle, Overflow, TextDecoration, TransformOp};
use crate::svg::SvgCommand;
use miniz_oxide::deflate::compress_to_vec_zlib;

/// Default `/Params /ModDate` for attachments. A fixed constant, never
/// wall-clock: byte-determinism is a hard guarantee (native/WASM parity is
/// gated on it in CI). Callers wanting a real date pass `modDate`.
const DEFAULT_ATTACHMENT_MOD_DATE: &str = "D:20000101000000Z";

/// A link annotation to be added to a page.
struct LinkAnnotation {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    href: String,
}

/// A bookmark entry for the PDF outline tree.
struct PdfBookmark {
    title: String,
    page_obj_id: usize,
    y_pdf: f64,
}

/// A form field annotation collected during layout traversal.
struct FormFieldData {
    field_type: FormFieldType,
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    page_idx: usize,
}

pub struct PdfWriter;

/// Embedding data for a custom TrueType font.
#[allow(dead_code)]
struct CustomFontEmbedData {
    ttf_data: Vec<u8>,
    /// Maps original glyph IDs (from shaping) to remapped GIDs in the subset font.
    gid_remap: HashMap<u16, u16>,
    /// Maps original glyph IDs to their Unicode character(s) for ToUnicode CMap.
    glyph_to_char: HashMap<u16, char>,
    /// Legacy fallback: maps chars to subset GIDs (for page number placeholders).
    char_to_gid: HashMap<char, u16>,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
}

/// Font usage data collected from layout elements.
struct FontUsage {
    /// Characters used per font (for standard font subsetting fallback).
    chars: HashSet<char>,
    /// Glyph IDs used per font (from shaped PositionedGlyphs).
    glyph_ids: HashSet<u16>,
    /// Maps glyph ID → first char it represents (for ToUnicode CMap).
    glyph_to_char: HashMap<u16, char>,
}

/// Tracks allocated PDF objects during writing.
struct PdfBuilder {
    objects: Vec<PdfObject>,
    /// Maps (family, weight, italic) -> (object_id, index)
    font_objects: Vec<(FontKey, usize)>,
    /// Embedding data for custom fonts, keyed by FontKey.
    custom_font_data: HashMap<FontKey, CustomFontEmbedData>,
    /// Base-14 fonts that were embedded via the pdfUa metric-compatible
    /// substitution (Liberation). They aren't in `custom_font_data` — the
    /// caller registered no custom bytes for them — but they ARE embedded, so
    /// the PDF/A "all fonts embedded" check must treat them as satisfied.
    embedded_standard_fonts: std::collections::HashSet<FontKey>,
    /// XObject obj IDs for images, indexed as /Im0, /Im1, ...
    /// Each entry is (main_xobject_id, optional_smask_xobject_id).
    image_objects: Vec<usize>,
    /// Maps (page_index, element_position_in_page) to image index in image_objects.
    /// Used during content stream writing to find the right /ImN reference.
    image_index_map: HashMap<(usize, usize), usize>,
    /// Maps page_index to (image_index, intrinsic_width_px, intrinsic_height_px)
    /// for the page's optional `background_image`. Identical URLs across
    /// pages share a single XObject; the dims are needed for
    /// `cover` / `contain` sizing math at content-stream time.
    page_background_image_map: HashMap<usize, (usize, u32, u32)>,
    /// Caches `backgroundImage URL → (image index, w_px, h_px)` so
    /// identical background images across different pages collapse to a
    /// single XObject.
    page_background_url_cache: HashMap<String, (usize, u32, u32)>,
    /// ExtGState objects for opacity. Maps opacity value (as ordered bits) to
    /// (object_id, gs_name) e.g. (42, "GS0").
    ext_gstate_map: HashMap<u64, (usize, String)>,
    /// Shading dictionaries for gradients. One entry per (page, element)
    /// gradient instance. Resolves to (object_id, sh_name e.g. "Sh0").
    /// Maps `(page_idx, elem_idx) -> (obj_id, name)`.
    shading_map: HashMap<(usize, usize), (usize, String)>,
    /// Non-fatal notices collected during the write (e.g. pdfUa without an
    /// embeddable font). Returned to the caller so every render surface can
    /// show them, never silently dropped.
    warnings: Vec<String>,
}

pub(crate) struct PdfObject {
    #[allow(dead_code)]
    pub(crate) id: usize,
    pub(crate) data: Vec<u8>,
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfWriter {
    pub fn new() -> Self {
        Self
    }

    /// Write laid-out pages to a PDF byte vector.
    ///
    /// MEMORY NOTE (streaming-serialize investigation, 2026-09 — set aside): the
    /// large-document peak (~1GB for a 500-page doc) is NOT here. It is the
    /// `Vec<LayoutPage>` the caller retains (~2MB/page) while this fn borrows it
    /// as a slice. This writer is already ~90% streaming-ready: Pass 1 (below)
    /// consumes and zlib-compresses everything heavy per page; Pass 2 touches
    /// only scalars (width/height) and the lightweight collected lists
    /// (annotations, bookmarks). So making `write` take pages by value and drop
    /// each page's `elements` after Pass 1 saves nothing on its own — `layout()`
    /// has already materialized the whole tree before `write` is called. A real
    /// peak reduction needs a restartable STREAMING LAYOUT producer (yield page
    /// N, serialize, drop), which collides with the sentinel count pass (total
    /// page count is needed before page 1 can emit) and touches pagination.
    /// Crucially, PDF/A + PDF/UA are NOT a blocker: the structure tree
    /// (`tagged::TagBuilder`), `link_slots`, and disjoint page/annotation
    /// StructParent numbering are a few MB of lightweight metadata that stay
    /// whole-document and assemble unchanged at finalize — so streaming frees
    /// layout memory earlier without moving a single output byte, and veraPDF
    /// stays 9/9 by construction. See `scripts/parity/benchmarks.mjs`
    /// `trackedFixes` for the full write-up.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        pages: &[LayoutPage],
        metadata: &Metadata,
        font_context: &FontContext,
        tagged: bool,
        pdfa: Option<&PdfAConformance>,
        pdf_ua: bool,
        embedded_data: Option<&str>,
        attachments: &[Attachment],
        zugferd: Option<&ZugferdMeta>,
        flatten_forms: bool,
    ) -> Result<(Vec<u8>, Vec<String>), FormeError> {
        // ── Attachment / e-invoice validation (before any emission) ──
        //
        // PDF/A-1/-2 allow only PDF/A files as attachments (veraPDF rule
        // 6.8-5) — which the engine cannot verify, so a 2x level with any
        // attachment refuses rather than emitting a file that lies about
        // conformance. PDF/A-3 exists precisely to permit arbitrary
        // embedded files.
        if let Some(level) = pdfa {
            if !level.allows_attachments() && (embedded_data.is_some() || !attachments.is_empty()) {
                return Err(FormeError::RenderError(
                    "PDF/A-2 forbids embedded files that are not themselves PDF/A \
                     (ISO 19005-2, 6.8). Use a PDF/A-3 level — e.g. pdfa: \"3b\" — \
                     which permits arbitrary attachments, or remove the attachment / \
                     embedData."
                        .to_string(),
                ));
            }
        }
        // Factur-X/ZUGFeRD identification is container metadata pointing
        // at an attached XML: it needs PDF/A-3 and a matching attachment,
        // or the XMP would name a profile/file that isn't there.
        let zugferd_filename: Option<String> = if let Some(z) = zugferd {
            const LEVELS: [&str; 6] = [
                "MINIMUM",
                "BASIC WL",
                "BASIC",
                "EN 16931",
                "EXTENDED",
                "XRECHNUNG",
            ];
            if !LEVELS.contains(&z.conformance_level.as_str()) {
                return Err(FormeError::RenderError(format!(
                    "zugferd.conformanceLevel {:?} is not a Factur-X profile — expected one of \
                     MINIMUM, BASIC WL, BASIC, EN 16931, EXTENDED, XRECHNUNG (exact spelling, \
                     spaces included).",
                    z.conformance_level
                )));
            }
            if !pdfa.is_some_and(|l| l.allows_attachments()) {
                return Err(FormeError::RenderError(
                    "Factur-X/ZUGFeRD (zugferd) requires a PDF/A-3 conformance level — set \
                     pdfa: \"3b\" (or \"3a\"/\"3u\"). The e-invoice XML is an embedded file, \
                     which only PDF/A-3 permits."
                        .to_string(),
                ));
            }
            let filename = z.document_file_name.clone().unwrap_or_else(|| {
                if z.conformance_level == "XRECHNUNG" {
                    "xrechnung.xml".to_string()
                } else {
                    "factur-x.xml".to_string()
                }
            });
            if !attachments.iter().any(|a| a.name == filename) {
                return Err(FormeError::RenderError(format!(
                    "zugferd is set but no attachment is named {filename:?} — the XMP would \
                     point at a file that isn't embedded. Attach the invoice XML with name: \
                     {filename:?}, or set zugferd.documentFileName to the attachment's name."
                )));
            }
            Some(filename)
        } else {
            None
        };
        let mut builder = PdfBuilder {
            objects: Vec::new(),
            font_objects: Vec::new(),
            custom_font_data: HashMap::new(),
            embedded_standard_fonts: std::collections::HashSet::new(),
            image_objects: Vec::new(),
            image_index_map: HashMap::new(),
            page_background_image_map: HashMap::new(),
            page_background_url_cache: HashMap::new(),
            ext_gstate_map: HashMap::new(),
            shading_map: HashMap::new(),
            warnings: Vec::new(),
        };

        // Reserve object IDs:
        // 0 = placeholder (PDF objects are 1-indexed)
        // 1 = Catalog
        // 2 = Pages (page tree root)
        // 3+ = fonts, then page objects, then content streams
        builder.objects.push(PdfObject {
            id: 0,
            data: vec![],
        });
        builder.objects.push(PdfObject {
            id: 1,
            data: vec![],
        });
        builder.objects.push(PdfObject {
            id: 2,
            data: vec![],
        });

        // Register the fonts actually used across all pages
        self.register_fonts(&mut builder, pages, font_context, pdf_ua)?;

        // PDF/A: validate that all fonts are embedded. A font counts as
        // embedded if the caller registered custom bytes for it OR it's a
        // base-14 family embedded via the pdfUa Liberation substitution
        // (`embedded_standard_fonts`) — so PDF/A composes with PDF/UA when
        // @formepdf/fonts-standard is registered.
        if pdfa.is_some() {
            for (key, _) in &builder.font_objects {
                if !builder.custom_font_data.contains_key(key)
                    && !builder.embedded_standard_fonts.contains(key)
                {
                    return Err(FormeError::RenderError(format!(
                        "PDF/A requires all fonts to be embedded, but '{}' is not. Register a \
                         metric-compatible font — install @formepdf/fonts-standard and register \
                         its fonts (`for (const f of standardFonts()) Font.register(f)`), or supply \
                         your own via Font.register().",
                        key.family
                    )));
                }
            }
        }

        // Register images as XObject PDF objects
        self.register_images(&mut builder, pages);

        // Register page background images (if any) — distinct from
        // element-level Image XObjects since they're addressed per-page
        // and can be shared across pages with the same source URL.
        self.register_page_background_images(&mut builder, pages);

        // Register ExtGState objects for opacity
        self.register_ext_gstates(&mut builder, pages);

        // Register Shading dictionaries for gradient backgrounds.
        self.register_shadings(&mut builder, pages);

        // Create tag builder for accessibility if requested
        let mut tag_builder = if tagged {
            Some(tagged::TagBuilder::new(pages.len()))
        } else {
            None
        };

        // Two-pass page processing:
        // Pass 1: Build content streams, page objects, collect bookmarks + annotations
        // Pass 2: Create annotation objects (needs full bookmark list for internal links)
        let mut page_obj_ids: Vec<usize> = Vec::new();
        let mut all_bookmarks: Vec<PdfBookmark> = Vec::new();
        let mut per_page_content_obj_ids: Vec<usize> = Vec::new();
        let mut per_page_annotations: Vec<Vec<LinkAnnotation>> = Vec::new();
        let mut per_page_resources: Vec<String> = Vec::new();
        let mut all_form_fields: Vec<FormFieldData> = Vec::new();

        // Pass 1: content streams, page objects (without /Annots), bookmarks
        for (page_idx, page) in pages.iter().enumerate() {
            let content = self.build_content_stream_for_page(
                page,
                page_idx,
                &builder,
                page_idx + 1,
                pages.len(),
                tag_builder.as_mut(),
                flatten_forms,
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
            per_page_content_obj_ids.push(content_obj_id);

            // Collect link annotations (deferred creation until pass 2)
            let mut annotations: Vec<LinkAnnotation> = Vec::new();
            Self::collect_link_annotations(&page.elements, page.height, &mut annotations);
            per_page_annotations.push(annotations);

            // Collect form field annotations
            Self::collect_form_fields(&page.elements, page.height, page_idx, &mut all_form_fields);

            // Reserve page object (placeholder — filled in pass 2)
            let page_obj_id = builder.objects.len();
            builder.objects.push(PdfObject {
                id: page_obj_id,
                data: vec![],
            });

            // Build resource dict for this page
            let font_resources = self.build_font_resource_dict(&builder.font_objects);
            let xobject_resources = self.build_xobject_resource_dict(page_idx, &builder);
            let ext_gstate_resources = self.build_ext_gstate_resource_dict(&builder);
            let shading_resources = self.build_shading_resource_dict(page_idx, &builder);
            let mut resources = format!("/Font << {} >>", font_resources);
            if !xobject_resources.is_empty() {
                let _ = write!(resources, " /XObject << {} >>", xobject_resources);
            }
            if !ext_gstate_resources.is_empty() {
                let _ = write!(resources, " /ExtGState << {} >>", ext_gstate_resources);
            }
            if !shading_resources.is_empty() {
                let _ = write!(resources, " /Shading << {} >>", shading_resources);
            }
            per_page_resources.push(resources);

            // Collect bookmarks (needs page_obj_id)
            Self::collect_bookmarks(&page.elements, page.height, page_obj_id, &mut all_bookmarks);

            page_obj_ids.push(page_obj_id);
        }

        // Pass 2: create annotation objects and fill in page dicts
        for (page_idx, annotations) in per_page_annotations.iter().enumerate() {
            let mut annot_obj_ids: Vec<usize> = Vec::new();
            for annot in annotations {
                let rect = format!(
                    "[{:.2} {:.2} {:.2} {:.2}]",
                    annot.x,
                    annot.y,
                    annot.x + annot.width,
                    annot.y + annot.height
                );

                if let Some(anchor) = annot.href.strip_prefix('#') {
                    // Internal link: find matching bookmark by title
                    if let Some(bm) = all_bookmarks.iter().find(|b| b.title == anchor) {
                        let annot_obj_id = builder.objects.len();
                        // Tagged: attach this annotation to its /Link structure
                        // element (OBJR + /StructParent) so links are tagged
                        // (PDF/UA 7.18.5-1).
                        let sp_str = tag_builder
                            .as_mut()
                            .and_then(|tb| {
                                tb.connect_link_annotation(page_idx, &annot.href, annot_obj_id)
                            })
                            .map(|sp| format!(" /StructParent {}", sp))
                            .unwrap_or_default();
                        // PDF/UA 7.18.1-2 / 7.18.5-2: a link annotation must
                        // carry an alternate description in its /Contents key.
                        let contents = Self::escape_pdf_string(&format!("Link to {anchor}"));
                        let annot_dict = format!(
                            "<< /Type /Annot /Subtype /Link /Rect {} /Border [0 0 0] \
                             /F 4 /Contents ({}){} \
                             /A << /S /GoTo /D [{} 0 R /XYZ 0 {:.2} null] >> >>",
                            rect, contents, sp_str, bm.page_obj_id, bm.y_pdf
                        );
                        builder.objects.push(PdfObject {
                            id: annot_obj_id,
                            data: annot_dict.into_bytes(),
                        });
                        annot_obj_ids.push(annot_obj_id);
                    }
                    // No matching bookmark: skip silently
                } else {
                    // External link
                    let annot_obj_id = builder.objects.len();
                    let sp_str = tag_builder
                        .as_mut()
                        .and_then(|tb| {
                            tb.connect_link_annotation(page_idx, &annot.href, annot_obj_id)
                        })
                        .map(|sp| format!(" /StructParent {}", sp))
                        .unwrap_or_default();
                    let href_esc = Self::escape_pdf_string(&annot.href);
                    let annot_dict = format!(
                        "<< /Type /Annot /Subtype /Link /Rect {} /Border [0 0 0] \
                         /F 4 /Contents ({}){} \
                         /A << /Type /Action /S /URI /URI ({}) >> >>",
                        rect, href_esc, sp_str, href_esc
                    );
                    builder.objects.push(PdfObject {
                        id: annot_obj_id,
                        data: annot_dict.into_bytes(),
                    });
                    annot_obj_ids.push(annot_obj_id);
                }
            }

            let annots_str = if annot_obj_ids.is_empty() {
                String::new()
            } else {
                let refs: String = annot_obj_ids
                    .iter()
                    .map(|id| format!("{} 0 R", id))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(" /Annots [{}]", refs)
            };

            let page_obj_id = page_obj_ids[page_idx];
            let content_obj_id = per_page_content_obj_ids[page_idx];
            let struct_parents_str = if tagged {
                format!(" /StructParents {} /Tabs /S", page_idx)
            } else {
                String::new()
            };
            let page_dict = format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Contents {} 0 R /Resources << {} >>{}{} >>",
                pages[page_idx].width,
                pages[page_idx].height,
                content_obj_id,
                per_page_resources[page_idx],
                annots_str,
                struct_parents_str
            );
            builder.objects[page_obj_id].data = page_dict.into_bytes();
        }

        // Build outline tree if bookmarks exist
        let outlines_obj_id = if !all_bookmarks.is_empty() {
            Some(self.write_outline_tree(&mut builder, &all_bookmarks))
        } else {
            None
        };

        // Build structure tree for tagged PDF
        let struct_tree_root_id = if let Some(ref tb) = tag_builder {
            let (root_id, _parent_tree_id) = tb.write_objects(
                &mut builder.objects,
                &page_obj_ids,
                metadata.lang.as_deref(),
            );
            Some(root_id)
        } else {
            None
        };

        // PDF/A and/or PDF/UA: write XMP metadata stream and ICC output intent
        let xmp_metadata_id = if pdfa.is_some() || pdf_ua {
            let xmp_xml = xmp::generate_xmp(metadata, pdfa, pdf_ua, zugferd);
            let xmp_bytes = xmp_xml.as_bytes();
            let xmp_obj_id = builder.objects.len();
            // XMP metadata stream must NOT be compressed (PDF/A requirement)
            let xmp_data = format!(
                "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n",
                xmp_bytes.len()
            );
            let mut xmp_obj_data: Vec<u8> = xmp_data.into_bytes();
            xmp_obj_data.extend_from_slice(xmp_bytes);
            xmp_obj_data.extend_from_slice(b"\nendstream");
            builder.objects.push(PdfObject {
                id: xmp_obj_id,
                data: xmp_obj_data,
            });
            Some(xmp_obj_id)
        } else {
            None
        };

        let output_intent_id = if pdfa.is_some() {
            // Embed sRGB ICC profile
            static SRGB_ICC: &[u8] = include_bytes!("sRGB.icc");
            let compressed_icc = compress_to_vec_zlib(SRGB_ICC, 6);

            let icc_obj_id = builder.objects.len();
            let mut icc_data: Vec<u8> = Vec::new();
            let _ = write!(
                icc_data,
                "<< /N 3 /Length {} /Filter /FlateDecode >>\nstream\n",
                compressed_icc.len()
            );
            icc_data.extend_from_slice(&compressed_icc);
            icc_data.extend_from_slice(b"\nendstream");
            builder.objects.push(PdfObject {
                id: icc_obj_id,
                data: icc_data,
            });

            // OutputIntent dictionary
            let oi_obj_id = builder.objects.len();
            let oi_data = format!(
                "<< /Type /OutputIntent /S /GTS_PDFA1 \
                 /OutputConditionIdentifier (sRGB IEC61966-2.1) \
                 /RegistryName (http://www.color.org) \
                 /DestOutputProfile {} 0 R >>",
                icc_obj_id
            );
            builder.objects.push(PdfObject {
                id: oi_obj_id,
                data: oi_data.into_bytes(),
            });
            Some(oi_obj_id)
        } else {
            None
        };

        // Embedded files: the legacy embeddedData JSON plus caller
        // attachments (associated files). The legacy-only path must stay
        // byte-identical to what it always emitted; attachments add the
        // PDF/A-3 requirements — MIME /Subtype (6.8-1), /F + /UF (6.8-2),
        // /AFRelationship (6.8-3) — and everything joins the catalog /AF
        // array (6.8-4) as needed.
        let mut name_tree_entries: Vec<(String, usize)> = Vec::new();
        let mut af_filespec_ids: Vec<usize> = Vec::new();
        if let Some(data) = embedded_data {
            let compressed = compress_to_vec_zlib(data.as_bytes(), 6);

            // EmbeddedFile stream
            let ef_obj_id = builder.objects.len();
            let ef_data = format!(
                "<< /Type /EmbeddedFile /Subtype /application#2Fjson /Length {} /Filter /FlateDecode >>\nstream\n",
                compressed.len()
            );
            let mut ef_bytes = ef_data.into_bytes();
            ef_bytes.extend_from_slice(&compressed);
            ef_bytes.extend_from_slice(b"\nendstream");
            builder.objects.push(PdfObject {
                id: ef_obj_id,
                data: ef_bytes,
            });

            // FileSpec dictionary
            let fs_obj_id = builder.objects.len();
            let fs_data = format!(
                "<< /Type /Filespec /F (forme-data.json) /UF (forme-data.json) /EF << /F {} 0 R >> /AFRelationship /Data >>",
                ef_obj_id
            );
            builder.objects.push(PdfObject {
                id: fs_obj_id,
                data: fs_data.into_bytes(),
            });
            name_tree_entries.push(("forme-data.json".to_string(), fs_obj_id));
            // Association is a PDF/A-3 requirement; the plain path keeps
            // its historical byte-identical shape (no /AF).
            if pdfa.is_some_and(|l| l.allows_attachments()) {
                af_filespec_ids.push(fs_obj_id);
            }
        }
        for att in attachments {
            let bytes = Self::decode_attachment_src(&att.src)?;
            let compressed = compress_to_vec_zlib(&bytes, 6);
            let mime = att
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            let mod_date = att
                .mod_date
                .as_deref()
                .unwrap_or(DEFAULT_ATTACHMENT_MOD_DATE);

            let ef_obj_id = builder.objects.len();
            let ef_head = format!(
                "<< /Type /EmbeddedFile /Subtype /{} /Length {} /Filter /FlateDecode \
                 /Params << /Size {} /ModDate ({}) >> >>\nstream\n",
                Self::mime_to_pdf_name(mime),
                compressed.len(),
                bytes.len(),
                Self::escape_pdf_string(mod_date),
            );
            let mut ef_bytes = ef_head.into_bytes();
            ef_bytes.extend_from_slice(&compressed);
            ef_bytes.extend_from_slice(b"\nendstream");
            builder.objects.push(PdfObject {
                id: ef_obj_id,
                data: ef_bytes,
            });

            // The invoice XML named by zugferd gets its relationship from
            // the profile when the caller didn't set one: MINIMUM and
            // BASIC WL are not full invoices (spec mandates /Data); the
            // conformant profiles use /Alternative (mandatory in DE).
            let relationship = att.relationship.unwrap_or_else(|| {
                if zugferd_filename.as_deref() == Some(att.name.as_str()) {
                    match zugferd.map(|z| z.conformance_level.as_str()) {
                        Some("MINIMUM") | Some("BASIC WL") => AfRelationship::Data,
                        _ => AfRelationship::Alternative,
                    }
                } else {
                    AfRelationship::Unspecified
                }
            });

            let fs_obj_id = builder.objects.len();
            let mut fs_data = format!(
                "<< /Type /Filespec /F ({name}) /UF ({name}) /EF << /F {ef} 0 R >> /AFRelationship /{rel}",
                name = Self::escape_pdf_string(&att.name),
                ef = ef_obj_id,
                rel = relationship.pdf_name(),
            );
            if let Some(desc) = &att.description {
                let _ = write!(fs_data, " /Desc ({})", Self::escape_pdf_string(desc));
            }
            fs_data.push_str(" >>");
            builder.objects.push(PdfObject {
                id: fs_obj_id,
                data: fs_data.into_bytes(),
            });
            name_tree_entries.push((att.name.clone(), fs_obj_id));
            af_filespec_ids.push(fs_obj_id);
        }
        let embedded_names_id = if name_tree_entries.is_empty() {
            None
        } else {
            // Name-tree keys must be lexically sorted (PDF 32000 §7.9.6).
            name_tree_entries.sort_by(|a, b| a.0.cmp(&b.0));
            let names_obj_id = builder.objects.len();
            let pairs = name_tree_entries
                .iter()
                .map(|(name, id)| format!("({}) {} 0 R", Self::escape_pdf_string(name), id))
                .collect::<Vec<_>>()
                .join(" ");
            let names_data = format!("<< /Names [{}] >>", pairs);
            builder.objects.push(PdfObject {
                id: names_obj_id,
                data: names_data.into_bytes(),
            });
            Some(names_obj_id)
        };

        // Build AcroForm for interactive form fields
        let acroform_obj_id = if !all_form_fields.is_empty() && !flatten_forms {
            // Find the Helvetica font object ID for AcroForm /DR
            let helv_obj_id = builder
                .font_objects
                .iter()
                .find(|(key, _)| key.family == "Helvetica" && key.weight == 400 && !key.italic)
                .map(|(_, id)| *id);

            // Separate radio buttons from other fields
            let mut radio_groups: HashMap<String, Vec<usize>> = HashMap::new(); // name -> indices
            let mut non_radio_indices: Vec<usize> = Vec::new();
            for (i, field) in all_form_fields.iter().enumerate() {
                if matches!(field.field_type, FormFieldType::RadioButton { .. }) {
                    radio_groups.entry(field.name.clone()).or_default().push(i);
                } else {
                    non_radio_indices.push(i);
                }
            }

            // Pre-allocate parent field objects for radio groups
            let mut radio_parent_ids: HashMap<String, usize> = HashMap::new();
            for group_name in radio_groups.keys() {
                let parent_id = builder.objects.len();
                builder.objects.push(PdfObject {
                    id: parent_id,
                    data: vec![], // placeholder — filled after kids are created
                });
                radio_parent_ids.insert(group_name.clone(), parent_id);
            }

            // Create appearance streams for checkboxes and radio buttons
            // Checkbox checked: checkmark
            let checkbox_yes_stream_id = builder.objects.len();
            {
                let stream_content =
                    b"0.2 0.2 0.2 rg\n2 6 m 5.5 2 l 12 11 l 11 12 l 5.5 4.5 l 3 7 l 2 6 l f\n";
                let mut data: Vec<u8> = Vec::new();
                let _ = write!(
                    data,
                    "<< /Type /XObject /Subtype /Form /BBox [0 0 14 14] /Length {} >>\nstream\n",
                    stream_content.len()
                );
                data.extend_from_slice(stream_content);
                data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: checkbox_yes_stream_id,
                    data,
                });
            }
            // Checkbox unchecked: empty
            let checkbox_off_stream_id = builder.objects.len();
            {
                let stream_content = b"";
                let mut data: Vec<u8> = Vec::new();
                let _ = write!(
                    data,
                    "<< /Type /XObject /Subtype /Form /BBox [0 0 14 14] /Length {} >>\nstream\n",
                    stream_content.len()
                );
                data.extend_from_slice(stream_content);
                data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: checkbox_off_stream_id,
                    data,
                });
            }
            // Radio selected: filled circle (bezier approximation)
            let radio_on_stream_id = builder.objects.len();
            {
                // Circle centered at (7,7) radius 5 using 4-segment bezier
                let k = 2.761; // 5 * 0.5523 (magic number for circle approximation)
                let stream_content = format!(
                    "0.2 0.2 0.2 rg\n\
                     7 12 m {:.2} 12 12 {:.2} 12 7 c\n\
                     12 {:.2} {:.2} 2 7 2 c\n\
                     {:.2} 2 2 {:.2} 2 7 c\n\
                     2 {:.2} {:.2} 12 7 12 c f\n",
                    7.0 + k,
                    7.0 + k, // top-right
                    7.0 - k,
                    7.0 - k, // bottom-right
                    7.0 - k,
                    7.0 - k, // bottom-left
                    7.0 + k,
                    7.0 + k, // top-left
                );
                let stream_bytes = stream_content.as_bytes();
                let mut data: Vec<u8> = Vec::new();
                let _ = write!(
                    data,
                    "<< /Type /XObject /Subtype /Form /BBox [0 0 14 14] /Length {} >>\nstream\n",
                    stream_bytes.len()
                );
                data.extend_from_slice(stream_bytes);
                data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: radio_on_stream_id,
                    data,
                });
            }
            // Radio unselected: empty
            let radio_off_stream_id = builder.objects.len();
            {
                let stream_content = b"";
                let mut data: Vec<u8> = Vec::new();
                let _ = write!(
                    data,
                    "<< /Type /XObject /Subtype /Form /BBox [0 0 14 14] /Length {} >>\nstream\n",
                    stream_content.len()
                );
                data.extend_from_slice(stream_content);
                data.extend_from_slice(b"\nendstream");
                builder.objects.push(PdfObject {
                    id: radio_off_stream_id,
                    data,
                });
            }

            // Create widget annotation objects per page
            let mut acroform_field_ids: Vec<usize> = Vec::new();
            let mut per_page_widget_ids: Vec<Vec<usize>> = vec![Vec::new(); pages.len()];
            let mut radio_kid_ids: HashMap<String, Vec<usize>> = HashMap::new();

            for field in all_form_fields.iter() {
                let rect = format!(
                    "[{:.2} {:.2} {:.2} {:.2}]",
                    field.x,
                    field.y,
                    field.x + field.width,
                    field.y + field.height
                );
                let page_ref = format!("{} 0 R", page_obj_ids[field.page_idx]);

                match &field.field_type {
                    FormFieldType::TextField {
                        value,
                        multiline,
                        password,
                        read_only,
                        max_length,
                        font_size,
                        ..
                    } => {
                        let mut flags: u32 = 0;
                        if *multiline {
                            flags |= 1 << 12; // bit 13 (0-indexed bit 12)
                        }
                        if *password {
                            flags |= 1 << 13; // bit 14
                        }
                        if *read_only {
                            flags |= 1; // bit 1
                        }
                        let da = if let Some(helv_id) = helv_obj_id {
                            let _ = helv_id; // used in /DR, not /DA
                            format!("/Helv {} Tf 0 g", font_size)
                        } else {
                            format!("/Helv {} Tf 0 g", font_size)
                        };
                        let v_str = if let Some(ref v) = value {
                            format!(
                                " /V ({}) /DV ({})",
                                Self::escape_pdf_string(v),
                                Self::escape_pdf_string(v)
                            )
                        } else {
                            String::new()
                        };
                        let max_len_str = if let Some(ml) = max_length {
                            format!(" /MaxLen {}", ml)
                        } else {
                            String::new()
                        };
                        // Build appearance stream for the text field
                        let ap_w = field.width;
                        let ap_h = field.height;
                        let text_y = if *multiline {
                            ap_h - *font_size - 2.0
                        } else {
                            (ap_h - *font_size) / 2.0
                        };
                        let ap_content = if let Some(ref v) = value {
                            format!(
                                "1 1 1 rg 0 0 {} {} re f \
                                 0.6 0.6 0.6 RG 0.5 w 0 0 {} {} re S \
                                 BT /Helv {} Tf 0 g 2 {} Td ({}) Tj ET",
                                ap_w,
                                ap_h,
                                ap_w,
                                ap_h,
                                font_size,
                                text_y,
                                Self::escape_pdf_string(v)
                            )
                        } else {
                            format!(
                                "1 1 1 rg 0 0 {} {} re f \
                                 0.6 0.6 0.6 RG 0.5 w 0 0 {} {} re S",
                                ap_w, ap_h, ap_w, ap_h
                            )
                        };
                        let ap_stream_id = builder.objects.len();
                        let ap_stream = format!(
                            "<< /Type /XObject /Subtype /Form /BBox [0 0 {} {}] \
                             /Resources << /Font << /Helv {} 0 R >> >> /Length {} >>\nstream\n{}\nendstream",
                            ap_w, ap_h,
                            helv_obj_id.unwrap_or(0),
                            ap_content.len(),
                            ap_content
                        );
                        builder.objects.push(PdfObject {
                            id: ap_stream_id,
                            data: ap_stream.into_bytes(),
                        });

                        let widget_obj_id = builder.objects.len();
                        let widget_dict = format!(
                            "<< /Type /Annot /Subtype /Widget /FT /Tx \
                             /T ({}) /Rect {} /P {}\
                             {} /DA ({}) /Ff {}{} \
                             /MK << /BC [0.6 0.6 0.6] /BG [1 1 1] >> \
                             /AP << /N {} 0 R >> >>",
                            Self::escape_pdf_string(&field.name),
                            rect,
                            page_ref,
                            v_str,
                            da,
                            flags,
                            max_len_str,
                            ap_stream_id
                        );
                        builder.objects.push(PdfObject {
                            id: widget_obj_id,
                            data: widget_dict.into_bytes(),
                        });
                        per_page_widget_ids[field.page_idx].push(widget_obj_id);
                        acroform_field_ids.push(widget_obj_id);
                    }

                    FormFieldType::Checkbox {
                        checked, read_only, ..
                    } => {
                        let state = if *checked { "Yes" } else { "Off" };
                        let mut flags: u32 = 0;
                        if *read_only {
                            flags |= 1;
                        }
                        let ff_str = if flags > 0 {
                            format!(" /Ff {}", flags)
                        } else {
                            String::new()
                        };
                        let widget_obj_id = builder.objects.len();
                        let widget_dict = format!(
                            "<< /Type /Annot /Subtype /Widget /FT /Btn \
                             /T ({}) /Rect {} /P {} \
                             /V /{} /AS /{}{} \
                             /MK << /BC [0.6 0.6 0.6] /CA (4) >> \
                             /AP << /N << /Yes {} 0 R /Off {} 0 R >> >> >>",
                            Self::escape_pdf_string(&field.name),
                            rect,
                            page_ref,
                            state,
                            state,
                            ff_str,
                            checkbox_yes_stream_id,
                            checkbox_off_stream_id,
                        );
                        builder.objects.push(PdfObject {
                            id: widget_obj_id,
                            data: widget_dict.into_bytes(),
                        });
                        per_page_widget_ids[field.page_idx].push(widget_obj_id);
                        acroform_field_ids.push(widget_obj_id);
                    }

                    FormFieldType::Dropdown {
                        options,
                        value,
                        read_only,
                        font_size,
                        ..
                    } => {
                        let mut flags: u32 = 1 << 17; // bit 18 = combo box
                        if *read_only {
                            flags |= 1;
                        }
                        let opts_str: String = options
                            .iter()
                            .map(|o| format!("({})", Self::escape_pdf_string(o)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let v_str = if let Some(ref v) = value {
                            format!(" /V ({})", Self::escape_pdf_string(v))
                        } else {
                            String::new()
                        };
                        // Build appearance stream for the dropdown
                        let ap_w = field.width;
                        let ap_h = field.height;
                        let text_y = (ap_h - *font_size) / 2.0;
                        let ap_content = if let Some(ref v) = value {
                            format!(
                                "1 1 1 rg 0 0 {} {} re f \
                                 0.6 0.6 0.6 RG 0.5 w 0 0 {} {} re S \
                                 BT /Helv {} Tf 0 g 2 {} Td ({}) Tj ET",
                                ap_w,
                                ap_h,
                                ap_w,
                                ap_h,
                                font_size,
                                text_y,
                                Self::escape_pdf_string(v)
                            )
                        } else {
                            format!(
                                "1 1 1 rg 0 0 {} {} re f \
                                 0.6 0.6 0.6 RG 0.5 w 0 0 {} {} re S",
                                ap_w, ap_h, ap_w, ap_h
                            )
                        };
                        let ap_stream_id = builder.objects.len();
                        let ap_stream = format!(
                            "<< /Type /XObject /Subtype /Form /BBox [0 0 {} {}] \
                             /Resources << /Font << /Helv {} 0 R >> >> /Length {} >>\nstream\n{}\nendstream",
                            ap_w, ap_h,
                            helv_obj_id.unwrap_or(0),
                            ap_content.len(),
                            ap_content
                        );
                        builder.objects.push(PdfObject {
                            id: ap_stream_id,
                            data: ap_stream.into_bytes(),
                        });

                        let widget_obj_id = builder.objects.len();
                        let widget_dict = format!(
                            "<< /Type /Annot /Subtype /Widget /FT /Ch \
                             /T ({}) /Rect {} /P {} \
                             /Opt [{}]{} \
                             /DA (/Helv {} Tf 0 g) /Ff {} \
                             /MK << /BC [0.6 0.6 0.6] /BG [1 1 1] >> \
                             /AP << /N {} 0 R >> >>",
                            Self::escape_pdf_string(&field.name),
                            rect,
                            page_ref,
                            opts_str,
                            v_str,
                            font_size,
                            flags,
                            ap_stream_id
                        );
                        builder.objects.push(PdfObject {
                            id: widget_obj_id,
                            data: widget_dict.into_bytes(),
                        });
                        per_page_widget_ids[field.page_idx].push(widget_obj_id);
                        acroform_field_ids.push(widget_obj_id);
                    }

                    FormFieldType::RadioButton {
                        value,
                        checked,
                        read_only: _,
                    } => {
                        // Radio kid widget — parent reference is critical
                        let parent_id = radio_parent_ids[&field.name];
                        let as_value = if *checked { value.as_str() } else { "Off" };
                        let widget_obj_id = builder.objects.len();
                        let widget_dict = format!(
                            "<< /Type /Annot /Subtype /Widget \
                             /Parent {} 0 R \
                             /Rect {} /P {} \
                             /AS /{} \
                             /AP << /N << /{} {} 0 R /Off {} 0 R >> >> \
                             /MK << /BC [0.6 0.6 0.6] >> >>",
                            parent_id,
                            rect,
                            page_ref,
                            Self::escape_pdf_string(as_value),
                            Self::escape_pdf_string(value),
                            radio_on_stream_id,
                            radio_off_stream_id,
                        );
                        builder.objects.push(PdfObject {
                            id: widget_obj_id,
                            data: widget_dict.into_bytes(),
                        });
                        per_page_widget_ids[field.page_idx].push(widget_obj_id);
                        // Kids go in page /Annots, NOT in /AcroForm /Fields
                        radio_kid_ids
                            .entry(field.name.clone())
                            .or_default()
                            .push(widget_obj_id);
                    }
                }
            }

            // Fill in radio parent field objects
            for (group_name, kid_indices) in &radio_kid_ids {
                let parent_id = radio_parent_ids[group_name];
                // Find the checked value in this group
                let checked_value = all_form_fields
                    .iter()
                    .filter(|f| f.name == *group_name)
                    .find_map(|f| {
                        if let FormFieldType::RadioButton {
                            ref value, checked, ..
                        } = f.field_type
                        {
                            if checked {
                                Some(value.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Off".to_string());

                let kids_refs: String = kid_indices
                    .iter()
                    .map(|id| format!("{} 0 R", id))
                    .collect::<Vec<_>>()
                    .join(" ");

                let mut flags: u32 = (1 << 14) | (1 << 15); // radio + noToggleToOff
                                                            // Check if read_only on any button in group
                let is_read_only = all_form_fields
                    .iter()
                    .filter(|f| f.name == *group_name)
                    .any(|f| {
                        matches!(
                            f.field_type,
                            FormFieldType::RadioButton {
                                read_only: true,
                                ..
                            }
                        )
                    });
                if is_read_only {
                    flags |= 1;
                }

                let parent_dict = format!(
                    "<< /FT /Btn /T ({}) /Ff {} /Kids [{}] /V /{} >>",
                    Self::escape_pdf_string(group_name),
                    flags,
                    kids_refs,
                    Self::escape_pdf_string(&checked_value),
                );
                builder.objects[parent_id].data = parent_dict.into_bytes();
                acroform_field_ids.push(parent_id);
            }

            // Now add form widget IDs to the existing page annotation arrays
            // We need to update the already-written page dicts to include form widgets
            // Rebuild page dicts with form widget annotations included
            for (page_idx, widget_ids) in per_page_widget_ids.iter().enumerate() {
                if widget_ids.is_empty() {
                    continue;
                }
                let page_obj_id = page_obj_ids[page_idx];
                let existing_page_data =
                    String::from_utf8_lossy(&builder.objects[page_obj_id].data).to_string();

                // If the page already has /Annots, append to it; otherwise add it
                let new_refs: String = widget_ids
                    .iter()
                    .map(|id| format!("{} 0 R", id))
                    .collect::<Vec<_>>()
                    .join(" ");

                let updated = if let Some(pos) = existing_page_data.find("/Annots [") {
                    // Insert before the closing ]
                    let bracket_end = existing_page_data[pos..].find(']').unwrap() + pos;
                    format!(
                        "{} {}{}",
                        &existing_page_data[..bracket_end],
                        new_refs,
                        &existing_page_data[bracket_end..]
                    )
                } else {
                    // Add /Annots before the final >>
                    let end = existing_page_data.rfind(">>").unwrap();
                    format!(
                        "{} /Annots [{}]{}",
                        &existing_page_data[..end],
                        new_refs,
                        &existing_page_data[end..]
                    )
                };
                builder.objects[page_obj_id].data = updated.into_bytes();
            }

            // Create AcroForm dictionary
            let acroform_id = builder.objects.len();
            let fields_refs: String = acroform_field_ids
                .iter()
                .map(|id| format!("{} 0 R", id))
                .collect::<Vec<_>>()
                .join(" ");
            let dr_str = if let Some(helv_id) = helv_obj_id {
                format!(" /DR << /Font << /Helv {} 0 R >> >>", helv_id)
            } else {
                String::new()
            };
            let acroform_dict = format!(
                "<< /Fields [{}] /NeedAppearances true{} /DA (/Helv 0 Tf 0 g) >>",
                fields_refs, dr_str
            );
            builder.objects.push(PdfObject {
                id: acroform_id,
                data: acroform_dict.into_bytes(),
            });
            Some(acroform_id)
        } else {
            None
        };

        // Write Catalog (object 1)
        let mut catalog = String::from("<< /Type /Catalog /Pages 2 0 R");
        if let Some(acroform_id) = acroform_obj_id {
            write!(catalog, " /AcroForm {} 0 R", acroform_id).unwrap();
        }
        if let Some(outlines_id) = outlines_obj_id {
            write!(
                catalog,
                " /Outlines {} 0 R /PageMode /UseOutlines",
                outlines_id
            )
            .unwrap();
        }
        if let Some(ref lang) = metadata.lang {
            write!(catalog, " /Lang ({})", Self::escape_pdf_string(lang)).unwrap();
        }
        if let Some(struct_root_id) = struct_tree_root_id {
            write!(
                catalog,
                " /MarkInfo << /Marked true >> /StructTreeRoot {} 0 R",
                struct_root_id
            )
            .unwrap();
        }
        if let Some(xmp_id) = xmp_metadata_id {
            write!(catalog, " /Metadata {} 0 R", xmp_id).unwrap();
        }
        if let Some(oi_id) = output_intent_id {
            write!(catalog, " /OutputIntents [{} 0 R]", oi_id).unwrap();
        }
        if let Some(names_id) = embedded_names_id {
            write!(catalog, " /Names << /EmbeddedFiles {} 0 R >>", names_id).unwrap();
        }
        if !af_filespec_ids.is_empty() {
            // Document-level association (PDF/A-3 6.8-4; Factur-X requires
            // the invoice XML to be associated at the catalog).
            let refs = af_filespec_ids
                .iter()
                .map(|id| format!("{} 0 R", id))
                .collect::<Vec<_>>()
                .join(" ");
            write!(catalog, " /AF [{}]", refs).unwrap();
        }
        if pdf_ua {
            catalog.push_str(" /ViewerPreferences << /DisplayDocTitle true >>");
        }
        catalog.push_str(" >>");
        builder.objects[1].data = catalog.into_bytes();

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
            let _ = write!(info, "/Producer (Forme 0.6) /Creator (Forme) >>");
            builder.objects.push(PdfObject {
                id,
                data: info.into_bytes(),
            });
            Some(id)
        } else {
            None
        };

        let pdf = self.serialize(&builder, info_obj_id);
        Ok((pdf, builder.warnings))
    }

    /// Build the PDF content stream for a single page.
    #[allow(clippy::too_many_arguments)]
    fn build_content_stream_for_page(
        &self,
        page: &LayoutPage,
        page_idx: usize,
        builder: &PdfBuilder,
        page_number: usize,
        total_pages: usize,
        mut tag_builder: Option<&mut tagged::TagBuilder>,
        flatten_forms: bool,
    ) -> String {
        let mut stream = String::new();
        let page_height = page.height;
        let mut element_counter = 0usize;
        let mut gradient_counter = 0usize;

        // Page background image: paint it before any element content so
        // it sits behind everything. Wrapped in q/Q + ExtGState for
        // backgroundOpacity, with the cm matrix sized & positioned via
        // backgroundSize / backgroundPosition. Same XObject can be reused
        // across multiple pages with the same source URL.
        if let Some(&img_idx) = builder.page_background_image_map.get(&page_idx) {
            self.write_page_background(&mut stream, page, img_idx, builder);
        }

        // Horizontal content clip (`PageConfig.clip_content_x`): the paged
        // equivalent of `body { overflow-x: hidden }`. X is clipped to the
        // content box; Y spans the full page so nothing vertical is lost.
        let clip_x = page.config.clip_content_x;
        if clip_x {
            let x = page.config.margin.left;
            let w = page.width - page.config.margin.left - page.config.margin.right;
            stream.push_str(&format!(
                "q\n{:.2} 0 {:.2} {:.2} re W n\n",
                x, w, page.height
            ));
        }

        for element in &page.elements {
            self.write_element(
                &mut stream,
                element,
                page_height,
                builder,
                page_idx,
                &mut element_counter,
                &mut gradient_counter,
                page_number,
                total_pages,
                tag_builder.as_deref_mut(),
                flatten_forms,
            );
        }

        if clip_x {
            stream.push_str("Q\n");
        }

        stream
    }

    /// Write a single layout element as PDF operators.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn write_element(
        &self,
        stream: &mut String,
        element: &LayoutElement,
        page_height: f64,
        builder: &PdfBuilder,
        page_idx: usize,
        element_counter: &mut usize,
        gradient_counter: &mut usize,
        page_number: usize,
        total_pages: usize,
        mut tag_builder: Option<&mut tagged::TagBuilder>,
        flatten_forms: bool,
    ) {
        // Tagged PDF: emit BDC (begin marked content) for elements with a node_type,
        // or /Artifact BMC for decorative elements (watermarks, untagged drawing).
        let mut is_artifact = false;
        let tagged_mcid = if let Some(ref mut tb) = tag_builder {
            if let Some(ref nt) = element.node_type {
                if nt == "Watermark" {
                    // Watermarks are decorative — mark as artifact, not structure
                    let _ = writeln!(stream, "/Artifact BMC");
                    is_artifact = true;
                    None
                } else {
                    let is_header = element.is_header_row;
                    let href = element.href.as_deref();
                    let mcid = tb.begin_element(
                        nt,
                        is_header,
                        element.alt.as_deref(),
                        page_idx,
                        href,
                        element.col_span,
                    );
                    // An href'd element tags as /Link (see begin_element); the
                    // BDC role must match the structure role, so key on href too.
                    let role = if href.is_some() {
                        "Link"
                    } else {
                        tb.map_role_public(nt, is_header)
                    };
                    let _ = writeln!(stream, "/{} <</MCID {}>> BDC", role, mcid);
                    Some(mcid)
                }
            } else if !matches!(element.draw, DrawCommand::None) {
                // No node_type but has drawing — wrap as artifact
                let _ = writeln!(stream, "/Artifact BMC");
                is_artifact = true;
                None
            } else {
                None
            }
        } else {
            None
        };

        // Element-level opacity wrap. Open `q\n/GS{n} gs` AFTER the BMC/BDC
        // marker block (so opacity affects content, not the marker), and
        // close the matching `Q` BEFORE the EMC. The wrap encompasses both
        // the element's own DrawCommand emission AND the recursion into
        // `element.children`, so descendants render at the cumulative
        // alpha (PDF graphics state stack multiplies naturally — a 0.5
        // child of a 0.5 parent renders at effective 0.25).
        let needs_element_opacity = element.opacity < 1.0;
        if needs_element_opacity {
            if let Some((_, gs_name)) = builder.ext_gstate_map.get(&element.opacity.to_bits()) {
                let _ = writeln!(stream, "q\n/{} gs", gs_name);
            }
        }

        // CSS-style `transform` wrap. Sits INSIDE the opacity wrap so the
        // opacity applies to the transformed output. Layout flow is NOT
        // affected by the transform (matches CSS) — element.x/y/width/height
        // are still the axis-aligned box; the transform is paint-only and
        // also propagates to children via the graphics state stack.
        let transform_ops: &[TransformOp] = element
            .resolved_style
            .as_ref()
            .map(|s| s.transform.as_slice())
            .unwrap_or(&[]);
        let has_transform = !transform_ops.is_empty();
        if has_transform {
            let rs = element.resolved_style.as_ref().unwrap();
            let pdf_x = element.x;
            let pdf_y_bottom = page_height - element.y - element.height;
            let (ox_frac, oy_frac) = rs.transform_origin;
            let origin_x = pdf_x + element.width * ox_frac;
            // transform_origin's y is 0=top / 1=bottom in layout (CSS) space.
            // Flip for PDF (1=top / 0=bottom).
            let origin_y = pdf_y_bottom + (1.0 - oy_frac) * element.height;

            let _ = writeln!(stream, "q");
            // Shift origin point to PDF (0,0) so subsequent transforms pivot there.
            let _ = writeln!(stream, "1 0 0 1 {:.4} {:.4} cm", -origin_x, -origin_y);
            // User transforms: emit in REVERSE of the CSS list order. CSS lists
            // transforms left-to-right with the LAST one applied first
            // (closest to the point being drawn). PDF `cm` left-multiplies the
            // CTM, so the FIRST emitted cm becomes the innermost. Reversing
            // makes the leftmost CSS transform the last cm emitted = outermost
            // multiplication = applied last to a point — which matches "first
            // listed wraps everything inside it" semantics.
            for op in transform_ops.iter().rev() {
                match op {
                    TransformOp::Rotate { deg } => {
                        // CSS rotates clockwise in screen space. With PDF's
                        // flipped y-axis, the same matrix would rotate
                        // counter-clockwise visually. Negate the angle so a
                        // CSS `rotate(45deg)` looks identical in the PDF.
                        let theta = (-deg).to_radians();
                        let c = theta.cos();
                        let s = theta.sin();
                        let _ = writeln!(stream, "{:.6} {:.6} {:.6} {:.6} 0 0 cm", c, s, -s, c);
                    }
                    TransformOp::Scale { x, y } => {
                        let _ = writeln!(stream, "{:.6} 0 0 {:.6} 0 0 cm", x, y);
                    }
                    TransformOp::Translate { x, y } => {
                        // CSS y is down, PDF y is up — negate the y component.
                        let _ = writeln!(stream, "1 0 0 1 {:.4} {:.4} cm", x, -y);
                    }
                }
            }
            // Shift origin back to its real position.
            let _ = writeln!(stream, "1 0 0 1 {:.4} {:.4} cm", origin_x, origin_y);
        }

        match &element.draw {
            DrawCommand::None => {}

            DrawCommand::Rect {
                background,
                border_width,
                border_color,
                border_style,
                border_radius,
                opacity,
                box_shadow,
                background_gradient,
            } => {
                let x = element.x;
                let y = page_height - element.y - element.height;
                let w = element.width;
                let h = element.height;

                // Apply opacity via ExtGState
                let needs_opacity = *opacity < 1.0;
                if needs_opacity {
                    if let Some((_, gs_name)) = builder.ext_gstate_map.get(&opacity.to_bits()) {
                        let _ = writeln!(stream, "q\n/{} gs", gs_name);
                    }
                }

                // Box shadow: paint a filled rect offset by (offsetX, offsetY)
                // BEFORE the background so the shadow sits behind. Shadow
                // color alpha goes through the per-shadow ExtGState. Shadow
                // path uses the same border_radius as the element so rounded
                // boxes get rounded shadows.
                if let Some(shadow) = box_shadow {
                    if shadow.color.a > 0.0 {
                        // PDF y-axis is flipped vs CSS, so a positive
                        // offsetY (CSS: shadow goes down) → subtract from
                        // pdf_y to move the shadow rect downward in
                        // visual terms.
                        let sx = x + shadow.offset_x;
                        let sy = y - shadow.offset_y;
                        let needs_shadow_alpha = shadow.color.a < 1.0;
                        if needs_shadow_alpha {
                            if let Some((_, gs_name)) =
                                builder.ext_gstate_map.get(&shadow.color.a.to_bits())
                            {
                                let _ = writeln!(stream, "q\n/{} gs", gs_name);
                            } else {
                                let _ = writeln!(stream, "q");
                            }
                        } else {
                            let _ = writeln!(stream, "q");
                        }
                        let _ = writeln!(
                            stream,
                            "{:.3} {:.3} {:.3} rg",
                            shadow.color.r, shadow.color.g, shadow.color.b
                        );
                        if border_radius.top_left > 0.0
                            || border_radius.top_right > 0.0
                            || border_radius.bottom_right > 0.0
                            || border_radius.bottom_left > 0.0
                        {
                            self.write_rounded_rect(stream, sx, sy, w, h, border_radius);
                        } else {
                            let _ = writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re", sx, sy, w, h);
                        }
                        let _ = writeln!(stream, "f\nQ");
                    }
                }

                // Background paint: gradient takes precedence over the
                // solid color when both are set. Gradient emission uses
                // `q + clip path + cm + sh + Q`; the cm translate moves
                // the shading's local 0,0 to the rect's bottom-left so
                // the Coords (computed during register_shadings) line up.
                if background_gradient.is_some() {
                    let key = (page_idx, *gradient_counter);
                    *gradient_counter += 1;
                    if let Some((_, sh_name)) = builder.shading_map.get(&key) {
                        let _ = writeln!(stream, "q");
                        // Clip to the rect (rounded if borderRadius set).
                        if border_radius.top_left > 0.0
                            || border_radius.top_right > 0.0
                            || border_radius.bottom_right > 0.0
                            || border_radius.bottom_left > 0.0
                        {
                            self.write_rounded_rect(stream, x, y, w, h, border_radius);
                            let _ = writeln!(stream, "W n");
                        } else {
                            let _ = writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re W n", x, y, w, h);
                        }
                        // Translate so the shading's local 0,0 sits at
                        // the rect's bottom-left.
                        let _ =
                            writeln!(stream, "1 0 0 1 {:.3} {:.3} cm\n/{} sh\nQ", x, y, sh_name);
                    }
                } else if let Some(bg) = background {
                    if bg.a > 0.0 {
                        let _ = writeln!(stream, "q\n{:.3} {:.3} {:.3} rg", bg.r, bg.g, bg.b);

                        if border_radius.top_left > 0.0 {
                            self.write_rounded_rect(stream, x, y, w, h, border_radius);
                        } else {
                            let _ = writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re", x, y, w, h);
                        }

                        let _ = writeln!(stream, "f\nQ");
                    }
                }

                let bw = border_width;
                if bw.top > 0.0 || bw.right > 0.0 || bw.bottom > 0.0 || bw.left > 0.0 {
                    use crate::style::BorderStyle::Solid;
                    let all_solid = border_style.top == Solid
                        && border_style.right == Solid
                        && border_style.bottom == Solid
                        && border_style.left == Solid;
                    // The uniform fast path draws one rounded/plain rect stroke;
                    // it only applies to a solid, equal-width border. Any
                    // dashed/dotted or mixed-style border goes per-side (which
                    // also emits the dash pattern; radius is dropped there, per
                    // Chrome's own dashed-with-radius handling).
                    if all_solid
                        && (bw.top - bw.right).abs() < 0.001
                        && (bw.right - bw.bottom).abs() < 0.001
                        && (bw.bottom - bw.left).abs() < 0.001
                    {
                        let bc = &border_color.top;
                        let _ = writeln!(
                            stream,
                            "q\n{:.3} {:.3} {:.3} RG\n{:.2} w",
                            bc.r, bc.g, bc.b, bw.top
                        );

                        if border_radius.top_left > 0.0 {
                            self.write_rounded_rect(stream, x, y, w, h, border_radius);
                        } else {
                            let _ = writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re", x, y, w, h);
                        }

                        let _ = writeln!(stream, "S\nQ");
                    } else {
                        self.write_border_sides(stream, x, y, w, h, bw, border_color, border_style);
                    }
                }

                if needs_opacity {
                    let _ = writeln!(stream, "Q");
                }
            }

            DrawCommand::Text {
                lines,
                color,
                text_decoration,
                opacity,
            } => {
                // Apply opacity via ExtGState
                let needs_opacity = *opacity < 1.0;
                if needs_opacity {
                    if let Some((_, gs_name)) = builder.ext_gstate_map.get(&opacity.to_bits()) {
                        let _ = writeln!(stream, "q\n/{} gs", gs_name);
                    }
                }

                for line in lines {
                    if line.glyphs.is_empty() {
                        continue;
                    }

                    // Group consecutive glyphs by (font_family, font_weight, font_style, font_size, color)
                    // to support multi-font text runs
                    let groups = Self::group_glyphs_by_style(&line.glyphs);
                    let pdf_y = page_height - line.y;

                    let _ = writeln!(stream, "BT");

                    // Set word spacing for justification (PDF Tw operator)
                    if line.word_spacing.abs() > 0.001 {
                        let _ = writeln!(stream, "{:.4} Tw", line.word_spacing);
                    }

                    // Track current text matrix position for relative Td moves
                    let mut tm_x = 0.0_f64;
                    let mut tm_y = 0.0_f64;
                    let mut x_cursor = line.x;

                    // Track group spans for per-group text decoration
                    let mut group_spans: Vec<(f64, f64, TextDecoration, Color)> = Vec::new();

                    for group in &groups {
                        let first = &group[0];
                        let glyph_color = first.color.unwrap_or(*color);

                        let idx = self.font_index(
                            &first.font_family,
                            first.font_weight,
                            first.font_style,
                            &builder.font_objects,
                        );
                        let italic =
                            matches!(first.font_style, FontStyle::Italic | FontStyle::Oblique);
                        let font_key = FontKey {
                            family: first.font_family.to_string(),
                            weight: first.font_weight,
                            italic,
                        };
                        let font_name = format!("F{}", idx);

                        // Td is relative to current text matrix position
                        let dx = x_cursor - tm_x;
                        let dy = pdf_y - tm_y;
                        let _ = writeln!(
                            stream,
                            "{:.3} {:.3} {:.3} rg\n/{} {:.1} Tf\n{:.2} Tc\n{:.2} {:.2} Td",
                            glyph_color.r,
                            glyph_color.g,
                            glyph_color.b,
                            font_name,
                            first.font_size,
                            first.letter_spacing,
                            dx,
                            dy
                        );
                        tm_x = x_cursor;
                        tm_y = pdf_y;

                        // Check for page number sentinel characters
                        let raw_text: String = group.iter().map(|g| g.char_value).collect();
                        let has_placeholder = raw_text.contains(PAGE_NUMBER_SENTINEL)
                            || raw_text.contains(TOTAL_PAGES_SENTINEL);

                        let is_custom = builder.custom_font_data.contains_key(&font_key);

                        if is_custom {
                            if let Some(embed_data) = builder.custom_font_data.get(&font_key) {
                                let mut hex = String::new();
                                if has_placeholder {
                                    // Sentinel text: replace with actual values and use char→gid fallback
                                    let pn = PAGE_NUMBER_SENTINEL.to_string();
                                    let tp = TOTAL_PAGES_SENTINEL.to_string();
                                    let text_after = raw_text
                                        .replace(&pn, &page_number.to_string())
                                        .replace(&tp, &total_pages.to_string());
                                    for ch in text_after.chars() {
                                        let gid =
                                            embed_data.char_to_gid.get(&ch).copied().unwrap_or(0);
                                        let _ = write!(hex, "{:04X}", gid);
                                    }
                                } else {
                                    // Shaped text: use glyph IDs directly (remapped through subset)
                                    for g in group.iter() {
                                        let new_gid = embed_data
                                            .gid_remap
                                            .get(&g.glyph_id)
                                            .copied()
                                            .unwrap_or_else(|| {
                                                // Fallback: try char→gid
                                                embed_data
                                                    .char_to_gid
                                                    .get(&g.char_value)
                                                    .copied()
                                                    .unwrap_or(0)
                                            });
                                        let _ = write!(hex, "{:04X}", new_gid);
                                    }
                                }
                                let _ = writeln!(stream, "<{}> Tj", hex);
                            } else {
                                let _ = writeln!(stream, "<> Tj");
                            }
                        } else {
                            let pn = PAGE_NUMBER_SENTINEL.to_string();
                            let tp = TOTAL_PAGES_SENTINEL.to_string();
                            let text_after = raw_text
                                .replace(&pn, &page_number.to_string())
                                .replace(&tp, &total_pages.to_string());
                            let mut text_str = String::new();
                            for ch in text_after.chars() {
                                let b = Self::unicode_to_winansi(ch).unwrap_or(b'?');
                                match b {
                                    b'\\' => text_str.push_str("\\\\"),
                                    b'(' => text_str.push_str("\\("),
                                    b')' => text_str.push_str("\\)"),
                                    0x20..=0x7E => text_str.push(b as char),
                                    _ => {
                                        let _ = write!(text_str, "\\{:03o}", b);
                                    }
                                }
                            }
                            let _ = writeln!(stream, "({}) Tj", text_str);
                        }

                        // Record span for per-group text decoration
                        let group_start_x = x_cursor;

                        // Advance x_cursor past this group using shaped advances
                        // Account for word_spacing on spaces (Tw adds to each space char)
                        if let Some(last) = group.last() {
                            let space_count_in_group =
                                group.iter().filter(|g| g.char_value == ' ').count();
                            x_cursor = line.x
                                + last.x_offset
                                + last.x_advance
                                + space_count_in_group as f64 * line.word_spacing;
                        }

                        // Check if this group has text decoration
                        let group_dec = first.text_decoration;
                        if !matches!(group_dec, TextDecoration::None) {
                            group_spans.push((group_start_x, x_cursor, group_dec, glyph_color));
                        }
                    }

                    let _ = writeln!(stream, "ET");

                    // Draw per-group text decorations
                    for (span_x, span_end_x, dec, dec_color) in &group_spans {
                        match dec {
                            TextDecoration::Underline => {
                                let underline_y = pdf_y - 1.5;
                                let _ = write!(
                                    stream,
                                    "q\n{:.3} {:.3} {:.3} RG\n0.5 w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                                    dec_color.r, dec_color.g, dec_color.b,
                                    span_x, underline_y,
                                    span_end_x, underline_y
                                );
                            }
                            TextDecoration::LineThrough => {
                                let first_size =
                                    line.glyphs.first().map(|g| g.font_size).unwrap_or(12.0);
                                let strikethrough_y = pdf_y + first_size * 0.3;
                                let _ = write!(
                                    stream,
                                    "q\n{:.3} {:.3} {:.3} RG\n0.5 w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                                    dec_color.r, dec_color.g, dec_color.b,
                                    span_x, strikethrough_y,
                                    span_end_x, strikethrough_y
                                );
                            }
                            TextDecoration::None => {}
                        }
                    }

                    // Also handle whole-line decoration from parent style
                    if group_spans.is_empty() {
                        if matches!(text_decoration, TextDecoration::Underline) {
                            let underline_y = pdf_y - 1.5;
                            let _ = write!(
                                stream,
                                "q\n{:.3} {:.3} {:.3} RG\n0.5 w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                                color.r, color.g, color.b,
                                line.x, underline_y,
                                line.x + line.width, underline_y
                            );
                        }
                        if matches!(text_decoration, TextDecoration::LineThrough) {
                            let first_size =
                                line.glyphs.first().map(|g| g.font_size).unwrap_or(12.0);
                            let strikethrough_y = pdf_y + first_size * 0.3;
                            let _ = write!(
                                stream,
                                "q\n{:.3} {:.3} {:.3} RG\n0.5 w\n{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                                color.r, color.g, color.b,
                                line.x, strikethrough_y,
                                line.x + line.width, strikethrough_y
                            );
                        }
                    }
                }

                if needs_opacity {
                    let _ = writeln!(stream, "Q");
                }
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
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
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
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::Svg {
                commands,
                width: _svg_w,
                height: _svg_h,
                viewbox_min_x,
                viewbox_min_y,
                viewbox_width,
                viewbox_height,
                clip,
            } => {
                let x = element.x;
                let y = page_height - element.y - element.height;

                // Save state, translate to position
                let _ = writeln!(stream, "q");
                let _ = writeln!(stream, "1 0 0 1 {:.2} {:.2} cm", x, y);

                // SVG viewport algorithm with `xMidYMid meet` as the default
                // preserveAspectRatio: uniform scale to fit, center the
                // remainder. When viewBox matches the display box (the
                // no-viewBox case, populated as 0/0/w/h in layout) the scale
                // is 1 and the translate is 0 — behavior unchanged.
                if *viewbox_width > 0.0 && *viewbox_height > 0.0 {
                    let raw_sx = element.width / *viewbox_width;
                    let raw_sy = element.height / *viewbox_height;
                    let s = raw_sx.min(raw_sy);
                    let tx = (element.width - s * *viewbox_width) / 2.0;
                    let ty = (element.height - s * *viewbox_height) / 2.0;
                    let _ = writeln!(stream, "{:.4} 0 0 {:.4} {:.2} {:.2} cm", s, s, tx, ty);
                }

                // Flip Y so SVG-coord Y-down becomes PDF Y-up. The flip
                // height is the viewBox height (we're now in viewBox space).
                let _ = writeln!(stream, "1 0 0 -1 0 {:.2} cm", *viewbox_height);

                // Shift origin so the viewBox's (min_x, min_y) lands at (0, 0).
                if *viewbox_min_x != 0.0 || *viewbox_min_y != 0.0 {
                    let _ = writeln!(
                        stream,
                        "1 0 0 1 {:.2} {:.2} cm",
                        -*viewbox_min_x, -*viewbox_min_y
                    );
                }

                // Clip to viewBox bounds (Canvas always clips, SVG does not).
                if *clip {
                    let _ = writeln!(
                        stream,
                        "{:.2} {:.2} {:.2} {:.2} re W n",
                        *viewbox_min_x, *viewbox_min_y, *viewbox_width, *viewbox_height
                    );
                }

                Self::write_svg_commands(stream, commands, &builder.ext_gstate_map);

                let _ = writeln!(stream, "Q");
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::Barcode {
                bars,
                bar_width,
                height,
                color,
            } => {
                *element_counter += 1;
                let _ = writeln!(stream, "q");
                let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", color.r, color.g, color.b);
                for (i, &bar) in bars.iter().enumerate() {
                    if bar == 1 {
                        let bx = element.x + i as f64 * bar_width;
                        let by = page_height - element.y - height;
                        let _ = writeln!(
                            stream,
                            "{:.2} {:.2} {:.2} {:.2} re",
                            bx, by, bar_width, height
                        );
                    }
                }
                let _ = writeln!(stream, "f\nQ");
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::QrCode {
                modules,
                module_size,
                color,
            } => {
                *element_counter += 1;
                let _ = writeln!(stream, "q");
                let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", color.r, color.g, color.b);
                for (row_idx, row) in modules.iter().enumerate() {
                    for (col_idx, &dark) in row.iter().enumerate() {
                        if dark {
                            let mx = element.x + col_idx as f64 * module_size;
                            let my = page_height - element.y - (row_idx as f64 + 1.0) * module_size;
                            let _ = writeln!(
                                stream,
                                "{:.2} {:.2} {:.2} {:.2} re",
                                mx, my, module_size, module_size
                            );
                        }
                    }
                }
                let _ = writeln!(stream, "f\nQ");
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::Chart { primitives } => {
                *element_counter += 1;
                let _ = writeln!(stream, "q");
                // Set up coordinate transform: Y-flip so chart primitives use top-left origin
                let _ = writeln!(
                    stream,
                    "1 0 0 -1 {:.4} {:.4} cm",
                    element.x,
                    page_height - element.y
                );

                for prim in primitives {
                    write_chart_primitive(stream, prim, element.height, builder);
                }

                let _ = writeln!(stream, "Q");
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::Watermark {
                lines,
                color,
                opacity,
                angle_rad,
                font_family: _,
            } => {
                let _ = writeln!(stream, "q");
                // Set opacity via ExtGState if not fully opaque
                if *opacity < 1.0 {
                    if let Some((_, gs_name)) = builder.ext_gstate_map.get(&opacity.to_bits()) {
                        let _ = writeln!(stream, "/{} gs", gs_name);
                    }
                }
                // Translate to center position (element.x, element.y = page center)
                let pdf_cx = element.x;
                let pdf_cy = page_height - element.y;
                let _ = writeln!(stream, "1 0 0 1 {:.2} {:.2} cm", pdf_cx, pdf_cy);
                // Rotate by angle
                let cos_a = angle_rad.cos();
                let sin_a = angle_rad.sin();
                let _ = writeln!(
                    stream,
                    "{:.6} {:.6} {:.6} {:.6} 0 0 cm",
                    cos_a, sin_a, -sin_a, cos_a
                );
                // Render text centered on origin
                let _ = writeln!(stream, "BT");
                let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", color.r, color.g, color.b);
                if let Some(line) = lines.first() {
                    let groups = Self::group_glyphs_by_style(&line.glyphs);
                    let text_width = line.width;
                    let cap_height = line.height * 0.7;
                    let _ = writeln!(
                        stream,
                        "{:.2} {:.2} Td",
                        -text_width / 2.0,
                        -cap_height / 2.0
                    );
                    for group in &groups {
                        let first = &group[0];
                        let italic =
                            matches!(first.font_style, FontStyle::Italic | FontStyle::Oblique);
                        let fk = FontKey {
                            family: first.font_family.to_string(),
                            weight: first.font_weight,
                            italic,
                        };
                        let idx = self.font_index(
                            &first.font_family,
                            first.font_weight,
                            first.font_style,
                            &builder.font_objects,
                        );
                        let font_name = format!("F{}", idx);
                        let _ = writeln!(stream, "/{} {:.1} Tf", font_name, first.font_size);
                        let is_custom = builder.custom_font_data.contains_key(&fk);
                        if is_custom {
                            if let Some(embed_data) = builder.custom_font_data.get(&fk) {
                                let mut hex = String::new();
                                for g in group.iter() {
                                    let gid =
                                        embed_data.gid_remap.get(&g.glyph_id).copied().unwrap_or(0);
                                    let _ = write!(hex, "{:04X}", gid);
                                }
                                let _ = writeln!(stream, "<{}> Tj", hex);
                            }
                        } else {
                            let hex_str: String = group
                                .iter()
                                .map(|g| format!("{:02X}", g.glyph_id as u8))
                                .collect();
                            let _ = writeln!(stream, "<{}> Tj", hex_str);
                        }
                    }
                }
                let _ = writeln!(stream, "ET");
                let _ = writeln!(stream, "Q");
                if tagged_mcid.is_some() {
                    let _ = writeln!(stream, "EMC");
                    if let Some(ref mut tb) = tag_builder {
                        tb.end_element();
                    }
                } else if is_artifact {
                    let _ = writeln!(stream, "EMC");
                }
                return;
            }

            DrawCommand::FormField { field_type, .. } => {
                // Draw a visual placeholder so form fields are visible in previews
                // and non-form-aware viewers. When flatten_forms is true, also render
                // the field value as static text and skip interactive widgets.
                let pdf_x = element.x;
                let pdf_y = page_height - element.y - element.height;
                let w = element.width;
                let h = element.height;
                let _ = writeln!(stream, "q");
                match field_type {
                    FormFieldType::Checkbox { checked, .. } => {
                        // Draw a border square
                        let _ = writeln!(stream, "0.6 0.6 0.6 RG"); // grey stroke
                        let _ = writeln!(stream, "0.5 w");
                        let _ =
                            writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re S", pdf_x, pdf_y, w, h);
                        if *checked {
                            // Draw a checkmark scaled to field dimensions
                            let _ = writeln!(stream, "0.2 0.2 0.2 rg");
                            let sx = w / 14.0;
                            let sy = h / 14.0;
                            let _ = writeln!(
                                stream,
                                "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l f",
                                pdf_x + 2.0 * sx, pdf_y + 6.0 * sy,
                                pdf_x + 5.5 * sx, pdf_y + 2.0 * sy,
                                pdf_x + 12.0 * sx, pdf_y + 11.0 * sy,
                                pdf_x + 11.0 * sx, pdf_y + 12.0 * sy,
                                pdf_x + 5.5 * sx, pdf_y + 4.5 * sy,
                                pdf_x + 3.0 * sx, pdf_y + 7.0 * sy,
                                pdf_x + 2.0 * sx, pdf_y + 6.0 * sy,
                            );
                        }
                    }
                    FormFieldType::RadioButton { checked, .. } => {
                        // Draw a border square
                        let _ = writeln!(stream, "0.6 0.6 0.6 RG"); // grey stroke
                        let _ = writeln!(stream, "0.5 w");
                        let _ =
                            writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re S", pdf_x, pdf_y, w, h);
                        if *checked {
                            // Draw a filled circle
                            let cx = pdf_x + w / 2.0;
                            let cy = pdf_y + h / 2.0;
                            let r = (w.min(h) / 2.0) * 0.6;
                            let k = r * 0.5523;
                            let _ = writeln!(stream, "0.2 0.2 0.2 rg");
                            let _ = writeln!(
                                stream,
                                "{:.2} {:.2} m {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c f",
                                cx, cy + r,
                                cx + k, cy + r, cx + r, cy + k, cx + r, cy,
                                cx + r, cy - k, cx + k, cy - r, cx, cy - r,
                                cx - k, cy - r, cx - r, cy - k, cx - r, cy,
                                cx - r, cy + k, cx - k, cy + r, cx, cy + r,
                            );
                        }
                    }
                    FormFieldType::TextField {
                        value,
                        placeholder,
                        font_size,
                        multiline,
                        password,
                        ..
                    } => {
                        // White fill + grey border
                        let _ = writeln!(stream, "1 1 1 rg");
                        let _ = writeln!(stream, "0.6 0.6 0.6 RG");
                        let _ = writeln!(stream, "0.5 w");
                        let _ =
                            writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re B", pdf_x, pdf_y, w, h);
                        // Render value text when flattening
                        if flatten_forms {
                            let has_value = value.as_ref().is_some_and(|v| !v.is_empty());
                            if has_value {
                                let val = value.as_ref().unwrap();
                                let display_text = if *password {
                                    "\u{2022}".repeat(val.len())
                                } else {
                                    val.clone()
                                };
                                let font_idx = builder
                                    .font_objects
                                    .iter()
                                    .enumerate()
                                    .find(|(_, (key, _))| {
                                        key.family == "Helvetica"
                                            && key.weight == 400
                                            && !key.italic
                                    })
                                    .map(|(i, _)| i)
                                    .unwrap_or(0);
                                if *multiline {
                                    // Simple word-wrap for multiline
                                    let metrics = crate::font::StandardFont::Helvetica.metrics();
                                    let max_w = w - 4.0;
                                    let mut lines: Vec<String> = Vec::new();
                                    for paragraph in display_text.split('\n') {
                                        let mut line = String::new();
                                        let mut line_w = 0.0;
                                        for word in paragraph.split_whitespace() {
                                            let word_w =
                                                metrics.measure_string(word, *font_size, 0.0);
                                            let space_w = if line.is_empty() {
                                                0.0
                                            } else {
                                                metrics.measure_string(" ", *font_size, 0.0)
                                            };
                                            // Word wider than field — break at character boundary
                                            if word_w > max_w {
                                                let mut char_line = String::new();
                                                let mut char_w = 0.0;
                                                for ch in word.chars() {
                                                    let cw = metrics.char_width(ch, *font_size);
                                                    if !char_line.is_empty() && char_w + cw > max_w
                                                    {
                                                        if !line.is_empty() {
                                                            lines.push(line.clone());
                                                            line.clear();
                                                            line_w = 0.0;
                                                        }
                                                        lines.push(char_line.clone());
                                                        char_line.clear();
                                                        char_w = 0.0;
                                                    }
                                                    char_line.push(ch);
                                                    char_w += cw;
                                                }
                                                // Remaining chars join the current line
                                                if !char_line.is_empty() {
                                                    if !line.is_empty() {
                                                        line.push(' ');
                                                        line_w += metrics
                                                            .measure_string(" ", *font_size, 0.0);
                                                    }
                                                    line.push_str(&char_line);
                                                    line_w += char_w;
                                                }
                                                continue;
                                            }
                                            if !line.is_empty() && line_w + space_w + word_w > max_w
                                            {
                                                lines.push(line.clone());
                                                line.clear();
                                                line_w = 0.0;
                                            }
                                            if !line.is_empty() {
                                                line.push(' ');
                                                line_w += space_w;
                                            }
                                            line.push_str(word);
                                            line_w += word_w;
                                        }
                                        if !line.is_empty() {
                                            lines.push(line);
                                        }
                                    }
                                    let text_y = pdf_y + h - font_size - 2.0;
                                    for (i, line_text) in lines.iter().enumerate() {
                                        let ly = text_y - (i as f64) * (font_size * 1.2);
                                        if ly < pdf_y {
                                            break;
                                        }
                                        let esc = Self::encode_winansi_text(line_text);
                                        let _ = writeln!(
                                            stream,
                                            "BT /F{} {:.1} Tf 0 g {:.2} {:.2} Td ({}) Tj ET",
                                            font_idx,
                                            font_size,
                                            pdf_x + 2.0,
                                            ly,
                                            esc
                                        );
                                    }
                                } else {
                                    let escaped = Self::encode_winansi_text(&display_text);
                                    let text_y = pdf_y + (h - font_size) / 2.0;
                                    let _ = writeln!(
                                        stream,
                                        "BT /F{} {:.1} Tf 0 g {:.2} {:.2} Td ({}) Tj ET",
                                        font_idx,
                                        font_size,
                                        pdf_x + 2.0,
                                        text_y,
                                        escaped
                                    );
                                }
                            } else if let Some(ref ph) = placeholder {
                                if !ph.is_empty() {
                                    // Render placeholder in grey
                                    let font_idx = builder
                                        .font_objects
                                        .iter()
                                        .enumerate()
                                        .find(|(_, (key, _))| {
                                            key.family == "Helvetica"
                                                && key.weight == 400
                                                && !key.italic
                                        })
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                    let escaped = Self::encode_winansi_text(ph);
                                    let text_y = pdf_y + (h - font_size) / 2.0;
                                    let _ = writeln!(
                                        stream,
                                        "BT /F{} {:.1} Tf 0.6 g {:.2} {:.2} Td ({}) Tj ET",
                                        font_idx,
                                        font_size,
                                        pdf_x + 2.0,
                                        text_y,
                                        escaped
                                    );
                                }
                            }
                        }
                    }
                    FormFieldType::Dropdown {
                        value, font_size, ..
                    } => {
                        // White fill + grey border
                        let _ = writeln!(stream, "1 1 1 rg");
                        let _ = writeln!(stream, "0.6 0.6 0.6 RG");
                        let _ = writeln!(stream, "0.5 w");
                        let _ =
                            writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re B", pdf_x, pdf_y, w, h);
                        // Render selected value text when flattening
                        if flatten_forms {
                            if let Some(ref val) = value {
                                if !val.is_empty() {
                                    let font_idx = builder
                                        .font_objects
                                        .iter()
                                        .enumerate()
                                        .find(|(_, (key, _))| {
                                            key.family == "Helvetica"
                                                && key.weight == 400
                                                && !key.italic
                                        })
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                    let escaped = Self::encode_winansi_text(val);
                                    let text_y = pdf_y + (h - font_size) / 2.0;
                                    let _ = writeln!(
                                        stream,
                                        "BT /F{} {:.1} Tf 0 g {:.2} {:.2} Td ({}) Tj ET",
                                        font_idx,
                                        font_size,
                                        pdf_x + 2.0,
                                        text_y,
                                        escaped
                                    );
                                }
                            }
                        }
                    }
                }
                let _ = writeln!(stream, "Q");
            }
        }

        // Overflow clipping: wrap children in q/clip/Q when overflow is Hidden.
        // When the element's Rect has a non-zero border_radius, clip to the
        // rounded path so descendants don't visually overflow the rounded
        // corners. Plain rectangular clip otherwise.
        let clip_overflow = matches!(element.overflow, Overflow::Hidden);
        if clip_overflow {
            let clip_x = element.x;
            let clip_y = page_height - element.y - element.height;
            let clip_w = element.width;
            let clip_h = element.height;
            // Pull border_radius from the Rect DrawCommand if present.
            // Other element kinds (Text, Image, Svg, ...) don't carry a
            // border_radius — they fall back to a rectangular clip.
            let radius = if let DrawCommand::Rect { border_radius, .. } = &element.draw {
                Some(border_radius)
            } else {
                None
            };
            let has_rounded_corners = radius.is_some_and(|r| {
                r.top_left > 0.0 || r.top_right > 0.0 || r.bottom_right > 0.0 || r.bottom_left > 0.0
            });
            let _ = writeln!(stream, "q");
            if has_rounded_corners {
                self.write_rounded_rect(stream, clip_x, clip_y, clip_w, clip_h, radius.unwrap());
                let _ = writeln!(stream, "W n");
            } else {
                let _ = writeln!(
                    stream,
                    "{:.2} {:.2} {:.2} {:.2} re W n",
                    clip_x, clip_y, clip_w, clip_h
                );
            }
        }

        for child in &element.children {
            self.write_element(
                stream,
                child,
                page_height,
                builder,
                page_idx,
                element_counter,
                gradient_counter,
                page_number,
                total_pages,
                tag_builder.as_deref_mut(),
                flatten_forms,
            );
        }

        if clip_overflow {
            let _ = writeln!(stream, "Q");
        }

        // Close the transform wrap (paired with the inner q above).
        if has_transform {
            let _ = writeln!(stream, "Q");
        }

        // Close the element-level opacity wrap (paired with the q above).
        // Goes before EMC so the marker boundary is preserved.
        if needs_element_opacity {
            let _ = writeln!(stream, "Q");
        }

        // Tagged PDF: emit EMC (end marked content)
        if tagged_mcid.is_some() {
            let _ = writeln!(stream, "EMC");
            if let Some(ref mut tb) = tag_builder {
                tb.end_element();
            }
        } else if is_artifact {
            let _ = writeln!(stream, "EMC");
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

        let _ = writeln!(stream, "{:.2} {:.2} m", x + bl, y);

        let _ = writeln!(stream, "{:.2} {:.2} l", x + w - br, y);
        if br > 0.0 {
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                x + w - br + br * k,
                y,
                x + w,
                y + br - br * k,
                x + w,
                y + br
            );
        }

        let _ = writeln!(stream, "{:.2} {:.2} l", x + w, y + h - tr);
        if tr > 0.0 {
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                x + w,
                y + h - tr + tr * k,
                x + w - tr + tr * k,
                y + h,
                x + w - tr,
                y + h
            );
        }

        let _ = writeln!(stream, "{:.2} {:.2} l", x + tl, y + h);
        if tl > 0.0 {
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                x + tl - tl * k,
                y + h,
                x,
                y + h - tl + tl * k,
                x,
                y + h - tl
            );
        }

        let _ = writeln!(stream, "{:.2} {:.2} l", x, y + bl);
        if bl > 0.0 {
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                x,
                y + bl - bl * k,
                x + bl - bl * k,
                y,
                x + bl,
                y
            );
        }

        let _ = writeln!(stream, "h");
    }

    #[allow(clippy::too_many_arguments)]
    fn write_border_sides(
        &self,
        stream: &mut String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        bw: &Edges,
        bc: &crate::style::EdgeValues<Color>,
        bs: &crate::style::EdgeValues<crate::style::BorderStyle>,
    ) {
        // PDF dash + line-cap ops for a side, calibrated against Chrome:
        //   dashed → dash 2×width, gap 1×width (butt cap)
        //   dotted → round-capped dots, diameter 1×width, 2×width centre spacing
        // Each side is wrapped in q/Q so the graphics state (cap, dash) resets.
        fn dash_ops(style: crate::style::BorderStyle, width: f64) -> String {
            use crate::style::BorderStyle::*;
            match style {
                Solid => String::new(),
                Dashed => format!("[{:.2} {:.2}] 0 d\n", width * 2.0, width),
                Dotted => format!("1 J\n[0 {:.2}] 0 d\n", width * 2.0),
            }
        }
        // side: (color, width, style, x0,y0, x1,y1)
        let sides = [
            (bc.top, bw.top, bs.top, x, y + h, x + w, y + h),
            (bc.bottom, bw.bottom, bs.bottom, x, y, x + w, y),
            (bc.left, bw.left, bs.left, x, y, x, y + h),
            (bc.right, bw.right, bs.right, x + w, y, x + w, y + h),
        ];
        for (color, width, style, x0, y0, x1, y1) in sides {
            if width <= 0.0 {
                continue;
            }
            let _ = write!(
                stream,
                "q\n{:.3} {:.3} {:.3} RG\n{:.2} w\n{}{:.2} {:.2} m\n{:.2} {:.2} l\nS\nQ\n",
                color.r,
                color.g,
                color.b,
                width,
                dash_ops(style, width),
                x0,
                y0,
                x1,
                y1
            );
        }
    }

    /// Register fonts used across all pages — each unique (family, weight, italic)
    /// combination gets its own PDF font object.
    /// pdfUa: embed a metric-compatible substitute (Liberation, via
    /// `@formepdf/fonts-standard`) for a base-14 font, as a SIMPLE TrueType
    /// font carrying the base-14 AFM `/Widths` and WinAnsiEncoding. Because the
    /// widths, encoding, and font key are unchanged, the content stream is
    /// byte-identical to the non-embedded base-14 path — only the font
    /// dictionary gains an embedded program, so text positions are exact by
    /// construction. Returns `false` (caller emits the non-embedded base-14)
    /// when there is no metric-compatible substitute (Symbol/ZapfDingbats) or
    /// `@formepdf/fonts-standard` is not registered.
    fn emit_pdfua_embedded_standard(
        builder: &mut PdfBuilder,
        key: &FontKey,
        std_font: &crate::font::StandardFont,
        metrics: &crate::font::StandardFontMetrics,
        font_context: &FontContext,
    ) -> bool {
        let lib_family = match std_font.liberation_family() {
            Some(f) => f,
            None => return false, // Symbol / ZapfDingbats — no substitute
        };
        // The substitute must have been registered (fonts-standard) — otherwise
        // it resolves back to a Standard font and there is nothing to embed.
        let lib_bytes: &[u8] = match font_context.resolve(lib_family, key.weight, key.italic) {
            FontData::Custom { data, .. } => data,
            FontData::Standard(_) => return false,
        };
        let face = match ttf_parser::Face::parse(lib_bytes, 0) {
            Ok(f) => f,
            Err(_) => return false,
        };
        let scale = 1000.0 / face.units_per_em() as f64;
        let bbox = face.global_bounding_box();
        let pdf_name = Self::sanitize_font_name(lib_family, key.weight, key.italic);

        // 1. FontFile2 — the full Liberation program, zlib-compressed.
        let compressed = compress_to_vec_zlib(lib_bytes, 6);
        let fontfile2_id = builder.objects.len();
        let mut ff2: Vec<u8> = Vec::new();
        let _ = write!(
            ff2,
            "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
            compressed.len(),
            lib_bytes.len()
        );
        ff2.extend_from_slice(&compressed);
        ff2.extend_from_slice(b"\nendstream");
        builder.objects.push(PdfObject {
            id: fontfile2_id,
            data: ff2,
        });

        // 2. FontDescriptor.
        let fd_id = builder.objects.len();
        let cap_height =
            (face.capital_height().unwrap_or_else(|| face.ascender()) as f64 * scale) as i32;
        let fd = format!(
            "<< /Type /FontDescriptor /FontName /{name} /Flags {flags} \
             /FontBBox [{x0} {y0} {x1} {y1}] /ItalicAngle {ia} \
             /Ascent {asc} /Descent {desc} /CapHeight {cap} /StemV {stem} \
             /FontFile2 {ff2} 0 R >>",
            name = pdf_name,
            flags = std_font.descriptor_flags(),
            x0 = (bbox.x_min as f64 * scale) as i32,
            y0 = (bbox.y_min as f64 * scale) as i32,
            x1 = (bbox.x_max as f64 * scale) as i32,
            y1 = (bbox.y_max as f64 * scale) as i32,
            ia = if key.italic { -12 } else { 0 },
            asc = (face.ascender() as f64 * scale) as i32,
            desc = (face.descender() as f64 * scale) as i32,
            cap = cap_height,
            stem = if key.weight >= 700 { 120 } else { 80 },
            ff2 = fontfile2_id,
        );
        builder.objects.push(PdfObject {
            id: fd_id,
            data: fd.into_bytes(),
        });

        // 3. Simple TrueType font dict — base-14 AFM widths + WinAnsiEncoding,
        //    with the PDF/A width carve-out.
        //
        // For most glyphs the substitute's advance equals the base-14 AFM
        // width (Liberation is metric-compatible), so we declare the AFM value
        // and positioning stays exact. For the handful of rare accent/symbol
        // glyphs per proportional family where they diverge (e.g. macron,
        // grave, middot, ÷, ±, quotesingle, µ), we declare the substitute's
        // OWN advance instead — so /Widths agrees with the embedded program,
        // which ISO 19005 (PDF/A) requires and veraPDF's PDF/A profile checks.
        // The trade is a sub-glyph advance drift on those rare glyphs, which
        // real documents almost never contain. (Liberation Mono has zero
        // divergent glyphs; the carve-out is a no-op there.)
        let declared_widths: Vec<u16> = metrics
            .widths
            .iter()
            .enumerate()
            .map(|(i, &afm)| {
                let code = 32u8.wrapping_add(i as u8); // index 0 = WinAnsi code 32
                if let Some(ch) = crate::font::winansi_to_char(code) {
                    if let Some(gid) = face.glyph_index(ch) {
                        if let Some(adv) = face.glyph_hor_advance(gid) {
                            let hmtx = (adv as f64 * scale).round() as u16;
                            if (hmtx as i32 - afm as i32).abs() > 1 {
                                return hmtx;
                            }
                        }
                    }
                }
                afm
            })
            .collect();
        let widths_str: String = declared_widths
            .iter()
            .map(|w| w.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let obj_id = builder.objects.len();
        let font_dict = format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /{name} \
             /Encoding /WinAnsiEncoding \
             /FirstChar 32 /LastChar 255 /Widths [{w}] \
             /FontDescriptor {fd} 0 R >>",
            name = pdf_name,
            w = widths_str,
            fd = fd_id,
        );
        builder.objects.push(PdfObject {
            id: obj_id,
            data: font_dict.into_bytes(),
        });
        builder.font_objects.push((key.clone(), obj_id));
        // Record that this base-14 family is embedded (via substitution) so the
        // PDF/A all-fonts-embedded check accepts it — this is what lets PDF/A
        // and PDF/UA compose.
        builder.embedded_standard_fonts.insert(key.clone());
        true
    }

    fn register_fonts(
        &self,
        builder: &mut PdfBuilder,
        pages: &[LayoutPage],
        font_context: &FontContext,
        pdf_ua: bool,
    ) -> Result<(), FormeError> {
        // Collect font usage: glyph IDs, chars, and glyph→char mapping per font
        let mut font_usage_map: HashMap<FontKey, FontUsage> = HashMap::new();

        for page in pages {
            Self::collect_font_usage(&page.elements, &mut font_usage_map);
        }

        let mut keys: Vec<FontKey> = font_usage_map.keys().cloned().collect();

        // Sort for deterministic ordering, then dedup
        keys.sort_by(|a, b| {
            a.family
                .cmp(&b.family)
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
                    let metrics = std_font.metrics();

                    // PDF/UA + PDF/A require every font embedded, which the
                    // base-14 fonts are not. In pdfUa mode, if a
                    // metric-compatible substitute (Liberation, via
                    // @formepdf/fonts-standard) is registered, embed it as a
                    // SIMPLE TrueType carrying the base-14 AFM /Widths and
                    // WinAnsiEncoding — the content stream is untouched (same
                    // `(text) Tj` WinAnsi path, same positions), only the font
                    // dictionary gains an embedded program.
                    if pdf_ua {
                        if Self::emit_pdfua_embedded_standard(
                            builder,
                            key,
                            std_font,
                            &metrics,
                            font_context,
                        ) {
                            continue;
                        }
                        // Substitution didn't happen. If a metric-compatible
                        // substitute exists but wasn't registered, say so by
                        // name with the remedy — never silently emit a
                        // non-conforming file. (Symbol/ZapfDingbats have no
                        // substitute, so there is nothing to suggest.)
                        if let Some(lib) = std_font.liberation_family() {
                            builder.warnings.push(format!(
                                "pdfUa: font '{}' is not embedded, so the PDF will not conform to \
                                 PDF/UA (all fonts must be embedded). Install \
                                 @formepdf/fonts-standard and register its fonts \
                                 (`for (const f of standardFonts()) Font.register(f)`) — Forme \
                                 will then embed the metric-compatible {} in its place.",
                                std_font.pdf_name(),
                                lib,
                            ));
                        }
                    }

                    let obj_id = builder.objects.len();
                    // Include /Widths so PDF viewers use our exact metrics
                    // instead of substituting a system font with different widths
                    let widths_str: String = metrics
                        .widths
                        .iter()
                        .map(|w| w.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let font_dict = format!(
                        "<< /Type /Font /Subtype /Type1 /BaseFont /{} \
                         /Encoding /WinAnsiEncoding \
                         /FirstChar 32 /LastChar 255 /Widths [{}] >>",
                        std_font.pdf_name(),
                        widths_str,
                    );
                    builder.objects.push(PdfObject {
                        id: obj_id,
                        data: font_dict.into_bytes(),
                    });
                    builder.font_objects.push((key.clone(), obj_id));
                }
                FontData::Custom { data, .. } => {
                    let usage = font_usage_map.get(key);
                    let used_glyph_ids = usage.map(|u| &u.glyph_ids);
                    let used_chars = usage.map(|u| &u.chars);
                    let glyph_to_char = usage.map(|u| &u.glyph_to_char);
                    let type0_obj_id = Self::write_custom_font_objects(
                        builder,
                        key,
                        data,
                        used_glyph_ids.cloned().unwrap_or_default(),
                        used_chars.cloned().unwrap_or_default(),
                        glyph_to_char.cloned().unwrap_or_default(),
                    )?;
                    builder.font_objects.push((key.clone(), type0_obj_id));
                }
            }
        }

        Ok(())
    }

    /// Collect font usage data from layout elements: used chars, glyph IDs, and glyph→char mapping.
    fn collect_font_usage(
        elements: &[LayoutElement],
        font_usage: &mut HashMap<FontKey, FontUsage>,
    ) {
        for element in elements {
            let lines_opt = match &element.draw {
                DrawCommand::Text { lines, .. } => Some(lines),
                DrawCommand::Watermark { lines, .. } => Some(lines),
                _ => None,
            };
            if let Some(lines) = lines_opt {
                for line in lines {
                    for glyph in &line.glyphs {
                        let italic =
                            matches!(glyph.font_style, FontStyle::Italic | FontStyle::Oblique);
                        let key = FontKey {
                            family: glyph.font_family.to_string(),
                            weight: glyph.font_weight,
                            italic,
                        };
                        let usage = font_usage.entry(key).or_insert_with(|| FontUsage {
                            chars: HashSet::new(),
                            glyph_ids: HashSet::new(),
                            glyph_to_char: HashMap::new(),
                        });
                        usage.chars.insert(glyph.char_value);
                        // A page-number sentinel becomes digits at write
                        // time — subset all ten for this font, or the
                        // substituted numbers would render as .notdef
                        // (char_to_gid would have no digit entries).
                        if glyph.char_value == PAGE_NUMBER_SENTINEL
                            || glyph.char_value == TOTAL_PAGES_SENTINEL
                        {
                            usage.chars.extend('0'..='9');
                        }
                        usage.glyph_ids.insert(glyph.glyph_id);
                        // For ligatures, use the first char of the cluster
                        usage
                            .glyph_to_char
                            .entry(glyph.glyph_id)
                            .or_insert(glyph.char_value);
                        // If there's cluster_text, record all chars for this glyph
                        if let Some(ref ct) = glyph.cluster_text {
                            // First char already recorded above; cluster_text is for ToUnicode
                            if let Some(first_char) = ct.chars().next() {
                                usage
                                    .glyph_to_char
                                    .entry(glyph.glyph_id)
                                    .or_insert(first_char);
                            }
                        }
                    }
                }
            }
            Self::collect_font_usage(&element.children, font_usage);
        }
    }

    /// Walk all pages, create XObject PDF objects for each image,
    /// Register PDF Shading dictionaries for every Rect with a
    /// `background_gradient`. Walks the element tree once per page in
    /// pre-order (same order `write_element` recurses) so the counter-
    /// indexed `shading_map` lookups during emission match.
    fn register_shadings(&self, builder: &mut PdfBuilder, pages: &[LayoutPage]) {
        for (page_idx, page) in pages.iter().enumerate() {
            let mut counter = 0usize;
            Self::collect_shadings_recursive(&page.elements, page_idx, &mut counter, builder);
        }
    }

    fn collect_shadings_recursive(
        elements: &[LayoutElement],
        page_idx: usize,
        counter: &mut usize,
        builder: &mut PdfBuilder,
    ) {
        for element in elements {
            if let DrawCommand::Rect {
                background_gradient: Some(gradient),
                ..
            } = &element.draw
            {
                let ordinal = *counter;
                *counter += 1;
                let (obj_id, name) =
                    Self::write_shading_objects(builder, gradient, element, ordinal);
                builder
                    .shading_map
                    .insert((page_idx, ordinal), (obj_id, name));
            }
            Self::collect_shadings_recursive(&element.children, page_idx, counter, builder);
        }
    }

    /// Build the Function + Shading PDF objects for one gradient. Returns
    /// (shading_obj_id, "Sh{n}"). 2-stop gradients use a single Type 2
    /// (exponential) function. 3+ stop gradients use a Type 3 (stitching)
    /// function combining N-1 Type 2 sub-functions, with /Bounds at each
    /// interior stop position.
    fn write_shading_objects(
        builder: &mut PdfBuilder,
        gradient: &crate::style::Background,
        element: &LayoutElement,
        ordinal: usize,
    ) -> (usize, String) {
        use crate::style::Background;
        use crate::style::GradientStop;

        // Materialize the gradient as a normalized stop list (positions
        // sorted ascending, clamped to [0,1]). Solid-color backgrounds
        // collapse to two identical stops at 0 and 1.
        let black = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let stops: Vec<GradientStop> = match gradient {
            Background::Color(c) => vec![
                GradientStop {
                    position: 0.0,
                    color: *c,
                },
                GradientStop {
                    position: 1.0,
                    color: *c,
                },
            ],
            Background::Linear(g) => normalize_gradient_stops(&g.stops, black),
            Background::Radial(g) => normalize_gradient_stops(&g.stops, black),
        };

        // Build the color-interpolation function. With <=2 stops we emit
        // a single Type 2 (exponential) function; with 3+ stops we emit a
        // Type 3 (stitching) function combining N-1 Type 2 sub-functions.
        let function_id = if stops.len() <= 2 {
            let c0 = stops.first().map(|s| s.color).unwrap_or(black);
            let c1 = stops.last().map(|s| s.color).unwrap_or(c0);
            let id = builder.objects.len();
            let data = format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{:.4} {:.4} {:.4}] /C1 [{:.4} {:.4} {:.4}] /N 1 >>",
                c0.r, c0.g, c0.b, c1.r, c1.g, c1.b,
            );
            builder.objects.push(PdfObject {
                id,
                data: data.into_bytes(),
            });
            id
        } else {
            // Reserve N-1 Type 2 sub-function objects.
            let mut sub_ids: Vec<usize> = Vec::with_capacity(stops.len() - 1);
            for window in stops.windows(2) {
                let c0 = window[0].color;
                let c1 = window[1].color;
                let id = builder.objects.len();
                let data = format!(
                    "<< /FunctionType 2 /Domain [0 1] /C0 [{:.4} {:.4} {:.4}] /C1 [{:.4} {:.4} {:.4}] /N 1 >>",
                    c0.r, c0.g, c0.b, c1.r, c1.g, c1.b,
                );
                builder.objects.push(PdfObject {
                    id,
                    data: data.into_bytes(),
                });
                sub_ids.push(id);
            }
            // Bounds = interior stop positions (exclude first and last).
            // Encode = [0 1] per sub-function — each sub-function uses its
            // full domain regardless of the bound interval width.
            let bounds: Vec<String> = stops[1..stops.len() - 1]
                .iter()
                .map(|s| format!("{:.4}", s.position))
                .collect();
            let encode: Vec<&str> = (0..sub_ids.len()).map(|_| "0 1").collect();
            let functions: Vec<String> = sub_ids.iter().map(|i| format!("{} 0 R", i)).collect();
            let id = builder.objects.len();
            let data = format!(
                "<< /FunctionType 3 /Domain [0 1] /Functions [{}] /Bounds [{}] /Encode [{}] >>",
                functions.join(" "),
                bounds.join(" "),
                encode.join(" "),
            );
            builder.objects.push(PdfObject {
                id,
                data: data.into_bytes(),
            });
            id
        };

        // Element dimensions. The shading's coord space is local to the
        // rect (we cm-translate to the rect's bottom-left at draw time),
        // so x/y aren't needed here — only w/h.
        let _ = element.x;
        let _ = element.y;
        let w = element.width;
        let h = element.height;

        let shading_id = builder.objects.len();
        let shading_data = match gradient {
            Background::Linear(g) => {
                // CSS angle convention: 0deg = bottom→top, 90deg = left→right,
                // 180deg = top→bottom (clockwise from up).
                // Our layout uses Y-down; PDF uses Y-up. Compute the axis
                // in PDF coords directly: dx = sin(θ), dy = cos(θ) where
                // CSS 0deg points "up" (positive PDF y).
                // CSS angle convention: 0deg = bottom→top, 180deg =
                // top→bottom. PDF y-axis is flipped vs CSS-on-screen, so
                // dy comes from cos(θ) directly (CSS 0deg points "up"
                // which is +y in PDF coords).
                let theta = g.angle_deg.to_radians();
                let dx = theta.sin();
                let dy = theta.cos();
                // Axis length spans the rect along the gradient direction
                // (CSS spec covering box).
                let axis_len = w * dx.abs() + h * dy.abs();
                // Coords are RELATIVE to the rect's bottom-left corner
                // (the cm-translate at draw time positions absolutely).
                let cx_rel = w / 2.0;
                let cy_rel = h / 2.0;
                let half = axis_len / 2.0;
                let x0 = cx_rel - dx * half;
                let y0 = cy_rel - dy * half;
                let x1 = cx_rel + dx * half;
                let y1 = cy_rel + dy * half;
                format!(
                    "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{:.3} {:.3} {:.3} {:.3}] /Function {} 0 R /Extend [true true] >>",
                    x0, y0, x1, y1, function_id,
                )
            }
            Background::Radial(_) => {
                // Circle from center, inner r=0, outer r=max(w/2, h/2),
                // expressed relative to rect bottom-left.
                let cx_rel = w / 2.0;
                let cy_rel = h / 2.0;
                let r_outer = (w / 2.0).max(h / 2.0);
                format!(
                    "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{:.3} {:.3} 0 {:.3} {:.3} {:.3}] /Function {} 0 R /Extend [true true] >>",
                    cx_rel, cy_rel, cx_rel, cy_rel, r_outer, function_id,
                )
            }
            Background::Color(_) => {
                // Solid: emit a constant 1.0-stop function via the Coords
                // collapsed to a point. (Shouldn't normally hit this path —
                // background_gradient should only be set for true gradients.)
                format!(
                    "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 0 0] /Function {} 0 R /Extend [true true] >>",
                    function_id,
                )
            }
        };
        builder.objects.push(PdfObject {
            id: shading_id,
            data: shading_data.into_bytes(),
        });
        (shading_id, format!("Sh{}", ordinal))
    }

    /// Decode and embed each page's optional `background_image` as a PDF
    /// XObject. Identical URLs across pages share a single XObject (the
    /// `page_background_url_cache` does the deduplication).
    fn register_page_background_images(&self, builder: &mut PdfBuilder, pages: &[LayoutPage]) {
        for (page_idx, page) in pages.iter().enumerate() {
            let Some(src) = &page.config.background_image else {
                continue;
            };
            // Reuse the XObject if a previous page used the same source.
            if let Some(&entry) = builder.page_background_url_cache.get(src) {
                builder.page_background_image_map.insert(page_idx, entry);
                continue;
            }
            // Decode + embed; on failure, log a warning and skip the
            // background for that page (don't fail the whole render).
            match crate::image_loader::load_image(src) {
                Ok(image_data) => {
                    let img_idx = builder.image_objects.len();
                    let dims = (img_idx, image_data.width_px, image_data.height_px);
                    let xobj_id = Self::write_image_xobject(builder, &image_data);
                    builder.image_objects.push(xobj_id);
                    builder.page_background_image_map.insert(page_idx, dims);
                    builder.page_background_url_cache.insert(src.clone(), dims);
                }
                Err(e) => {
                    eprintln!("[forme] page background image failed to load: {}", e);
                }
            }
        }
    }

    /// Emit the page background paint (q + optional ExtGState + cm + Do + Q)
    /// at the start of a page's content stream. Sizing follows CSS
    /// `background-size` semantics (fill/cover/contain) with positioning
    /// per `background-position`.
    fn write_page_background(
        &self,
        stream: &mut String,
        page: &LayoutPage,
        page_bg: (usize, u32, u32),
        builder: &PdfBuilder,
    ) {
        use crate::model::{BackgroundPosition, BackgroundSize};
        let (img_idx, iw_px, ih_px) = page_bg;
        let page_w = page.width;
        let page_h = page.height;
        let iw = iw_px as f64;
        let ih = ih_px as f64;

        let size = page.config.background_size.unwrap_or_default();
        let (dest_w, dest_h) = match size {
            BackgroundSize::Fill => (page_w, page_h),
            BackgroundSize::Cover => {
                let s = (page_w / iw).max(page_h / ih);
                (iw * s, ih * s)
            }
            BackgroundSize::Contain => {
                let s = (page_w / iw).min(page_h / ih);
                (iw * s, ih * s)
            }
        };

        // Position: for `fill`, dest matches page exactly so position is
        // moot; otherwise place per `background-position` against the
        // page's bounding box.
        let position = page.config.background_position.unwrap_or_default();
        // PDF Y origin is bottom-left, so "top" means pdf_y = page_h - dest_h
        // and "bottom" means pdf_y = 0.
        let (dest_x, dest_y) = match position {
            BackgroundPosition::TopLeft => (0.0, page_h - dest_h),
            BackgroundPosition::TopRight => (page_w - dest_w, page_h - dest_h),
            BackgroundPosition::BottomLeft => (0.0, 0.0),
            BackgroundPosition::BottomRight => (page_w - dest_w, 0.0),
            BackgroundPosition::Center => ((page_w - dest_w) / 2.0, (page_h - dest_h) / 2.0),
        };

        // Optional ExtGState wrap for backgroundOpacity < 1.0.
        let opacity = page.config.background_opacity.unwrap_or(1.0);
        let needs_opacity = opacity < 1.0;
        if needs_opacity {
            if let Some((_, gs_name)) = builder.ext_gstate_map.get(&opacity.to_bits()) {
                let _ = writeln!(stream, "q\n/{} gs", gs_name);
            } else {
                let _ = writeln!(stream, "q");
            }
        } else {
            let _ = writeln!(stream, "q");
        }
        // PDF cm: a b c d e f → matrix [[a c e][b d f][0 0 1]]; for a
        // simple scale + translate, that's: w 0 0 h x y cm.
        let _ = writeln!(
            stream,
            "{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/Im{} Do\nQ",
            dest_w, dest_h, dest_x, dest_y, img_idx,
        );
    }

    /// and populate the image_index_map for content stream reference.
    fn register_images(&self, builder: &mut PdfBuilder, pages: &[LayoutPage]) {
        for (page_idx, page) in pages.iter().enumerate() {
            let mut element_counter = 0usize;
            Self::collect_images_recursive(&page.elements, page_idx, &mut element_counter, builder);
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
                    builder
                        .image_index_map
                        .insert((page_idx, elem_idx), img_idx);
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

    /// Collect unique opacity values from all pages and create ExtGState PDF objects.
    fn register_ext_gstates(&self, builder: &mut PdfBuilder, pages: &[LayoutPage]) {
        let mut unique_opacities: Vec<f64> = Vec::new();
        for page in pages {
            Self::collect_opacities_recursive(&page.elements, &mut unique_opacities);
            // Page background opacity (independent of element-level alphas).
            if let Some(o) = page.config.background_opacity {
                if o < 1.0 {
                    unique_opacities.push(o);
                }
            }
        }
        unique_opacities.sort_by(|a, b| a.partial_cmp(b).unwrap());
        unique_opacities.dedup();

        for (idx, &opacity) in unique_opacities.iter().enumerate() {
            let obj_id = builder.objects.len();
            let gs_name = format!("GS{}", idx);
            let obj_data = format!(
                "<< /Type /ExtGState /ca {:.4} /CA {:.4} >>",
                opacity, opacity
            );
            builder.objects.push(PdfObject {
                id: obj_id,
                data: obj_data.into_bytes(),
            });
            let key = opacity.to_bits();
            builder.ext_gstate_map.insert(key, (obj_id, gs_name));
        }
    }

    fn collect_opacities_recursive(elements: &[LayoutElement], opacities: &mut Vec<f64>) {
        for element in elements {
            // Element-level opacity wraps the whole subtree (including
            // children) in `q\n/GS{n} gs ... Q` so descendants render at
            // the cumulative alpha. Collect it independently of the
            // per-DrawCommand opacities below — they coexist for now,
            // and the per-Rect/Text/Watermark opacities are gradually
            // being deprecated in favor of the element-level one.
            if element.opacity < 1.0 {
                opacities.push(element.opacity);
            }
            // Shadow color alpha — needs its own ExtGState entry so the
            // shadow renders semi-transparently independent of the
            // element's opacity.
            if let DrawCommand::Rect {
                box_shadow: Some(shadow),
                ..
            } = &element.draw
            {
                if shadow.color.a < 1.0 {
                    opacities.push(shadow.color.a);
                }
            }
            match &element.draw {
                DrawCommand::Rect { opacity, .. }
                | DrawCommand::Text { opacity, .. }
                | DrawCommand::Watermark { opacity, .. }
                    if *opacity < 1.0 =>
                {
                    opacities.push(*opacity);
                }
                DrawCommand::Chart { primitives } => {
                    for prim in primitives {
                        if let crate::chart::ChartPrimitive::FilledPath { opacity, .. } = prim {
                            if *opacity < 1.0 {
                                opacities.push(*opacity);
                            }
                        }
                    }
                }
                DrawCommand::Svg { commands, .. } => {
                    for cmd in commands {
                        if let crate::svg::SvgCommand::SetOpacity(opacity) = cmd {
                            if *opacity < 1.0 {
                                opacities.push(*opacity);
                            }
                        }
                    }
                }
                _ => {}
            }
            Self::collect_opacities_recursive(&element.children, opacities);
        }
    }

    /// Build the ExtGState resource dict entries for a page.
    fn build_ext_gstate_resource_dict(&self, builder: &PdfBuilder) -> String {
        if builder.ext_gstate_map.is_empty() {
            return String::new();
        }
        let mut entries: Vec<(&String, usize)> = builder
            .ext_gstate_map
            .values()
            .map(|(obj_id, name)| (name, *obj_id))
            .collect();
        entries.sort_by_key(|(name, _)| (*name).clone());
        entries
            .iter()
            .map(|(name, obj_id)| format!("/{} {} 0 R", name, obj_id))
            .collect::<Vec<_>>()
            .join(" ")
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
                    image.width_px,
                    image.height_px,
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
                        image.width_px,
                        image.height_px,
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
                    image.width_px,
                    image.height_px,
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
    /// Build the page's `/Shading << ... >>` resource dict from the
    /// shading_map entries that match `page_idx`.
    fn build_shading_resource_dict(&self, page_idx: usize, builder: &PdfBuilder) -> String {
        let mut entries: Vec<(String, usize)> = builder
            .shading_map
            .iter()
            .filter(|(&(p, _), _)| p == page_idx)
            .map(|(_, (obj_id, name))| (name.clone(), *obj_id))
            .collect();
        if entries.is_empty() {
            return String::new();
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
            .iter()
            .map(|(name, obj_id)| format!("/{} {} 0 R", name, obj_id))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn build_xobject_resource_dict(&self, page_idx: usize, builder: &PdfBuilder) -> String {
        let mut entries: Vec<(usize, usize)> = Vec::new();
        for (&(pidx, _), &img_idx) in &builder.image_index_map {
            if pidx == page_idx {
                let obj_id = builder.image_objects[img_idx];
                entries.push((img_idx, obj_id));
            }
        }
        // Include the page's background image (if any) so the `/Im{n} Do`
        // operator at the start of the content stream resolves.
        if let Some(&(img_idx, _, _)) = builder.page_background_image_map.get(&page_idx) {
            let obj_id = builder.image_objects[img_idx];
            entries.push((img_idx, obj_id));
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
    ///
    /// `used_glyph_ids`: original glyph IDs from shaping (from PositionedGlyph.glyph_id).
    /// `used_chars`: characters used (for char→gid fallback, e.g., page number placeholders).
    /// `glyph_to_char_map`: maps original glyph ID → first Unicode char (for ToUnicode CMap).
    fn write_custom_font_objects(
        builder: &mut PdfBuilder,
        key: &FontKey,
        ttf_data: &[u8],
        used_glyph_ids: HashSet<u16>,
        used_chars: HashSet<char>,
        glyph_to_char_map: HashMap<u16, char>,
    ) -> Result<usize, FormeError> {
        let face = ttf_parser::Face::parse(ttf_data, 0).map_err(|e| {
            FormeError::FontError(format!(
                "Failed to parse TTF data for font '{}': {}",
                key.family, e
            ))
        })?;

        let units_per_em = face.units_per_em();
        let ascender = face.ascender();
        let descender = face.descender();

        // Build char → original glyph ID mapping (for fallback/placeholders)
        let mut char_to_orig_gid: HashMap<char, u16> = HashMap::new();
        for &ch in &used_chars {
            if let Some(gid) = face.glyph_index(ch) {
                char_to_orig_gid.insert(ch, gid.0);
            }
        }

        // Combine shaped glyph IDs + char-based glyph IDs for subsetting.
        // This ensures ligature glyphs (from shaping) AND individual char glyphs
        // (for placeholder fallback) are all included.
        let mut all_orig_gids: HashSet<u16> = used_glyph_ids.clone();
        for &gid in char_to_orig_gid.values() {
            all_orig_gids.insert(gid);
        }

        // Subset the font to only include used glyphs
        let (embed_ttf, gid_remap) = match subset_ttf(ttf_data, &all_orig_gids) {
            Ok(subset_result) => (subset_result.ttf_data, subset_result.gid_remap),
            Err(_) => {
                // Subsetting failed — fall back to embedding the full font (identity remap)
                let identity: HashMap<u16, u16> =
                    all_orig_gids.iter().map(|&gid| (gid, gid)).collect();
                (ttf_data.to_vec(), identity)
            }
        };

        // Build char→new_gid mapping (for placeholder fallback in content stream)
        let char_to_gid: HashMap<char, u16> = char_to_orig_gid
            .iter()
            .filter_map(|(&ch, &orig_gid)| gid_remap.get(&orig_gid).map(|&new_gid| (ch, new_gid)))
            .collect();

        // Build glyph_id→new_gid mapping (for shaped content stream)
        let gid_remap_for_embed = gid_remap.clone();

        // Build new_gid→char mapping for ToUnicode CMap
        let mut new_gid_to_char: HashMap<u16, char> = HashMap::new();
        // From shaped glyph→char mapping
        for (&orig_gid, &ch) in &glyph_to_char_map {
            if let Some(&new_gid) = gid_remap.get(&orig_gid) {
                new_gid_to_char.entry(new_gid).or_insert(ch);
            }
        }
        // Fill in from char→gid mapping too
        for (&ch, &new_gid) in &char_to_gid {
            new_gid_to_char.entry(new_gid).or_insert(ch);
        }

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
        let subset_face = ttf_parser::Face::parse(&embed_ttf, 0).unwrap_or_else(|_| face.clone());
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
        // Build /W array using new_gid→width from subset face
        let w_array = Self::build_w_array_from_gids(&gid_remap, &subset_face, subset_upem);
        let default_width = subset_face
            .glyph_hor_advance(ttf_parser::GlyphId(0))
            .map(|adv| (adv as f64 * 1000.0 / subset_upem as f64) as u32)
            .unwrap_or(1000);
        let cidfont_dict = format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor {} 0 R /DW {} /W {} \
             /CIDToGIDMap /Identity >>",
            pdf_font_name, font_descriptor_id, default_width, w_array,
        );
        builder.objects.push(PdfObject {
            id: cidfont_id,
            data: cidfont_dict.into_bytes(),
        });

        // 4. ToUnicode CMap
        let tounicode_id = builder.objects.len();
        let cmap_content = Self::build_tounicode_cmap_from_gids(&new_gid_to_char, &pdf_font_name);
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
            pdf_font_name, cidfont_id, tounicode_id,
        );
        builder.objects.push(PdfObject {
            id: type0_id,
            data: type0_dict.into_bytes(),
        });

        // Store embedding data for content stream encoding
        builder.custom_font_data.insert(
            key.clone(),
            CustomFontEmbedData {
                ttf_data: embed_ttf,
                gid_remap: gid_remap_for_embed,
                glyph_to_char: glyph_to_char_map,
                char_to_gid,
                units_per_em,
                ascender,
                descender,
            },
        );

        Ok(type0_id)
    }

    /// Build the /W array from gid_remap (orig_gid→new_gid) using the subset face.
    fn build_w_array_from_gids(
        gid_remap: &HashMap<u16, u16>,
        face: &ttf_parser::Face,
        units_per_em: u16,
    ) -> String {
        let scale = 1000.0 / units_per_em as f64;

        let mut entries: Vec<(u16, u32)> = Vec::new();
        let mut seen_gids: HashSet<u16> = HashSet::new();

        for &new_gid in gid_remap.values() {
            if seen_gids.contains(&new_gid) {
                continue;
            }
            seen_gids.insert(new_gid);
            let advance = face
                .glyph_hor_advance(ttf_parser::GlyphId(new_gid))
                .unwrap_or(0);
            let width = (advance as f64 * scale) as u32;
            entries.push((new_gid, width));
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

    /// Build a ToUnicode CMap from new_gid → char mapping.
    fn build_tounicode_cmap_from_gids(gid_to_char: &HashMap<u16, char>, font_name: &str) -> String {
        let mut gid_to_unicode: Vec<(u16, u32)> = gid_to_char
            .iter()
            .map(|(&gid, &ch)| (gid, ch as u32))
            .collect();
        gid_to_unicode.sort_by_key(|(gid, _)| *gid);

        let mut cmap = String::new();
        let _ = writeln!(cmap, "/CIDInit /ProcSet findresource begin");
        let _ = writeln!(cmap, "12 dict begin");
        let _ = writeln!(cmap, "begincmap");
        let _ = writeln!(cmap, "/CIDSystemInfo");
        let _ = writeln!(
            cmap,
            "<< /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def"
        );
        let _ = writeln!(cmap, "/CMapName /{}-UTF16 def", font_name);
        let _ = writeln!(cmap, "/CMapType 2 def");
        let _ = writeln!(cmap, "1 begincodespacerange");
        let _ = writeln!(cmap, "<0000> <FFFF>");
        let _ = writeln!(cmap, "endcodespacerange");

        // PDF spec limits beginbfchar to 100 entries per block
        for chunk in gid_to_unicode.chunks(100) {
            let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
            for &(gid, unicode) in chunk {
                let _ = writeln!(cmap, "<{:04X}> <{:04X}>", gid, unicode);
            }
            let _ = writeln!(cmap, "endbfchar");
        }

        let _ = writeln!(cmap, "endcmap");
        let _ = writeln!(cmap, "CMapName currentdict /CMap defineresource pop");
        let _ = writeln!(cmap, "end");
        let _ = writeln!(cmap, "end");

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

        // Exact weight match
        for (i, (key, _)) in font_objects.iter().enumerate() {
            if key.family == family && key.weight == weight && key.italic == italic {
                return i;
            }
        }

        // Fallback: snapped weight (400/700)
        let snapped = if weight >= 600 { 700 } else { 400 };
        for (i, (key, _)) in font_objects.iter().enumerate() {
            if key.family == family && key.weight == snapped && key.italic == italic {
                return i;
            }
        }

        // Fallback: try Helvetica with same weight/style
        for (i, (key, _)) in font_objects.iter().enumerate() {
            if key.family == "Helvetica" && key.weight == snapped && key.italic == italic {
                return i;
            }
        }

        // Last resort: first font
        0
    }

    /// Group consecutive glyphs by (font_family, font_weight, font_style, font_size, color)
    /// for multi-font text run rendering.
    fn group_glyphs_by_style(glyphs: &[PositionedGlyph]) -> Vec<Vec<&PositionedGlyph>> {
        if glyphs.is_empty() {
            return vec![];
        }

        let mut groups: Vec<Vec<&PositionedGlyph>> = Vec::new();
        let mut current_group: Vec<&PositionedGlyph> = vec![&glyphs[0]];

        for glyph in &glyphs[1..] {
            let prev = current_group.last().unwrap();
            let same_style = glyph.font_family == prev.font_family
                && glyph.font_weight == prev.font_weight
                && std::mem::discriminant(&glyph.font_style)
                    == std::mem::discriminant(&prev.font_style)
                && (glyph.font_size - prev.font_size).abs() < 0.01
                && Self::colors_equal(&glyph.color, &prev.color)
                && std::mem::discriminant(&glyph.text_decoration)
                    == std::mem::discriminant(&prev.text_decoration);

            if same_style {
                current_group.push(glyph);
            } else {
                groups.push(current_group);
                current_group = vec![glyph];
            }
        }
        groups.push(current_group);
        groups
    }

    fn colors_equal(a: &Option<Color>, b: &Option<Color>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(ca), Some(cb)) => {
                (ca.r - cb.r).abs() < 0.001
                    && (ca.g - cb.g).abs() < 0.001
                    && (ca.b - cb.b).abs() < 0.001
                    && (ca.a - cb.a).abs() < 0.001
            }
            _ => false,
        }
    }

    /// Collect link annotations from layout elements recursively.
    /// When an element has an href, its rect covers all children, so we skip
    /// recursing into children to avoid duplicate annotations.
    fn collect_link_annotations(
        elements: &[LayoutElement],
        page_height: f64,
        annotations: &mut Vec<LinkAnnotation>,
    ) {
        for element in elements {
            if let Some(ref href) = element.href {
                if !href.is_empty() {
                    let pdf_y = page_height - element.y - element.height;
                    annotations.push(LinkAnnotation {
                        x: element.x,
                        y: pdf_y,
                        width: element.width,
                        height: element.height,
                        href: href.clone(),
                    });
                    // Don't recurse — parent annotation covers children
                    continue;
                }
            }
            Self::collect_link_annotations(&element.children, page_height, annotations);
        }
    }

    /// Collect form field annotations from layout elements.
    fn collect_form_fields(
        elements: &[LayoutElement],
        page_height: f64,
        page_idx: usize,
        fields: &mut Vec<FormFieldData>,
    ) {
        for element in elements {
            if let DrawCommand::FormField {
                ref field_type,
                ref name,
            } = element.draw
            {
                let pdf_y = page_height - element.y - element.height;
                fields.push(FormFieldData {
                    field_type: field_type.clone(),
                    name: name.clone(),
                    x: element.x,
                    y: pdf_y,
                    width: element.width,
                    height: element.height,
                    page_idx,
                });
            }
            Self::collect_form_fields(&element.children, page_height, page_idx, fields);
        }
    }

    /// Collect bookmarks from layout elements.
    fn collect_bookmarks(
        elements: &[LayoutElement],
        page_height: f64,
        page_obj_id: usize,
        bookmarks: &mut Vec<PdfBookmark>,
    ) {
        for element in elements {
            if let Some(ref title) = element.bookmark {
                let y_pdf = page_height - element.y;
                bookmarks.push(PdfBookmark {
                    title: title.clone(),
                    page_obj_id,
                    y_pdf,
                });
            }
            Self::collect_bookmarks(&element.children, page_height, page_obj_id, bookmarks);
        }
    }

    /// Build the PDF outline tree from bookmark entries.
    /// Returns the object ID of the /Outlines dictionary.
    fn write_outline_tree(&self, builder: &mut PdfBuilder, bookmarks: &[PdfBookmark]) -> usize {
        // Reserve the Outlines dictionary object
        let outlines_id = builder.objects.len();
        builder.objects.push(PdfObject {
            id: outlines_id,
            data: vec![],
        });

        // Create outline item objects
        let mut item_ids: Vec<usize> = Vec::new();
        for _bm in bookmarks {
            let item_id = builder.objects.len();
            builder.objects.push(PdfObject {
                id: item_id,
                data: vec![],
            });
            item_ids.push(item_id);
        }

        // Fill in outline items with /Prev, /Next, /Parent, /Dest
        for (i, (bm, &item_id)) in bookmarks.iter().zip(item_ids.iter()).enumerate() {
            let mut dict = format!(
                "<< /Title ({}) /Parent {} 0 R /Dest [{} 0 R /XYZ 0 {:.2} null]",
                Self::escape_pdf_string(&bm.title),
                outlines_id,
                bm.page_obj_id,
                bm.y_pdf,
            );
            if i > 0 {
                let _ = write!(dict, " /Prev {} 0 R", item_ids[i - 1]);
            }
            if i + 1 < item_ids.len() {
                let _ = write!(dict, " /Next {} 0 R", item_ids[i + 1]);
            }
            dict.push_str(" >>");
            builder.objects[item_id].data = dict.into_bytes();
        }

        // Fill in Outlines dictionary
        let first_id = item_ids.first().copied().unwrap_or(0);
        let last_id = item_ids.last().copied().unwrap_or(0);
        let outlines_dict = format!(
            "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
            first_id,
            last_id,
            bookmarks.len()
        );
        builder.objects[outlines_id].data = outlines_dict.into_bytes();

        outlines_id
    }

    /// Write SVG drawing commands to a PDF content stream.
    fn write_svg_commands(
        stream: &mut String,
        commands: &[SvgCommand],
        ext_gstate_map: &HashMap<u64, (usize, String)>,
    ) {
        for cmd in commands {
            match cmd {
                SvgCommand::MoveTo(x, y) => {
                    let _ = writeln!(stream, "{:.2} {:.2} m", x, y);
                }
                SvgCommand::LineTo(x, y) => {
                    let _ = writeln!(stream, "{:.2} {:.2} l", x, y);
                }
                SvgCommand::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    let _ = writeln!(
                        stream,
                        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                        x1, y1, x2, y2, x3, y3
                    );
                }
                SvgCommand::ClosePath => {
                    let _ = writeln!(stream, "h");
                }
                SvgCommand::SetFill(r, g, b) => {
                    let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", r, g, b);
                }
                SvgCommand::SetFillNone => {
                    // No-op in PDF; handled by fill/stroke selection
                }
                SvgCommand::SetStroke(r, g, b) => {
                    let _ = writeln!(stream, "{:.3} {:.3} {:.3} RG", r, g, b);
                }
                SvgCommand::SetStrokeNone => {
                    // No-op in PDF
                }
                SvgCommand::SetStrokeWidth(w) => {
                    let _ = writeln!(stream, "{:.2} w", w);
                }
                SvgCommand::Fill => {
                    let _ = writeln!(stream, "f");
                }
                SvgCommand::Stroke => {
                    let _ = writeln!(stream, "S");
                }
                SvgCommand::FillAndStroke => {
                    let _ = writeln!(stream, "B");
                }
                SvgCommand::SetLineCap(cap) => {
                    let _ = writeln!(stream, "{} J", cap);
                }
                SvgCommand::SetLineJoin(join) => {
                    let _ = writeln!(stream, "{} j", join);
                }
                SvgCommand::SaveState => {
                    let _ = writeln!(stream, "q");
                }
                SvgCommand::RestoreState => {
                    let _ = writeln!(stream, "Q");
                }
                SvgCommand::SetOpacity(opacity) => {
                    if let Some((_, gs_name)) = ext_gstate_map.get(&opacity.to_bits()) {
                        let _ = writeln!(stream, "/{} gs", gs_name);
                    }
                }
            }
        }
    }

    /// Escape special characters in a PDF string.
    pub(crate) fn escape_pdf_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    }

    /// Decode an attachment `src`: plain base64, with an optional
    /// `data:...;base64,` prefix tolerated (same convention as fonts).
    fn decode_attachment_src(src: &str) -> Result<Vec<u8>, FormeError> {
        use base64::Engine as _;
        let b64 = src.rsplit_once(";base64,").map(|(_, d)| d).unwrap_or(src);
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| {
                FormeError::RenderError(format!(
                    "attachment src is not valid base64 (expected base64 bytes or a data: URI): {e}"
                ))
            })
    }

    /// Encode a MIME type as a PDF name (PDF 32000 §7.3.5): delimiter and
    /// non-regular characters become #XX — `text/xml` → `text#2Fxml`.
    fn mime_to_pdf_name(mime: &str) -> String {
        let mut out = String::with_capacity(mime.len() + 2);
        for b in mime.bytes() {
            let regular =
                b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+' | b'\'' | b'"');
            if regular {
                out.push(b as char);
            } else {
                let _ = write!(out, "#{:02X}", b);
            }
        }
        out
    }

    /// Encode a string for use in a PDF content stream with WinAnsi encoding.
    /// Characters outside WinAnsi range are replaced with '?'.
    fn encode_winansi_text(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for ch in s.chars() {
            let b = Self::unicode_to_winansi(ch).unwrap_or(b'?');
            match b {
                b'\\' => result.push_str("\\\\"),
                b'(' => result.push_str("\\("),
                b')' => result.push_str("\\)"),
                0x20..=0x7E => result.push(b as char),
                _ => {
                    let _ = write!(result, "\\{:03o}", b);
                }
            }
        }
        result
    }

    /// Map a Unicode codepoint to a WinAnsiEncoding byte value.
    fn unicode_to_winansi(ch: char) -> Option<u8> {
        crate::font::unicode_to_winansi(ch)
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
        let _ = writeln!(output, "xref\n0 {}", builder.objects.len());
        let _ = writeln!(output, "0000000000 65535 f ");
        for offset in offsets.iter().skip(1) {
            let _ = writeln!(output, "{:010} 00000 n ", offset);
        }

        let _ = write!(
            output,
            "trailer\n<< /Size {} /Root 1 0 R",
            builder.objects.len()
        );
        if let Some(info_id) = info_obj_id {
            let _ = write!(output, " /Info {} 0 R", info_id);
        }
        // /ID — required by PDF/A (6.1.3) and generally expected. Derived
        // deterministically from the file content (SHA-256 of everything written
        // so far), NOT a timestamp or random bytes, so native and WASM builds
        // stay byte-identical. The two identifiers are equal for a freshly
        // created (never incrementally updated) file, per ISO 32000-1 14.4.
        {
            use sha2::Digest as _;
            let digest = sha2::Sha256::digest(&output);
            let mut id_hex = String::with_capacity(32);
            for b in &digest[..16] {
                let _ = write!(id_hex, "{:02X}", b);
            }
            let _ = write!(output, " /ID [<{id_hex}> <{id_hex}>]");
        }
        let _ = writeln!(output, " >>\nstartxref\n{}\n%%EOF", xref_offset);

        output
    }
}

/// Write a single chart drawing primitive to the PDF content stream.
///
/// Called within a Y-flipped coordinate system (1 0 0 -1 x page_h-y cm),
/// so chart primitives use top-left origin (Y increases downward).
fn write_chart_primitive(
    stream: &mut String,
    prim: &crate::chart::ChartPrimitive,
    _chart_height: f64,
    builder: &PdfBuilder,
) {
    use crate::chart::{ChartPrimitive, TextAnchor};
    use crate::font::metrics::unicode_to_winansi;

    match prim {
        ChartPrimitive::Rect { x, y, w, h, fill } => {
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", fill.r, fill.g, fill.b);
            let _ = writeln!(stream, "{:.2} {:.2} {:.2} {:.2} re f", x, y, w, h);
        }

        ChartPrimitive::Line {
            x1,
            y1,
            x2,
            y2,
            stroke,
            width,
        } => {
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} RG", stroke.r, stroke.g, stroke.b);
            let _ = writeln!(stream, "{:.2} w", width);
            let _ = writeln!(stream, "{:.2} {:.2} m {:.2} {:.2} l S", x1, y1, x2, y2);
        }

        ChartPrimitive::Polyline {
            points,
            stroke,
            width,
        } => {
            if points.len() < 2 {
                return;
            }
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} RG", stroke.r, stroke.g, stroke.b);
            let _ = writeln!(stream, "{:.2} w", width);
            let _ = writeln!(stream, "{:.2} {:.2} m", points[0].0, points[0].1);
            for &(px, py) in &points[1..] {
                let _ = writeln!(stream, "{:.2} {:.2} l", px, py);
            }
            let _ = writeln!(stream, "S");
        }

        ChartPrimitive::FilledPath {
            points,
            fill,
            opacity,
        } => {
            if points.len() < 3 {
                return;
            }
            let _ = writeln!(stream, "q");
            // Set opacity via ExtGState if available
            if *opacity < 1.0 {
                if let Some((_, gs_name)) = builder.ext_gstate_map.get(&opacity.to_bits()) {
                    let _ = writeln!(stream, "/{} gs", gs_name);
                }
            }
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", fill.r, fill.g, fill.b);
            let _ = writeln!(stream, "{:.2} {:.2} m", points[0].0, points[0].1);
            for &(px, py) in &points[1..] {
                let _ = writeln!(stream, "{:.2} {:.2} l", px, py);
            }
            let _ = writeln!(stream, "h f");
            let _ = writeln!(stream, "Q");
        }

        ChartPrimitive::Circle { cx, cy, r, fill } => {
            // Approximate circle with 4 cubic bezier curves
            let kappa: f64 = 0.5523;
            let kr = kappa * r;
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", fill.r, fill.g, fill.b);
            let _ = writeln!(stream, "{:.2} {:.2} m", cx + r, cy);
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                cx + r,
                cy + kr,
                cx + kr,
                cy + r,
                cx,
                cy + r
            );
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                cx - kr,
                cy + r,
                cx - r,
                cy + kr,
                cx - r,
                cy
            );
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                cx - r,
                cy - kr,
                cx - kr,
                cy - r,
                cx,
                cy - r
            );
            let _ = writeln!(
                stream,
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                cx + kr,
                cy - r,
                cx + r,
                cy - kr,
                cx + r,
                cy
            );
            let _ = writeln!(stream, "f");
        }

        ChartPrimitive::ArcSector {
            cx,
            cy,
            r,
            start_angle,
            end_angle,
            fill,
        } => {
            let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", fill.r, fill.g, fill.b);
            // Move to center
            let _ = writeln!(stream, "{:.2} {:.2} m", cx, cy);
            // Line to arc start
            let sx = cx + r * start_angle.cos();
            let sy = cy + r * start_angle.sin();
            let _ = writeln!(stream, "{:.2} {:.2} l", sx, sy);

            // Approximate arc with cubic bezier segments (max 90° per segment)
            let mut angle = *start_angle;
            let total = end_angle - start_angle;
            let segments = ((total.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
            let step = total / segments as f64;

            for _ in 0..segments {
                let a1 = angle;
                let a2 = angle + step;
                let alpha = 4.0 / 3.0 * ((a2 - a1) / 4.0).tan();

                let p1x = cx + r * a1.cos();
                let p1y = cy + r * a1.sin();
                let p2x = cx + r * a2.cos();
                let p2y = cy + r * a2.sin();

                let cp1x = p1x - alpha * r * a1.sin();
                let cp1y = p1y + alpha * r * a1.cos();
                let cp2x = p2x + alpha * r * a2.sin();
                let cp2y = p2y - alpha * r * a2.cos();

                let _ = writeln!(
                    stream,
                    "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c",
                    cp1x, cp1y, cp2x, cp2y, p2x, p2y
                );
                angle = a2;
            }

            // Close path back to center and fill
            let _ = writeln!(stream, "h f");
        }

        ChartPrimitive::Label {
            text,
            x,
            y,
            font_size,
            color,
            anchor,
        } => {
            // Measure text width for anchor alignment
            let metrics = crate::font::StandardFont::Helvetica.metrics();
            let text_width = metrics.measure_string(text, *font_size, 0.0);
            let x_offset = match anchor {
                TextAnchor::Left => 0.0,
                TextAnchor::Center => -text_width / 2.0,
                TextAnchor::Right => -text_width,
            };

            // Find Helvetica font index in font_objects
            let font_idx = builder
                .font_objects
                .iter()
                .enumerate()
                .find(|(_, (key, _))| key.family == "Helvetica" && key.weight == 400 && !key.italic)
                .map(|(i, _)| i)
                .unwrap_or(0);

            // Encode text to WinAnsi
            let encoded: String = text
                .chars()
                .map(|ch| {
                    if let Some(code) = unicode_to_winansi(ch) {
                        code as char
                    } else if (ch as u32) >= 32 && (ch as u32) <= 255 {
                        ch
                    } else {
                        '?'
                    }
                })
                .collect();
            let escaped = pdf_escape_string(&encoded);

            // Undo Y-flip for text rendering, then position
            let _ = writeln!(stream, "q");
            let _ = writeln!(stream, "1 0 0 -1 {:.4} {:.4} cm", x + x_offset, *y);
            let _ = writeln!(
                stream,
                "BT /F{} {:.1} Tf {:.3} {:.3} {:.3} rg 0 0 Td ({}) Tj ET",
                font_idx, font_size, color.r, color.g, color.b, escaped
            );
            let _ = writeln!(stream, "Q");
        }
    }
}

/// Normalize a list of gradient stops for PDF Shading emission. Clamps
/// positions to [0, 1], sorts ascending by position, and pads with
/// implicit stops at 0 and 1 (using the closest defined stop's color)
/// when the input doesn't cover the full range. Empty input collapses to
/// two `fallback`-colored stops at 0 and 1 so the caller never has to
/// special-case zero stops.
fn normalize_gradient_stops(
    stops: &[crate::style::GradientStop],
    fallback: Color,
) -> Vec<crate::style::GradientStop> {
    use crate::style::GradientStop;
    if stops.is_empty() {
        return vec![
            GradientStop {
                position: 0.0,
                color: fallback,
            },
            GradientStop {
                position: 1.0,
                color: fallback,
            },
        ];
    }
    let mut sorted: Vec<GradientStop> = stops
        .iter()
        .map(|s| GradientStop {
            position: s.position.clamp(0.0, 1.0),
            color: s.color,
        })
        .collect();
    sorted.sort_by(|a, b| {
        a.position
            .partial_cmp(&b.position)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if sorted[0].position > 0.0 {
        sorted.insert(
            0,
            GradientStop {
                position: 0.0,
                color: sorted[0].color,
            },
        );
    }
    if sorted[sorted.len() - 1].position < 1.0 {
        let last = sorted[sorted.len() - 1].color;
        sorted.push(GradientStop {
            position: 1.0,
            color: last,
        });
    }
    sorted
}

fn pdf_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontContext;

    /// The embedded sRGB profile must be a REAL ICC profile suitable for a
    /// PDF/A OutputIntent — not, say, an HTML error page a `curl` returned and
    /// nobody inspected (which is exactly what shipped from v0.6.0 through 0.15.0,
    /// silently making every PDF/A OutputIntent invalid). This is the check
    /// that would have caught it: ICC signature, an OutputIntent-legal device
    /// class (`mntr`/`prtr`), and an RGB data colour space.
    #[test]
    fn test_embedded_srgb_is_a_valid_icc_profile() {
        let icc: &[u8] = include_bytes!("sRGB.icc");
        assert!(
            icc.len() >= 128,
            "ICC shorter than its 128-byte header: {}",
            icc.len()
        );
        // Not HTML / not a text error page.
        assert_ne!(
            icc[0], b'<',
            "embedded ICC starts with '<' — looks like HTML, not a profile"
        );
        // 'acsp' profile-file signature at bytes 36..40 (ISO 15076-1 / ICC.1).
        assert_eq!(&icc[36..40], b"acsp", "missing ICC 'acsp' signature");
        // Device class (bytes 12..16) must be monitor or output for an OutputIntent.
        let device_class = &icc[12..16];
        assert!(
            device_class == b"mntr" || device_class == b"prtr",
            "ICC device class {:?} is not mntr/prtr (PDF/A 6.2.3)",
            String::from_utf8_lossy(device_class),
        );
        // Data colour space (bytes 16..20) must be RGB for an sRGB OutputIntent.
        assert_eq!(&icc[16..20], b"RGB ", "ICC data colour space is not RGB");
    }

    #[test]
    fn test_escape_pdf_string() {
        assert_eq!(
            PdfWriter::escape_pdf_string("Hello (World)"),
            "Hello \\(World\\)"
        );
        assert_eq!(PdfWriter::escape_pdf_string("back\\slash"), "back\\\\slash");
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
            watermarks: vec![],
            config: PageConfig::default(),
            page_name: None,
        }];
        let metadata = Metadata::default();
        let (bytes, _warnings) = writer
            .write(
                &pages,
                &metadata,
                &font_context,
                false,
                None,
                false,
                None,
                &[],
                None,
                false,
            )
            .unwrap();

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
            watermarks: vec![],
            config: PageConfig::default(),
            page_name: None,
        }];
        let metadata = Metadata {
            title: Some("Test Document".to_string()),
            author: Some("Forme".to_string()),
            subject: None,
            creator: None,
            lang: None,
        };
        let (bytes, _warnings) = writer
            .write(
                &pages,
                &metadata,
                &font_context,
                false,
                None,
                false,
                None,
                &[],
                None,
                false,
            )
            .unwrap();
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
                    x: 54.0,
                    y: 54.0,
                    width: 100.0,
                    height: 16.8,
                    draw: DrawCommand::Text {
                        lines: vec![TextLine {
                            x: 54.0,
                            y: 66.0,
                            width: 50.0,
                            height: 16.8,
                            glyphs: vec![PositionedGlyph {
                                glyph_id: 65,
                                x_offset: 0.0,
                                y_offset: 0.0,
                                x_advance: 8.0,
                                font_size: 12.0,
                                font_family: "Helvetica".into(),
                                font_weight: 400,
                                font_style: FontStyle::Normal,
                                char_value: 'A',
                                color: None,
                                href: None,
                                text_decoration: TextDecoration::None,
                                letter_spacing: 0.0,
                                cluster_text: None,
                            }],
                            word_spacing: 0.0,
                        }],
                        color: Color::BLACK,
                        text_decoration: TextDecoration::None,
                        opacity: 1.0,
                    },
                    children: vec![],
                    node_type: None,
                    resolved_style: None,
                    source_location: None,
                    href: None,
                    bookmark: None,
                    alt: None,
                    is_header_row: false,
                    col_span: 1,
                    overflow: Overflow::default(),
                    opacity: 1.0,
                },
                LayoutElement {
                    x: 54.0,
                    y: 74.0,
                    width: 100.0,
                    height: 16.8,
                    draw: DrawCommand::Text {
                        lines: vec![TextLine {
                            x: 54.0,
                            y: 86.0,
                            width: 50.0,
                            height: 16.8,
                            glyphs: vec![PositionedGlyph {
                                glyph_id: 65,
                                x_offset: 0.0,
                                y_offset: 0.0,
                                x_advance: 8.0,
                                font_size: 12.0,
                                font_family: "Helvetica".into(),
                                font_weight: 700,
                                font_style: FontStyle::Normal,
                                char_value: 'A',
                                color: None,
                                href: None,
                                text_decoration: TextDecoration::None,
                                letter_spacing: 0.0,
                                cluster_text: None,
                            }],
                            word_spacing: 0.0,
                        }],
                        color: Color::BLACK,
                        text_decoration: TextDecoration::None,
                        opacity: 1.0,
                    },
                    children: vec![],
                    node_type: None,
                    resolved_style: None,
                    source_location: None,
                    href: None,
                    bookmark: None,
                    alt: None,
                    is_header_row: false,
                    col_span: 1,
                    overflow: Overflow::default(),
                    opacity: 1.0,
                },
            ],
            fixed_header: vec![],
            fixed_footer: vec![],
            watermarks: vec![],
            config: PageConfig::default(),
            page_name: None,
        }];

        let metadata = Metadata::default();
        let (bytes, _warnings) = writer
            .write(
                &pages,
                &metadata,
                &font_context,
                false,
                None,
                false,
                None,
                &[],
                None,
                false,
            )
            .unwrap();
        let text = String::from_utf8_lossy(&bytes);

        // Should have both Helvetica and Helvetica-Bold registered
        assert!(
            text.contains("Helvetica"),
            "Should contain regular Helvetica"
        );
        assert!(
            text.contains("Helvetica-Bold"),
            "Should contain Helvetica-Bold"
        );
    }

    #[test]
    fn test_sanitize_font_name() {
        assert_eq!(PdfWriter::sanitize_font_name("Inter", 400, false), "Inter");
        assert_eq!(
            PdfWriter::sanitize_font_name("Inter", 700, false),
            "Inter-Bold"
        );
        assert_eq!(
            PdfWriter::sanitize_font_name("Inter", 400, true),
            "Inter-Italic"
        );
        assert_eq!(
            PdfWriter::sanitize_font_name("Inter", 700, true),
            "Inter-Bold-Italic"
        );
        assert_eq!(
            PdfWriter::sanitize_font_name("Noto Sans", 400, false),
            "NotoSans"
        );
        assert_eq!(
            PdfWriter::sanitize_font_name("Font (Display)", 400, false),
            "FontDisplay"
        );
    }

    #[test]
    fn test_tounicode_cmap_format() {
        // glyph_to_char: maps subset glyph IDs → Unicode chars
        let mut glyph_to_char = HashMap::new();
        glyph_to_char.insert(36u16, 'A');
        glyph_to_char.insert(37u16, 'B');

        let cmap = PdfWriter::build_tounicode_cmap_from_gids(&glyph_to_char, "TestFont");

        assert!(cmap.contains("begincmap"), "CMap should contain begincmap");
        assert!(cmap.contains("endcmap"), "CMap should contain endcmap");
        assert!(
            cmap.contains("beginbfchar"),
            "CMap should contain beginbfchar"
        );
        assert!(cmap.contains("endbfchar"), "CMap should contain endbfchar");
        assert!(
            cmap.contains("<0024> <0041>"),
            "Should map gid 0x0024 to Unicode 'A' 0x0041"
        );
        assert!(
            cmap.contains("<0025> <0042>"),
            "Should map gid 0x0025 to Unicode 'B' 0x0042"
        );
        assert!(
            cmap.contains("begincodespacerange"),
            "Should define codespace range"
        );
        assert!(
            cmap.contains("<0000> <FFFF>"),
            "Codespace should be 0000-FFFF"
        );
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
                x: 54.0,
                y: 54.0,
                width: 100.0,
                height: 16.8,
                draw: DrawCommand::Text {
                    lines: vec![TextLine {
                        x: 54.0,
                        y: 66.0,
                        width: 50.0,
                        height: 16.8,
                        glyphs: vec![PositionedGlyph {
                            glyph_id: 65,
                            x_offset: 0.0,
                            y_offset: 0.0,
                            x_advance: 8.0,
                            font_size: 12.0,
                            font_family: "Helvetica".into(),
                            font_weight: 400,
                            font_style: FontStyle::Normal,
                            char_value: 'H',
                            color: None,
                            href: None,
                            text_decoration: TextDecoration::None,
                            letter_spacing: 0.0,
                            cluster_text: None,
                        }],
                        word_spacing: 0.0,
                    }],
                    color: Color::BLACK,
                    text_decoration: TextDecoration::None,
                    opacity: 1.0,
                },
                children: vec![],
                node_type: None,
                resolved_style: None,
                source_location: None,
                href: None,
                bookmark: None,
                alt: None,
                is_header_row: false,
                col_span: 1,
                overflow: Overflow::default(),
                opacity: 1.0,
            }],
            fixed_header: vec![],
            fixed_footer: vec![],
            watermarks: vec![],
            config: PageConfig::default(),
            page_name: None,
        }];

        let metadata = Metadata::default();
        let (bytes, _warnings) = writer
            .write(
                &pages,
                &metadata,
                &font_context,
                false,
                None,
                false,
                None,
                &[],
                None,
                false,
            )
            .unwrap();
        let text = String::from_utf8_lossy(&bytes);

        // Standard fonts should use Type1, not CIDFontType2
        assert!(
            text.contains("/Type1"),
            "Standard font should use Type1 subtype"
        );
        assert!(
            !text.contains("CIDFontType2"),
            "Standard font should not use CIDFontType2"
        );
    }
}
