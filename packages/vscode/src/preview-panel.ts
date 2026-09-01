import * as vscode from 'vscode';
import { readFile, writeFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { dirname, basename, join } from 'node:path';
import {
  renderFromFile,
  renderFromSource,
  renderHtmlFromFile,
  renderHtmlFromSource,
  renderSvelteFromFile,
  renderSvelteFromSource,
  renderVueFromFile,
  renderVueFromSource,
  type RenderResult,
} from '@formepdf/renderer';
import type { LayoutStore, SelectionEvent } from './layout-store.js';

const DEBOUNCE_MS = 400;

export class FormePreviewPanel {
  private static currentPanel: FormePreviewPanel | undefined;

  private static readonly _onDataContent = new vscode.EventEmitter<string | null>();
  static readonly onDataContent = FormePreviewPanel._onDataContent.event;

  private panel: vscode.WebviewPanel;
  private fileUri: vscode.Uri;
  private store: LayoutStore;
  private disposables: vscode.Disposable[] = [];
  private fileDisposables: vscode.Disposable[] = [];
  private debounceTimer: ReturnType<typeof setTimeout> | undefined;
  private statusBarItem: vscode.StatusBarItem;
  private isReady = false;
  private pendingRender = false;
  private lastPdf: Uint8Array | null = null;
  private dataFilePath: string | null = null;
  private dataFileWatcher: vscode.FileSystemWatcher | null = null;
  private writingDataFile = false;

  static createOrShow(
    context: vscode.ExtensionContext,
    fileUri: vscode.Uri,
    toSide: boolean,
    store: LayoutStore,
    isAutoOpen = false,
  ) {
    // If panel exists, switch to new file or just reveal
    if (FormePreviewPanel.currentPanel) {
      const isSameFile = FormePreviewPanel.currentPanel.fileUri.toString() === fileUri.toString();

      if (!isSameFile) {
        // Just switch files, don't reveal (panel is already visible)
        FormePreviewPanel.currentPanel.switchToFile(fileUri);
      } else if (!isAutoOpen) {
        // Only reveal for manual commands on the same file
        FormePreviewPanel.currentPanel.panel.reveal(undefined, false);
      }
      // For auto-open on same file, do nothing (no reveal needed)
      return;
    }

    // Create new panel
    const viewColumn = toSide ? vscode.ViewColumn.Beside : vscode.ViewColumn.Active;
    const panel = vscode.window.createWebviewPanel(
      'formePreview',
      `Forme Preview`,
      { viewColumn, preserveFocus: isAutoOpen },
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [],
      },
    );

    FormePreviewPanel.currentPanel = new FormePreviewPanel(context, panel, fileUri, store);
  }

  static highlightElement(sel: SelectionEvent | null): void {
    if (FormePreviewPanel.currentPanel?.isReady) {
      FormePreviewPanel.currentPanel.panel.webview.postMessage({
        type: 'highlightElement',
        path: sel?.path ?? null,
        pageIdx: sel?.pageIdx ?? -1,
      });
    }
  }

  static updateData(data: unknown, context: vscode.ExtensionContext, raw?: string): void {
    const instance = FormePreviewPanel.currentPanel;
    if (!instance) return;

    context.workspaceState.update(
      `forme.data.${instance.fileUri.toString()}`,
      data,
    );

    // Write back to companion data file using the raw string to preserve formatting
    if (instance.dataFilePath && raw) {
      const uri = vscode.Uri.file(instance.dataFilePath);
      instance.writingDataFile = true;
      vscode.workspace.fs.writeFile(uri, Buffer.from(raw, 'utf-8')).then(
        () => { setTimeout(() => { instance.writingDataFile = false; }, 500); },
        () => { instance.writingDataFile = false; },
      );
    }
    instance.render();
  }

  static hoverElement(sel: SelectionEvent | null): void {
    if (FormePreviewPanel.currentPanel?.isReady) {
      FormePreviewPanel.currentPanel.panel.webview.postMessage({
        type: 'hoverElement',
        path: sel?.path ?? null,
        pageIdx: sel?.pageIdx ?? -1,
      });
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

    // Setup file-specific listeners
    this.setupFileListeners();

    // Cleanup
    panel.onDidDispose(() => this.dispose(), undefined, this.disposables);

    // Update panel title
    this.updatePanelTitle();
  }

  private async switchToFile(newFileUri: vscode.Uri) {
    // Cancel any pending renders
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = undefined;
    }

    // Clear current state
    this.store.setSelection(null);
    this.pendingRender = false;
    this.lastPdf = null;

    // Dispose file-specific listeners
    this.disposeFileListeners();

    // Clear the webview while switching
    this.panel.webview.postMessage({ type: 'clear' });

    // Update file URI
    this.fileUri = newFileUri;

    // Update panel title
    this.updatePanelTitle();

    // Setup new file listeners
    this.setupFileListeners();

    // Send new data state and render
    await this.sendDataState();
    this.render();
  }

  private updatePanelTitle() {
    this.panel.title = `Forme: ${vscode.workspace.asRelativePath(this.fileUri)}`;
  }

  private setupFileListeners() {
    // Listen for document changes (debounced, uses editor buffer)
    const changeListener = vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.uri.toString() === this.fileUri.toString()) {
        this.scheduleRender(e.document.getText());
      }
    });
    this.fileDisposables.push(changeListener);

    // Listen for saves (immediate render from disk)
    const saveListener = vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.uri.toString() === this.fileUri.toString()) {
        if (this.debounceTimer) clearTimeout(this.debounceTimer);
        this.render();
      }
    });
    this.fileDisposables.push(saveListener);
  }

  private disposeFileListeners() {
    // Dispose all file-specific listeners
    for (const d of this.fileDisposables) {
      d.dispose();
    }
    this.fileDisposables = [];

    // Dispose data file watcher
    if (this.dataFileWatcher) {
      this.dataFileWatcher.dispose();
      this.dataFileWatcher = null;
    }
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

    if (msg.type === 'downloadPdf') {
      this.downloadPdf();
    }
  }

  private async sendDataState() {
    // Auto-detect companion data file
    const filePath = this.fileUri.fsPath;
    const base = filePath.replace(/\.(tsx|jsx|ts|js|py|svelte|vue)$/, '');

    const dataFiles = [
      `${base}.data.json`,
      `${base}-data.json`,
      `${base}.json`,
    ];

    let dataContent: string | null = null;
    this.dataFilePath = null;
    for (const candidate of dataFiles) {
      try {
        dataContent = await readFile(candidate, 'utf-8');
        this.dataFilePath = candidate;
        break;
      } catch {
        continue;
      }
    }

    // Watch the companion data file for external changes
    this.setupDataFileWatcher();

    this.panel.webview.postMessage({
      type: 'init',
      hasData: !!dataContent,
      dataContent,
    });

    // Emit data content to the tree provider
    FormePreviewPanel._onDataContent.fire(dataContent);
  }

  private setupDataFileWatcher() {
    // Clean up previous watcher
    if (this.dataFileWatcher) {
      this.dataFileWatcher.dispose();
      this.dataFileWatcher = null;
    }

    if (!this.dataFilePath) return;

    const pattern = new vscode.RelativePattern(
      dirname(this.dataFilePath),
      basename(this.dataFilePath),
    );
    this.dataFileWatcher = vscode.workspace.createFileSystemWatcher(pattern);

    const onDataFileChange = async () => {
      if (!this.dataFilePath || this.writingDataFile) return;
      try {
        const content = await readFile(this.dataFilePath, 'utf-8');
        // Clear in-memory override so render uses the file
        this.context.workspaceState.update(
          `forme.data.${this.fileUri.toString()}`,
          undefined,
        );
        // Push new content to the Data tab
        this.panel.webview.postMessage({
          type: 'dataUpdate',
          content,
        });
        FormePreviewPanel._onDataContent.fire(content);
        this.render();
      } catch { /* file may have been deleted */ }
    };

    this.dataFileWatcher.onDidChange(onDataFileChange);
    this.fileDisposables.push(this.dataFileWatcher);
  }

  private scheduleRender(source?: string) {
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.debounceTimer = setTimeout(() => this.render(source), DEBOUNCE_MS);
  }

  private async render(source?: string) {
    if (!this.isReady) {
      this.pendingRender = true;
      return;
    }

    const filePath = this.fileUri.fsPath;

    // Python files use a separate render path
    if (filePath.endsWith('.py')) {
      return this.renderPython(filePath);
    }

    // HTML files go straight to the engine — no bundling, no companion data,
    // no asset resolution. The source string is the document; the engine owns
    // pagination via the document's own @page rules. Converges into the same
    // emitResult tail as JSX.
    if (filePath.endsWith('.html')) {
      try {
        const result = source
          ? renderHtmlFromSource(source)
          : await renderHtmlFromFile(filePath);
        this.emitResult(result);
      } catch (err) {
        this.emitError(err);
      }
      return;
    }

    // Find companion data file. For JSX the data is passed to a component
    // function; for `.svelte`/`.vue` it becomes the template's props. Both
    // share this resolution and the page-size override below.
    const base = filePath.replace(/\.(tsx|jsx|ts|js|py|html|svelte|vue)$/, '');
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
      const renderOpts = {
        dataPath,
        data: overrideData,
        pageSize: pageSize ?? undefined,
      };
      const dir = dirname(filePath);

      // Dispatch on input format. `.svelte`/`.vue` are SFC inputs (compiled
      // then serialized); everything else is JSX — where the renderer detects
      // react vs preact from the import signature. All branches converge on
      // the same emitResult tail. Editor-buffer content renders from source
      // when available, otherwise from disk.
      let result: RenderResult;
      if (filePath.endsWith('.svelte')) {
        result = source
          ? await renderSvelteFromSource(source, dir, renderOpts)
          : await renderSvelteFromFile(filePath, renderOpts);
      } else if (filePath.endsWith('.vue')) {
        result = source
          ? await renderVueFromSource(source, dir, renderOpts)
          : await renderVueFromFile(filePath, renderOpts);
      } else {
        result = source
          ? await renderFromSource(source, dir, { ...renderOpts, sourcefile: filePath })
          : await renderFromFile(filePath, renderOpts);
      }

      this.emitResult(result);
    } catch (err) {
      this.emitError(err);
    }
  }

  /// The format-agnostic render tail: ship the PDF + layout + warnings to
  /// the webview, feed the layout store (tree + inspector), and update the
  /// status bar. Shared verbatim by the JSX and HTML paths — any input that
  /// produces a RenderResult lands here.
  private emitResult(result: RenderResult) {
    this.lastPdf = result.pdf;
    const pdfBase64 = Buffer.from(result.pdf).toString('base64');

    this.panel.webview.postMessage({
      type: 'pdfData',
      pdf: pdfBase64,
      layout: result.layout,
      renderTime: result.renderTimeMs,
      warnings: result.warnings ?? [],
    });

    // Push layout to store for tree + inspector
    if (result.layout) {
      this.store.setLayout(result.layout);
    }

    const pageCount = result.layout?.pages?.length ?? 0;
    this.statusBarItem.text = `$(file-pdf) ${pageCount} page${pageCount !== 1 ? 's' : ''} · ${result.renderTimeMs}ms`;
  }

  private emitError(err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    this.panel.webview.postMessage({
      type: 'error',
      message,
    });
    this.statusBarItem.text = `$(error) Forme: build error`;
  }

  private async renderPython(filePath: string) {
    const start = Date.now();
    const pythonPath = await this.findPythonInterpreter();
    if (!pythonPath) {
      vscode.window.showErrorMessage(
        'Python not found. Install Python or the VS Code Python extension.',
      );
      return;
    }

    // Pass companion data file path as env var if it exists
    const base = filePath.replace(/\.py$/, '');
    const dataCandidates = [
      `${base}.data.json`,
      `${base}-data.json`,
      `${base}.json`,
    ];
    const env: Record<string, string> = { ...process.env as Record<string, string> };
    for (const candidate of dataCandidates) {
      try {
        await readFile(candidate);
        env.FORME_DATA = candidate;
        break;
      } catch {
        continue;
      }
    }

    try {
      const pdfBytes = await new Promise<Buffer>((resolve, reject) => {
        execFile(
          pythonPath,
          [filePath],
          { maxBuffer: 50 * 1024 * 1024, encoding: 'buffer', cwd: dirname(filePath), env },
          (error, stdout, stderr) => {
            if (error) {
              const stderrStr = stderr ? stderr.toString('utf-8') : '';
              reject(new Error(stderrStr || error.message));
              return;
            }
            if (stderr && stderr.length > 0) {
              const stderrStr = stderr.toString('utf-8').trim();
              // Only treat stderr as error if stdout is empty
              if (!stdout || stdout.length === 0) {
                reject(new Error(stderrStr));
                return;
              }
            }
            resolve(stdout as Buffer);
          },
        );
      });

      // Validate PDF header
      if (pdfBytes.length < 5 || pdfBytes.subarray(0, 5).toString('ascii') !== '%PDF-') {
        this.panel.webview.postMessage({
          type: 'error',
          message: 'Output is not a valid PDF. Make sure your script writes PDF bytes to stdout.\n\nExample:\n  import sys\n  sys.stdout.buffer.write(pdf_bytes)',
        });
        this.statusBarItem.text = `$(error) Forme: invalid output`;
        return;
      }

      const renderTimeMs = Date.now() - start;
      this.lastPdf = new Uint8Array(pdfBytes);
      const pdfBase64 = pdfBytes.toString('base64');

      this.panel.webview.postMessage({
        type: 'pdfData',
        pdf: pdfBase64,
        layout: null,
        renderTime: renderTimeMs,
      });

      this.statusBarItem.text = `$(file-pdf) Python · ${renderTimeMs}ms`;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      this.panel.webview.postMessage({
        type: 'error',
        message: `Script error:\n\n${message}`,
      });
      this.statusBarItem.text = `$(error) Forme: script error`;
    }
  }

  private async findPythonInterpreter(): Promise<string | null> {
    // Try the VS Code Python extension first
    try {
      const pythonExt = vscode.extensions.getExtension('ms-python.python');
      if (pythonExt) {
        if (!pythonExt.isActive) {
          await pythonExt.activate();
        }
        const api = pythonExt.exports;
        // Modern API (2023+): getActiveEnvironmentPath
        if (api?.environments?.getActiveEnvironmentPath) {
          const envPath = api.environments.getActiveEnvironmentPath();
          if (envPath?.path) return envPath.path;
        }
        // Older API: settings.getExecutionDetails
        if (api?.settings?.getExecutionDetails) {
          const details = api.settings.getExecutionDetails(this.fileUri);
          if (details?.execCommand?.[0]) return details.execCommand[0];
        }
      }
    } catch {
      // Python extension API failed, fall through to fallback
    }

    // Fallback: check python3, then python
    for (const cmd of ['python3', 'python']) {
      const found = await new Promise<boolean>((resolve) => {
        execFile(cmd, ['--version'], (error) => resolve(!error));
      });
      if (found) return cmd;
    }

    return null;
  }

  private async downloadPdf() {
    if (!this.lastPdf) return;

    const templateName = basename(this.fileUri.fsPath).replace(/\.(tsx|jsx|ts|js|py|svelte|vue)$/, '');
    const pdfName = `${templateName}.pdf`;

    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    const outputDir = workspaceFolder
      ? workspaceFolder.uri.fsPath
      : dirname(this.fileUri.fsPath);
    const outputPath = join(outputDir, pdfName);

    await writeFile(outputPath, this.lastPdf);

    const action = await vscode.window.showInformationMessage(
      `Saved to ${pdfName}`,
      'Open',
    );
    if (action === 'Open') {
      const uri = vscode.Uri.file(outputPath);
      await vscode.commands.executeCommand('revealInExplorer', uri);
    }
  }

  private dispose() {
    FormePreviewPanel.currentPanel = undefined;

    // Cancel pending renders
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }

    // Dispose file-specific listeners
    this.disposeFileListeners();

    // Dispose general listeners
    for (const d of this.disposables) {
      d.dispose();
    }

    // Dispose status bar
    this.statusBarItem.dispose();
  }
}
