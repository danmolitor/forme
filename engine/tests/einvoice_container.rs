//! PDF/A-3 + e-invoice (Factur-X/ZUGFeRD) container — fails-first.
//!
//! Tier 1 scope: the CONTAINER only. Caller supplies the invoice XML; the
//! engine embeds it as an associated file in a PDF/A-3 document with the
//! Factur-X XMP identification. No CII generation, no profile semantics.
//!
//! Spec facts pinned here (verified against the veraPDF part-2/3 rule set
//! and Factur-X §6, cross-checked with Mustangproject):
//! - PDF/A-3 == PDF/A-2 except clause 6.8: embedded files are permitted,
//!   and each needs /Subtype (MIME) on the EF stream, /F + /UF and
//!   /AFRelationship on the Filespec, and association via the catalog
//!   /AF array (veraPDF 6.8-1..4).
//! - PDF/A-2 FORBIDS non-PDF/A embedded files (6.8-5) — attachments under
//!   a 2x level must error by name, not silently emit a lying file.
//! - Factur-X: filename factur-x.xml, MIME text/xml, AFRelationship
//!   /Data for MINIMUM + BASIC WL (not full invoices), /Alternative
//!   for BASIC / EN 16931 / EXTENDED; XMP fx: schema (namespace
//!   urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0# — trailing #
//!   mandatory) with DocumentType/DocumentFileName/Version/
//!   ConformanceLevel and a pdfaExtension description block.

/// Base64 of a caller-supplied XML payload (contents are opaque to the
/// container — a tiny stand-in keeps the test readable; the CI gate uses
/// the real EN 16931 corpus sample).
fn xml_b64(xml: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(xml.as_bytes())
}

const TINY_XML: &str = "<?xml version=\"1.0\"?><rsm:CrossIndustryInvoice xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\"/>";

fn einvoice_doc(pdfa: &str, profile: &str, extra: &str) -> String {
    format!(
        r#"{{
        "children": [
            {{ "kind": {{ "type": "Text", "content": "Invoice 2026-001" }}, "style": {{}}, "children": [] }}
        ],
        "metadata": {{ "title": "Invoice 2026-001", "lang": "en-US" }},
        "defaultPage": {{ "size": "A4", "margin": {{ "top": 54, "right": 54, "bottom": 54, "left": 54 }}, "wrap": true }},
        "fonts": [{{ "family": "TestSans", "src": "data:font/ttf;base64,{font}", "weight": 400, "italic": false }}],
        "defaultStyle": {{ "fontFamily": "TestSans" }},
        "pdfa": "{pdfa}",
        "attachments": [{{
            "name": "factur-x.xml",
            "src": "{xml}",
            "mimeType": "text/xml"
        }}],
        "zugferd": {{ "conformanceLevel": "{profile}" }}{extra}
    }}"#,
        font = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            include_bytes!("../fonts/NotoSans-Regular.ttf")
        ),
        xml = xml_b64(TINY_XML),
        pdfa = pdfa,
        profile = profile,
        extra = extra,
    )
}

fn pdf_text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).to_string()
}

// ── The conformant container ───────────────────────────────────────

#[test]
fn a3b_facturx_container_emits_required_structures() {
    let pdf = forme::render_json(&einvoice_doc("3b", "EN 16931", "")).expect("renders");
    let text = pdf_text(&pdf);

    // veraPDF 6.8-1: MIME subtype on the EF stream — Factur-X says text/xml.
    assert!(text.contains("/Subtype /text#2Fxml"), "EF MIME text/xml");
    // veraPDF 6.8-2: both /F and /UF.
    assert!(text.contains("/F (factur-x.xml)"), "/F filename");
    assert!(text.contains("/UF (factur-x.xml)"), "/UF filename");
    // veraPDF 6.8-3: AFRelationship — EN 16931 defaults to /Alternative.
    assert!(
        text.contains("/AFRelationship /Alternative"),
        "EN 16931 profile => /Alternative"
    );
    // veraPDF 6.8-4: document-level association — catalog /AF array.
    assert!(text.contains("/AF ["), "catalog /AF array");
    // Factur-X should-have: /Params with ModDate + Size.
    assert!(text.contains("/Params <<"), "/Params on EF stream");
    assert!(text.contains("/ModDate (D:"), "/ModDate present");
    // XMP identification.
    assert!(
        text.contains("<pdfaid:part>3</pdfaid:part>"),
        "pdfaid part 3"
    );
    assert!(
        text.contains("urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#"),
        "fx namespace with trailing #"
    );
    assert!(text.contains("<fx:DocumentType>INVOICE</fx:DocumentType>"));
    assert!(text.contains("<fx:DocumentFileName>factur-x.xml</fx:DocumentFileName>"));
    assert!(text.contains("<fx:Version>1.0</fx:Version>"));
    assert!(text.contains("<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>"));
    // PDF/A requires the extension schema DESCRIPTION for custom XMP props.
    assert!(
        text.contains("<pdfaSchema:namespaceURI>urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#</pdfaSchema:namespaceURI>"),
        "fx pdfaExtension description block"
    );
}

