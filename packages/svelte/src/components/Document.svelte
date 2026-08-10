<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { CertificationConfig, FontRegistration, Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    title?: string;
    author?: string;
    subject?: string;
    creator?: string;
    /** Document language (BCP 47 tag, e.g. "en-US"). Emitted as /Lang in the PDF Catalog. */
    lang?: string;
    /** Default style applied to the entire document. Sets global fontFamily, fontSize, color, etc. */
    style?: Style;
    /** Whether to produce a tagged (accessible) PDF with structure tree. */
    tagged?: boolean;
    /** PDF/A conformance level. "2a" requires tagging, "2b" is visual-only compliance. */
    pdfa?: '2a' | '2b';
    /** When true, the PDF claims PDF/UA-1 conformance. Forces tagging. */
    pdfUa?: boolean;
    /** Digital certification configuration. Certifies the PDF with an X.509 certificate. */
    certification?: CertificationConfig;
    /** @deprecated Use certification */
    signature?: CertificationConfig;
    /** Per-document custom fonts. Merged with `Font.register()` globals; document fonts win on conflict. */
    fonts?: FontRegistration[];
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-document props={encodeProps('Document', rest)}>{@render children?.()}</forme-document>
