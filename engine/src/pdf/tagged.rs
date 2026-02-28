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
}

/// A child of a structure element.
enum StructKid {
    /// Reference to another structure element by index.
    StructRef(usize),
    /// Reference to marked content on a page.
    MarkedContent { page_idx: usize, mcid: u32 },
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
}

impl TagBuilder {
    /// Create a new TagBuilder with a root "Document" structure element.
    pub fn new(num_pages: usize) -> Self {
        let root = StructElement {
            role: "Document",
            parent_idx: 0,
            kids: Vec::new(),
            alt: None,
        };
        TagBuilder {
            elements: vec![root],
            parent_stack: vec![0],
            page_mcid_counters: vec![0; num_pages],
            mcid_to_struct: Vec::new(),
            inside_paragraph: false,
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
    ) -> u32 {
        let role = self.map_role(node_type, is_header_row);
        let was_inside_paragraph = self.inside_paragraph;
        if role == "P" {
            self.inside_paragraph = true;
        }

        let parent_idx = *self.parent_stack.last().unwrap_or(&0);
        let elem_idx = self.elements.len();

        // Allocate MCID on this page
        let mcid = self.page_mcid_counters[page_idx];
        self.page_mcid_counters[page_idx] += 1;

        let elem = StructElement {
            role,
            parent_idx,
            kids: vec![StructKid::MarkedContent { page_idx, mcid }],
            alt: alt.map(|s| s.to_string()),
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

        mcid
    }

    /// End the current structure element. Must be called after `begin_element`.
    pub fn end_element(&mut self) {
        if let Some(idx) = self.parent_stack.pop() {
            // If we're leaving a paragraph, reset the flag
            if self.elements[idx].role == "P" {
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
            _ => "Div",
        }
    }

    /// Write all structure tree objects to the PDF builder.
    /// Returns `(struct_tree_root_obj_id, parent_tree_obj_id)`.
    pub fn write_objects(
        &self,
        objects: &mut Vec<super::PdfObject>,
        page_obj_ids: &[usize],
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
            let data = format!(
                "<< /Type /StructTreeRoot /K [{kids}] /ParentTree {pt} 0 R /RoleMap {rm} 0 R >>",
                kids = kids_str,
                pt = parent_tree_id,
                rm = role_map_id,
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

        let parent_tree_data = format!("<< /Nums [{}] >>", nums.trim());
        objects[parent_tree_id].data = parent_tree_data.into_bytes();

        // RoleMap: identity mapping for our standard roles
        let role_map_data = "<< /Document /Document /Div /Div /P /P /Span /Span /Table /Table \
             /TR /TR /TH /TH /TD /TD /Figure /Figure >>"
            .to_string();
        objects[role_map_id].data = role_map_data.into_bytes();

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

        let mcid = tb.begin_element("View", false, None, 0);
        assert_eq!(mcid, 0);

        let mcid2 = tb.begin_element("Text", false, None, 0);
        assert_eq!(mcid2, 1);
        tb.end_element(); // Text

        tb.end_element(); // View

        assert_eq!(tb.elements.len(), 3); // Document, Div, P
        assert_eq!(tb.elements[1].role, "Div");
        assert_eq!(tb.elements[2].role, "P");
    }

    #[test]
    fn test_nested_text_maps_to_span() {
        let mut tb = TagBuilder::new(1);

        // Outer Text → P
        let _mcid = tb.begin_element("Text", false, None, 0);
        assert_eq!(tb.elements.last().unwrap().role, "P");

        // Inner Text → Span (because inside_paragraph)
        let _mcid = tb.begin_element("Text", false, None, 0);
        assert_eq!(tb.elements.last().unwrap().role, "Span");

        tb.end_element();
        tb.end_element();
    }

    #[test]
    fn test_table_header_maps_to_th() {
        let mut tb = TagBuilder::new(1);

        tb.begin_element("Table", false, None, 0);
        tb.begin_element("TableRow", true, None, 0);

        // Cell in header row → TH
        tb.begin_element("TableCell", true, None, 0);
        assert_eq!(tb.elements.last().unwrap().role, "TH");
        tb.end_element();

        tb.end_element(); // TR
        tb.end_element(); // Table

        // Body row
        tb.begin_element("TableRow", false, None, 0);
        tb.begin_element("TableCell", false, None, 0);
        assert_eq!(tb.elements.last().unwrap().role, "TD");
        tb.end_element();
        tb.end_element();
    }

    #[test]
    fn test_figure_with_alt_text() {
        let mut tb = TagBuilder::new(1);

        tb.begin_element("Image", false, Some("A photo of a cat"), 0);
        let elem = tb.elements.last().unwrap();
        assert_eq!(elem.role, "Figure");
        assert_eq!(elem.alt.as_deref(), Some("A photo of a cat"));
        tb.end_element();
    }

    #[test]
    fn test_parent_tree_consistency() {
        let mut tb = TagBuilder::new(2);

        // Page 0: 2 elements
        tb.begin_element("Text", false, None, 0);
        tb.end_element();
        tb.begin_element("Text", false, None, 0);
        tb.end_element();

        // Page 1: 1 element
        tb.begin_element("Text", false, None, 1);
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