#[test]
fn minimum_profile_defaults_to_data_relationship() {
    // MINIMUM / BASIC WL are not full invoices: spec MANDATES /Data.
    let pdf = forme::render_json(&einvoice_doc("3b", "MINIMUM", "")).expect("renders");
    let text = pdf_text(&pdf);
    assert!(text.contains("/AFRelationship /Data"), "MINIMUM => /Data");
    assert!(!text.contains("/AFRelationship /Alternative"));
    assert!(text.contains("<fx:ConformanceLevel>MINIMUM</fx:ConformanceLevel>"));
}

#[test]
fn basic_wl_xmp_value_carries_the_space() {
    // The XMP value is "BASIC WL" — with a space (Mustang validates this).
    let pdf = forme::render_json(&einvoice_doc("3b", "BASIC WL", "")).expect("renders");
    let text = pdf_text(&pdf);
    assert!(text.contains("<fx:ConformanceLevel>BASIC WL</fx:ConformanceLevel>"));
    assert!(text.contains("/AFRelationship /Data"), "BASIC WL => /Data");
}

#[test]
fn composes_with_pdfua() {
    // A Factur-X invoice that is ALSO PDF/UA-1: both identification
    // schemas and both extension-schema descriptions in one XMP packet.
    let pdf = forme::render_json(&einvoice_doc("3a", "EN 16931", r#", "pdfUa": true"#))
        .expect("pdfa 3a + pdfUa + facturx renders");
    let text = pdf_text(&pdf);
    assert!(text.contains("<pdfaid:part>3</pdfaid:part>"));
    assert!(text.contains("<pdfaid:conformance>A</pdfaid:conformance>"));
    assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
    assert!(text.contains("<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>"));
    assert!(text.contains("http://www.aiim.org/pdfua/ns/id/"));
}

#[test]
fn deterministic_output_byte_identical_across_renders() {
    // ModDate defaults deterministically (never wall-clock) — two renders
    // of the same document are the same bytes, same as everything else.
    let json = einvoice_doc("3b", "EN 16931", "");
    let a = forme::render_json(&json).expect("render a");
    let b = forme::render_json(&json).expect("render b");
    assert_eq!(a, b, "e-invoice container must stay byte-deterministic");
}

// ── The honest boundary: errors by name ────────────────────────────

#[test]
fn pdfa2_with_attachment_errors_by_name() {
    // PDF/A-2 forbids non-PDF/A embedded files (veraPDF 6.8-5). Emitting
    // a file that LIES about conformance is worse than refusing.
    let err = forme::render_json(&einvoice_doc("2b", "EN 16931", ""))
        .expect_err("pdfa 2b + attachment must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("PDF/A-2") && msg.contains("3b"),
        "error names the constraint and the fix: {msg}"
    );
}

#[test]
fn pdfa2_with_embedded_data_errors_by_name() {
    // The pre-existing latent bug: embeddedData under PDF/A-2 produced a
    // non-conformant file silently. Now a named error.
    let json = r#"{
        "children": [{ "kind": { "type": "Text", "content": "x" }, "style": {}, "children": [] }],
        "metadata": {},
        "defaultPage": { "size": "A4", "margin": { "top": 54, "right": 54, "bottom": 54, "left": 54 }, "wrap": true },
        "pdfa": "2b",
        "embeddedData": "{\"k\":1}"
    }"#;
    let err = forme::render_json(json).expect_err("embeddedData under 2b must refuse");
    assert!(err.to_string().contains("PDF/A-2"), "{err}");
}

#[test]
fn zugferd_without_pdfa3_errors_by_name() {
    let json = einvoice_doc("3b", "EN 16931", "").replace(r#""pdfa": "3b","#, "");
    let err = forme::render_json(&json).expect_err("zugferd without pdfa must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("PDF/A-3"),
        "error names the requirement: {msg}"
    );
}

#[test]
fn zugferd_without_matching_attachment_errors_by_name() {
    let json = einvoice_doc("3b", "EN 16931", "").replace("factur-x.xml", "invoice.xml");
    // fx:DocumentFileName defaults to factur-x.xml; no attachment carries
    // that name now — refuse rather than emit an XMP pointing at nothing.
    let err = forme::render_json(&json).expect_err("no matching attachment must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("factur-x.xml"),
        "error names the filename: {msg}"
    );
}

#[test]
fn zugferd_with_unknown_profile_errors_by_name() {
    let err = forme::render_json(&einvoice_doc("3b", "COMFORT PLUS", ""))
        .expect_err("unknown conformance level must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("COMFORT PLUS"),
        "error echoes the bad value: {msg}"
    );
}

// ── Plain attachments without the e-invoice XMP ────────────────────

#[test]
fn a3b_plain_attachment_without_zugferd_is_fine() {
    // Generic PDF/A-3 attachment (no Factur-X claim): no fx XMP, but all
    // the A-3 embedded-file requirements still hold.
    let json = einvoice_doc("3b", "EN 16931", "").replace(
        r#""zugferd": { "conformanceLevel": "EN 16931" }"#,
        r#""zugferd": null"#,
    );
    let pdf = forme::render_json(&json).expect("renders");
    let text = pdf_text(&pdf);
    assert!(!text.contains("fx:ConformanceLevel"), "no fx XMP");
    assert!(text.contains("/AFRelationship"), "6.8-3 still holds");
    assert!(text.contains("/AF ["), "6.8-4 still holds");
    assert!(text.contains("<pdfaid:part>3</pdfaid:part>"));
}
