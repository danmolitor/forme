# Handoff: Northmoor document system — 30 business document templates

## Overview

A single family of thirty print-first business document templates for a fictional
industrial manufacturer, **Northmoor Industrial Group, Inc.** The premise: an invoice
and an employment contract from this set must read as coming from the same company.
Density and page structure vary by document; the typographic spine does not.

The set is intended to be implemented as a **document/template engine** — server- or
client-rendered documents that paginate correctly, print correctly, and export to PDF.
It is not an app UI. Pagination is a first-class feature: running headers on
continuation pages, page counters (`Page 2 of 3`), tables that repeat their header row
across pages, carried-forward/brought-forward subtotal rows, and deliberate page breaks.

The thirty templates, grouped as delivered:

**Billing & finance (10)** — invoice; detailed invoice with grouped line items running
to three pages; credit note; receipt; quotation; statement of account with aged buckets;
purchase order; remittance advice; expense report; payslip.

**Correspondence & legal (10)** — letterhead (clean first page, running header after);
cover letter; internal memorandum; employment contract with numbered clauses (3 pp);
mutual NDA (2 pp); service agreement (3 pp incl. schedules); offer letter; termination
letter; reference letter; policy acknowledgement.

**Reports, logistics & certificates (10)** — annual report (cover, contents, financial
highlights); monthly operating report with KPI blocks; laboratory certificate of analysis
with reference ranges; final inspection checklist; 4 × 6 in shipping label; packing slip;
delivery note with signature block; certificate of completion; product catalogue (2 pp);
meeting minutes with action owners.

## About the design files

The three `.dc.html` files in `design/` are **design references authored in HTML** —
prototypes that show the intended look, type scale, rule set, table treatment and
pagination. They are **not production code to copy**. The task is to recreate these
documents in the target codebase's existing environment, using its established patterns.

Practical notes for whoever implements this:

- These documents are **printed and emailed as PDF**, so the natural target is a
  print/PDF pipeline rather than a screen UI. Reasonable choices: HTML + CSS Paged Media
  (WeasyPrint, Prince, Paged.js, headless Chrome `printToPDF`), React-PDF, or a
  server-side templating layer (Jinja/Liquid/Handlebars) feeding one of those.
- If the codebase already has a document or invoice renderer, extend it. Do not
  introduce a second one.
- Every document is data-driven in practice: parties, line items, totals, dates,
  references. Model them as templates + a data contract, not as hand-authored pages.
- The prototypes fake pagination by laying out fixed 816 × 1056 px page boxes side by
  side in a gallery. In production, real pagination replaces that: `@page`, repeated
  `<thead>`, `break-inside: avoid`, running headers/footers.

Each file opens standalone in a browser. `design/support.js` is the prototype runtime
that renders them; it is scaffolding, not part of the design, and should not be ported.

## Fidelity

**High fidelity.** Colours, type scale, tracking, rule weights, table treatment,
margins and copy are final and deliberate. Recreate them exactly. Where the prototype
and this README disagree, this README wins — it was written from the shipped values.

Content is plausible and internally consistent by design (see *Content model* below);
treat it as fixture data, not as strings to hard-code.

## Design tokens

### Colour

| Token | Value | Use |
| --- | --- | --- |
| `ink` | `#111111` | All body text, table heads, structural rules, signature rules |
| `paper` | `#FDFCFA` | Page stock (warm off-white) |
| `text-secondary` | `#4A463F` | Addresses, table label columns, secondary prose |
| `text-tertiary` | `#5F5A53` | Footer notes, running header text, KPI descriptions |
| `label` | `#8A857E` | Uppercase micro-labels, page counters, signature captions |
| `rule-hair` | `#E2DED7` | Footer divider, light section separators |
| `rule-row` | `#EAE6DF` | Table row separators |
| `rule-band` | `#CFC9C0` | Grouped-section header underline |
| `accent` | `#7B2233` (oxblood) | See *Accent policy* |
| `desk` | `#DCD8D1` | Gallery background only — not part of any document |

