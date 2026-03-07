import * as vscode from 'vscode';
import { readFile } from 'node:fs/promises';
import { renderFromFile } from '@formepdf/renderer';
import type { LayoutStore, SelectionEvent } from './layout-store.js';

const DEBOUNCE_MS = 400;

export class FormePreviewPanel {
  private static panels = new Map<string, FormePreviewPanel>();

  private panel: vscode.WebviewPanel;
  private fileUri: vscode.Uri;
  private store: LayoutStore;
  private disposables: vscode.Disposable[] = [];
  private debounceTimer: ReturnType<typeof setTimeout> | undefined;
  private statusBarItem: vscode.StatusBarItem;
  private isReady = false;
  private pendingRender = false;

  static has(fileUri: vscode.Uri): boolean {
    return FormePreviewPanel.panels.has(fileUri.toString());
  }

  static createOrShow(
    context: vscode.ExtensionContext,
    fileUri: vscode.Uri,
    toSide: boolean,
    store: LayoutStore,
  ) {
    const key = fileUri.toString();
    const existing = FormePreviewPanel.panels.get(key);
    if (existing) {
      existing.panel.reveal();
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      'formePreview',
      `Forme: ${vscode.workspace.asRelativePath(fileUri)}`,
      toSide ? vscode.ViewColumn.Beside : vscode.ViewColumn.Active,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [],
      },
    );

