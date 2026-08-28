# forme-pdf-html

HTML + print-CSS to PDF, on the [Forme](https://formepdf.com) engine.
No headless browser. No Chromium cold start. Runs anywhere the engine
compiles — which is everywhere Rust and WASM go.

**Satori for paginated documents**: like Satori, this is a deliberately
documented *subset* of HTML/CSS — but where Satori renders one fixed-size
frame, Forme's engine is page-native. Content flows into pages; tables
repeat their headers; widows and orphans are controlled; page breaks
happen where they should.

```rust
use forme_pdf_html::{render_html, HtmlOptions};

let html = std::fs::read_to_string("invoice.html")?;
let out = render_html(&html, &HtmlOptions::default())?;
std::fs::write("invoice.pdf", &out.pdf)?;
for warning in &out.warnings {
    eprintln!("{warning}"); // everything outside the subset, named
}
```

Or from the command line:

```
forme-html invoice.html
forme-html invoice.html --css print.css --page-size Letter --margin 36
```

## The subset — the constitution

Everything in the left column works and is tested. Everything in the right
column is **out**, says so here, and lands in the `warnings` list at render
time rather than failing silently. Requests to grow the subset are welcome;
the bar is "what does an invoice, report, statement, or contract need."

### Elements

| In | Out |
|---|---|
| `div`, `section`, `article`, `header`, `footer`, `main`, `aside`, `nav`, `address`, `figure`, `blockquote`, `hr` | JavaScript of any kind (`<script>` is skipped) |
| `h1`–`h6`, `p`, `br` | `<canvas>`, `<video>`, `<audio>`, `<iframe>` |
| `span`, `b`/`strong`, `i`/`em`, `u`, `s`/`strike`/`del`, `a`, `small`, `code`, `mark`, `sub`, `sup` | forms (`<input>`, `<select>`, ...) |
| `table`, `thead`, `tbody`, `tfoot`, `tr`, `td`, `th` (+ `colspan`/`rowspan`; `<thead>` rows repeat on every page) | |
| `ul`, `ol` (+ `start`), `li` | |
| `img` (block-level) — data URIs and local files only | external `http(s)` image fetching |
| | inline `<img>` mid-paragraph — the engine's text model has no inline-replaced box (spike verdict); engine design, not a gap |
| `style` blocks (anywhere in the document) | `<link rel="stylesheet">` (pass CSS via `HtmlOptions::css`) |

### Selectors

| In | Out (warned, selector skipped) |
|---|---|
| type (`td`), class (`.total`), id (`#header`), universal (`*`) | pseudo-elements (`::before`, `::after`) |
| compounds (`td.amount`, `p.note.small`) | attribute selectors (`[type=text]`) |
| descendant (`table td`) and child (`ul > li`) combinators | sibling combinators (`+`, `~`) |
| grouping (`h1, h2`) | `:nth-of-type` and other tree pseudo-classes (pending) |
| `:first-child`, `:last-child`, `:nth-child(even\|odd\|an+b)` — the zebra-stripe family | `:hover` and interaction pseudo-classes — permanent: print has no hover |
| `!important` | |

Cascade order: UA defaults → stylesheet rules by (specificity, source
order) → inline `style=""` → `!important` stylesheet rules → `!important`
inline. `HtmlOptions::css` is appended after the document's own styles, so
its equal-specificity rules win ties. When one selector in a group is
unsupported, only that selector is skipped — the rest of the group and the
rule body still apply (friendlier than CSS's drop-the-whole-rule, and
noted in the warnings).

### Properties

| In | Out |
|---|---|
| `margin`, `padding` (+ longhands, 1–4 value shorthands), CSS margin collapsing | floats — real layout work for an old-template audience; demand decides |
| `border`, `border-top/right/bottom/left`, `border-width`, `border-color`, `border-radius` | `position: fixed/sticky`, transforms, animation |
| `border-collapse` on tables (single-owner-per-edge emulation; `tr` borders redistribute to cells; CSS's widest-border-wins conflict rule is approximated — the earlier edge wins) | |
| `width`, `height` (`px`, `pt`, `em`, `rem`, `%`, `in`, `cm`, `mm`) | CSS Grid — flex covers document layouts |
| | `min-width`/`max-width`/`min-height`/`max-height` — pending: the engine clamps these only in flex-shrink and table-row paths today |
| | `dashed`/`dotted` border styles — pending: the PDF stroke path has no dash patterns yet (style keywords parse and are ignored) |
| `font-family` (fallback chains; generics `sans-serif`/`serif`/`monospace` map to Helvetica/Times/Courier), `font-size`, `font-weight`, `font-style`, `line-height` | CSS variables |
| provided fonts: `options.fonts` / `--font Family=path.ttf` | `@font-face` fetching — remote srcs are never fetched (loud, family-naming warnings); local srcs pending (use `--font`); `@import` never fetched |
| `color`, `background-color`, `background` (solid colors) | gradients, background images — engine paint work, not a mapping gap |
| `text-align`, `text-decoration`, `text-transform` (Unicode-aware), `letter-spacing` | percentage margins/padding (warned, treated as 0) |
| `display: block / flex / none`, `flex-direction`, `justify-content`, `align-items`, `gap` | |
| `position: absolute` + `top/right/bottom/left` — containing block is the element's PARENT (not the nearest positioned ancestor); offsets without `position: absolute` warn | |

### Paged media — the point of the whole thing

| In | Pending (warned) |
|---|---|
| `@page` `size` (named, dimensions, `landscape`) and `margin` | `@page :first` / `:left` / `:right` variants |
| `break-before` / `break-after` / `break-inside: avoid` | margin boxes (`@top-center`, ...) for running headers/footers |
| legacy `page-break-*` aliases (the wkhtmltopdf-era spelling) | page counters (`counter(page)` / `counter(pages)`) |
| `orphans` / `widows` | `@page` `bleed` / `marks` |
| `<thead>` repetition across breaks, table-cell overflow preservation (engine-native) | |

Page-geometry precedence: an explicit `HtmlOptions`/CLI value overrides the
document's `@page` rule, which overrides the defaults — the same way a
print dialog overrides a stylesheet.

**Verified against Chrome print output**: the report fixture
(`tests/fixtures/report.html`) renders with page-for-page identical break
positions in both engines — `break-after` isolating the summary page,
`break-inside: avoid` moving a table to the next page whole, and the
legacy alias opening its own page. Frozen reference:
`tests/fixtures/report.chrome-reference.pdf`.

**How this compares.** wkhtmltopdf (archived 2023) and DomPDF never
shipped margin boxes or reliable break control. Chrome only gained
margin-box generated content in Chrome 131 (late 2024) and still requires
a running browser to use it. The engine underneath this crate has had
running headers/footers, header repetition, and widow/orphan control from
the start — the pending column above is CSS syntax wiring, not layout
capability.

## Using web fonts

Templates in the wild say `@import url('https://fonts.googleapis.com/...')`
or `@font-face { src: url(https://...) }`. Nothing here fetches the
network — by design — and instead of a silent Helvetica swap you get
warnings naming the import, the skipped `@font-face` family, and every
`font-family` that references it. The one-step migration:

1. Download the TTF (for Google Fonts: pick the family → Download all, or
   fetch the URL inside their CSS).
2. Hand it over under the same family name:

```bash
forme-html invoice.html --font 'Inter=fonts/Inter-Regular.ttf'                         --font 'Inter:bold=fonts/Inter-Bold.ttf'
```

```js
renderHtml(html, {
  fonts: [
    { family: 'Inter', data: fs.readFileSync('fonts/Inter-Regular.ttf') },
    { family: 'Inter', data: fs.readFileSync('fonts/Inter-Bold.ttf'), weight: 700 },
  ],
});
```

Your CSS keeps saying `font-family: "Inter", sans-serif` — the declared
name stays first in the chain, so providing the font later changes
nothing else. Fonts are subsetted automatically; only glyphs used in the
document embed.

## Semantics worth knowing

- **Whitespace collapses like a browser's.** Sloppy real-world markup —
  indentation, newlines inside inline elements — renders with single
  spaces, verified against Chrome's print output.
- **Margins collapse like CSS.** Adjacent siblings and parent/first-child
  margins collapse (max of positives + min of negatives), so default-margin
  `h1` + `p` sequences don't double-space. Flex containers don't collapse,
  per spec.
- **`em` resolves correctly**: against the parent for `font-size`, against
  the element's own computed size for everything else.
- **Warnings are the contract.** Anything outside the subset is named in
  `HtmlOutput::warnings` — templates never silently lose styling.

## Status

Pre-release (`0.0.x`), Rust API + CLI. The npm package (`@formepdf/html`,
WASM — Node, browser, edge) is the packaging phase of the roadmap.
