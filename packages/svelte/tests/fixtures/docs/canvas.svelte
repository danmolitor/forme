<script lang="ts">
  import { Document, Page, Canvas } from '@formepdf/svelte';
  import type { CanvasContext } from '@formepdf/svelte';

  function draw(ctx: CanvasContext) {
    ctx.setFillColor(59, 130, 246);
    ctx.circle(50, 50, 40);
    ctx.fill();
  }
</script>

<Document>
  <Page size="A4" margin={40}>
    <Canvas width={100} height={100} {draw} />
  </Page>
</Document>
