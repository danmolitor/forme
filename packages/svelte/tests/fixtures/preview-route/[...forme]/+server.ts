/**
 * Demo SvelteKit catch-all route for the preview helper, written
 * exactly as a user would mount it (mounted at /dev/pdf-preview in the
 * E2E test). Tests import GET and call it with RequestEvent-shaped
 * objects instead of booting a Vite server.
 */
import { formePreview } from '../../../../src/preview/index.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from '../../hello-world.svelte';

export const GET = formePreview(HelloWorld, {
  props: { name: 'Svelte', items: ['alpha', 'beta'], showFooter: true },
  pollMs: 500,
});
