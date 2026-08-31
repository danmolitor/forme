import * as vscode from 'vscode';
import { FormePreviewPanel } from './preview-panel.js';
import { LayoutStore } from './layout-store.js';
import { ComponentTreeProvider } from './component-tree-provider.js';
import { InspectorViewProvider } from './inspector-view-provider.js';

// workspaceState key: the set of .html files the user has opted into
// previewing (URI strings). HTML preview is command-driven — there's no
// import signature to auto-detect on — so once previewed we remember the
// file to restore its isFormeFile affordances on reopen.
const HTML_PREVIEWS_KEY = 'forme.htmlPreviews';

// Module-scoped so detectFormeFile can consult workspaceState without
// threading context through every call site.
let extContext: vscode.ExtensionContext | undefined;

export function activate(context: vscode.ExtensionContext) {
  extContext = context;
  // One-time welcome message on first install
  const hasShownWelcome = context.globalState.get('forme.welcomeShown');
  if (!hasShownWelcome) {
    context.globalState.update('forme.welcomeShown', true);
    vscode.window.showInformationMessage(
      'Welcome to Forme! Sign up at app.formepdf.com to manage templates, get an API key, and render from your application.',
      'Sign Up',
      'Dismiss',
    ).then(selection => {
      if (selection === 'Sign Up') {
        vscode.env.openExternal(vscode.Uri.parse('https://accounts.formepdf.com/sign-up?redirect_url=https%3A%2F%2Fapp.formepdf.com%2F'));
      }
    });
  }

  const store = new LayoutStore();
  const treeProvider = new ComponentTreeProvider();
  const inspectorProvider = new InspectorViewProvider(context.extensionUri);

  // Register tree webview
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      ComponentTreeProvider.viewType,
      treeProvider,
    ),
  );

  // Register inspector webview
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      InspectorViewProvider.viewType,
      inspectorProvider,
    ),
  );

  // Tree selection → store
  context.subscriptions.push(
    treeProvider.onSelect((path) => {
      const sel = store.resolveElementByPath(path);
      if (sel) {
        store.setSelection(sel);
      }
    }),
  );

  // Tree hover → preview highlight (transient, doesn't change selection)
  context.subscriptions.push(
    treeProvider.onHover((path) => {
      if (path) {
        const sel = store.resolveElementByPath(path);
        FormePreviewPanel.hoverElement(sel);
      } else {
        FormePreviewPanel.hoverElement(null);
      }
    }),
  );

  // Store selection → inspector + preview highlight + tree sync
  context.subscriptions.push(
    store.onSelectionChanged((sel) => {
      inspectorProvider.updateElement(sel);
      FormePreviewPanel.highlightElement(sel);
      treeProvider.selectPath(sel?.path ?? null);
    }),
  );

  // Store layout → tree
  context.subscriptions.push(
    store.onLayoutChanged((layout) => {
      treeProvider.updateLayout(layout);
    }),
  );

  // Preview data content → tree data tab
  context.subscriptions.push(
    FormePreviewPanel.onDataContent((content) => {
      treeProvider.setDataContent(content);
    }),
  );

  // Tree data edit → preview re-render
  context.subscriptions.push(
    treeProvider.onDataChanged(({ data, raw }) => {
      FormePreviewPanel.updateData(data, context, raw);
    }),
  );

  // Track active Forme files for editor title button + auto-open
  updateFormeContext(vscode.window.activeTextEditor);
  maybeAutoOpen(context, vscode.window.activeTextEditor, store);
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      updateFormeContext(editor);
      maybeAutoOpen(context, editor, store);
    }),
  );

  // Register commands. Both accept an optional URI so the explorer
  // context-menu entry (which passes the clicked file) works alongside the
  // editor-title button (which uses the active editor).
  const openPreview = (toSide: boolean) => (uri?: vscode.Uri) => {
    const target = uri ?? vscode.window.activeTextEditor?.document.uri;
    if (!target) return;
    rememberIfHtml(context, target);
    FormePreviewPanel.createOrShow(context, target, toSide, store, false);
    updateFormeContext(vscode.window.activeTextEditor);
  };
  context.subscriptions.push(
    vscode.commands.registerCommand('forme.openPreview', openPreview(false)),
    vscode.commands.registerCommand('forme.openPreviewToSide', openPreview(true)),
  );

  context.subscriptions.push(store);
}

export function deactivate() {}

function updateFormeContext(editor: vscode.TextEditor | undefined) {
  const isFormeFile = editor ? detectFormeFile(editor.document) : false;
  vscode.commands.executeCommand('setContext', 'forme.isFormeFile', isFormeFile);
}

function detectFormeFile(doc: vscode.TextDocument): boolean {
  const text = doc.getText();
  if (['typescriptreact', 'javascriptreact'].includes(doc.languageId)) {
    // Matches @formepdf/react and @formepdf/preact — both ride the JSX branch,
    // the renderer picks the reconciler from the import signature.
    return text.includes('@formepdf/react') || text.includes('formepdf');
  }
  if (doc.languageId === 'python') {
    return text.includes('import formepdf') || text.includes('from formepdf');
  }
  // `.svelte`/`.vue` DO have an import signature (unlike HTML), so they
  // auto-detect like JSX. Keyed on file extension rather than languageId so
  // detection works even when the Svelte/Vue language extensions aren't
  // installed (the signature check is the real gate).
  if (doc.fileName.endsWith('.svelte')) {
    return text.includes('@formepdf/svelte');
  }
  if (doc.fileName.endsWith('.vue')) {
    return text.includes('@formepdf/vue');
  }
  // HTML has no import signature to sniff — it's a document, not a script —
  // so it counts as a Forme file only once the user has opted in by
  // previewing it at least once.
  if (doc.fileName.endsWith('.html')) {
    return isRememberedHtml(doc.uri);
  }
  return false;
}

function isRememberedHtml(uri: vscode.Uri): boolean {
  const list = extContext?.workspaceState.get<string[]>(HTML_PREVIEWS_KEY, []) ?? [];
  return list.includes(uri.toString());
}

function rememberIfHtml(context: vscode.ExtensionContext, uri: vscode.Uri): void {
  if (!uri.fsPath.endsWith('.html')) return;
  const list = context.workspaceState.get<string[]>(HTML_PREVIEWS_KEY, []);
  if (!list.includes(uri.toString())) {
    context.workspaceState.update(HTML_PREVIEWS_KEY, [...list, uri.toString()]);
  }
}

function maybeAutoOpen(
  context: vscode.ExtensionContext,
  editor: vscode.TextEditor | undefined,
  store: LayoutStore,
) {
  if (!editor) return;
  const autoOpen = vscode.workspace
    .getConfiguration('forme')
    .get<boolean>('autoOpen', false);
  if (!autoOpen) return;
  if (!detectFormeFile(editor.document)) return;
  // Always update the preview for the current file (single panel now follows the editor)
  FormePreviewPanel.createOrShow(context, editor.document.uri, true, store, true);
}
