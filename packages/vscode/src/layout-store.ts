import * as vscode from 'vscode';
import type {
  LayoutInfo,
  PageInfo,
  ElementInfo,
} from '@formepdf/core';

export type { LayoutInfo, PageInfo, ElementInfo };

// Alias for use in tree/inspector (same shape as ElementInfo)
export type LayoutElement = ElementInfo;
export type LayoutPage = PageInfo;

export interface SelectionEvent {
  element: LayoutElement;
  pageIdx: number;
  path: number[];
  ancestors: string[];
  ancestorElements: LayoutElement[];
}

export class LayoutStore {
  private layoutData: LayoutInfo | null = null;
  private selection: SelectionEvent | null = null;

  private readonly _onLayoutChanged = new vscode.EventEmitter<LayoutInfo>();
  readonly onLayoutChanged = this._onLayoutChanged.event;

  private readonly _onSelectionChanged =
    new vscode.EventEmitter<SelectionEvent | null>();
  readonly onSelectionChanged = this._onSelectionChanged.event;

  setLayout(layout: LayoutInfo): void {
    this.layoutData = layout;
    this._onLayoutChanged.fire(layout);
  }

  getLayout(): LayoutInfo | null {
    return this.layoutData;
  }

  setSelection(sel: SelectionEvent | null): void {
    this.selection = sel;
    this._onSelectionChanged.fire(sel);
  }

  getSelection(): SelectionEvent | null {
    return this.selection;
  }

  resolveElementByPath(path: number[]): SelectionEvent | null {
    if (!this.layoutData || !path || path.length === 0) return null;

    const pageIdx = path[0];
    const page = this.layoutData.pages[pageIdx];
    if (!page) return null;

    if (path.length === 1) {
      return {
        element: {
          nodeType: 'Page',
          kind: 'Page',
          x: 0,
          y: 0,
          width: page.width,
          height: page.height,
          style: page.elements[0]?.style || {} as ElementInfo['style'],
          children: page.elements,
        } as ElementInfo,
        pageIdx,
        path,
        ancestors: [],
        ancestorElements: [],
      };
    }

    let current: LayoutElement[] | undefined = page.elements;
    let element: LayoutElement | null = null;
    const ancestors: string[] = ['Page'];
    const ancestorElements: LayoutElement[] = [];

    for (let i = 1; i < path.length; i++) {
      const idx = path[i];
      if (!current || idx >= current.length) return null;
      element = current[idx];
      if (i < path.length - 1) {
        ancestors.push(element.nodeType);
        ancestorElements.push(element);
        current = element.children;
      }
    }

    return element
      ? { element, pageIdx, path, ancestors, ancestorElements }
      : null;
  }

  dispose(): void {
    this._onLayoutChanged.dispose();
    this._onSelectionChanged.dispose();
  }
}
