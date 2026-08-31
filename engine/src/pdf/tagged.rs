//! # Tagged PDF Structure Tree Builder
//!
//! Produces the structure tree required for PDF accessibility (PDF/UA).
//! The structure tree maps visual content to semantic roles (P, Span, Table, etc.)
//! via Marked Content sequences (BDC/EMC) in content streams.
//!
//! ## How It Works
//!
//! 1. During content stream writing, `begin_element` / `end_element` bracket
//!    each layout element with BDC/EMC operators carrying an MCID.
//! 2. After all pages are written, `write_objects` serializes the accumulated
//!    structure elements as PDF objects: StructTreeRoot, structure elements,
//!    and the ParentTree (a number tree mapping page StructParents indices
//!    to arrays of structure element refs).

use std::fmt::Write as FmtWrite;

/// A structure element in the tagged PDF tree.
struct StructElement {
    /// Role tag: "Document", "Div", "P", "Span", "Table", "TR", "TH", "TD", "Figure".
    role: &'static str,
    /// Index of parent in elements vec (0 = self for root).
    parent_idx: usize,
    /// Children: either nested structure elements or marked content refs.
    kids: Vec<StructKid>,
    /// Alt text for figures.
    alt: Option<String>,
    /// Column span, for table cells (PDF/UA 7.2-43 / `/ColSpan`). 1 otherwise.
    col_span: u32,
}

/// A child of a structure element.
enum StructKid {
    /// Reference to another structure element by index.
    StructRef(usize),
    /// Reference to marked content on a page.
    MarkedContent { page_idx: usize, mcid: u32 },
    /// Reference to a PDF object (OBJR) — used to attach a link annotation to
    /// its /Link structure element (PDF/UA 7.18.5-1).
    ObjectRef(usize),
}

/// A /Link structure element awaiting connection to its annotation, recorded
/// during the content pass and matched to the annotation (by page + href) in
/// the annotation pass.
struct LinkSlot {
    page_idx: usize,
    elem_idx: usize,
    href: String,
    matched: bool,
}

/// Builds the tagged PDF structure tree during content stream writing.
pub struct TagBuilder {
    elements: Vec<StructElement>,
    parent_stack: Vec<usize>,
    /// Per-page MCID counter.
    page_mcid_counters: Vec<u32>,
    /// Maps (page_idx, mcid) → structure element index (for ParentTree).
    mcid_to_struct: Vec<(usize, u32, usize)>,
    /// Tracks whether we're inside a "P" element (to map nested Text → Span).
    inside_paragraph: bool,
    /// /Link structure elements awaiting connection to their annotations.
    link_slots: Vec<LinkSlot>,
    /// StructParent number → structure element index, for link annotations.
    /// Their numbers start above the page StructParents range (page indices),
    /// so the ParentTree keyspace stays disjoint.
    annot_parents: Vec<(u32, usize)>,
    /// Next StructParent number to hand out for a link annotation.
    next_annot_struct_parent: u32,
    /// Indices of synthetic /LBody elements — auto-created to wrap a list
    /// item's non-label content (PDF/UA 7.2-20) and closed together with their
    /// /LI, since the caller emits no matching end_element for them.
    synthetic_lbody: std::collections::HashSet<usize>,
}

impl TagBuilder {
    /// Create a new TagBuilder with a root "Document" structure element.
    pub fn new(num_pages: usize) -> Self {
        let root = StructElement {
            role: "Document",
            parent_idx: 0,
            kids: Vec::new(),
            alt: None,
            col_span: 1,
        };
        TagBuilder {
            elements: vec![root],
            parent_stack: vec![0],
            page_mcid_counters: vec![0; num_pages],
            mcid_to_struct: Vec::new(),
            inside_paragraph: false,
            link_slots: Vec::new(),
            annot_parents: Vec::new(),
            // Page StructParents occupy 0..num_pages; annotation StructParents
            // start after them so the two never collide in the ParentTree.
            next_annot_struct_parent: num_pages as u32,
            synthetic_lbody: std::collections::HashSet::new(),
        }
    }

