import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock vscode module before importing LayoutStore
vi.mock('vscode', () => {
  class EventEmitter<T> {
    private listeners: Array<(e: T) => void> = [];
    event = (listener: (e: T) => void) => {
      this.listeners.push(listener);
      return { dispose: () => {} };
    };
    fire(data: T) {
      for (const listener of this.listeners) listener(data);
    }
    dispose() {
      this.listeners = [];
    }
  }
  return { EventEmitter };
});

import { LayoutStore } from '../src/layout-store.js';
import type { LayoutInfo, ElementInfo } from '../src/layout-store.js';

function makeElement(nodeType: string, children: ElementInfo[] = []): ElementInfo {
  return {
    nodeType,
    kind: nodeType,
    x: 0,
    y: 0,
    width: 100,
    height: 50,
    style: {} as ElementInfo['style'],
    children,
  };
}

function makeLayout(pages: Array<{ elements: ElementInfo[] }> = []): LayoutInfo {
  return {
    pages: pages.map((p) => ({
      width: 595,
      height: 842,
      elements: p.elements,
    })),
  } as LayoutInfo;
}

describe('LayoutStore', () => {
  let store: LayoutStore;

  beforeEach(() => {
    store = new LayoutStore();
  });

  it('getLayout() returns null initially', () => {
    expect(store.getLayout()).toBeNull();
  });

  it('getSelection() returns null initially', () => {
    expect(store.getSelection()).toBeNull();
  });

  it('setLayout() stores layout and fires onLayoutChanged', () => {
    const fired: LayoutInfo[] = [];
    store.onLayoutChanged((layout) => fired.push(layout));

    const layout = makeLayout([{ elements: [makeElement('View')] }]);
    store.setLayout(layout);

    expect(store.getLayout()).toBe(layout);
    expect(fired).toHaveLength(1);
    expect(fired[0]).toBe(layout);
  });

  it('setSelection() stores selection and fires onSelectionChanged', () => {
    const fired: unknown[] = [];
    store.onSelectionChanged((sel) => fired.push(sel));

    const sel = {
      element: makeElement('Text'),
      pageIdx: 0,
      path: [0, 0],
      ancestors: ['Page'],
      ancestorElements: [],
    };
    store.setSelection(sel);

    expect(store.getSelection()).toBe(sel);
    expect(fired).toHaveLength(1);
    expect(fired[0]).toBe(sel);
  });

  it('setSelection(null) clears selection', () => {
    const sel = {
      element: makeElement('Text'),
      pageIdx: 0,
      path: [0, 0],
      ancestors: ['Page'],
      ancestorElements: [],
    };
    store.setSelection(sel);
    store.setSelection(null);

    expect(store.getSelection()).toBeNull();
  });

  describe('resolveElementByPath()', () => {
    it('returns null when no layout is set', () => {
      expect(store.resolveElementByPath([0, 0])).toBeNull();
    });

    it('returns null for empty path', () => {
      store.setLayout(makeLayout([{ elements: [] }]));
      expect(store.resolveElementByPath([])).toBeNull();
    });

    it('returns null for invalid page index', () => {
      store.setLayout(makeLayout([{ elements: [] }]));
      expect(store.resolveElementByPath([5])).toBeNull();
    });

    it('returns page element for single-index path', () => {
      const layout = makeLayout([{ elements: [makeElement('View')] }]);
      store.setLayout(layout);

      const result = store.resolveElementByPath([0]);
      expect(result).not.toBeNull();
      expect(result!.element.nodeType).toBe('Page');
      expect(result!.pageIdx).toBe(0);
      expect(result!.element.width).toBe(595);
      expect(result!.element.height).toBe(842);
    });

    it('resolves a nested element path', () => {
      const innerText = makeElement('Text');
      const view = makeElement('View', [innerText]);
      const layout = makeLayout([{ elements: [view] }]);
      store.setLayout(layout);

      const result = store.resolveElementByPath([0, 0, 0]);
      expect(result).not.toBeNull();
      expect(result!.element).toBe(innerText);
      expect(result!.ancestors).toEqual(['Page', 'View']);
      expect(result!.ancestorElements).toEqual([view]);
    });

    it('returns null for out-of-bounds child index', () => {
      const layout = makeLayout([{ elements: [makeElement('View')] }]);
      store.setLayout(layout);

      expect(store.resolveElementByPath([0, 5])).toBeNull();
    });
  });
});
