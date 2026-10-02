Fonts for the text extraction regressions in `packages/core/tests/unicode-extraction.test.ts`.
Each font is under the SIL Open Font License; the matching OFL file is included.

Sources:
- Noto Sans Hebrew: https://fonts.gstatic.com/s/notosanshebrew/v50/or3HQ7v33eiDljA1IufXTtVf7V6RvEEdhQlk0LlGxCyaeNKYZC0sqk3xXGiXd4qtoiJltutR2g.ttf
- Noto Naskh Arabic: https://fonts.gstatic.com/s/notonaskharabic/v44/RrQ5bpV-9Dd1b1OAGA6M9PkyDuVBePeKNaxcsss0Y7bwvc5krK0z9_Mnuw.ttf
- Noto Sans Devanagari: https://fonts.gstatic.com/s/notosansdevanagari/v30/TuGoUUFzXI5FBtUq5a8bjKYTZjtRU6Sgv3NaV_SNmI0b8QQCQmHn6B2OHjbL_08AlXQly-AzoFoW4Ow.ttf

After the usual workspace and WASM build:

```sh
cd packages/core
npm test -- tests/unicode-extraction.test.ts
```

The tests compare PDF.js output with the original string, for both plain and styled text.