    /// Begin a structure element for a layout node. Returns the MCID to use
    /// in the BDC operator. Call `end_element` after the content is written.
    pub fn begin_element(
        &mut self,
        node_type: &str,
        is_header_row: bool,
        alt: Option<&str>,
        page_idx: usize,
        href: Option<&str>,
        col_span: u32,
    ) -> u32 {
        // An element carrying an href is a link: it tags as a /Link structure
        // element (overriding its node_type role) so the annotation can attach
        // to it (PDF/UA 7.18.5-1). The BDC role at the call site uses the same
        // href check, so content marking and structure agree.
        let role = if href.is_some() {
            "Link"
        } else {
            self.map_role(node_type, is_header_row)
        };
        let was_inside_paragraph = self.inside_paragraph;
        // Headings act like paragraphs for the inner-text → Span downgrade
        // rule, so a nested Text inside an H1 maps to a Span rather than
        // spawning a P child of the H1.
        if matches!(role, "P" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6") {
            self.inside_paragraph = true;
        }

        let mut parent_idx = *self.parent_stack.last().unwrap_or(&0);

        // PDF/UA 7.2-20: an /LI may contain only /Lbl and /LBody. The label
        // (marker) tags as /Lbl directly; the item's first non-label child
        // opens a synthetic /LBody that wraps the rest of the content. Once
        // open, the /LBody is the parent, so this only fires for the first
        // content child.
        if self.elements[parent_idx].role == "LI" && role != "Lbl" {
            let lbody_idx = self.elements.len();
            self.elements.push(StructElement {
                role: "LBody",
                parent_idx,
                kids: Vec::new(),
                alt: None,
                col_span: 1,
            });
            self.elements[parent_idx]
                .kids
                .push(StructKid::StructRef(lbody_idx));
            self.synthetic_lbody.insert(lbody_idx);
            self.parent_stack.push(lbody_idx);
            parent_idx = lbody_idx;
        }

        let elem_idx = self.elements.len();

        // Allocate MCID on this page
        let mcid = self.page_mcid_counters[page_idx];
        self.page_mcid_counters[page_idx] += 1;

        let elem = StructElement {
            role,
            parent_idx,
            kids: vec![StructKid::MarkedContent { page_idx, mcid }],
            alt: alt.map(|s| s.to_string()),
            col_span,
        };
        self.elements.push(elem);

        // Register as child of parent
        self.elements[parent_idx]
            .kids
            .push(StructKid::StructRef(elem_idx));

        // Track for ParentTree
        self.mcid_to_struct.push((page_idx, mcid, elem_idx));

        // Push onto parent stack so nested elements become children
        self.parent_stack.push(elem_idx);

        // Store state for paragraph tracking
        if !was_inside_paragraph && role == "P" {
            // We just entered a paragraph
        }

        // Record a link slot so the annotation pass can attach the annotation
        // (OBJR + /StructParent) to this /Link element.
        if let Some(h) = href {
            self.link_slots.push(LinkSlot {
                page_idx,
                elem_idx,
                href: h.to_string(),
                matched: false,
            });
        }

        mcid
    }

    /// Attach a link annotation to its /Link structure element: add an OBJR
    /// kid pointing at the annotation, and allocate a StructParent number that
    /// the ParentTree maps back to the /Link element. Returns the number to
    /// write as the annotation's /StructParent, or `None` if no /Link element
    /// on this page carries this href (e.g. an internal link whose annotation
    /// was skipped for a missing bookmark). Matches by (page, href) in order,
    /// which is robust to skips and to how the two passes traverse the tree.
    pub fn connect_link_annotation(
        &mut self,
        page_idx: usize,
        href: &str,
        annot_obj_id: usize,
    ) -> Option<u32> {
        let slot = self
            .link_slots
            .iter_mut()
            .find(|s| !s.matched && s.page_idx == page_idx && s.href == href)?;
        slot.matched = true;
        let elem_idx = slot.elem_idx;
        self.elements[elem_idx]
            .kids
            .push(StructKid::ObjectRef(annot_obj_id));
        let sp = self.next_annot_struct_parent;
        self.next_annot_struct_parent += 1;
        self.annot_parents.push((sp, elem_idx));
        Some(sp)
    }

    /// End the current structure element. Must be called after `begin_element`.
    pub fn end_element(&mut self) {
        // A synthetic /LBody (wrapping a list item's content) has no matching
        // caller end_element — it sits on top of its /LI when the item closes,
        // so pop it together with the /LI.
        if let Some(&top) = self.parent_stack.last() {
            if self.synthetic_lbody.contains(&top) {
                self.parent_stack.pop();
            }
        }
        if let Some(idx) = self.parent_stack.pop() {
            // If we're leaving a paragraph-like element (P or any heading),
            // reset the flag so the next sibling text gets the P role again.
            if matches!(
                self.elements[idx].role,
                "P" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6"
            ) {
                self.inside_paragraph = false;
            }
        }
    }

