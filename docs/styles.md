# Styles

Every Forme component accepts a `style` prop with CSS-like properties. Styles are passed as a JavaScript object.

```tsx
<View style={{ flexDirection: 'row', gap: 12, padding: 16, backgroundColor: '#f8fafc' }}>
  <Text style={{ fontSize: 14, fontWeight: 700, color: '#1e293b' }}>Hello</Text>
</View>
```

## Layout

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `width` | `number \| string` | auto | Element width in points, or a percentage string (e.g., `"50%"`) |
| `height` | `number \| string` | auto | Element height in points, or a percentage string |
| `minWidth` | `number \| string` | - | Minimum width |
| `minHeight` | `number \| string` | - | Minimum height |
| `maxWidth` | `number \| string` | - | Maximum width |
| `maxHeight` | `number \| string` | - | Maximum height |
| `flexDirection` | `"column"` \| `"row"` \| `"column-reverse"` \| `"row-reverse"` | `"column"` | Direction of flex layout |
| `flexGrow` | `number` | `0` | How much this element grows to fill available space |
| `flexShrink` | `number` | `1` | How much this element shrinks when space is tight |
| `flexBasis` | `number \| string` | auto | Initial size before growing/shrinking |
| `flexWrap` | `"nowrap"` \| `"wrap"` \| `"wrap-reverse"` | `"nowrap"` | Whether children wrap to new lines |
| `justifyContent` | `"flex-start"` \| `"flex-end"` \| `"center"` \| `"space-between"` \| `"space-around"` \| `"space-evenly"` | `"flex-start"` | Alignment along the main axis |
| `alignItems` | `"flex-start"` \| `"flex-end"` \| `"center"` \| `"stretch"` \| `"baseline"` | `"stretch"` | Alignment along the cross axis |
| `alignSelf` | `"flex-start"` \| `"flex-end"` \| `"center"` \| `"stretch"` \| `"baseline"` | - | Override parent's `alignItems` for this element |
| `gap` | `number` | `0` | Space between children (both row and column gap) |
| `rowGap` | `number` | - | Space between rows (overrides `gap` for row direction) |
| `columnGap` | `number` | - | Space between columns (overrides `gap` for column direction) |

### Layout examples

```tsx
{/* Centered content */}
<View style={{ justifyContent: 'center', alignItems: 'center', height: 200 }}>
  <Text>Centered</Text>
</View>

{/* Row with space between */}
<View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
  <Text>Left</Text>
  <Text>Right</Text>
</View>

{/* Equal-width columns */}
<View style={{ flexDirection: 'row', gap: 16 }}>
  <View style={{ flexGrow: 1 }}><Text>Column 1</Text></View>
  <View style={{ flexGrow: 1 }}><Text>Column 2</Text></View>
  <View style={{ flexGrow: 1 }}><Text>Column 3</Text></View>
</View>
```

## Spacing

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `padding` | `number \| { top, right, bottom, left }` | `0` | Inner spacing. A single number applies to all sides. |
| `margin` | `number \| { top, right, bottom, left }` | `0` | Outer spacing. A single number applies to all sides. |

Margins in Forme are always additive (like flexbox gap). There is no CSS margin collapsing.

### Spacing examples

```tsx
{/* Uniform padding */}
<View style={{ padding: 16 }}>
  <Text>16pt padding on all sides</Text>
</View>

{/* Per-side margin */}
<View style={{ margin: { top: 24, right: 0, bottom: 12, left: 0 } }}>
  <Text>Different top and bottom margins</Text>
</View>
```

## Typography

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `fontSize` | `number` | `12` | Font size in points |
| `fontFamily` | `string` | `"Helvetica"` | Font family name. Standard PDF fonts: Helvetica, Times, Courier. Custom TTF fonts can be embedded. |
| `fontWeight` | `number \| "normal" \| "bold"` | `"normal"` | Font weight. Numeric values (100-900) or keywords. |
| `fontStyle` | `"normal"` \| `"italic"` \| `"oblique"` | `"normal"` | Italic or oblique text |
| `lineHeight` | `number` | `1.2` | Line height as a multiplier of font size |
| `textAlign` | `"left"` \| `"center"` \| `"right"` \| `"justify"` | `"left"` | Horizontal text alignment |
| `letterSpacing` | `number` | `0` | Extra space between characters in points |
| `textDecoration` | `"none"` \| `"underline"` \| `"line-through"` | `"none"` | Text decoration |
| `textTransform` | `"none"` \| `"uppercase"` \| `"lowercase"` \| `"capitalize"` | `"none"` | Text case transformation |

