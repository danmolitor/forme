// Cross-adapter parity: equivalent documents authored with @formepdf/preact
// and @formepdf/react must serialize to deep-equal document-model JSON. This
// is the load-bearing test bar for the Preact adapter — if this drifts, so
// does the "identical output" claim in the README.
//
// Each fixture pair uses its own JSX runtime via a `@jsxImportSource` pragma
// at the top of the file. The test file itself uses whichever runtime this
// package's tsconfig selects — that only affects the JSX (none is used here).
import { describe, it, expect } from 'vitest';
import { serialize as serializeReact } from '@formepdf/react';
// Import from the built package (not ../src/) so component identity matches
// the fixtures. Both sides then share the same Document/Page/etc. references.
import { serialize as serializePreact } from '@formepdf/preact';

import HelloWorldPreact from './fixtures/hello-world.preact.js';
import HelloWorldReact from './fixtures/hello-world.react.js';
import KitchenSinkPreact from './fixtures/kitchen-sink.preact.js';
import KitchenSinkReact from './fixtures/kitchen-sink.react.js';
import TablePreact from './fixtures/table.preact.js';
import TableReact from './fixtures/table.react.js';

describe('cross-adapter parity: preact vs react', () => {
  it('hello-world with default props', () => {
    const preactDoc = serializePreact(HelloWorldPreact({}) as any);
    const reactDoc = serializeReact(HelloWorldReact({}) as any);
    expect(preactDoc).toEqual(reactDoc);
  });

  it('hello-world with props (interpolation, map, conditional)', () => {
    const props = { name: 'Preact', items: ['alpha', 'beta'], showFooter: true };
    const preactDoc = serializePreact(HelloWorldPreact(props) as any);
    const reactDoc = serializeReact(HelloWorldReact(props) as any);
    expect(preactDoc).toEqual(reactDoc);
  });

  it('kitchen-sink: document props, CSS shorthands, headings, lists, inline formatting', () => {
    const preactDoc = serializePreact(KitchenSinkPreact({ discount: 25 }) as any);
    const reactDoc = serializeReact(KitchenSinkReact({ discount: 25 }) as any);
    expect(preactDoc).toEqual(reactDoc);
  });

  it('table: header row, column widths, 8 body rows', () => {
    const preactDoc = serializePreact(TablePreact() as any);
    const reactDoc = serializeReact(TableReact() as any);
    expect(preactDoc).toEqual(reactDoc);
  });
});
