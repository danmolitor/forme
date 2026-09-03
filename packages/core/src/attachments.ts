/**
 * PDF attachments + Factur-X/ZUGFeRD e-invoice container options.
 *
 * Environment-neutral (Node, browser, workers): used by every entry
 * point so the option mapping cannot drift between them.
 *
 * Tier-1 container support only: the caller supplies the invoice XML;
 * Forme embeds it as a PDF/A-3 associated file with the Factur-X XMP
 * identification. Forme does NOT generate or validate EN 16931 semantic
 * content.
 */

/** `/AFRelationship` values (PDF 2.0 §14.13, required by PDF/A-3). */
export type AfRelationship = 'Data' | 'Source' | 'Alternative' | 'Supplement' | 'Unspecified';

export interface AttachmentOptions {
  /** Filename recorded in the PDF (e.g. `factur-x.xml`). */
  name: string;
  /** File content: bytes, or a string treated as UTF-8 text. */
  data: Uint8Array | string;
  /** MIME type (PDF/A-3 requires one); default `application/octet-stream`. */
  mimeType?: string;
  /** Default `Unspecified` (the Factur-X path derives it from the profile). */
  relationship?: AfRelationship;
  /** Optional human-readable description. */
  description?: string;
  /**
   * `/Params /ModDate` as a PDF date string (`D:YYYYMMDDHHmmSSZ`).
   * Defaults to a fixed constant — never wall-clock — so output stays
   * byte-deterministic.
   */
  modDate?: string;
}

/** Factur-X / ZUGFeRD profile names, exactly as XMP spells them. */
export type FacturXProfile = 'MINIMUM' | 'BASIC WL' | 'BASIC' | 'EN 16931' | 'EXTENDED' | 'XRECHNUNG';

export interface FacturXOptions {
  /** The invoice XML (caller-supplied; Forme does not generate or validate it). */
  xml: Uint8Array | string;
  /** Factur-X profile — drives XMP `fx:ConformanceLevel` and the default `/AFRelationship`. */
  profile: FacturXProfile;
  /** Attachment filename; default `factur-x.xml` (`xrechnung.xml` for XRECHNUNG). */
  filename?: string;
  /** Override the profile-derived `/AFRelationship`. */
  relationship?: AfRelationship;
  /** XMP `fx:Version`; default `1.0`. */
  version?: string;
  /** `/Params /ModDate`; deterministic default. */
  modDate?: string;
}

function toBase64(data: Uint8Array | string): string {
  const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
  // Node fast path; browser/workers fall back to btoa over a binary string.
  const B = (globalThis as { Buffer?: { from(b: Uint8Array): { toString(e: string): string } } }).Buffer;
  if (B) return B.from(bytes).toString('base64');
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

/**
 * Map `attachments` / `facturX` options onto the serialized document.
 * Shared by the Node, browser, and worker entries.
 */
export function applyAttachmentOptions(
  doc: Record<string, unknown>,
  options?: { attachments?: AttachmentOptions[]; facturX?: FacturXOptions },
): void {
  const attachments: Record<string, unknown>[] = [];
  for (const a of options?.attachments ?? []) {
    attachments.push({
      name: a.name,
      src: toBase64(a.data),
      mimeType: a.mimeType,
      relationship: a.relationship,
      description: a.description,
      modDate: a.modDate,
    });
  }
  const fx = options?.facturX;
  if (fx) {
    const filename = fx.filename ?? (fx.profile === 'XRECHNUNG' ? 'xrechnung.xml' : 'factur-x.xml');
    attachments.push({
      name: filename,
      src: toBase64(fx.xml),
      mimeType: 'text/xml',
      relationship: fx.relationship,
      modDate: fx.modDate,
    });
    doc.zugferd = {
      conformanceLevel: fx.profile,
      documentFileName: filename,
      version: fx.version,
    };
  }
  if (attachments.length > 0) {
    doc.attachments = attachments;
  }
}
