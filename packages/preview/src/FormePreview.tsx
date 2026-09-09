import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactElement } from 'react';
import type { RenderHtmlOptions } from '@formepdf/html';
import { renderForPreview, fontsMatch } from './render.js';
import type { PreviewFont, RenderHtmlFn } from './render.js';

export interface FormePreviewProps {
  /** The document HTML — the same string you POST to your server render. */
  html: string;
  /**
   * Render options — THE SAME object you pass to the server's `renderHtml`.
   * There is deliberately no `previewOptions`: preview and output agree because
   * there is one configuration. Should be referentially stable (memoize it).
   */
  options?: RenderHtmlOptions;
  /**
   * Optional parity check. If you fingerprint the fonts your server was given
   * (`fontFingerprint(...)`) and pass it here, the preview shows a visible
   * banner when the fonts it holds don't match — turning a silent font
   * fall-back into a loud one.
   */
  expectedFontFingerprint?: string;
  /** Debounce before (re)rendering, for live-edit. Default 150ms. */
  debounceMs?: number;
  onWarnings?: (warnings: string[]) => void;
  onError?: (error: Error) => void;
  onRender?: (info: { passes: number; warnings: string[]; bytes: number }) => void;
  className?: string;
  style?: CSSProperties;
  /** Accessible title for the preview iframe. */
  title?: string;
}

// One engine load per app, shared across every <FormePreview>. Lazily imported
// so the ~7.7MB WASM stays out of the initial bundle and loads on first preview.
let enginePromise: Promise<RenderHtmlFn> | null = null;
function loadEngine(): Promise<RenderHtmlFn> {
  if (!enginePromise) {
    enginePromise = import('@formepdf/html/browser').then((m) => m.renderHtml as RenderHtmlFn);
  }
  return enginePromise;
}

type Status = 'loading' | 'rendering' | 'ready' | 'error';

/**
 * Renders `html` through the Forme engine in the browser and shows the ACTUAL
 * PDF in an iframe (the browser's native viewer). Because it calls the same
 * `renderHtml` the server calls with the same options, what you see is what the
 * server produces — parity by construction. It is not a viewer, editor, or
 * toolbar: it shows what the engine produced, pages in order.
 */
export function FormePreview(props: FormePreviewProps): ReactElement {
  const { html, options, expectedFontFingerprint, debounceMs = 150, className, style, title = 'PDF preview' } = props;

  const [url, setUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<Status>('loading');
  const [error, setError] = useState<string | null>(null);

  // Latest callbacks without re-triggering the render effect on identity change.
  const cbs = useRef({ onWarnings: props.onWarnings, onError: props.onError, onRender: props.onRender });
  cbs.current = { onWarnings: props.onWarnings, onError: props.onError, onRender: props.onRender };

  const versionRef = useRef(0);
  const urlRef = useRef<string | null>(null);

  const fontMismatch =
    expectedFontFingerprint != null &&
    !fontsMatch(options?.fonts as PreviewFont[] | undefined, expectedFontFingerprint);

  useEffect(() => {
    const version = ++versionRef.current;
    // Keep the last good frame visible while re-rendering; only show the full
    // skeleton before the first successful render.
    setStatus(urlRef.current ? 'rendering' : 'loading');
    setError(null);

    const timer = setTimeout(() => {
      void (async () => {
        try {
          const renderHtml = await loadEngine();
          if (version !== versionRef.current) return;
          const result = await renderForPreview(html, options, renderHtml);
          if (version !== versionRef.current) return;

          const next = URL.createObjectURL(new Blob([result.pdf as BlobPart], { type: 'application/pdf' }));
          if (urlRef.current) URL.revokeObjectURL(urlRef.current);
          urlRef.current = next;
          setUrl(next);
          setStatus('ready');
          cbs.current.onWarnings?.(result.warnings);
          cbs.current.onRender?.({ passes: result.passes, warnings: result.warnings, bytes: result.pdf.length });
        } catch (e) {
          if (version !== versionRef.current) return;
          const err = e instanceof Error ? e : new Error(String(e));
          setError(err.message);
          setStatus('error');
          cbs.current.onError?.(err);
        }
      })();
    }, debounceMs);

    return () => clearTimeout(timer);
    // `options` should be referentially stable (documented); debounce + the
    // version guard absorb over-renders if it isn't.
  }, [html, options, debounceMs]);

  // Revoke the outstanding object URL on unmount.
  useEffect(() => () => { if (urlRef.current) URL.revokeObjectURL(urlRef.current); }, []);

  return (
    <div className={className} style={{ position: 'relative', minHeight: 200, ...style }}>
      {url && (
        <iframe
          src={url}
          title={title}
          style={{ width: '100%', height: '100%', border: 0, display: 'block' }}
        />
      )}

      {fontMismatch && (
        <div style={bannerStyle('#7c2d12', '#fed7aa')} role="status">
          Preview fonts differ from the server output — this preview may not match the produced PDF.
        </div>
      )}

      {status === 'error' && (
        <div style={bannerStyle('#7f1d1d', '#fecaca')} role="alert">
          Render failed: {error}
        </div>
      )}

      {(status === 'loading' || status === 'rendering') && (
        <div style={overlayStyle(status === 'loading')} aria-live="polite">
          {status === 'loading' ? 'Loading preview…' : 'Rendering…'}
        </div>
      )}
    </div>
  );
}

function bannerStyle(bg: string, fg: string): CSSProperties {
  return {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    padding: '6px 10px',
    background: bg,
    color: fg,
    font: '12px/1.4 system-ui, sans-serif',
    zIndex: 2,
  };
}

function overlayStyle(opaque: boolean): CSSProperties {
  return {
    position: 'absolute',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    // 'loading' has no frame behind it → solid; 'rendering' floats over the
    // last good frame → subtle.
    background: opaque ? '#fafafa' : 'rgba(250,250,250,0.55)',
    color: '#52525b',
    font: '13px/1.4 system-ui, sans-serif',
    zIndex: 1,
    pointerEvents: 'none',
  };
}