The accent is a single tokenised value. Alternates shipped as options and all
print-safe: `#14375F` ink blue, `#1F4034` forest, `#111111` all-black. Expose it as
one variable (`--ac` in the prototype); nothing else in the system is coloured.

**Grayscale requirement.** Every document must survive black-and-white printing. The
accent is only ever used at weights and sizes where it degrades to a dark neutral
without losing meaning. No information is carried by colour alone — the flagged lab
result is also bold and captioned "Exceeds"; the amount due is also the largest figure
on the page.

### Accent policy — one colour, a different job per document class

This is the core rule of the system. Do not generalise it into "accent everywhere".

| Class | Where the accent is allowed |
| --- | --- |
| Billing & finance | 4px head rule; masthead monogram fill and sub-line; 1.5px keyline above each section subtotal row; the amount-due rule and figure; the oldest aged bucket; document stamps ("Credit", "Paid in full") |
| Correspondence & legal | Head rule and masthead only. Contracts, NDAs, termination letters get **no** coloured figure, tab or stamp. Clause numbers, section rules and tables stay ink. Restraint here is deliberate. |
| Annual report | Full-bleed accent field on the cover with the monogram knocked out in outline; contents folio numbers; KPI keylines |
| Operational reports | 2px keyline above each KPI figure; grouped-section headers; a single flagged out-of-range result |
| Catalogue & lab | One solid accent tab (section marker / status), price emphasis |
| Certificate | 2px accent frame with a 1px accent outline offset 6px; accent rule under the title; accent underline beneath the recipient name |
| Shipping label | None. Pure black — it comes off a thermal printer. |

### Typography

System fonts only. Two voices, no others.

- **Georgia, 'Times New Roman', serif** — document titles, body prose, and every figure
  that matters (totals, KPI values, aged buckets, folios).
- **Helvetica, Arial, sans-serif** — labels, table heads, addresses, table body, running
  headers, page counters. Anything structural.

| Role | Spec |
| --- | --- |
| Document title (`.dtitle`) | Georgia 31px / 1.0, `letter-spacing: -0.013em`, sentence case |
| Certificate title | Georgia 46px / 1.06, `-0.02em` |
| Annual report cover title | Georgia 58px / 1.02, `-0.026em` |
| Annual report cover year | Georgia 88px / 0.8, `-0.035em` |
| Amount due / hero figure | Georgia 34px / 0.9, tabular numerals, accent |
| KPI value | Georgia 34px / 1.0, `-0.02em`, tabular numerals |
| Aged bucket figure | Georgia 19px / 1.0, tabular numerals |
| Reference number (document head, right) | Georgia 20px / 1.0, tabular numerals |
| Body prose | Georgia 10.5px / 1.7, colour `#1A1A1A`, paragraph margin 0 0 11px, measure 78–82 ch |
| Numbered clause text | Georgia 9.9px / 1.62 |
| Clause number | Helvetica bold 8.5px / 1.72, ink, fixed 36px column, 14px gap |
| Micro-label (`.lbl`) | Helvetica bold 7px / 1.4, `letter-spacing: 0.17em`, uppercase, `#8A857E` |
| Masthead wordmark | Helvetica bold 14px / 1.0, `letter-spacing: 0.26em`, uppercase |
| Masthead sub-line | Helvetica bold 7px / 1.4, `0.17em`, uppercase, accent |
| Section heading (`.sh`) | Helvetica bold 7.5px / 1.0, `0.2em`, uppercase, 1px ink bottom rule, 6px padding, 16px/9px margins |
| Table header cell | Helvetica bold 7px / 1.3, `0.16em`, uppercase, ink, 1px ink bottom rule, padding `0 10px 7px 0` |
| Table body cell | Helvetica 9.5px / 1.5, padding `6px 10px 6px 0`, 1px `#EAE6DF` bottom rule |
| Grouped-section header row | Helvetica bold 7.5px / 1.4, `0.17em`, uppercase, accent, 1px `#CFC9C0` bottom rule, 15px top padding, no fill |
| Section subtotal row | Helvetica bold 9.5px, ink, 1.5px accent top rule, no bottom rule, 10px top padding |
| Addresses / meta values | Helvetica 9.5px / 1.62 (addresses 9px / 1.7, `#4A463F`) |
| Footer note (`.foot`) | Helvetica 8.5px / 1.7, `#5F5A53`, 1px `#E2DED7` top rule, 10px padding |
| Running header (`.rhd`) | Helvetica 7.5px, `0.15em`, uppercase, `#5F5A53`; the leading `<b>` is accent at `0.2em`; 1px ink bottom rule |
| Page counter (`.pgnum`) | Helvetica 7.5px, `0.14em`, uppercase, `#8A857E` |
| Continuation marker (`.cont`) | Helvetica bold 7px, `0.17em`, uppercase, accent |
| Catalogue / status tab (`.tab`) | Helvetica bold 7px, `0.19em`, uppercase, white on accent, padding 7px 10px |
| Stamp (`.stamp`) | Helvetica bold 7.5px, `0.2em`, uppercase, accent text, 1.5px accent border, padding 7px 11px |