    /// Map a layout node_type to a PDF structure role (public for BDC tag).
    pub fn map_role_public(&self, node_type: &str, is_header_row: bool) -> &'static str {
        self.map_role(node_type, is_header_row)
    }

    /// Map a layout node_type to a PDF structure role.
    fn map_role(&self, node_type: &str, is_header_row: bool) -> &'static str {
        match node_type {
            "View" | "FixedHeader" | "FixedFooter" => "Div",
            "Text" => {
                if self.inside_paragraph {
                    "Span"
                } else {
                    "P"
                }
            }
            // Semantic headings — map to PDF/UA heading roles. PDF/A-2a and
            // PDF/UA both treat /H1.../H6 as standard structure elements.
            "H1" => "H1",
            "H2" => "H2",
            "H3" => "H3",
            "H4" => "H4",
            "H5" => "H5",
            "H6" => "H6",
            // Lists: List → /L, ListItem → /LI, Lbl (the marker text) →
            // /Lbl. PDF/UA-1 + PDF/A-2a both recognize these as standard
            // structure elements. We don't currently wrap each item's
            // content in an explicit /LBody — viewers tolerate the
            // shorthand of placing content directly under /LI.
            "List" => "L",
            "ListItem" => "LI",
            "Lbl" => "Lbl",
            "TextLine" => "Span",
            "Image" => "Figure",
            "Svg" => "Figure",
            "Table" => "Table",
            "TableRow" => "TR",
            "TableCell" => {
                if is_header_row {
                    "TH"
                } else {
                    "TD"
                }
            }
            "TextField" | "Checkbox" | "Dropdown" | "RadioButton" => "Form",
            _ => "Div",
        }
    }

    /// Write all structure tree objects to the PDF builder.
    /// Returns `(struct_tree_root_obj_id, parent_tree_obj_id)`.
    pub fn write_objects(
        &self,
        objects: &mut Vec<super::PdfObject>,
        page_obj_ids: &[usize],
        lang: Option<&str>,
    ) -> (usize, usize) {
        let num_pages = page_obj_ids.len();

        // Allocate object IDs for all structure elements
        let base_id = objects.len();
        let elem_obj_ids: Vec<usize> = (0..self.elements.len()).map(|i| base_id + i).collect();

        // Reserve slots
        for i in 0..self.elements.len() {
            objects.push(super::PdfObject {
                id: base_id + i,
                data: Vec::new(),
            });
        }

        // ParentTree object
        let parent_tree_id = objects.len();
        objects.push(super::PdfObject {
            id: parent_tree_id,
            data: Vec::new(),
        });

        // RoleMap object
        let role_map_id = objects.len();
        objects.push(super::PdfObject {
            id: role_map_id,
            data: Vec::new(),
        });

        // Build StructTreeRoot (element 0 = "Document")
        let root_obj_id = elem_obj_ids[0];
        {
            let root = &self.elements[0];
            let kids_str = self.format_kids(&root.kids, &elem_obj_ids, page_obj_ids);
            let lang_str = if let Some(l) = lang {
                format!(" /Lang ({})", super::PdfWriter::escape_pdf_string(l))
            } else {
                String::new()
            };
            let data = format!(
                "<< /Type /StructTreeRoot /K [{kids}] /ParentTree {pt} 0 R /RoleMap {rm} 0 R{lang} >>",
                kids = kids_str,
                pt = parent_tree_id,
                rm = role_map_id,
                lang = lang_str,
            );
            objects[root_obj_id].data = data.into_bytes();
        }

        // Write each structure element (skip 0 = root, handled above)
        for (i, elem) in self.elements.iter().enumerate().skip(1) {
            let obj_id = elem_obj_ids[i];
            let parent_obj_id = elem_obj_ids[elem.parent_idx];
            let kids_str = self.format_kids(&elem.kids, &elem_obj_ids, page_obj_ids);

            let mut dict = format!(
                "<< /Type /StructElem /S /{role} /P {parent} 0 R /K [{kids}]",
                role = elem.role,
                parent = parent_obj_id,
                kids = kids_str,
            );

            if let Some(ref alt) = elem.alt {
                let escaped = super::PdfWriter::escape_pdf_string(alt);
                let _ = write!(dict, " /Alt ({})", escaped);
            }

            // Table cell attributes in a single /A dict with owner /Table:
            //   - /Scope /Column on every TH (7.5-1) — Forme header rows label
            //     the columns beneath them, so they are column headers.
            //   - /ColSpan on any cell spanning more than one column (7.2-43) —
            //     without it veraPDF counts unequal columns per row.
            if elem.role == "TH" || elem.role == "TD" {
                let mut attrs = String::from(" /A << /O /Table");
                if elem.role == "TH" {
                    attrs.push_str(" /Scope /Column");
                }
                if elem.col_span > 1 {
                    let _ = write!(attrs, " /ColSpan {}", elem.col_span);
                }
                attrs.push_str(" >>");
                // Only emit /A when it carries an attribute (a plain TD with no
                // span needs none).
                if attrs != " /A << /O /Table >>" {
                    dict.push_str(&attrs);
                }
            }

            dict.push_str(" >>");
            objects[obj_id].data = dict.into_bytes();
        }

        // Build ParentTree: maps page StructParents index → array of struct elem refs
        // For each page, the array has one entry per MCID on that page
        let mut nums = String::new();
        for page_idx in 0..num_pages {
            let mcid_count = self.page_mcid_counters[page_idx];
            if mcid_count == 0 {
                continue;
            }

            // Build array of struct element refs for this page, ordered by MCID
            let mut refs: Vec<(u32, usize)> = self
                .mcid_to_struct
                .iter()
                .filter(|(pi, _, _)| *pi == page_idx)
                .map(|(_, mcid, elem_idx)| (*mcid, elem_obj_ids[*elem_idx]))
                .collect();
            refs.sort_by_key(|(mcid, _)| *mcid);

            let ref_strs: Vec<String> =
                refs.iter().map(|(_, oid)| format!("{} 0 R", oid)).collect();
            let _ = write!(nums, " {} [{}]", page_idx, ref_strs.join(" "));
        }

        // Link annotations: each StructParent number maps to the single /Link
        // structure element it belongs to (not an array — an annotation has
        // exactly one owning element). Numbers are disjoint from page indices.
        for (sp, elem_idx) in &self.annot_parents {
            let _ = write!(nums, " {} {} 0 R", sp, elem_obj_ids[*elem_idx]);
        }

        let parent_tree_data = format!("<< /Nums [{}] >>", nums.trim());
        objects[parent_tree_id].data = parent_tree_data.into_bytes();

        // RoleMap: empty. Every role Forme emits (Document, Div, P, Span,
        // H1..H6, L, LI, Lbl, Figure, Table, TR, TH, TD, Form) is already a
        // standard PDF 1.7 structure type, so none needs remapping. The
        // RoleMap only ever maps *non-standard* roles to standard ones —
        // mapping a standard type to itself (e.g. /Div /Div) is a circular
        // mapping that PDF/UA-1 (clause 7.1-6) rejects and that invalidated
        // the entire structure tree in veraPDF.
        objects[role_map_id].data = b"<< >>".to_vec();

        (root_obj_id, parent_tree_id)
    }

    /// Format the /K array entries for a structure element.
    fn format_kids(
        &self,
        kids: &[StructKid],
        elem_obj_ids: &[usize],
        page_obj_ids: &[usize],
    ) -> String {
        let mut parts = Vec::new();
        for kid in kids {
            match kid {
                StructKid::StructRef(idx) => {
                    parts.push(format!("{} 0 R", elem_obj_ids[*idx]));
                }
                StructKid::MarkedContent { page_idx, mcid } => {
                    parts.push(format!(
                        "<< /Type /MCR /Pg {} 0 R /MCID {} >>",
                        page_obj_ids[*page_idx], mcid
                    ));
                }
                StructKid::ObjectRef(obj_id) => {
                    parts.push(format!("<< /Type /OBJR /Obj {} 0 R >>", obj_id));
                }
            }
        }
        parts.join(" ")
    }

    /// Get the number of MCIDs emitted on a given page.
    #[cfg(test)]
    pub fn page_mcid_count(&self, page_idx: usize) -> u32 {
        self.page_mcid_counters.get(page_idx).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_builder_basic() {
        let mut tb = TagBuilder::new(1);

        let mcid = tb.begin_element("View", false, None, 0, None, 1);
        assert_eq!(mcid, 0);

        let mcid2 = tb.begin_element("Text", false, None, 0, None, 1);
        assert_eq!(mcid2, 1);
        tb.end_element(); // Text

        tb.end_element(); // View

        assert_eq!(tb.elements.len(), 3); // Document, Div, P
        assert_eq!(tb.elements[1].role, "Div");
        assert_eq!(tb.elements[2].role, "P");
    }

    #[test]
    fn role_map_has_no_circular_self_mappings() {
        // PDF/UA-1 clause 7.1-6: a RoleMap that maps a standard structure type
        // to itself (e.g. /Div /Div) is a *circular* mapping and invalidates
        // the whole structure tree in veraPDF. Forme emits only standard PDF
        // 1.7 roles, so none belong in the RoleMap — it must not self-map any.
        let mut tb = TagBuilder::new(1);
        tb.begin_element("View", false, None, 0, None, 1);
        tb.begin_element("Text", false, None, 0, None, 1);
        tb.end_element();
        tb.end_element();

        let mut objects: Vec<super::super::PdfObject> = vec![super::super::PdfObject {
            id: 0,
            data: Vec::new(),
        }];
        let page_obj_ids = vec![0usize];
        let (root_id, _) = tb.write_objects(&mut objects, &page_obj_ids, Some("en-US"));

        // Resolve the RoleMap object from the StructTreeRoot's /RoleMap ref, so
        // the check inspects the RoleMap itself — not a StructElem, whose
        // `/S /P /P {parent}` legitimately contains "/P /P" (type then Parent).
        let root = String::from_utf8_lossy(&objects[root_id].data).into_owned();
        let rm_id: usize = root
            .split("/RoleMap ")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .expect("StructTreeRoot must reference a RoleMap");
        let role_map = String::from_utf8_lossy(&objects[rm_id].data).into_owned();

        // Any "/X /X" self-map is circular. Scan token pairs in the RoleMap.
        let toks: Vec<&str> = role_map.trim_matches(|c| c == '<' || c == '>' || c == ' ').split_whitespace().collect();
        let self_map = toks.windows(2).any(|w| w[0] == w[1] && w[0].starts_with('/'));
        assert!(
            !self_map,
            "RoleMap must not self-map standard structure types (veraPDF 7.1-6 circular mapping): {role_map}"
        );
    }

    #[test]
    fn test_nested_text_maps_to_span() {
        let mut tb = TagBuilder::new(1);

        // Outer Text → P
        let _mcid = tb.begin_element("Text", false, None, 0, None, 1);
        assert_eq!(tb.elements.last().unwrap().role, "P");

        // Inner Text → Span (because inside_paragraph)
        let _mcid = tb.begin_element("Text", false, None, 0, None, 1);
        assert_eq!(tb.elements.last().unwrap().role, "Span");

        tb.end_element();
        tb.end_element();
    }

    #[test]
    fn test_table_header_maps_to_th() {
        let mut tb = TagBuilder::new(1);

        tb.begin_element("Table", false, None, 0, None, 1);
        tb.begin_element("TableRow", true, None, 0, None, 1);

        // Cell in header row → TH
        tb.begin_element("TableCell", true, None, 0, None, 1);
        assert_eq!(tb.elements.last().unwrap().role, "TH");
        tb.end_element();

        tb.end_element(); // TR
        tb.end_element(); // Table

        // Body row
        tb.begin_element("TableRow", false, None, 0, None, 1);
        tb.begin_element("TableCell", false, None, 0, None, 1);
        assert_eq!(tb.elements.last().unwrap().role, "TD");
        tb.end_element();
        tb.end_element();
    }

    #[test]
    fn test_figure_with_alt_text() {
        let mut tb = TagBuilder::new(1);

        tb.begin_element("Image", false, Some("A photo of a cat"), 0, None, 1);
        let elem = tb.elements.last().unwrap();
        assert_eq!(elem.role, "Figure");
        assert_eq!(elem.alt.as_deref(), Some("A photo of a cat"));
        tb.end_element();
    }

    #[test]
    fn test_parent_tree_consistency() {
        let mut tb = TagBuilder::new(2);

        // Page 0: 2 elements
        tb.begin_element("Text", false, None, 0, None, 1);
        tb.end_element();
        tb.begin_element("Text", false, None, 0, None, 1);
        tb.end_element();

        // Page 1: 1 element
        tb.begin_element("Text", false, None, 1, None, 1);
        tb.end_element();

        assert_eq!(tb.page_mcid_count(0), 2);
        assert_eq!(tb.page_mcid_count(1), 1);

        // Verify mcid_to_struct entries
        assert_eq!(tb.mcid_to_struct.len(), 3);
        assert_eq!(tb.mcid_to_struct[0], (0, 0, 1)); // page 0, mcid 0, elem 1
        assert_eq!(tb.mcid_to_struct[1], (0, 1, 2)); // page 0, mcid 1, elem 2
        assert_eq!(tb.mcid_to_struct[2], (1, 0, 3)); // page 1, mcid 0, elem 3
    }
}
