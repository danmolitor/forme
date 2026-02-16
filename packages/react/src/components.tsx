import type {
  DocumentProps,
  PageProps,
  ViewProps,
  TextProps,
  ImageProps,
  TableProps,
  RowProps,
  CellProps,
  FixedProps,
} from './types.js';

/** Root document container. Must be the top-level element. */
export function Document(_props: DocumentProps): null {
  return null;
}

/** A page boundary with size and margin configuration. */
export function Page(_props: PageProps): null {
  return null;
}

/** A flex container, analogous to a <div>. */
export function View(_props: ViewProps): null {
  return null;
}

/** A text node. Children are flattened to a single string. */
export function Text(_props: TextProps): null {
  return null;
}

/** An image element. */
export function Image(_props: ImageProps): null {
  return null;
}

/** A table container. Children should be <Row> elements. */
export function Table(_props: TableProps): null {
  return null;
}

/** A table row. Use header={true} for repeating header rows. */
export function Row(_props: RowProps): null {
  return null;
}

/** A table cell inside a <Row>. */
export function Cell(_props: CellProps): null {
  return null;
}

/** A fixed element that repeats on every page (header or footer). */
export function Fixed(_props: FixedProps): null {
  return null;
}

/** An explicit page break. */
export function PageBreak(_props: object): null {
  return null;
}
