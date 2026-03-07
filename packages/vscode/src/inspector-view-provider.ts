import * as vscode from 'vscode';
import type { SelectionEvent, LayoutElement } from './layout-store.js';

export class InspectorViewProvider implements vscode.WebviewViewProvider {
  static readonly viewType = 'forme.inspector';

  private view?: vscode.WebviewView;

  constructor(private readonly extensionUri: vscode.Uri) {}

  resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken,
  ): void {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
    };

    webviewView.webview.html = this.getHtml();

    webviewView.webview.onDidReceiveMessage((msg) => {
      if (msg.type === 'openFile') {
        const uri = vscode.Uri.file(msg.file);
        const position = new vscode.Position(msg.line - 1, msg.column - 1);
        vscode.window.showTextDocument(uri, {
          selection: new vscode.Range(position, position),
          viewColumn: vscode.ViewColumn.One,
        });
      }

      if (msg.type === 'copyStyle') {
        vscode.env.clipboard.writeText(msg.text);
        vscode.window.showInformationMessage('Style copied to clipboard');
      }
    });
  }

  updateElement(sel: SelectionEvent | null): void {
    if (!this.view) return;
    this.view.webview.postMessage({
      type: 'updateElement',
      selection: sel
        ? {
            element: sel.element,
            ancestors: sel.ancestors,
            ancestorElements: sel.ancestorElements,
          }
        : null,
    });
  }

  private getHtml(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  :root {
    --font-mono: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size);
    color: var(--vscode-foreground);
    background: var(--vscode-sideBar-background);
    overflow-y: auto;
  }

  .empty-state {
    padding: 20px 16px;
    text-align: center;
    color: var(--vscode-descriptionForeground);
    font-size: 12px;
  }

  .inspector-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--vscode-panel-border);
  }
  .breadcrumb {
    font-size: 11px;
    color: var(--vscode-descriptionForeground);
    font-family: var(--font-mono);
    margin-bottom: 2px;
  }
  .node-label {
    font-weight: 600;
    font-size: 13px;
  }
  .node-dims {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--vscode-descriptionForeground);
  }
  .node-source {
    margin-top: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .source-path {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--vscode-descriptionForeground);
  }
  .open-editor-btn {
    background: var(--vscode-button-secondaryBackground);
    border: none;
    border-radius: 3px;
    padding: 1px 6px;
    font-size: 10px;
    color: var(--vscode-button-secondaryForeground);
    cursor: pointer;
  }
  .open-editor-btn:hover {
    background: var(--vscode-button-secondaryHoverBackground);
  }

  /* Box Model */
  .box-model {
    padding: 16px;
    border-bottom: 1px solid var(--vscode-panel-border);
  }
  .section-title {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--vscode-descriptionForeground);
    margin-bottom: 12px;
  }
  .box-model-diagram {
    position: relative;
    width: 100%;
    aspect-ratio: 1.6;
    font-family: var(--font-mono);
    font-size: 10px;
    user-select: none;
  }
  .box-margin {
    position: absolute; inset: 0;
    background: rgba(251, 146, 60, 0.12);
    border: 1px dashed rgba(251, 146, 60, 0.4);
    border-radius: 4px;
  }
  .box-border-area {
    position: absolute; inset: 18%;
    background: rgba(96, 165, 250, 0.12);
    border: 1px solid rgba(96, 165, 250, 0.4);
    border-radius: 3px;
  }
  .box-padding {
    position: absolute; inset: 30%;
    background: rgba(74, 222, 128, 0.12);
    border: 1px dashed rgba(74, 222, 128, 0.4);
    border-radius: 2px;
  }
  .box-content {
    position: absolute; inset: 42%;
    background: rgba(96, 165, 250, 0.2);
    border-radius: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--vscode-descriptionForeground);
    font-size: 9px;
  }
  .box-label {
    position: absolute;
    font-size: 9px;
  }
  .box-label.top { top: 2px; left: 50%; transform: translateX(-50%); }
  .box-label.bottom { bottom: 2px; left: 50%; transform: translateX(-50%); }
  .box-label.left { left: 4px; top: 50%; transform: translateY(-50%); }
  .box-label.right { right: 4px; top: 50%; transform: translateY(-50%); }
  .box-label.margin-label { color: #fb923c; }
  .box-label.border-label { color: #60a5fa; }
  .box-label.padding-label { color: #4ade80; }

  /* Style sections */
  .style-section {
    padding: 12px 16px;
    border-bottom: 1px solid var(--vscode-panel-border);
  }
  .style-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 2px 0;
    font-size: 11px;
  }
  .style-row .prop { color: var(--vscode-descriptionForeground); }
  .style-row .val {
    font-family: var(--font-mono);
    text-align: right;
  }
  .color-swatch {
    display: inline-block;
    width: 10px; height: 10px;
    border-radius: 2px;
    border: 1px solid var(--vscode-panel-border);
    vertical-align: middle;
    margin-right: 4px;
  }

  /* Actions */
  .inspector-actions {
    padding: 12px 16px;
  }
  .copy-style-btn {
    width: 100%;
    background: var(--vscode-button-secondaryBackground);
    border: none;
    border-radius: 4px;
    padding: 8px 12px;
    font-size: 12px;
    color: var(--vscode-button-secondaryForeground);
    cursor: pointer;
  }
  .copy-style-btn:hover {
    background: var(--vscode-button-secondaryHoverBackground);
  }
</style>
</head>
<body>
  <div id="empty-state" class="empty-state">Click an element to inspect</div>
  <div id="content" style="display:none">
    <div class="inspector-header">
      <div class="breadcrumb" id="breadcrumb"></div>
      <div class="node-label" id="node-label"></div>
      <div class="node-dims" id="node-dims"></div>
      <div class="node-source" id="node-source" style="display:none"></div>
    </div>
    <div class="box-model">
      <div class="section-title">Box Model</div>
      <div class="box-model-diagram">
        <div class="box-margin">
          <span class="box-label top margin-label" id="bm-mt">0</span>
          <span class="box-label bottom margin-label" id="bm-mb">0</span>
          <span class="box-label left margin-label" id="bm-ml">0</span>
          <span class="box-label right margin-label" id="bm-mr">0</span>
        </div>
        <div class="box-border-area">
          <span class="box-label top border-label" id="bm-bt">0</span>
          <span class="box-label bottom border-label" id="bm-bb">0</span>
          <span class="box-label left border-label" id="bm-bl">0</span>
          <span class="box-label right border-label" id="bm-br">0</span>
        </div>
        <div class="box-padding">
          <span class="box-label top padding-label" id="bm-pt">0</span>
          <span class="box-label bottom padding-label" id="bm-pb">0</span>
          <span class="box-label left padding-label" id="bm-pl">0</span>
          <span class="box-label right padding-label" id="bm-pr">0</span>
        </div>
        <div class="box-content" id="bm-content">0 x 0</div>
      </div>
    </div>
    <div id="styles"></div>
    <div class="inspector-actions" id="actions"></div>
  </div>
<script>
  const vscode = acquireVsCodeApi();

  const emptyState = document.getElementById('empty-state');
  const content = document.getElementById('content');

  function fmt(n) {
    return Number.isInteger(n) ? String(n) : n.toFixed(1).replace(/\\.0$/, '');
  }

  function colorToHex(c) {
    if (!c) return 'transparent';
    const r = Math.round(c.r * 255).toString(16).padStart(2, '0');
    const g = Math.round(c.g * 255).toString(16).padStart(2, '0');
    const b = Math.round(c.b * 255).toString(16).padStart(2, '0');
    return '#' + r + g + b;
  }

  function colorHtml(c) {
    const hex = colorToHex(c);
    return '<span class="color-swatch" style="background:' + hex + '"></span>' + hex;
  }

  function edgesShorthand(e) {
    const t = fmt(e.top), r = fmt(e.right), b = fmt(e.bottom), l = fmt(e.left);
    if (t === r && r === b && b === l) return t;
    if (t === b && l === r) return t + ' ' + r;
    return t + ' ' + r + ' ' + b + ' ' + l;
  }

  function cornerShorthand(c) {
    const tl = fmt(c.top_left), tr = fmt(c.top_right), br = fmt(c.bottom_right), bl = fmt(c.bottom_left);
    if (tl === tr && tr === br && br === bl) return tl;
    return tl + ' ' + tr + ' ' + br + ' ' + bl;
  }

  function escapeHtml(s) {
    return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  }

  function renderSection(title, props) {
    let html = '<div class="style-section"><div class="section-title">' + title + '</div>';
    for (const [prop, val] of props) {
      html += '<div class="style-row"><span class="prop">' + prop + '</span><span class="val">' + val + '</span></div>';
    }
    html += '</div>';
    return html;
  }

  function buildJsxStyleString(s) {
    const enumMap = {
      'Column': 'column', 'Row': 'row',
      'FlexStart': 'flex-start', 'FlexEnd': 'flex-end',
      'Center': 'center', 'SpaceBetween': 'space-between',
      'SpaceAround': 'space-around', 'SpaceEvenly': 'space-evenly',
      'Stretch': 'stretch', 'Baseline': 'baseline',
      'NoWrap': 'nowrap', 'Wrap': 'wrap',
      'Normal': 'normal', 'Italic': 'italic',
      'Left': 'left', 'Right': 'right', 'Justify': 'justify',
      'None': 'none', 'Underline': 'underline', 'LineThrough': 'line-through',
      'Uppercase': 'uppercase', 'Lowercase': 'lowercase', 'Capitalize': 'capitalize',
    };
    function mapEnum(v) { return enumMap[v] || v; }
    function colorEq(c, r, g, b) {
      return c && Math.abs(c.r - r) < 0.001 && Math.abs(c.g - g) < 0.001 && Math.abs(c.b - b) < 0.001;
    }
    function edgesAllZero(e) { return e.top === 0 && e.right === 0 && e.bottom === 0 && e.left === 0; }
    function edgesUniform(e) { return e.top === e.right && e.right === e.bottom && e.bottom === e.left; }
    function cornersAllZero(c) { return c.top_left === 0 && c.top_right === 0 && c.bottom_right === 0 && c.bottom_left === 0; }
    function cornersUniform(c) { return c.top_left === c.top_right && c.top_right === c.bottom_right && c.bottom_right === c.bottom_left; }
    function fmtEdges(e) {
      if (edgesUniform(e)) return String(e.top);
      return '{ top: ' + e.top + ', right: ' + e.right + ', bottom: ' + e.bottom + ', left: ' + e.left + ' }';
    }
    function fmtCorners(c) {
      if (cornersUniform(c)) return String(c.top_left);
      return '{ topLeft: ' + c.top_left + ', topRight: ' + c.top_right + ', bottomRight: ' + c.bottom_right + ', bottomLeft: ' + c.bottom_left + ' }';
    }

    const props = [];
    if (s.width != null) props.push('width: ' + s.width);
    if (s.height != null) props.push('height: ' + s.height);
    if (s.minWidth != null) props.push('minWidth: ' + s.minWidth);
    if (s.minHeight != null) props.push('minHeight: ' + s.minHeight);
    if (s.maxWidth != null) props.push('maxWidth: ' + s.maxWidth);
    if (s.maxHeight != null) props.push('maxHeight: ' + s.maxHeight);
    if (s.flexDirection !== 'Column') props.push("flexDirection: '" + mapEnum(s.flexDirection) + "'");
    if (s.justifyContent !== 'FlexStart') props.push("justifyContent: '" + mapEnum(s.justifyContent) + "'");
    if (s.alignItems !== 'Stretch') props.push("alignItems: '" + mapEnum(s.alignItems) + "'");
    if (s.alignSelf) props.push("alignSelf: '" + mapEnum(s.alignSelf) + "'");
    if (s.flexWrap !== 'NoWrap') props.push("flexWrap: '" + mapEnum(s.flexWrap) + "'");
    if (s.flexGrow > 0) props.push('flexGrow: ' + s.flexGrow);
    if (s.flexShrink !== 1) props.push('flexShrink: ' + s.flexShrink);
    if (s.flexBasis != null) props.push('flexBasis: ' + s.flexBasis);
    if (s.gap > 0) props.push('gap: ' + s.gap);
    if (s.rowGap > 0) props.push('rowGap: ' + s.rowGap);
    if (s.columnGap > 0) props.push('columnGap: ' + s.columnGap);
    if (!edgesAllZero(s.margin)) props.push('margin: ' + fmtEdges(s.margin));
    if (!edgesAllZero(s.padding)) props.push('padding: ' + fmtEdges(s.padding));
    if (!edgesAllZero(s.borderWidth)) props.push('borderWidth: ' + fmtEdges(s.borderWidth));
    if (s.fontFamily !== 'Helvetica') props.push("fontFamily: '" + s.fontFamily + "'");
    if (s.fontSize !== 12) props.push('fontSize: ' + s.fontSize);
    if (s.fontWeight !== 400) props.push('fontWeight: ' + s.fontWeight);
    if (s.fontStyle !== 'Normal') props.push("fontStyle: '" + mapEnum(s.fontStyle) + "'");
    if (s.lineHeight !== 1.4) props.push('lineHeight: ' + s.lineHeight);
    if (s.textAlign !== 'Left') props.push("textAlign: '" + mapEnum(s.textAlign) + "'");
    if (s.letterSpacing !== 0) props.push('letterSpacing: ' + s.letterSpacing);
    if (s.textDecoration !== 'None') props.push("textDecoration: '" + mapEnum(s.textDecoration) + "'");
    if (s.textTransform !== 'None') props.push("textTransform: '" + mapEnum(s.textTransform) + "'");
    if (s.opacity < 1) props.push('opacity: ' + s.opacity);
    if (!colorEq(s.color, 0, 0, 0)) props.push("color: '" + colorToHex(s.color) + "'");
    if (s.backgroundColor) props.push("backgroundColor: '" + colorToHex(s.backgroundColor) + "'");
    if (!cornersAllZero(s.borderRadius)) props.push('borderRadius: ' + fmtCorners(s.borderRadius));

    if (props.length === 0) return null;
    return '{ ' + props.join(', ') + ' }';
  }

  function updateInspector(data) {
    if (!data) {
      emptyState.style.display = '';
      content.style.display = 'none';
      return;
    }
    emptyState.style.display = 'none';
    content.style.display = '';

    const el = data.element;
    const s = el.style;
    const ancestors = data.ancestors || [];
    const ancestorElements = data.ancestorElements || [];

    // Breadcrumb
    const breadcrumb = document.getElementById('breadcrumb');
    if (ancestors.length > 0) {
      breadcrumb.textContent = ancestors.join(' > ');
      breadcrumb.style.display = '';
    } else {
      breadcrumb.style.display = 'none';
    }

    // Header
    const nodeTypeColors = {
      View: '#3b82f6', Text: '#eab308', Image: '#a855f7',
      Table: '#22c55e', TableRow: '#22c55e', TableCell: '#22c55e',
      FixedHeader: '#ef4444', FixedFooter: '#ef4444',
    };
    const color = nodeTypeColors[el.nodeType] || '#a1a1aa';
    document.getElementById('node-label').innerHTML = '<span style="color:' + color + '">' + el.nodeType + '</span>';
    document.getElementById('node-dims').textContent = fmt(el.width) + ' \\u00d7 ' + fmt(el.height) + ' at (' + fmt(el.x) + ', ' + fmt(el.y) + ')';

    // Source location
    const sourceEl = document.getElementById('node-source');
    let sl = el.sourceLocation;
    if (!sl) {
      for (let i = ancestorElements.length - 1; i >= 0; i--) {
        if (ancestorElements[i].sourceLocation) { sl = ancestorElements[i].sourceLocation; break; }
      }
    }
    if (sl) {
      const fileName = sl.file.split('/').pop();
      sourceEl.innerHTML = '<span class="source-path">' + fileName + ':' + sl.line + ':' + sl.column + '</span> <button class="open-editor-btn" id="open-btn">Open</button>';
      sourceEl.style.display = '';
      document.getElementById('open-btn').onclick = function() {
        vscode.postMessage({ type: 'openFile', file: sl.file, line: sl.line, column: sl.column });
      };
    } else {
      sourceEl.style.display = 'none';
    }

    // Box model
    document.getElementById('bm-mt').textContent = fmt(s.margin.top);
    document.getElementById('bm-mb').textContent = fmt(s.margin.bottom);
    document.getElementById('bm-ml').textContent = fmt(s.margin.left);
    document.getElementById('bm-mr').textContent = fmt(s.margin.right);
    document.getElementById('bm-bt').textContent = fmt(s.borderWidth.top);
    document.getElementById('bm-bb').textContent = fmt(s.borderWidth.bottom);
    document.getElementById('bm-bl').textContent = fmt(s.borderWidth.left);
    document.getElementById('bm-br').textContent = fmt(s.borderWidth.right);
    document.getElementById('bm-pt').textContent = fmt(s.padding.top);
    document.getElementById('bm-pb').textContent = fmt(s.padding.bottom);
    document.getElementById('bm-pl').textContent = fmt(s.padding.left);
    document.getElementById('bm-pr').textContent = fmt(s.padding.right);

    const cw = el.width - s.padding.left - s.padding.right - s.borderWidth.left - s.borderWidth.right;
    const ch = el.height - s.padding.top - s.padding.bottom - s.borderWidth.top - s.borderWidth.bottom;
    document.getElementById('bm-content').textContent = fmt(Math.max(0, cw)) + ' \\u00d7 ' + fmt(Math.max(0, ch));

    // Computed styles
    let html = '';

    const sizeProps = [];
    if (s.width != null) sizeProps.push(['width', s.width + 'pt']);
    if (s.height != null) sizeProps.push(['height', s.height + 'pt']);
    if (s.minWidth != null) sizeProps.push(['min-width', fmt(s.minWidth) + 'pt']);
    if (s.minHeight != null) sizeProps.push(['min-height', fmt(s.minHeight) + 'pt']);
    if (s.maxWidth != null) sizeProps.push(['max-width', fmt(s.maxWidth) + 'pt']);
    if (s.maxHeight != null) sizeProps.push(['max-height', fmt(s.maxHeight) + 'pt']);
    if (sizeProps.length) html += renderSection('Sizing', sizeProps);

    const posProps = [];
    if (s.position && s.position !== 'Relative') {
      posProps.push(['position', s.position.toLowerCase()]);
      if (s.top != null) posProps.push(['top', fmt(s.top) + 'pt']);
      if (s.right != null) posProps.push(['right', fmt(s.right) + 'pt']);
      if (s.bottom != null) posProps.push(['bottom', fmt(s.bottom) + 'pt']);
      if (s.left != null) posProps.push(['left', fmt(s.left) + 'pt']);
    }
    if (posProps.length) html += renderSection('Positioning', posProps);

    const layoutProps = [];
    if (s.flexDirection !== 'Column') layoutProps.push(['flex-direction', s.flexDirection]);
    if (s.justifyContent !== 'FlexStart') layoutProps.push(['justify-content', s.justifyContent]);
    if (s.alignItems !== 'Stretch') layoutProps.push(['align-items', s.alignItems]);
    if (s.alignSelf) layoutProps.push(['align-self', s.alignSelf]);
    if (s.flexWrap !== 'NoWrap') layoutProps.push(['flex-wrap', s.flexWrap]);
    if (s.flexGrow > 0) layoutProps.push(['flex-grow', String(s.flexGrow)]);
    if (s.flexShrink !== 1) layoutProps.push(['flex-shrink', String(s.flexShrink)]);
    if (s.flexBasis != null) layoutProps.push(['flex-basis', s.flexBasis + 'pt']);
    if (s.gap > 0) layoutProps.push(['gap', fmt(s.gap) + 'pt']);
    if (s.rowGap > 0 && s.rowGap !== s.gap) layoutProps.push(['row-gap', fmt(s.rowGap) + 'pt']);
    if (s.columnGap > 0 && s.columnGap !== s.gap) layoutProps.push(['column-gap', fmt(s.columnGap) + 'pt']);
    if (layoutProps.length) html += renderSection('Layout', layoutProps);

    const spacingProps = [];
    const marginStr = edgesShorthand(s.margin);
    const paddingStr = edgesShorthand(s.padding);
    const borderStr = edgesShorthand(s.borderWidth);
    if (marginStr !== '0') spacingProps.push(['margin', marginStr]);
    if (paddingStr !== '0') spacingProps.push(['padding', paddingStr]);
    if (borderStr !== '0') spacingProps.push(['border-width', borderStr]);
    if (spacingProps.length) html += renderSection('Spacing', spacingProps);

    const typoProps = [];
    typoProps.push(['font-family', s.fontFamily]);
    typoProps.push(['font-size', fmt(s.fontSize) + 'pt']);
    if (s.fontWeight !== 400) typoProps.push(['font-weight', String(s.fontWeight)]);
    if (s.fontStyle !== 'Normal') typoProps.push(['font-style', s.fontStyle]);
    if (s.lineHeight !== 1.4) typoProps.push(['line-height', fmt(s.lineHeight)]);
    if (s.textAlign !== 'Left') typoProps.push(['text-align', s.textAlign]);
    if (s.letterSpacing !== 0) typoProps.push(['letter-spacing', fmt(s.letterSpacing) + 'pt']);
    if (s.textDecoration !== 'None') typoProps.push(['text-decoration', s.textDecoration.toLowerCase()]);
    if (s.textTransform !== 'None') typoProps.push(['text-transform', s.textTransform.toLowerCase()]);
    html += renderSection('Typography', typoProps);

    const visualProps = [];
    visualProps.push(['color', colorHtml(s.color)]);
    if (s.backgroundColor) visualProps.push(['background', colorHtml(s.backgroundColor)]);
    if (s.opacity < 1) visualProps.push(['opacity', fmt(s.opacity)]);
    const radiusStr = cornerShorthand(s.borderRadius);
    if (radiusStr !== '0') visualProps.push(['border-radius', radiusStr]);
    html += renderSection('Background & Border', visualProps);

    const linkProps = [];
    if (el.href) linkProps.push(['href', escapeHtml(el.href)]);
    if (el.bookmark) linkProps.push(['bookmark', escapeHtml(el.bookmark)]);
    if (linkProps.length) html += renderSection('Link & Bookmark', linkProps);

    const pageProps = [];
    if (s.breakable) pageProps.push(['breakable', 'true']);
    if (s.breakBefore) pageProps.push(['break-before', 'true']);
    if (s.minWidowLines !== 2) pageProps.push(['min-widow-lines', String(s.minWidowLines)]);
    if (s.minOrphanLines !== 2) pageProps.push(['min-orphan-lines', String(s.minOrphanLines)]);
    if (pageProps.length) html += renderSection('Page Behavior', pageProps);

    document.getElementById('styles').innerHTML = html;

    // Copy Style button
    const styleStr = buildJsxStyleString(s);
    const actionsEl = document.getElementById('actions');
    if (styleStr) {
      actionsEl.innerHTML = '<button class="copy-style-btn" id="copy-btn">Copy Style</button>';
      document.getElementById('copy-btn').onclick = function() {
        vscode.postMessage({ type: 'copyStyle', text: styleStr });
      };
    } else {
      actionsEl.innerHTML = '';
    }
  }

  window.addEventListener('message', function(event) {
    const msg = event.data;
    if (msg.type === 'updateElement') {
      updateInspector(msg.selection);
    }
  });
</script>
</body>
</html>`;
  }
}
