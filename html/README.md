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
| `style` blocks (anywhere in the document); the `forme-html` CLI additionally resolves **local** `<link rel="stylesheet">` relative to the input file and inlines it at its source position | `<link rel="stylesheet">` in the library (WASM/API) — never fetched; warns with the href and the remedy (`--css` / `options.css`). Absolute `http(s)` links are never fetched even by the CLI. |

### Selectors

| In | Out (warned, selector skipped) |
|---|---|
| type (`td`), class (`.total`), id (`#header`), universal (`*`) | pseudo-elements (`::before`, `::after`) |
| compounds (`td.amount`, `p.note.small`) | attribute selectors (`[type=text]`) |
| descendant (`table td`) and child (`ul > li`) combinators | sibling combinators (`+`, `~`) |
| grouping (`h1, h2`) | `:only-child`, `:only-of-type` and remaining tree pseudo-classes (pending) |
| `:first-child`, `:last-child`, `:nth-child(even\|odd\|an+b)` — the zebra-stripe family | `:hover` and interaction pseudo-classes — permanent: print has no hover |
| `:first-of-type`, `:last-of-type`, `:nth-of-type(even\|odd\|an+b)`, `:nth-last-child(even\|odd\|an+b)`, `:nth-last-of-type(even\|odd\|an+b)` (count from the end) | |
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
| `margin`, `padding` (+ longhands, 1–4 value shorthands), CSS margin collapsing | `float` / `clear` — text-wrap-around-a-float needs per-line available width the single-width line breaker has no representation for; warned by name with a remedy (flex / `position: absolute`) rather than silently dropped |
| `border`, `border-top/right/bottom/left`, `border-width`, `border-color`, `border-radius`, `border-style` (`solid`/`dashed`/`dotted`, per side). Dash metrics match Chrome: dashed = dash 2×width / gap 1×width; dotted = round dots, diameter 1×width, 2×width centre spacing. `double`/`groove`/`ridge`/`inset`/`outset` fall back to solid. Under a dashed/dotted border, `border-radius` is dropped (per-side straight strokes), as Chrome does for dashed corners. | `position: fixed/sticky`, transforms, animation |
| `border-collapse` on tables (single-owner-per-edge emulation; `tr` borders redistribute to cells; CSS's widest-border-wins conflict rule is approximated — the earlier edge wins; `border-radius` is ignored under collapse, per spec and Chrome) | |
| `break-inside: avoid` on `<tr>` — honored: rows are atomic by engine design. A row taller than the page content area is **not** sliced across pages; it is placed whole and overflows (atomicity is the guarantee, not fragmentation). | `break-inside` on `<thead>`/`<tbody>` — pending; use it on the table. Slicing an over-tall row across pages is out of scope. |
| `width`, `height` (`px`, `pt`, `em`, `rem`, `%`, `in`, `cm`, `mm`) | CSS Grid — flex covers document layouts |
| `max-width`, `min-width`, `min-height` on block-level boxes — the centered column (`max-width` + `margin: 0 auto`) works | `max-height` — pending: down-clamping is clipping semantics; flex-item min/max — pending |
| `font-family` (fallback chains; generics `sans-serif`/`serif`/`monospace` map to Helvetica/Times/Courier), `font-size`, `font-weight`, `font-style`, `line-height` | CSS variables |
| provided fonts: `options.fonts` / `--font Family=path.ttf` | `@font-face` fetching — remote srcs are never fetched (loud, family-naming warnings); local srcs pending (use `--font`); `@import` never fetched |
| `color`, `background-color`, `background` (solid colors) | gradients, background images — engine paint work, not a mapping gap |
| `text-align` (incl. `justify` — Knuth-Plass + real inter-word distribution), `text-decoration`, `text-transform` (Unicode-aware), `letter-spacing` | percentage margins/padding (warned, treated as 0) |
| `&nbsp;` and friends: entities decode and U+00A0 survives whitespace collapsing (non-breaking, non-collapsing) | |
| `vertical-align: top/middle/bottom/baseline` on table cells + the legacy `valign` attribute. `baseline` aligns cells' first text baselines across a row (the shorter-font cells shove down; the row grows to fit, never clips). This engine's baseline sits exactly `font_size` below the line-box top — there is no font-ascent metric — so baseline alignment is **exact within the engine's own baseline model**; it diverges from Chrome only to the degree a font's ascent differs from its em-size | |
| `display: block / flex / none`, `flex-direction`, `justify-content`, `align-items`, `gap` | |
| `position: relative` + `top/right/bottom/left` — the element keeps its normal-flow space; the painted box is offset by the resolved values (`left`/`top` positive, `right`/`bottom` negative), siblings do not move | percentage offsets (warned, treated as 0); relative on inline runs |
| `position: absolute` + `top/right/bottom/left` — containing block is the **nearest positioned ancestor** (an element with `position: relative`/`absolute`), or the page content box when none exists — matching browser semantics; offsets without `position: relative`/`absolute` warn | `position: fixed`/`sticky`, `z-index` stacking order |

### Paged media — the point of the whole thing

| In | Pending (warned) |
|---|---|
| `@page` `size` (named, dimensions, `landscape`) and `margin` | `@page :first` / `:left` / `:right` variants |
| `break-before` / `break-after` / `break-inside: avoid` | margin boxes (`@top-center`, ...) for running headers/footers |
| legacy `page-break-*` aliases (the wkhtmltopdf-era spelling) | page counters (`counter(page)` / `counter(pages)`) |
| `orphans` / `widows` | `@page` `bleed` / `marks` |
| `<thead>` repetition across breaks, table-cell overflow preservation (engine-native) | |

**`@media` media-type evaluation**: this is a paged PDF renderer, so
`print` is its native media type — `@media print` (and `all`, `only
print`, comma lists with a match) join the cascade with normal
specificity; `@media screen` is excluded silently, exactly like Chrome's
print path. Templates styled for Puppeteer's print-default render
correctly. **Feature queries** `min-width` / `max-width` / `width` and
`orientation` are evaluated against the **page content box** (page size
minus margins) — a paged renderer has no window, so the page is the only
honest viewport; `orientation` derives from the page's own dimensions.
`print and (min-width: 600px)` and `and`-chains of evaluable features
evaluate fully. Anything still unmodeled (`prefers-color-scheme`, `not`,
etc.) keeps the conservative exclude-with-named-warning — rules never
apply under a condition that wasn't understood. Nested `@media` and
`@page` inside `@media print` both work.

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
