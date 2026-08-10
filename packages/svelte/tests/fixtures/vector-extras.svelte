<script lang="ts">
  import type { CanvasContext } from '../../src/index.js';
  import { Document, Page, View, Text, Canvas, Watermark, PageBreak } from '../../src/index.js';

  interface Props {
    accent?: [number, number, number];
  }

  let { accent = [59, 130, 246] }: Props = $props();

  const draw = (ctx: CanvasContext) => {
    // Fills
    ctx.setFillColor(accent[0], accent[1], accent[2]);
    ctx.rect(10, 10, 80, 40);
    ctx.fill();
    ctx.setFillColor(16, 185, 129);
    ctx.circle(150, 30, 14);
    ctx.fillAndStroke();
    ctx.ellipse(150, 75, 24, 10);
    ctx.fill();

    // Paths
    ctx.save();
    ctx.setStrokeColor(220, 38, 38);
    ctx.setLineWidth(2);
    ctx.setLineCap(1);
    ctx.setLineJoin(1);
    ctx.moveTo(10, 70);
    ctx.bezierCurveTo(30, 40, 60, 100, 90, 70);
    ctx.quadraticCurveTo(100, 60, 110, 70);
    ctx.lineTo(120, 90);
    ctx.closePath();
    ctx.stroke();
    ctx.restore();
    ctx.line(0, 100, 200, 100);

    // Arcs
    ctx.arc(60, 60, 25, 0, Math.PI * 1.5);
    ctx.stroke();
    ctx.arc(60, 60, 18, Math.PI / 4, Math.PI, true);
    ctx.stroke();
  };
</script>

<Document title="Vector Extras Parity">
  <Page size="A4" margin={40}>
    <Watermark text="DRAFT" fontSize={72} color="rgba(200,30,30,0.15)" angle={-30} />
    <Watermark text="CONFIDENTIAL" />
    <Text style={{ fontSize: 18, marginBottom: 12 }}>First page</Text>
    <Canvas width={200} height={110} {draw} style={{ marginBottom: 8 }} />
    <PageBreak />
    <View style={{ padding: 6 }}>
      <Text>Second page</Text>
    </View>
  </Page>
</Document>