### Typography examples

```tsx
{/* Heading */}
<Text style={{ fontSize: 24, fontWeight: 700, letterSpacing: -0.5 }}>
  Large Heading
</Text>

{/* Body text */}
<Text style={{ fontSize: 10, lineHeight: 1.6, color: '#475569' }}>
  Body text with comfortable line height for readability.
</Text>

{/* Label */}
<Text style={{ fontSize: 8, textTransform: 'uppercase', letterSpacing: 1, color: '#94a3b8' }}>
  Section Label
</Text>
```

## Background and Border

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `color` | `string` | `"#000000"` | Text color. Hex string (e.g., `"#1e293b"`). |
| `backgroundColor` | `string` | - | Background fill color |
| `opacity` | `number` | `1.0` | Element opacity (0.0 to 1.0) |
| `borderWidth` | `number \| { top, right, bottom, left }` | `0` | Border width in points. A single number applies to all sides. |
| `borderColor` | `string \| { top, right, bottom, left }` | `"#000000"` | Border color. A single string applies to all sides. |
| `borderRadius` | `number \| { topLeft, topRight, bottomRight, bottomLeft }` | `0` | Corner radius in points. A single number applies to all corners. |

### Background and border examples

```tsx
{/* Card with rounded corners */}
<View style={{ padding: 16, backgroundColor: '#f8fafc', borderRadius: 8, borderWidth: 1, borderColor: '#e2e8f0' }}>
  <Text>Card content</Text>
</View>

{/* Bottom border only (like an underline) */}
<View style={{ paddingBottom: 8, borderWidth: { top: 0, right: 0, bottom: 1, left: 0 }, borderColor: '#e2e8f0' }}>
  <Text>Underlined section</Text>
</View>

{/* Accent left border */}
<View style={{ paddingLeft: 12, borderWidth: { top: 0, right: 0, bottom: 0, left: 3 }, borderColor: '#2563eb' }}>
  <Text>Highlighted note</Text>
</View>
```

## Page Behavior

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `wrap` | `boolean` | `true` | Whether this element can break across pages. Set `false` to keep it on one page. |
| `breakBefore` | `boolean` | `false` | Force a page break before this element |
| `minWidowLines` | `number` | - | Minimum number of lines to keep at the top of a new page after a text break |
| `minOrphanLines` | `number` | - | Minimum number of lines to keep at the bottom of a page before a text break |

### Page behavior examples

```tsx
{/* Non-breakable card */}
<View wrap={false} style={{ padding: 16, backgroundColor: '#f1f5f9' }}>
  <Text style={{ fontWeight: 700 }}>Summary</Text>
  <Text>This entire block stays on one page.</Text>
</View>

{/* Force a new page before a section */}
<View style={{ breakBefore: true }}>
  <Text style={{ fontSize: 20, fontWeight: 700 }}>Chapter 2</Text>
</View>
```

## Style Inheritance

Some properties are inherited from parent elements, matching CSS behavior:

**Inherited:** `color`, `fontFamily`, `fontSize`, `fontWeight`, `fontStyle`, `lineHeight`, `textAlign`, `letterSpacing`

**Not inherited:** `width`, `height`, `padding`, `margin`, `backgroundColor`, `borderWidth`, `borderColor`, `borderRadius`, `flexDirection`, `gap`, `opacity`

```tsx
{/* fontSize and color are inherited by child Text elements */}
<View style={{ fontSize: 10, color: '#475569' }}>
  <Text>This text is 10pt and gray.</Text>
  <Text style={{ fontWeight: 700 }}>This text is also 10pt and gray, but bold.</Text>
</View>
```