    const instance = new FormePreviewPanel(context, panel, fileUri, store);
    FormePreviewPanel.panels.set(key, instance);
  }

  static highlightElement(sel: SelectionEvent | null): void {
    for (const instance of FormePreviewPanel.panels.values()) {
      if (instance.isReady) {
        instance.panel.webview.postMessage({
          type: 'highlightElement',
          path: sel?.path ?? null,
          pageIdx: sel?.pageIdx ?? -1,
        });
      }
    }
  }

  static hoverElement(sel: SelectionEvent | null): void {
    for (const instance of FormePreviewPanel.panels.values()) {
      if (instance.isReady) {
        instance.panel.webview.postMessage({
          type: 'hoverElement',
          path: sel?.path ?? null,
          pageIdx: sel?.pageIdx ?? -1,
        });
      }
    }
  }

  private constructor(
    private context: vscode.ExtensionContext,
    panel: vscode.WebviewPanel,
    fileUri: vscode.Uri,
    store: LayoutStore,
  ) {
    this.panel = panel;
    this.fileUri = fileUri;
    this.store = store;

    // Status bar
    this.statusBarItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Right,
      100,
    );
    this.statusBarItem.show();

    // Load webview HTML
    this.loadWebview();

    // Listen for messages from webview
    panel.webview.onDidReceiveMessage(
      (msg) => this.handleWebviewMessage(msg),
      undefined,
      this.disposables,
    );

    // Listen for document changes (debounced)
    vscode.workspace.onDidChangeTextDocument(
      (e) => {
        if (e.document.uri.toString() === fileUri.toString()) {
          this.scheduleRender();
        }
      },
      undefined,
      this.disposables,
    );

    // Listen for saves (immediate render)
    vscode.workspace.onDidSaveTextDocument(
      (doc) => {
        if (doc.uri.toString() === fileUri.toString()) {
          if (this.debounceTimer) clearTimeout(this.debounceTimer);
          this.render();
        }
      },
      undefined,
      this.disposables,
    );

    // Cleanup
    panel.onDidDispose(() => this.dispose(), undefined, this.disposables);
  }

  private async loadWebview() {
    try {
      // Preview HTML is copied to dist/preview/ by the esbuild config
      const previewPath = vscode.Uri.joinPath(
        this.context.extensionUri,
        'dist',
        'preview',
        'index.html',
      ).fsPath;
      let html = await readFile(previewPath, 'utf-8');

      this.panel.webview.html = html;
    } catch (err) {
      this.panel.webview.html = `<!DOCTYPE html><html><body>
        <h2>Failed to load Forme preview</h2>
        <pre>${err instanceof Error ? err.message : String(err)}</pre>
      </body></html>`;
    }
  }

  private handleWebviewMessage(msg: Record<string, unknown>) {
    if (msg.type === 'ready') {
      this.isReady = true;
      // Send initial render and data state
      this.sendDataState();
      this.render();
    }

    if (msg.type === 'openFile') {
      const file = msg.file as string;
      const line = (msg.line as number) || 1;
      const column = (msg.column as number) || 1;
      const uri = vscode.Uri.file(file);
      const position = new vscode.Position(line - 1, column - 1);
      vscode.window.showTextDocument(uri, {
        selection: new vscode.Range(position, position),
        viewColumn: vscode.ViewColumn.One,
      });
    }

    if (msg.type === 'elementSelected') {
      const path = msg.path as number[];
      const sel = this.store.resolveElementByPath(path);
      if (sel) {
        this.store.setSelection(sel);
      }
    }

    if (msg.type === 'elementDeselected') {
      this.store.setSelection(null);
    }

    if (msg.type === 'setPageSize' || msg.type === 'clearPageSize') {
      // Store page size override in workspace state
      if (msg.type === 'setPageSize') {
        this.context.workspaceState.update(
          `forme.pageSize.${this.fileUri.toString()}`,
          { width: msg.width, height: msg.height },
        );
      } else {
        this.context.workspaceState.update(
          `forme.pageSize.${this.fileUri.toString()}`,
          undefined,
        );
      }
      this.render();
    }

    if (msg.type === 'updateData') {
      this.context.workspaceState.update(
        `forme.data.${this.fileUri.toString()}`,
        msg.data,
      );
      this.render();
    }
  }

  private async sendDataState() {
    // Auto-detect companion data file
    const filePath = this.fileUri.fsPath;
    const base = filePath.replace(/\.(tsx|jsx|ts|js)$/, '');

    const dataFiles = [
      `${base}.data.json`,
      `${base}-data.json`,
      `${base}.json`,
    ];

    let dataContent: string | null = null;
    let dataPath: string | null = null;
    for (const candidate of dataFiles) {
      try {
        dataContent = await readFile(candidate, 'utf-8');
        dataPath = candidate;
        break;
      } catch {
        continue;
      }
    }

    this.panel.webview.postMessage({
      type: 'init',
      hasData: !!dataContent,
      dataContent,
    });
  }

  private scheduleRender() {
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.debounceTimer = setTimeout(() => this.render(), DEBOUNCE_MS);
  }

  private async render() {
    if (!this.isReady) {
      this.pendingRender = true;
      return;
    }

    const filePath = this.fileUri.fsPath;

    // Find companion data file
    const base = filePath.replace(/\.(tsx|jsx|ts|js)$/, '');
    const dataCandidates = [
      `${base}.data.json`,
      `${base}-data.json`,
      `${base}.json`,
    ];

    let dataPath: string | undefined;
    for (const candidate of dataCandidates) {
      try {
        await readFile(candidate);
        dataPath = candidate;
        break;
      } catch {
        continue;
      }
    }

    // Check for in-memory data override
    const overrideData = this.context.workspaceState.get(
      `forme.data.${this.fileUri.toString()}`,
    );

    // Check for page size override
    const pageSize = this.context.workspaceState.get<{
      width: number;
      height: number;
    }>(`forme.pageSize.${this.fileUri.toString()}`);

    try {
      const result = await renderFromFile(filePath, {
        dataPath,
        data: overrideData,
        pageSize: pageSize ?? undefined,
      });

      const pdfBase64 = Buffer.from(result.pdf).toString('base64');

      this.panel.webview.postMessage({
        type: 'pdfData',
        pdf: pdfBase64,
        layout: result.layout,
        renderTime: result.renderTimeMs,
      });

      // Push layout to store for tree + inspector
      if (result.layout) {
        this.store.setLayout(result.layout);
      }

      const pageCount = result.layout?.pages?.length ?? 0;
      this.statusBarItem.text = `$(file-pdf) ${pageCount} page${pageCount !== 1 ? 's' : ''} · ${result.renderTimeMs}ms`;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      this.panel.webview.postMessage({
        type: 'error',
        message,
      });
      this.statusBarItem.text = `$(error) Forme: build error`;
    }
  }

  private dispose() {
    const key = this.fileUri.toString();
    FormePreviewPanel.panels.delete(key);
    this.statusBarItem.dispose();
    for (const d of this.disposables) {
      d.dispose();
    }
  }
}
