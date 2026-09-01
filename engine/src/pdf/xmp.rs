//! # XMP Metadata for PDF/A and PDF/UA
//!
//! Generates the XMP (Extensible Metadata Platform) XML packet required by
//! PDF/A and PDF/UA. Written as an uncompressed metadata stream referenced
//! from the Catalog via `/Metadata`.

use crate::model::{Metadata, PdfAConformance};

/// Generate XMP metadata XML for PDF/A and/or PDF/UA documents.
pub fn generate_xmp(
    metadata: &Metadata,
    conformance: Option<&PdfAConformance>,
    pdf_ua: bool,
) -> String {
    let title = metadata.title.as_deref().unwrap_or("Untitled");
    let creator = metadata.creator.as_deref().unwrap_or("Forme");

    // Build namespace declarations
    let mut namespaces = vec![
        r#"xmlns:dc="http://purl.org/dc/elements/1.1/""#.to_string(),
        r#"xmlns:xmp="http://ns.adobe.com/xap/1.0/""#.to_string(),
        r#"xmlns:pdf="http://ns.adobe.com/pdf/1.3/""#.to_string(),
    ];
    if conformance.is_some() {
        namespaces.push(r#"xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/""#.to_string());
    }
    if pdf_ua {
        namespaces.push(r#"xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/""#.to_string());
    }
    // PDF/A requires every property to belong to a predefined schema OR be
    // described by an extension schema (ISO 19005-2, 6.6.2.3.1). `pdfuaid` (the
    // PDF/UA identifier) is NOT a PDF/A-predefined schema, so when a document is
    // BOTH PDF/A and PDF/UA we must declare the pdfaExtension description for it.
    let describe_pdfua_extension = conformance.is_some() && pdf_ua;
    if describe_pdfua_extension {
        namespaces
            .push(r#"xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/""#.to_string());
        namespaces.push(r#"xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#""#.to_string());
        namespaces
            .push(r#"xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#""#.to_string());
    }

    // Build conformance entries
    let mut entries = String::new();
    if let Some(conf) = conformance {
        let (part, level) = match conf {
            PdfAConformance::A2a => ("2", "A"),
            PdfAConformance::A2b => ("2", "B"),
        };
        entries.push_str(&format!(
            "      <pdfaid:part>{}</pdfaid:part>\n      <pdfaid:conformance>{}</pdfaid:conformance>\n",
            part, level
        ));
    }
    if pdf_ua {
        entries.push_str("      <pdfuaid:part>1</pdfuaid:part>\n");
    }
    // The extension-schema description that makes `pdfuaid:part` legal under
    // PDF/A. Standard boilerplate from the PDF/UA-in-PDF/A guidance.
    if describe_pdfua_extension {
        entries.push_str(
            r#"      <pdfaExtension:schemas>
        <rdf:Bag>
          <rdf:li rdf:parseType="Resource">
            <pdfaSchema:schema>PDF/UA identification schema</pdfaSchema:schema>
            <pdfaSchema:namespaceURI>http://www.aiim.org/pdfua/ns/id/</pdfaSchema:namespaceURI>
            <pdfaSchema:prefix>pdfuaid</pdfaSchema:prefix>
            <pdfaSchema:property>
              <rdf:Seq>
                <rdf:li rdf:parseType="Resource">
                  <pdfaProperty:name>part</pdfaProperty:name>
                  <pdfaProperty:valueType>Integer</pdfaProperty:valueType>
                  <pdfaProperty:category>internal</pdfaProperty:category>
                  <pdfaProperty:description>Indicates, which part of ISO 14289 standard is followed</pdfaProperty:description>
                </rdf:li>
              </rdf:Seq>
            </pdfaSchema:property>
          </rdf:li>
        </rdf:Bag>
      </pdfaExtension:schemas>
"#,
        );
    }

    let ns_str = namespaces
        .iter()
        .enumerate()
        .map(|(i, ns)| {
            if i == 0 {
                format!("\n      {}", ns)
            } else {
                format!("      {}", ns)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // XMP packet — must not be compressed per PDF/A spec
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""{ns}>
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">{title}</rdf:li>
        </rdf:Alt>
      </dc:title>
      <dc:creator>
        <rdf:Seq>
          <rdf:li>{creator}</rdf:li>
        </rdf:Seq>
      </dc:creator>
      <xmp:CreatorTool>Forme</xmp:CreatorTool>
      <pdf:Producer>Forme</pdf:Producer>
{entries}    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        ns = ns_str,
        title = xml_escape(title),
        creator = xml_escape(creator),
        entries = entries,
    )
}

/// Escape XML special characters.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xmp_contains_pdfa_conformance() {
        let metadata = Metadata {
            title: Some("Test".to_string()),
            ..Default::default()
        };
        let xmp = generate_xmp(&metadata, Some(&PdfAConformance::A2a), false);
        assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(xmp.contains("<pdfaid:conformance>A</pdfaid:conformance>"));
        assert!(!xmp.contains("pdfuaid"));
    }

    #[test]
    fn test_xmp_escapes_special_chars() {
        let metadata = Metadata {
            title: Some("A & B <C>".to_string()),
            ..Default::default()
        };
        let xmp = generate_xmp(&metadata, Some(&PdfAConformance::A2b), false);
        assert!(xmp.contains("A &amp; B &lt;C&gt;"));
        assert!(xmp.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
    }

    #[test]
    fn test_xmp_contains_pdfua_part() {
        let metadata = Metadata {
            title: Some("Accessible".to_string()),
            ..Default::default()
        };
        let xmp = generate_xmp(&metadata, None, true);
        assert!(xmp.contains("<pdfuaid:part>1</pdfuaid:part>"));
        assert!(xmp.contains("xmlns:pdfuaid"));
        assert!(!xmp.contains("pdfaid"));
    }

    #[test]
    fn test_xmp_both_pdfa_and_pdfua() {
        let metadata = Metadata {
            title: Some("Both".to_string()),
            ..Default::default()
        };
        let xmp = generate_xmp(&metadata, Some(&PdfAConformance::A2a), true);
        assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(xmp.contains("<pdfaid:conformance>A</pdfaid:conformance>"));
        assert!(xmp.contains("<pdfuaid:part>1</pdfuaid:part>"));
        assert!(xmp.contains("xmlns:pdfaid"));
        assert!(xmp.contains("xmlns:pdfuaid"));
    }

    #[test]
    fn test_xmp_pdfua_only_no_pdfa_entries() {
        let metadata = Metadata::default();
        let xmp = generate_xmp(&metadata, None, true);
        assert!(xmp.contains("<pdfuaid:part>1</pdfuaid:part>"));
        assert!(!xmp.contains("<pdfaid:part>"));
        assert!(!xmp.contains("<pdfaid:conformance>"));
    }
}
