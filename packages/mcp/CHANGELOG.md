# Changelog

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
- Output path validation prevents writing outside the working directory
- `render_custom_pdf` sandbox: strips imports/requires, shadows dangerous globals
- 30-second rendering timeout prevents hangs
- Better error messages with source/transpiled code snippets
- Trailing line comments no longer break bare JSX evaluation

### Security
- Output path traversal prevention (`../escape.pdf`, `/tmp/evil.pdf` blocked)
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