The whole hierarchy is carried by that contrast — 31px Georgia titles against 7px
tracked Helvetica labels, roughly 4:1. There are no coloured banners, filled cards,
rounded corners, gradients, icon sets or photography anywhere in the system, and none
should be added. All figures use tabular numerals (`font-variant-numeric: tabular-nums`)
and right alignment.

### Page geometry & spacing

| Token | Value |
| --- | --- |
| Page (Letter) | 816 × 1056 px at 96 dpi = 8.5 × 11 in |
| Page margins | 70px top, 76px left/right, 54px bottom |
| Shipping label | 384 × 576 px = 4 × 6 in, 16px padding, 1px ink border (die-cut edge) |
| Head accent rule | 4px, full bleed, top edge of every first page |
| Masthead divider | 2px ink, 20px below the masthead |
| Document head block | 27px top margin, 12px bottom padding, 1px ink bottom rule |
| Meta grid | CSS grid, 20px gap, 22px top margin |
| Section heading rhythm | 16px above, 9px below |
| Totals stack (`.tt`) | 340px wide, right-aligned, 20px top margin, 6px row padding |
| Amount-due block (`.due`) | 340px, 2px accent top rule, 13px top padding, 20px top margin |
| Signature group | flex, 52px gap, 22px top margin; rule 1px ink, 24px above, 7px caption padding |
| Page counter | absolutely positioned, 76px from each side, 26px from bottom |

Spacing scale in use: 4 · 6 · 7 · 9 · 10 · 12 · 14 · 16 · 20 · 22 · 26 · 27 · 34 · 52 · 76.
No border radius anywhere. No shadows in the documents (gallery paper shadow only).

## Structural components

Recreate these as reusable partials; every document is assembled from them.

1. **Masthead** — 44 × 44px accent square, white Georgia 20px monogram "N"; wordmark +
   accent sub-line to its right; right-aligned address block; 2px ink rule beneath.
   On continuation pages the masthead is replaced by the running header.
