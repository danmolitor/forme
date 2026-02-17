# Contributing to Forme

Thanks for your interest in contributing to Forme!

## Getting Started

1. Fork the repo
2. Clone your fork
3. Install dependencies: `npm install`
4. Build the engine: `cd engine && cargo test`
5. Build WASM: `cd packages/core && npm run build`
6. Build CLI: `cd packages/cli && npm run build`
7. Run the dev server: `node packages/cli/dist/index.js dev templates/invoice.tsx`

## Development

- **Engine (Rust):** `cd engine && cargo test`
- **React package:** `cd packages/react && npm test`
- **Core package:** `cd packages/core && npm run build`
- **CLI:** `cd packages/cli && npm run build`

## How to Contribute

**Bug reports:** Open an issue with a minimal reproduction. Include the TSX template, the expected output, and the actual output (screenshot or PDF).

**Bug fixes:** PRs welcome. Include a test that fails without the fix and passes with it.

**New features:** Open an issue first to discuss. We want to keep the API surface small and deliberate.

**Documentation:** Typo fixes, clarifications, and new examples are always welcome.

## Code Style

- Rust: `cargo fmt` and `cargo clippy`
- TypeScript: Standard formatting, strict types

## Architecture

See `CLAUDE.md` for a detailed architecture overview, known issues, and module responsibilities.
