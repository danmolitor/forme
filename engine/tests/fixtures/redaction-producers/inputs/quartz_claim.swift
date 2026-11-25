// Write the Quartz (CoreText) redaction fixture.
//
// Usage: swift quartz_claim.swift <out.pdf> <IBMPlexSerif-Regular.ttf>
//
// Draws the fixture lines with CoreText into a CGPDFContext, so the file is
// written by Quartz PDFContext itself. The font is IBM Plex Serif (OFL),
// loaded from the file, so no Apple system font is embedded. See
// ../README.md.

import CoreGraphics
import CoreText
import Foundation

let out = CommandLine.arguments[1]
guard let provider = CGDataProvider(url: URL(fileURLWithPath: CommandLine.arguments[2]) as CFURL),
      let plex = CGFont(provider) else {
    fatalError("cannot load the font file")
}
var box = CGRect(x: 0, y: 0, width: 612, height: 792)
let info: [CFString: Any] = [kCGPDFContextTitle: "Claim Summary"]
guard let ctx = CGContext(URL(fileURLWithPath: out) as CFURL, mediaBox: &box, info as CFDictionary) else {
    fatalError("cannot create PDF context")
}

func draw(_ text: String, size: CGFloat, y: CGFloat) {
    let font = CTFontCreateWithGraphicsFont(plex, size, nil, nil)
    let attrs: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): font]
    let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attrs))
    ctx.textPosition = CGPoint(x: 72, y: y)
    CTLineDraw(line, ctx)
}

ctx.beginPDFPage(nil)
var y: CGFloat = 792 - 72
draw("Claim Summary", size: 22, y: y)
y -= 36
for line in ["Policy 7781", "Claimant", "Jane Doe", "SSN", "123-45-6789", "Email", "jane.doe@example.com", "Status Approved"] {
    draw(line, size: 14, y: y)
    y -= 22
}
ctx.endPDFPage()
ctx.closePDF()