2. **Document head** — left: accent micro-label eyebrow (e.g. "Accounts receivable ·
   against invoice INV-4417") over the Georgia 31px title; right: micro-label over the
   Georgia 20px reference number, optionally a stamp or tab beneath. Closed by a 1px
   ink rule.
3. **Meta grid** — 3–4 column grid of micro-label + value pairs (bill to, ship to,
   dates, terms, references). Column count varies by document; the label/value pair
   does not.
4. **Data table** — tracked uppercase header on a 1px ink rule; body rows separated by
   1px `#EAE6DF`; grouped sections introduced by an accent header row and closed by a
   subtotal row with a 1.5px accent top rule. No zebra striping, no vertical rules, no
   fills.
5. **Totals stack** — 340px right-aligned label/value rows, closed by a 1px ink rule and
   a bold 10.5px total; the amount due sits below in its own accent-ruled block.
6. **Running header** — accent wordmark, parties, document reference, section, on a 1px
   ink rule. Present on every page after the first of a multi-page document.
7. **Footer note + page counter** — two-column footer note above a page counter that
   reads `Page n of m`. In the prototype the footer is pushed down with `margin-top: auto`
   inside a flex column; in production it becomes a running page footer.
8. **Signature block** — micro-label above, 1px ink rule, caption below. The two slots
   must never repeat the same word (not "Date / Date"); the caption carries either the
   named signatory, the actual date, or the expected format.
9. **Checkbox** — 11 × 11px, 1px ink border, no fill, no tick glyph.
10. **KPI block** — 2px accent top rule, micro-label, Georgia 34px figure, 8.5px
    description line.
11. **Contents row** — Georgia 11px label, dotted leader, accent Helvetica bold 9.5px
    folio.
12. **Continuation marker** — accent micro-label ("Continued on page 3 — schedule 3 and
    execution") at the foot of a page that runs on.
13. **Image placeholder (catalogue only)** — 45° `repeating-linear-gradient` in
    `#F2F1EF`/`#E6E4E0`, 1px `#CFCDC8` border, centred monospace caption naming the shot
    ("product shot — skid frame SK-96"). Replace with real photography at build time;
    do not substitute icons or illustration.

## Pagination behaviour

This is the part most likely to be lost in translation. Required behaviour:

- **First page** carries the accent head rule and full masthead. **Every subsequent page**
  carries the running header instead, with parties, reference and section.
- **Page counters** are `Page n of m` on every page, including single-page documents
  ("Page 1 of 1") — the counter is part of the system, not an overflow artefact.
- **Long tables repeat their header row** on each page (`<thead>` + `display: table-header-group`).
- **Carried forward / brought forward.** The detailed invoice (3 pp) closes page 2 with a
  bold "Carried forward to page 3 — 45,161.30" row and opens page 3 with the matching
  "Brought forward from page 2" row. Any table that breaks across a page must do this.
- **Deliberate breaks.** Groups of clauses, schedules and the execution block break at
  chosen points, not wherever the flow lands: contract clauses 1–3 / 4–8 / 9–13; NDA
  clauses 1–3 / 4–9; service agreement clauses 1–3 / 4–8 + schedules 1–2 / schedule 3 +
  execution. Signature blocks never split from the text they execute.
- **Initials line.** Multi-page legal documents carry `Initials ____ / ____` in the
  footer of every page.
- **Enclosures / copies** appear in the footer of the final page of correspondence.
- **Never clip.** Content must not silently overflow a fixed page box. In the prototype
  every page was measured to fit; in production, real flow pagination replaces the fixed
  boxes and the footer must always clear the page counter.

## Content model

Content is plausible, sequenced and self-consistent, and the numbers reconcile across
documents. Preserve this in fixtures — it is how the set is reviewed.

**Issuer.** Northmoor Industrial Group, Inc., 418 Ellingham Road, Suite 300, Rochester,
NY 14604 · (585) 274-0180 · EIN 16-1904472 · plant at 2200 Lyell Avenue, Rochester, NY 14606.
Bank: First Genesee Bank, account 8841 002937, routing 021304559.

**Counterparties.** Brightwater Foods LLC (Cleveland, OH) · Kestrel Medical Center
(Cleveland, OH) · Alder Creek Public Schools (Alder Creek, NY) · Tanaka Precision Parts,
Inc. (Elkhart, IN) · Lindqvist Instruments AB (Göteborg, Sweden).

**People.** R. A. Halloran (VP Commercial) · M. Okonjo (Controller) · K. Alvarez
(Purchasing) · E. T. Bhatt (HR Director) · P. N. Raghunathan (Quality Systems Manager) ·
H. Delacroix (Inspector, stamp 44) · D. Whitcomb (AR) · L. Nkemdirim (AP) · H. J. Speight
(General Counsel).

**Reconciling chains** (worth keeping as integration fixtures):

- INV-4417 (4,647.07 = 4,318.75 + 328.32 tax) → CN-0392 (600.48 credit) → RCT-9081
  (4,046.59 cash, balance 0.00) → all five appear as open items on the statement.
- Statement BW-0091 totals 35,731.24 and the aged buckets sum to the same figure:
  current 4,046.59 · 1–30 9,244.10 · 31–60 19,290.55 · 61–90 0.00 · 91+ 3,150.00.
- INV-4421 (3 pp): section subtotals 16,736.00 + 10,422.30 + 18,003.00 + 34,832.00 +
  5,645.00 = 85,638.30; less 5% retainage 4,281.92; plus tax on the 45,161.30 taxable
  base at 8% = 3,612.90; less 25,000.00 deposit → balance due 59,969.28.
- PO-77412 (10,940.00) → remittance RA-20268 (8,742.60 net of a 62.40 discount and a
  215.00 credit memo, with 1,240.00 held pending a material certificate).
- Release R-07 → inspection record QA-2026-2087 → packing slip PS-88214 (8 brackets back
  ordered) → shipping label (3 pieces, 418 lb) → delivery note DN-40118.
- Contract EMP-2026-0113 ↔ offer letter OFF-2026-0113 ↔ policy acknowledgement
  ACK-2026-20614 (same employee, same start date, 96,000.00 salary).
- Annual report: revenue 128,412 − cost of sales 96,670 = gross 31,742; − SG&A 20,456 =
  operating 11,286; − interest 1,204 − tax 2,164 = net 7,918.
- Payslip: gross 3,382.55 − deductions 1,150.34 = net 2,232.21; SS at 6.2% and Medicare
  at 1.45% of gross.

Date convention: `07 September 2026` in prose and meta blocks; `07 Sep 2026` in tables.
Amounts: `1,234.56`, no currency symbol in tables, `$` only on the hero amount-due figure;
negatives in parentheses. Copy is British-inflected formal register, sentence case in
titles, no exclamation marks, no marketing tone.

## Interactions & behaviour

These are documents, not an app. There is no hover state, no animation, no client-side
state in the design itself. What the implementation does need:

- **Data binding** for every field listed above; nothing is hard-coded.
- **Derived values** computed, not stored: subtotals, tax on the taxable base, retainage,
  aged buckets from invoice dates against a reporting date, page counts.
- **Print/PDF export** as the primary output path, at Letter (and A4 as a variant — keep
  page size a parameter; nothing in the layout depends on Letter's exact height).
- **The accent as one theme variable**, so a tenant or department can be re-skinned by
  changing a single value.
- **Locale/paper variants** if needed: A4 changes page height only; the type scale,
  margins and rule weights are unchanged.

## Assets

None. No photography, illustration, icon sets or logo files are used or needed.

- The logo is a **monogram**: a 44 × 44px accent square with a white Georgia "N".
  On the annual report cover it becomes a 52px outlined square, knocked out of the
  accent field. Substitute the real mark at the same optical weight.
- The **barcode** on the shipping label is a CSS `repeating-linear-gradient` standing in
  for a real symbology. Replace with a generated Code 128 / GS1-128 from the pro number.
- **Catalogue imagery** is placeholder only (see component 13); real product photography
  drops into those slots at the same aspect.
- Fonts are system fonts. Nothing to license, nothing to load.

## Files

In `design/`:

| File | Contents |
| --- | --- |
| `Billing and Finance.dc.html` | Templates 1–10, ids `1a`–`1j` (invoice → payslip), 14 pages |
| `Correspondence and Legal.dc.html` | Templates 11–20, ids `2a`–`2j` (letterhead → policy acknowledgement), 18 pages |
| `Reports Logistics and Certificates.dc.html` | Templates 21–30, ids `3a`–`3j` (annual report → meeting minutes), 14 pages + label |
| `support.js` | Prototype runtime. Scaffolding only — do not port. |

Each file opens directly in a browser and lays its documents out as a gallery of page
sheets; the `id` on each option (`1a`, `2d`, `3h`…) is the stable reference used in
review. Every document's shared styling lives in one `<style>` block at the top of each
file — that block is the typographic spine described above and is the single best
reference for exact values.
