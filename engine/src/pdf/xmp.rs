//! # XMP Metadata for PDF/A
//!
//! Generates the XMP (Extensible Metadata Platform) XML packet required by
//! PDF/A. Written as an uncompressed metadata stream referenced from the
//! Catalog via `/Metadata`.

use crate::model::{Metadata, PdfAConformance};

/// Generate XMP metadata XML for a PDF/A document.
pub fn generate_xmp(metadata: &Metadata, conformance: &PdfAConformance) -> String {
    let (part, conf) = match conformance {
        PdfAConformance::A2a => ("2", "A"),
        PdfAConformance::A2b => ("2", "B"),
    };

    let title = metadata.title.as_deref().unwrap_or("Untitled");
    let creator = metadata.creator.as_deref().unwrap_or("Forme");

    // XMP packet — must not be compressed per PDF/A spec
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
      xmlns:dc="http://purl.org/dc/elements/1.1/"
      xmlns:xmp="http://ns.adobe.com/xap/1.0/"
      xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
      xmlns:pdf="http://ns.adobe.com/pdf/1.3/">
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
      <pdf:Producer>Forme 0.1</pdf:Producer>
      <pdfaid:part>{part}</pdfaid:part>
      <pdfaid:conformance>{conf}</pdfaid:conformance>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        title = xml_escape(title),
        creator = xml_escape(creator),
        part = part,
        conf = conf,
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
        let xmp = generate_xmp(&metadata, &PdfAConformance::A2a);
        assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(xmp.contains("<pdfaid:conformance>A</pdfaid:conformance>"));
    }

    #[test]
    fn test_xmp_escapes_special_chars() {
        let metadata = Metadata {
            title: Some("A & B <C>".to_string()),
            ..Default::default()
        };
        let xmp = generate_xmp(&metadata, &PdfAConformance::A2b);
        assert!(xmp.contains("A &amp; B &lt;C&gt;"));
        assert!(xmp.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
    }
}
