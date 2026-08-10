import type { CanvasContext, CanvasOp } from './types.js';

/**
 * Execute a <Canvas> draw callback against a recording context, capturing
 * the drawing calls as a serializable CanvasOp list.
 */
export function recordCanvasOperations(draw: (ctx: CanvasContext) => void): CanvasOp[] {
  const operations: CanvasOp[] = [];

  // Create a recording context that captures draw calls as CanvasOp[]
  const ctx: CanvasContext = {
    moveTo(x, y) { operations.push({ op: 'MoveTo', x, y }); },
    lineTo(x, y) { operations.push({ op: 'LineTo', x, y }); },
    bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
      operations.push({ op: 'BezierCurveTo', cp1x, cp1y, cp2x, cp2y, x, y });
    },
    quadraticCurveTo(cpx, cpy, x, y) {
      operations.push({ op: 'QuadraticCurveTo', cpx, cpy, x, y });
    },
    closePath() { operations.push({ op: 'ClosePath' }); },
    rect(x, y, w, h) { operations.push({ op: 'Rect', x, y, width: w, height: h }); },
    circle(cx, cy, r) { operations.push({ op: 'Circle', cx, cy, r }); },
    ellipse(cx, cy, rx, ry) { operations.push({ op: 'Ellipse', cx, cy, rx, ry }); },
    arc(cx, cy, r, startAngle, endAngle, counterclockwise = false) {
      operations.push({ op: 'Arc', cx, cy, r, start_angle: startAngle, end_angle: endAngle, counterclockwise });
    },
    line(x1, y1, x2, y2) {
      operations.push({ op: 'MoveTo', x: x1, y: y1 });
      operations.push({ op: 'LineTo', x: x2, y: y2 });
      operations.push({ op: 'Stroke' });
    },
    stroke() { operations.push({ op: 'Stroke' }); },
    fill() { operations.push({ op: 'Fill' }); },
    fillAndStroke() { operations.push({ op: 'FillAndStroke' }); },
    setFillColor(r, g, b) { operations.push({ op: 'SetFillColor', r, g, b }); },
    setStrokeColor(r, g, b) { operations.push({ op: 'SetStrokeColor', r, g, b }); },
    setLineWidth(w) { operations.push({ op: 'SetLineWidth', width: w }); },
    setLineCap(cap) { operations.push({ op: 'SetLineCap', cap }); },
    setLineJoin(join) { operations.push({ op: 'SetLineJoin', join }); },
    save() { operations.push({ op: 'Save' }); },
    restore() { operations.push({ op: 'Restore' }); },
  };

  // Execute the draw callback to record operations
  draw(ctx);

  return operations;
}
