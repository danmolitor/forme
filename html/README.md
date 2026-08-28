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
| `img` — data URIs and local files only | external `http(s)` image fetching |
| `style` blocks (anywhere in the document) | `<link rel="stylesheet">` (pass CSS via `HtmlOptions::css`) |

### Selectors

| In | Out (warned, selector skipped) |
|---|---|
| type (`td`), class (`.total`), id (`#header`), universal (`*`) | pseudo-classes and pseudo-elements (`:hover`, `:first-child`, `::before`) |
| compounds (`td.amount`, `p.note.small`) | attribute selectors (`[type=text]`) |
| descendant (`table td`) and child (`ul > li`) combinators | sibling combinators (`+`, `~`) |
| grouping (`h1, h2`) | |
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
| `margin`, `padding` (+ longhands, 1–4 value shorthands), CSS margin collapsing | floats |
| `border`, `border-top/right/bottom/left`, `border-width`, `border-color`, `border-radius` | `position: absolute/fixed/sticky`, transforms, animation |
| `width`, `height` (`px`, `pt`, `em`, `rem`, `%`, `in`, `cm`, `mm`) | CSS Grid (flex covers document layouts) |
| `font-family` (fallback chains), `font-size`, `font-weight`, `font-style`, `line-height` | CSS variables |
| `color`, `background-color`, `background` (solid colors) | gradients, background images |
| `text-align`, `text-decoration` | percentage margins/padding (warned, treated as 0) |
| `display: block / flex / none`, `flex-direction`, `justify-content`, `align-items`, `gap` | |

### Paged media — the point of the whole thing

Working today, inherited from the engine: page-native pagination, `<thead>`
repetition across page breaks, widow/orphan control, table-cell overflow
preservation.

Next phase (in progress): `@page` (size, margins, `:first`/`:left`/`:right`),
margin boxes for running headers/footers, `break-before/after/inside`,
page counters ("Page X of Y"). `@page` rules currently warn as skipped.

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
