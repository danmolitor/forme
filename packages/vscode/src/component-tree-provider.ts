import * as vscode from 'vscode';
import type { LayoutInfo } from './layout-store.js';

const NODE_TYPE_COLORS: Record<string, string> = {
  View: '#3b82f6',
  Text: '#eab308',
  TextLine: '#eab308',
  Image: '#a855f7',
  Table: '#22c55e',
  TableRow: '#10b981',
  TableCell: '#34d399',
  FixedHeader: '#ef4444',
  FixedFooter: '#ef4444',
  Page: '#a1a1aa',
  Rect: '#6b7280',
  None: '#6b7280',
};

export class ComponentTreeProvider implements vscode.WebviewViewProvider {
  static readonly viewType = 'forme.componentTree';

  private view?: vscode.WebviewView;
  private layout: LayoutInfo | null = null;
  private selectedPath: number[] | null = null;
  private isReady = false;

  private readonly _onSelect = new vscode.EventEmitter<number[]>();
  readonly onSelect = this._onSelect.event;

  private readonly _onHover = new vscode.EventEmitter<number[] | null>();
  readonly onHover = this._onHover.event;

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
      if (msg.type === 'ready') {
        this.isReady = true;
        if (this.layout) this.sendLayout();
        if (this.selectedPath) this.sendSelection();
      }
      if (msg.type === 'select') {
        this._onSelect.fire(msg.path);
      }
      if (msg.type === 'hover') {
        this._onHover.fire(msg.path);
      }
      if (msg.type === 'hoverEnd') {
        this._onHover.fire(null);
      }
    });

    // Reset ready state — webview will signal when loaded
    this.isReady = false;
  }

  updateLayout(layout: LayoutInfo): void {
    this.layout = layout;
    this.sendLayout();
  }

  selectPath(path: number[] | null): void {
    this.selectedPath = path;
    this.sendSelection();
  }

  private sendLayout(): void {
    if (!this.view || !this.isReady) return;
    this.view.webview.postMessage({
      type: 'layout',
      data: this.layout,
      colors: NODE_TYPE_COLORS,
    });
  }

  private sendSelection(): void {
    if (!this.view || !this.isReady) return;
    this.view.webview.postMessage({
      type: 'select',
      path: this.selectedPath,
    });
  }

  dispose(): void {
    this._onSelect.dispose();
    this._onHover.dispose();
  }

  private getHtml(): string {
    // Using a separate script block to avoid template literal escaping issues
    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: var(--vscode-font-family);
    font-size: 12px;
    color: var(--vscode-foreground);
    background: var(--vscode-sideBar-background);
    overflow-y: auto;
    user-select: none;
  }
  .empty-state {
    padding: 20px 16px;
    text-align: center;
    color: var(--vscode-descriptionForeground);
    font-size: 12px;
  }
  #tree { padding: 4px 0; }
  .tree-node {
    display: flex;
    align-items: center;
    padding: 2px 8px 2px 0;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.1s;
    line-height: 22px;
  }
  .tree-node:hover {
    background: var(--vscode-list-hoverBackground);
  }
  .tree-node.selected {
    background: var(--vscode-list-activeSelectionBackground);
    color: var(--vscode-list-activeSelectionForeground);
  }
  .arrow {
    width: 16px;
    flex-shrink: 0;
    text-align: center;
    font-size: 10px;
    color: var(--vscode-descriptionForeground);
    cursor: pointer;
  }
  .arrow.has-children { color: var(--vscode-foreground); }
  .node-label { font-size: 12px; }
  .text-preview {
    color: var(--vscode-descriptionForeground);
    margin-left: 6px;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 140px;
  }
  .dim-label {
    color: var(--vscode-descriptionForeground);
    margin-left: 6px;
    font-size: 10px;
  }
  .tree-children { display: none; }
  .tree-children.expanded { display: block; }
</style>
</head>
<body>
  <div id="empty" class="empty-state">No layout data</div>
  <div id="tree" style="display:none"></div>
