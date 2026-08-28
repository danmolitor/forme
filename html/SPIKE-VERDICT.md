# Phase 0 Spike — Gate Verdict

**Date:** 2026-08-28
**Gate question:** Is the HTML→engine box mapping *fundamentally awkward*, or
*merely laborious*?
**Verdict: laborious, not awkward. Proceed to Phase 1.**

The fixture (`tests/fixtures/invoice.html` — deliberately sloppy source:
indentation, newlines between tags, comments, inline elements split across
lines, three default-margin sequences) renders to a single-page PDF whose
structure matches Chrome's print output side by side. Compare
`target/spike-invoice.pdf` (run `cargo test`) against the frozen reference
`tests/fixtures/invoice.chrome-reference.pdf`.

## Per-area assessment

| Mapping area | Verdict | Evidence |
|---|---|---|
| Whitespace collapsing | **mechanical** | A ~40-line streaming collapser (`InlineFlattener`) whose state spans inline-element boundaries. End-to-end assertions ("Due net 30 days." with single spaces, out of the *layout*, not the mapper) passed on the first full run. |
| Anonymous boxes | **mechanical** | A grouping loop in `map_children`: consecutive inline children → one Text node; whitespace-only inter-block text groups collapse to nothing and are dropped. No engine friction. |
| Margin collapse | **laborious, bounded** | Sibling collapse (max-of-positives + min-of-negatives) and parent/first-last-child collapse-through implemented in ~80 lines over the mapped tree; flex containers correctly excluded. The h1→p gap asserts exactly 16.08pt (collapsed max) vs the 28.08pt an additive engine would give. Deferred, documented: empty-block collapse-through; auto-margin pairs. |
| Inline runs | **mechanical** | HTML inline nesting flattens cleanly into the engine's flat `TextRun` model; runs carry only deltas and the engine resolves them against the Text node's style, exactly as the React serializer does. |
| Tables | **mechanical** | `thead`→`is_header`, colspan/rowspan attrs, and first-row width styles → `ColumnDef` all map 1:1. Header repetition comes free. |
| Units & em-resolution | **mechanical once ordered** | font-size resolves first (em vs parent), everything else against the element's own size. Locked by unit tests — h1's UA `0.67em` margin = 16.08pt against its own 24pt, not 8.04pt against the root. |
| UA defaults sans cascade | **fine for spike** | Tag-name lookup suffices. Phase 1's selector matching replaces it, not extends it. |

## The `<br>` probe (recorded either way, per plan)

**The engine honors `\n`.** UAX#14 emits a Mandatory break at newline and
both line-breaking paths (greedy and Knuth-Plass) flush the line on it
(`engine/src/text/mod.rs:250`, `knuth_plass.rs:203`). The mapper emits `\n`
into run content; no sibling-Text splitting was needed. Asserted: the
three-line address block renders as three stacked TextLines.

## Engine gaps found (recorded, NOT patched — per the zero-engine-changes rule)

1. **`measure_intrinsic_width` ignores `runs`** (`engine/src/layout/mod.rs:5275`
   measures only `content`, which is empty for runs-based Text; `Heading`
   falls to the children-recursion arm and measures 0 for leaf headings).
   Consequence before workaround: the flex-row address block measured 0 wide,
   rendered one character per line, 756pt tall, exploded the invoice to 4
   pages. **Workaround:** the mapper writes the concatenated run text into
   `content` as a shadow copy (layout ignores `content` when runs are
   non-empty; measurement reads it). Approximation: measured with the node's
   base style, so bold runs measure slightly narrow. **Phase 1 engine fix:**
   run-aware intrinsic measurement.
2. **No Table wrapper element is emitted** — `layout_table` pushes rows
   directly, so table-level `border`/`background` styles have no paint
   target. The fixture's `border: 1px solid #ccc` on `<table>` silently
   doesn't paint (visible in the Chrome side-by-side). Row/cell-level styles
   work. **Phase 1 engine fix or documented subset exclusion.**
3. **Robustness note:** before the width fix, the overflowing flex row
   produced an empty first page and *lost all subsequent siblings* (h1,
   table, everything). Not reproducible after the fix, but "flex item taller
   than a page swallows trailing content" deserves a targeted engine test in
   Phase 1.

## Known divergences vs the Chrome reference (deliberate)

- **Default font:** engine defaults to Helvetica; Chrome's UA default is
  Times. Structure and metrics otherwise line up.
- **Page margins:** engine default 54pt; headless Chrome's default print
  margins are smaller. `@page` parsing is Phase 2.
- Table border detail per gap #2 above.

## Scope debts (documented, deliberate)

Percentage margins/padding warn and resolve to 0. `background` shorthand
accepts bare colors only. Empty paragraphs are dropped rather than
collapse-through. Inline `<img>` (mixed into text flow) warns and skips —
standalone `<img>` works. External http(s) images warn and skip
(constitution: no fetching).

## Test surface

34 tests: 24 unit (parsing, em-resolution, UA defaults, mapping, collapse)
+ 9 integration on the fixture (single page, both margin-gap tripwires,
end-to-end whitespace, 8 table rows, H1/H2/Image node types, three-line
`<br>` address, `transform` → warnings not error, stylesheet text doesn't
render) + 1 doc test. `cargo fmt` clean, `cargo clippy --all-targets
-- -W clippy::all` zero warnings.

---

## Post-verdict addendum (2026-08-28, same day)

Phase 1 opened with the engine fixes; the three gaps above are now resolved:

1. **Fixed** — `measure_intrinsic_width` is run-aware (and Heading-aware,
   and takes the widest line of multi-line text). The mapper's
   shadow-content workaround is **deleted**; all spike tests pass against
   the real fix. Locked by three engine unit tests.
2. **Fixed** — `layout_table` emits a `Table` wrapper element per page
   fragment (clone semantics, like breakable Views). The fixture's 1px
   table border now paints. Coordinated change: `ElementNodeType` gained
   `'Table'`, the layout-shape contract test flipped, `getTableRows()`
   looks through wrappers, and pdf-testkit's extractor already accepted
   both shapes — its dogfood test passes against the new shape unchanged.
   Remaining at 0.14 release time: pdf-testkit's pinned union adds
   `'Table'` (33 entries) once the new `@formepdf/core` is installed.
3. **Fixed, and the distrust was earned** — the swallowed-siblings test
   was written before concluding anything, and it FAILED: the empty
   leading page was real, hidden by the width fix, reproducible with any
   flex line taller than a page. Root cause: `layout_flex_row`'s page-break
   check pushed a new page even when the current page was empty. Now
   guarded (`cursor.y > 0.0`, matching the other break sites) and pinned
   by `siblings_after_overflowing_flex_row_still_render`.
