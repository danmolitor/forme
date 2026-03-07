# Changelog

## [0.7.3] - 2026-03-07

_No changes._

## [0.7.2] - 2026-03-07

_No changes._

## [0.7.1] - 2026-03-07

### Added
- `style` prop on `<Document>` for global defaults (emits `defaultStyle` in JSON)
- `line(x1, y1, x2, y2)` convenience method on `CanvasContext`
- `defaultStyle` field on `FormeDocument` type

### Changed
- Image component JSDoc updated with concrete path examples (data URI, relative, absolute)

## [0.7.0] - 2026-03-06

_No changes._

## [0.6.2] - 2026-02-21

### Added
- `<Canvas>` component with recording `CanvasContext`
- `<BarChart>`, `<LineChart>`, `<PieChart>` chart components
- `<Watermark>` component
- `rgba()` and `rgb()` color parsing in `parseColor()`
- Chart legend flex-wrap support

## [0.6.1] - 2026-02-14

_No changes._

## [0.6.0] - 2026-02-07

### Added
- `<QrCode>` component
- CSS border shorthand parsing (`border: "1px solid #000"`)
- Edge string/array shorthands for padding and margin
- `alt` prop on `<Image>` and `<Svg>`
- `lang` prop on `<Document>`
- `href` prop on `<Image>` and `<Svg>`
- `repeat()` expansion for grid template strings
- Template proxy system (`createDataProxy`, `serializeTemplate`)
- Expression helpers (`expr.ts`) for template comparisons and arithmetic

## [0.4.0] - 2025-12-13

### Added
- `Font.register()` static API for custom font registration
- `<Document fonts={[...]}>` prop for per-document fonts
- Font merge strategy (global + document fonts keyed by family:weight:italic)

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- JSX component library: `<Document>`, `<Page>`, `<View>`, `<Text>`, `<Image>`, `<Svg>`, `<Link>`
- `serialize()` function to convert JSX tree to JSON document
- Style shorthand properties
- `<Page margin>` accepts strings and arrays

### Changed
- Package scope renamed from `@forme/react` to `@formepdf/react`
