# @formepdf/preact

Preact-native adapter for the [Forme](https://formepdf.com) PDF engine.

Same component set, same props, same serialized output as [`@formepdf/react`](https://www.npmjs.com/package/@formepdf/react) — authored with Preact's JSX runtime, without the React runtime in your bundle.

## Install

```bash
npm install @formepdf/preact @formepdf/core
```

Requires `preact ^10.19.0` as a peer. `@formepdf/core` is an optional peer — install it if you want to render locally; skip it if you're serializing to JSON and POSTing to the hosted API.

## Quickstart

```tsx
/** @jsxImportSource preact */
import { Document, Page, View, Text, renderDocument } from '@formepdf/preact';

const pdf = await renderDocument(
  <Document>
    <Page size="Letter" margin={36}>
      <Text style={{ fontSize: 24, fontWeight: 'bold' }}>Invoice #001</Text>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        <Text>Widget Pro</Text>
        <Text>$49.00</Text>
      </View>
    </Page>
  </Document>
);
```

Or configure `jsxImportSource: "preact"` in your `tsconfig.json` / bundler and drop the `/** @jsxImportSource */` comment.

## Component reference

Every component in `@formepdf/react` is exported here with the same name and the same props: `Document`, `Page`, `View`, `Text`, `H1`-`H6`, `Strong`, `Em`, `Code`, `Link`, `Image`, `Svg`, `QrCode`, `Barcode`, `Canvas`, `Watermark`, `Table` / `Row` / `Cell`, `OrderedList` / `UnorderedList` / `ListItem`, `BarChart` / `LineChart` / `PieChart` / `AreaChart` / `DotPlot`, `TextField` / `Checkbox` / `Dropdown` / `RadioButton`, `Fixed`, `PageBreak`. See the [components reference](https://docs.formepdf.com/components) — all examples work identically.

## Why a native Preact package vs `preact/compat`?

You can already use `@formepdf/react` with Preact via `preact/compat` aliasing. This package exists so you don't have to: no compat shim in your bundle, no unmet-peer warnings from npm about React not being installed, and the JSX runtime is Preact-native.

## License

MIT
