# Changelog

## [0.13.0] - 2026-08-27

### Added

- **Per-line `discount` on the invoice template.** Line items accept an optional `discount` — a decimal fraction off the line total, matching `taxRate`'s existing units rather than introducing a second convention. The items table grows a fifth column and widths re-fraction from `[0.45, 0.15, 0.2, 0.2]` to `[0.38, 0.1, 0.18, 0.14, 0.2]`. Discounted lines render as `-15%` in green; undiscounted ones render an em dash, so they read as "not applicable" rather than "discounted by nothing".

  Discount applies **before** tax — tax is charged on the discounted amount.

### Changed

- **`invoiceExample` expanded from 5 line items to 19.** The old fixture produced a two-page document with enough slack that it took fourteen more lines to tip to a third page, which barely exercised the header-repetition and break-across-pages paths on a template whose main job is paging a table correctly. It now runs to three pages, with three discounted lines so the new column has more than a single row of signal.

  This is example data, not schema — nothing calling `getTemplate('invoice')` with its own data is affected.

## [0.12.1] - 2026-08-26

_Version bump only — 0.12.1 fixes LayoutInfo/ElementInfo type declarations and adds `@formepdf/core/layout` accessor helpers. No changes to this package._

## [0.12.0] - 2026-08-25

_Version bump only — 0.12.0 introduces the new `@formepdf/preact` adapter (Preact 10 authoring). No changes to this package._

## [0.11.1] - 2026-08-20

_Version bump only — engine 0.11.1 fixes SVG `stroke-linecap` / `stroke-linejoin` via `@formepdf/core`._

## [0.11.0] - 2026-08-09

_Version bump only — aligned with the 0.11.0 monorepo release (see `@formepdf/shared` and `@formepdf/svelte` new packages, `@formepdf/react` internal refactor)._

## [0.10.5] - 2026-06-29

_Version bump only — engine 0.10.5 fixes table header page-break orphan + long-header contamination via `@formepdf/core`._

## [0.10.4] - 2026-06-05

_Version bump only._

## [0.10.3] - 2026-05-28

_Version bump only._

## [0.10.2] - 2026-05-21

_Version bump only._

## [0.10.1] - 2026-05-20

_Version bump only._

## [0.10.0] - 2026-05-19

_Dependency bump only._


## [0.9.2] - 2026-04-28

_Version bump only._

## [0.9.1] - 2026-04-06

_Dependency bump only._

## [0.9.0] - 2026-04-04

_Dependency bump only._

## [0.8.3] - 2026-04-01

_Dependency bump only._

## [0.8.2] - 2026-03-30

_Dependency bump only._

## [0.8.1] - 2026-03-30

### Changed
- Version bump to match engine 0.8.1

## [0.8.0] - 2026-03-29

_Dependency bump only._

## [0.7.12] - 2026-03-24

_Dependency bump only._

## [0.7.11] - 2026-03-23

### Added
- Initial release: shared package for built-in PDF templates and Zod schemas
- Templates: invoice, receipt, report, letter, shipping-label (with theme/logo support)
- `@formepdf/templates/schemas` sub-export for Zod schemas with descriptions, fields, and examples
- Shipping label uses `<QrCode>` for tracking (replaces fake barcode rectangles)