<script>
  (function() {
    var vscode = acquireVsCodeApi();
    var emptyEl = document.getElementById('empty');
    var treeEl = document.getElementById('tree');

    var layoutData = null;
    var colors = {};
    var selectedPath = null;

    function fmt(n) {
      return Number.isInteger(n) ? String(n) : n.toFixed(1).replace(/\\.0$/, '');
    }

    function escapeAttr(s) {
      return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
    }

    function escapeHtml(s) {
      return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function pathKey(path) {
      return path.join('-');
    }

    function findNodeByPath(path) {
      return treeEl.querySelector('[data-path="' + pathKey(path) + '"]');
    }

    function findChildrenByPath(path) {
      return treeEl.querySelector('.tree-children[data-path="' + pathKey(path) + '"]');
    }

    function renderTree() {
      if (!layoutData || !layoutData.pages.length) {
        emptyEl.style.display = '';
        treeEl.style.display = 'none';
        return;
      }
      emptyEl.style.display = 'none';
      treeEl.style.display = '';

      var html = '';
      for (var pi = 0; pi < layoutData.pages.length; pi++) {
        var page = layoutData.pages[pi];
        html += renderTreeNode({
          nodeType: 'Page',
          width: page.width,
          height: page.height,
          children: page.elements,
        }, [pi], 0, true);
      }
      treeEl.innerHTML = html;

      // Expand top 2 levels by default
      var allChildren = treeEl.querySelectorAll('.tree-children');
      for (var i = 0; i < allChildren.length; i++) {
        var el = allChildren[i];
        var depth = parseInt(el.dataset.depth, 10);
        if (depth < 2) {
          el.classList.add('expanded');
          var prev = el.previousElementSibling;
          if (prev) {
            var arrow = prev.querySelector('.arrow');
            if (arrow && arrow.classList.contains('has-children')) {
              arrow.textContent = '\\u25BE';
            }
          }
        }
      }

      if (selectedPath) {
        applySelection(selectedPath);
      }
    }

    function renderTreeNode(el, path, depth, isExpanded) {
      var pk = pathKey(path);
      var hasChildren = el.children && el.children.length > 0;
      var color = colors[el.nodeType] || '#6b7280';
      var indent = depth * 16;

      var extra = '';
      if (el.nodeType === 'TextLine' && el.textContent) {
        var preview = el.textContent.length > 30
          ? el.textContent.substring(0, 30) + '...'
          : el.textContent;
        extra = '<span class="text-preview">"' + escapeHtml(preview) + '"</span>';
      } else if (el.nodeType === 'Page' && el.width) {
        extra = '<span class="dim-label">' + fmt(el.width) + ' x ' + fmt(el.height) + '</span>';
      }

      var arrowChar = hasChildren ? (isExpanded ? '\\u25BE' : '\\u25B8') : '';
      var arrowClass = hasChildren ? 'arrow has-children' : 'arrow';

      var html = '<div class="tree-node" data-path="' + escapeAttr(pk) + '" data-json="' + escapeAttr(JSON.stringify(path)) + '" style="padding-left:' + (indent + 4) + 'px">';
      html += '<span class="' + arrowClass + '" data-toggle="' + escapeAttr(pk) + '">' + arrowChar + '</span>';
      html += '<span class="node-label" style="color:' + color + '">' + el.nodeType + '</span>';
      html += extra;
      html += '</div>';

      if (hasChildren) {
        html += '<div class="tree-children" data-depth="' + depth + '" data-path="' + escapeAttr(pk) + '">';
        for (var ci = 0; ci < el.children.length; ci++) {
          var childPath = path.concat([ci]);
          html += renderTreeNode(el.children[ci], childPath, depth + 1, false);
        }
        html += '</div>';
      }

      return html;
    }

    function applySelection(path) {
      // Clear previous
      var allSelected = treeEl.querySelectorAll('.tree-node.selected');
      for (var i = 0; i < allSelected.length; i++) {
        allSelected[i].classList.remove('selected');
      }

      if (!path) return;

      var node = findNodeByPath(path);
      if (!node) return;

      node.classList.add('selected');

      // Expand parent tree nodes to make visible
      var parent = node.parentElement;
      while (parent && parent !== treeEl) {
        if (parent.classList.contains('tree-children')) {
          parent.classList.add('expanded');
          var prev = parent.previousElementSibling;
          if (prev) {
            var arrow = prev.querySelector('.arrow');
            if (arrow && arrow.classList.contains('has-children')) {
              arrow.textContent = '\\u25BE';
            }
          }
        }
        parent = parent.parentElement;
      }

      node.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }

    // Click: toggle expand or select
    treeEl.addEventListener('click', function(e) {
      var arrow = e.target.closest('.arrow.has-children');
      if (arrow) {
        var children = findChildrenByPath(arrow.dataset.toggle.split('-').map(Number));
        if (!children) {
          // Try direct lookup
          children = treeEl.querySelector('.tree-children[data-path="' + arrow.dataset.toggle + '"]');
        }
        if (children) {
          var isExpanded = children.classList.toggle('expanded');
          arrow.textContent = isExpanded ? '\\u25BE' : '\\u25B8';
        }
        return;
      }

      var node = e.target.closest('.tree-node');
      if (node && node.dataset.json) {
        var path = JSON.parse(node.dataset.json);
        selectedPath = path;
        applySelection(path);
        vscode.postMessage({ type: 'select', path: path });
      }
    });

    // Hover: highlight in preview
    treeEl.addEventListener('mouseover', function(e) {
      var node = e.target.closest('.tree-node');
      if (node && node.dataset.json) {
        var path = JSON.parse(node.dataset.json);
        vscode.postMessage({ type: 'hover', path: path });
      }
    });

    treeEl.addEventListener('mouseleave', function() {
      vscode.postMessage({ type: 'hoverEnd' });
    });

    // Messages from extension
    window.addEventListener('message', function(event) {
      var msg = event.data;
      if (msg.type === 'layout') {
        layoutData = msg.data;
        colors = msg.colors || {};
        renderTree();
      }
      if (msg.type === 'select') {
        selectedPath = msg.path;
        applySelection(msg.path);
      }
    });

    // Signal ready
    vscode.postMessage({ type: 'ready' });
  })();
</script>
</body>
</html>`;
  }
}
