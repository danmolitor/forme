# Changelog

## [0.14.0] - 2026-08-29

_Security refresh of the Docker images (the published 0.10.5 images had accumulated base-image CVEs — C grade on Docker Hub). Rejoins the shared version line; 0.11.x–0.13.x have no image._

### Changed
- Moved off Debian entirely: builder `rust:1.88-bookworm` → `rust:1.98-alpine3.24`, runtime `debian:bookworm-slim` → `alpine:3.24` (musl). Debian-slim ships `perl-base` (Essential, can't be removed) with perpetual unfixed CVEs; Alpine's runtime carries none of that surface. `curl` dropped from the runtime image (PDFium fetched in a build stage)
- PDFium `chromium/7763` → `chromium/8021` (musl builds)
- Engine picked up at 0.14.0 via path dependency (everything since 0.10.5 — see engine CHANGELOG)

## [0.10.5] - 2026-06-29

### Fixed
- Engine bumped to 0.10.5 — table header page-break fix (see engine CHANGELOG)

## [0.10.4] - 2026-06-05

_Picks up engine 0.10.4 — four layout fixes: repeating-header page-count inflation, View-around-Table auto-height inflation, SVG viewBox scaling, and column-axis `marginTop: 'auto'`._

## [0.10.3] - 2026-05-28

_Picks up engine 0.10.3 — fixes `<Text style={{ width }}>` rendering at the parent's full width inside a flex row._

## [0.10.2] - 2026-05-21

_Picks up engine 0.10.2 — flex row percentage-width and grid page-break layout fixes._

## [0.10.0] - 2026-05-19

_Picks up engine 0.10.0 — six new visual style features (opacity cascade, wordSpacing, rounded clipping, boxShadow, page backgroundImage, CSS gradients)._

## [0.9.2] - 2026-04-28

_Version bump only._

## [0.9.1] - 2026-04-06

### Changed
- Bump rasterizer base image to 0.9.1

## [0.9.0] - 2026-04-04

### Added
- `flattenForms` query parameter on `/v1/render` and `/v1/render/:slug` endpoints
- `Content-Disposition: attachment` header on `/v1/render/:slug` responses
- `NotImplemented` error variant (HTTP 501, code `NOT_IMPLEMENTED`) for consistent error shapes on resource listing endpoints
- Preset name validation on `/v1/redact` (returns 400 for unknown preset names)
- Max 20 presets limit on `/v1/redact`
- Regex compilation validation on `/v1/redact` (returns 400 for invalid patterns)

### Changed
- Certify request fields renamed: `certificatePem` → `certificate`, `privateKeyPem` → `privateKey` (old names still accepted via `serde(alias)`)
- Error messages on `/v1/merge` aligned with hosted API wording

### Removed
- `/v1/sign` endpoint (use `/v1/certify` instead)
