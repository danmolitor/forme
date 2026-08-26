# Changelog

## [0.12.0] - 2026-08-25

_Version bump only — 0.12.0 introduces the new `@formepdf/preact` adapter (Preact 10 authoring). No changes to this package._

## [0.11.1] - 2026-08-20

_Version bump only — engine 0.11.1 fixes SVG `stroke-linecap` / `stroke-linejoin` via `@formepdf/core`._

## [0.11.0] - 2026-08-09

_Version bump only — aligned with the 0.11.0 monorepo release (see `@formepdf/shared` and `@formepdf/svelte` new packages, `@formepdf/react` internal refactor)._

## [0.10.5] - 2026-06-29

_Version bump only — engine 0.10.5 fixes table header page-break orphan + long-header contamination via `@formepdf/core`._

## [0.10.4] - 2026-06-05

_Bump `@formepdf/core` to 0.10.4 — picks up four engine layout fixes (Table repeating-header page-count, View-around-Table auto-height, SVG viewBox scaling, column-axis `marginTop: 'auto'`)._

## [0.10.3] - 2026-05-28

_Bump `@formepdf/core` to 0.10.3 — picks up the engine fix for `<Text style={{ width }}>` rendering at the parent's full width in a flex row._

## [0.10.2] - 2026-05-21

_Bump `@formepdf/core` to 0.10.2 — picks up engine flex row percentage-width and grid page-break layout fixes._

## [0.10.1] - 2026-05-20

_Bump `@formepdf/core` to 0.10.1 — picks up the Cloudflare Workers init crash fix and the missing `pkg-node/` tarball fix._

## [0.10.0] - 2026-05-19

### Security
- **`render_custom_pdf` sandbox hardening.** The previous regex-strip + `new Function(...)` evaluator was bypassable in one line via `new Function('return process')()`. The sync 30s timeout only covered the WASM render, not JSX evaluation — `while(true){}` hung the MCP server. `validateOutputPath` only resolved to absolute, no actual validation.
- New pipeline: AST denylist (acorn) for clear pre-execution errors; `node:worker_threads` isolation with 128 MB memory cap; `vm.Context` with `codeGeneration: false` to neuter `eval` and string-based `Function`; `vm.runInContext` with a 5s sync timeout; outer 10s wall-clock timeout backed by `worker.terminate()`. Asset src restricted to `data:` URIs (file paths and http(s) URLs blocked). Output path restricted to CWD by default; opt-in extra dirs via `FORME_MCP_OUTPUT_DIRS`.
- README rewritten to honestly describe the trust model: hardened for accidental misuse, not service-grade isolation. The sync-timeout limitation around async hangs is explicitly called out.

### Added
- `acorn` and `acorn-walk` as runtime dependencies (AST denylist)
- `FORME_MCP_OUTPUT_DIRS` environment variable for opting in additional output directories

### Changed
- `render_custom_pdf` tool description now includes a one-line trust-model statement so AI agents have context that the sandbox is local-trust, not internet-trust

## [0.9.2] - 2026-04-28

### Changed
- Sync MCP tool surfaces with the current `@formepdf/react` component set — keeps generated render prompts in lockstep with shipping components

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

## [0.7.13] - 2026-03-28

_Dependency bump only._

## [0.7.12] - 2026-03-24

_Dependency bump only._

## [0.7.11] - 2026-03-23

### Changed
- Templates and schemas imported from shared `@formepdf/templates` package (removed local copies)

## [0.7.10] - 2026-03-18

_Dependency bump only._

## [0.7.9] - 2026-03-17

_Dependency bump only._

## [0.7.8] - 2026-03-17

_Dependency bump only._

## [0.7.6] - 2026-03-13

### Added
- `extract_pdf` tool to extract embedded JSON data from Forme-generated PDFs
- `render_pdf` now auto-embeds template data for round-trip extraction

## [0.7.5] - 2026-03-12

### Removed
- Output path restriction — absolute paths now work (MCP client handles approval)

## [0.7.4] - 2026-03-11

### Added
- Theme customization for all templates (`primaryColor`, `fontFamily`, `margins`)
- Logo/image support for invoice (`company.logoUrl`) and letter (`sender.logoUrl`) templates
- `watermark` parameter on `render_pdf` tool to overlay text on every page
- MCP prompts: `generate-invoice`, `generate-report`, `create-custom-pdf`
- `Watermark`, `QrCode`, `BarChart`, `LineChart`, `PieChart`, `Canvas` available in `render_custom_pdf`
- Zod-to-JSON-Schema support for `ZodEnum`, `ZodUnion`, `ZodDefault`, `ZodLiteral`, string/number constraints
- Example data validation at startup (catches schema/example drift)

### Fixed
- Server version now reads from `package.json` instead of hardcoded `0.4.4`
- `render_custom_pdf` sandbox: strips imports/requires, shadows dangerous globals
- 30-second rendering timeout prevents hangs
- Better error messages with source/transpiled code snippets
- Trailing line comments no longer break bare JSX evaluation

### Security
- Code sandbox for `render_custom_pdf` (import/require/export stripped, globals shadowed)

## [0.7.3] - 2026-03-07

_No changes._

## [0.7.2] - 2026-03-07

_No changes._

## [0.7.1] - 2026-03-07

_No changes._

## [0.7.0] - 2026-03-06

_No changes._

## [0.6.2] - 2026-02-21

_No changes._

## [0.6.1] - 2026-02-14

_No changes._

## [0.6.0] - 2026-02-07

### Added
- Initial release: MCP server for AI-powered PDF generation
