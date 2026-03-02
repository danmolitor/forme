import React from 'react';
import { View, Text, Svg } from './components.js';
import type {
  BarChartProps,
  LineChartProps,
  PieChartProps,
  Style,
} from './types.js';

// ─── Helpers ────────────────────────────────────────────────────────

/** Round up to a "nice" axis maximum (1, 2, 5, 10, 20, 50, ...). */
function niceNumber(value: number): number {
  if (value <= 0) return 1;
  const exp = Math.floor(Math.log10(value));
  const frac = value / Math.pow(10, exp);
  let nice: number;
  if (frac <= 1.0) nice = 1;
  else if (frac <= 2.0) nice = 2;
  else if (frac <= 5.0) nice = 5;
  else nice = 10;
  return nice * Math.pow(10, exp);
}

/** Format number compactly for axis labels. */
function formatNumber(value: number): string {
  if (value >= 1_000_000) return (value / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M';
  if (value >= 1_000) return (value / 1_000).toFixed(1).replace(/\.0$/, '') + 'K';
  if (Number.isInteger(value)) return String(value);
  return value.toFixed(1);
}

/** Lighten a hex color toward white. amount=0 is original, amount=1 is white. */
function lightenColor(hex: string, amount: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  const lr = Math.round(r + (255 - r) * amount);
  const lg = Math.round(g + (255 - g) * amount);
  const lb = Math.round(b + (255 - b) * amount);
  return '#' + [lr, lg, lb].map(c => c.toString(16).padStart(2, '0')).join('');
}

/** Escape XML special characters in text. */
function escapeXml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// Chart layout constants
const AXIS_LABEL_FONT = 8;
const LABEL_MARGIN = 4;
const Y_AXIS_WIDTH = 40;
const X_AXIS_HEIGHT = 20;

// ─── BarChart ───────────────────────────────────────────────────────

/**
 * A bar chart rendered as SVG + positioned Text labels.
 *
 * @example
 * ```tsx
 * <BarChart width={400} height={200}
 *   data={[{ label: 'Jan', value: 120 }, { label: 'Feb', value: 90 }]}
 *   color="#1a365d" showGrid showValues />
 * ```
 */
export function BarChart(props: BarChartProps): React.ReactElement {
  const {
    width, height, data,
    color = '#1a365d',
    barGap = 4,
    showGrid = false,
    showValues = false,
    yAxisTicks = 5,
  } = props;

  if (data.length === 0) {
    return React.createElement(View, { style: { width, height, ...props.style } });
  }

  const plotLeft = Y_AXIS_WIDTH;
  const plotTop = 10;
  const plotWidth = width - plotLeft - 10;
  const plotHeight = height - plotTop - X_AXIS_HEIGHT;

  const maxValue = niceNumber(Math.max(...data.map(d => d.value)));

  // Build SVG content
  let svg = '';

  // Grid lines
  if (showGrid) {
    for (let t = 0; t <= yAxisTicks; t++) {
      const y = plotTop + plotHeight - (t / yAxisTicks) * plotHeight;
      svg += `<line x1="${plotLeft}" y1="${y}" x2="${plotLeft + plotWidth}" y2="${y}" stroke="#e2e8f0" stroke-width="0.5"/>`;
    }
  }

  // X axis
  svg += `<line x1="${plotLeft}" y1="${plotTop + plotHeight}" x2="${plotLeft + plotWidth}" y2="${plotTop + plotHeight}" stroke="#a0aec0" stroke-width="1"/>`;
  // Y axis
  svg += `<line x1="${plotLeft}" y1="${plotTop}" x2="${plotLeft}" y2="${plotTop + plotHeight}" stroke="#a0aec0" stroke-width="1"/>`;

  // Bars
  const barWidth = (plotWidth - barGap * (data.length + 1)) / data.length;
  for (let i = 0; i < data.length; i++) {
    const barH = (data[i].value / maxValue) * plotHeight;
    const x = plotLeft + barGap + i * (barWidth + barGap);
    const y = plotTop + plotHeight - barH;
    svg += `<rect x="${x}" y="${y}" width="${barWidth}" height="${barH}" fill="${escapeXml(color)}"/>`;
  }

  const viewBox = `0 0 ${width} ${height}`;

  // Y-axis tick labels + X-axis labels as positioned Text
  const labels: React.ReactElement[] = [];

  for (let t = 0; t <= yAxisTicks; t++) {
    const val = (t / yAxisTicks) * maxValue;
    const y = plotTop + plotHeight - (t / yAxisTicks) * plotHeight - AXIS_LABEL_FONT / 2;
    labels.push(
      React.createElement(Text, {
        key: `y-${t}`,
        style: {
          position: 'absolute' as const,
          top: y,
          left: 0,
          width: Y_AXIS_WIDTH - LABEL_MARGIN,
          fontSize: AXIS_LABEL_FONT,
          textAlign: 'right' as const,
          color: '#4a5568',
        },
      }, formatNumber(val))
    );
  }

  for (let i = 0; i < data.length; i++) {
    const x = plotLeft + barGap + i * (barWidth + barGap);
    labels.push(
      React.createElement(Text, {
        key: `x-${i}`,
        style: {
          position: 'absolute' as const,
          top: plotTop + plotHeight + LABEL_MARGIN,
          left: x,
          width: barWidth,
          fontSize: AXIS_LABEL_FONT,
          textAlign: 'center' as const,
          color: '#4a5568',
        },
      }, data[i].label)
    );

    if (showValues) {
      const barH = (data[i].value / maxValue) * plotHeight;
      const valY = plotTop + plotHeight - barH - AXIS_LABEL_FONT - 2;
      labels.push(
        React.createElement(Text, {
          key: `v-${i}`,
          style: {
            position: 'absolute' as const,
            top: valY,
            left: x,
            width: barWidth,
            fontSize: AXIS_LABEL_FONT,
            textAlign: 'center' as const,
            color: '#2d3748',
          },
        }, formatNumber(data[i].value))
      );
    }
  }

  return React.createElement(View, {
    style: { width, height, position: 'relative' as const, ...props.style } as Style,
  },
    React.createElement(Svg, { width, height, viewBox, content: svg }),
    ...labels,
  );
}

// ─── LineChart ───────────────────────────────────────────────────────

/**
 * A line chart rendered as SVG + positioned Text labels.
 *
 * @example
 * ```tsx
 * <LineChart width={400} height={200}
 *   data={[{ label: 'Jan', value: 120 }, { label: 'Feb', value: 90 }]}
 *   color="#2b6cb0" showDots showGrid showArea />
 * ```
 */
export function LineChart(props: LineChartProps): React.ReactElement {
  const {
    width, height, data,
    color = '#2b6cb0',
    strokeWidth = 2,
    showDots = false,
    showGrid = false,
    showArea = false,
  } = props;

  if (data.length === 0) {
    return React.createElement(View, { style: { width, height, ...props.style } });
  }

  const plotLeft = Y_AXIS_WIDTH;
  const plotTop = 10;
  const plotWidth = width - plotLeft - 10;
  const plotHeight = height - plotTop - X_AXIS_HEIGHT;

  const maxValue = niceNumber(Math.max(...data.map(d => d.value)));
  const yAxisTicks = 5;

  const pointX = (i: number) => plotLeft + (i / (data.length - 1 || 1)) * plotWidth;
  const pointY = (v: number) => plotTop + plotHeight - (v / maxValue) * plotHeight;

  let svg = '';

  // Grid lines
  if (showGrid) {
    for (let t = 0; t <= yAxisTicks; t++) {
      const y = plotTop + plotHeight - (t / yAxisTicks) * plotHeight;
      svg += `<line x1="${plotLeft}" y1="${y}" x2="${plotLeft + plotWidth}" y2="${y}" stroke="#e2e8f0" stroke-width="0.5"/>`;
    }
  }

  // Axes
  svg += `<line x1="${plotLeft}" y1="${plotTop + plotHeight}" x2="${plotLeft + plotWidth}" y2="${plotTop + plotHeight}" stroke="#a0aec0" stroke-width="1"/>`;
  svg += `<line x1="${plotLeft}" y1="${plotTop}" x2="${plotLeft}" y2="${plotTop + plotHeight}" stroke="#a0aec0" stroke-width="1"/>`;

  // Area fill
  if (showArea && data.length > 1) {
    const areaColor = lightenColor(color, 0.7);
    let pathD = `M ${pointX(0)} ${pointY(data[0].value)}`;
    for (let i = 1; i < data.length; i++) {
      pathD += ` L ${pointX(i)} ${pointY(data[i].value)}`;
    }
    pathD += ` L ${pointX(data.length - 1)} ${plotTop + plotHeight}`;
    pathD += ` L ${pointX(0)} ${plotTop + plotHeight} Z`;
    svg += `<path d="${pathD}" fill="${escapeXml(areaColor)}" stroke="none"/>`;
  }

  // Line
  if (data.length > 1) {
    let pathD = `M ${pointX(0)} ${pointY(data[0].value)}`;
    for (let i = 1; i < data.length; i++) {
      pathD += ` L ${pointX(i)} ${pointY(data[i].value)}`;
    }
    svg += `<path d="${pathD}" fill="none" stroke="${escapeXml(color)}" stroke-width="${strokeWidth}"/>`;
  }

  // Dots
  if (showDots) {
    for (let i = 0; i < data.length; i++) {
      const x = pointX(i);
      const y = pointY(data[i].value);
      svg += `<circle cx="${x}" cy="${y}" r="3" fill="${escapeXml(color)}"/>`;
    }
  }

  const viewBox = `0 0 ${width} ${height}`;

  const labels: React.ReactElement[] = [];

  // Y-axis labels
  for (let t = 0; t <= yAxisTicks; t++) {
    const val = (t / yAxisTicks) * maxValue;
    const y = plotTop + plotHeight - (t / yAxisTicks) * plotHeight - AXIS_LABEL_FONT / 2;
    labels.push(
      React.createElement(Text, {
        key: `y-${t}`,
        style: {
          position: 'absolute' as const,
          top: y,
          left: 0,
          width: Y_AXIS_WIDTH - LABEL_MARGIN,
          fontSize: AXIS_LABEL_FONT,
          textAlign: 'right' as const,
          color: '#4a5568',
        },
      }, formatNumber(val))
    );
  }

  // X-axis labels
  for (let i = 0; i < data.length; i++) {
    const x = pointX(i);
    const labelWidth = plotWidth / data.length;
    labels.push(
      React.createElement(Text, {
        key: `x-${i}`,
        style: {
          position: 'absolute' as const,
          top: plotTop + plotHeight + LABEL_MARGIN,
          left: x - labelWidth / 2,
          width: labelWidth,
          fontSize: AXIS_LABEL_FONT,
          textAlign: 'center' as const,
          color: '#4a5568',
        },
      }, data[i].label)
    );
  }

  return React.createElement(View, {
    style: { width, height, position: 'relative' as const, ...props.style } as Style,
  },
    React.createElement(Svg, { width, height, viewBox, content: svg }),
    ...labels,
  );
}

// ─── PieChart ───────────────────────────────────────────────────────

/**
 * A pie chart (or donut chart) rendered as SVG + positioned Text labels.
 *
 * Requires SVG arc path commands (A/a) — supported since engine v0.6.
 *
 * @example
 * ```tsx
 * <PieChart width={200} height={200}
 *   data={[
 *     { label: 'A', value: 40, color: '#1a365d' },
 *     { label: 'B', value: 30, color: '#2b6cb0' },
 *     { label: 'C', value: 30, color: '#63b3ed' },
 *   ]}
 *   showLabels />
 * ```
 */
export function PieChart(props: PieChartProps): React.ReactElement {
  const {
    width, height, data,
    showLabels = false,
    innerRadius = 0,
  } = props;

  const total = data.reduce((sum, d) => sum + d.value, 0);
  if (data.length === 0 || total <= 0) {
    return React.createElement(View, { style: { width, height, ...props.style } });
  }

  const cx = width / 2;
  const cy = height / 2;
  const labelMargin = showLabels ? 30 : 0;
  const outerR = Math.min(width - labelMargin * 2, height - labelMargin * 2) / 2;

  let svg = '';
  let startAngle = -Math.PI / 2; // Start from top

  const labels: React.ReactElement[] = [];

  for (let i = 0; i < data.length; i++) {
    const slice = data[i];
    const sliceAngle = (slice.value / total) * Math.PI * 2;
    const endAngle = startAngle + sliceAngle;

    // For full circle (single slice), use two arcs
    if (data.length === 1) {
      // Full circle — draw as ellipse-like two half arcs
      if (innerRadius > 0) {
        // Donut: outer arc + inner arc
        svg += `<path d="M ${cx} ${cy - outerR} A ${outerR} ${outerR} 0 1 1 ${cx} ${cy + outerR} A ${outerR} ${outerR} 0 1 1 ${cx} ${cy - outerR} Z M ${cx} ${cy - innerRadius} A ${innerRadius} ${innerRadius} 0 1 0 ${cx} ${cy + innerRadius} A ${innerRadius} ${innerRadius} 0 1 0 ${cx} ${cy - innerRadius} Z" fill="${escapeXml(slice.color)}" fill-rule="evenodd"/>`;
      } else {
        svg += `<circle cx="${cx}" cy="${cy}" r="${outerR}" fill="${escapeXml(slice.color)}"/>`;
      }
    } else {
      const largeArc = sliceAngle > Math.PI ? 1 : 0;
      const x1 = cx + outerR * Math.cos(startAngle);
      const y1 = cy + outerR * Math.sin(startAngle);
      const x2 = cx + outerR * Math.cos(endAngle);
      const y2 = cy + outerR * Math.sin(endAngle);

      let pathD: string;
      if (innerRadius > 0) {
        const ix1 = cx + innerRadius * Math.cos(startAngle);
        const iy1 = cy + innerRadius * Math.sin(startAngle);
        const ix2 = cx + innerRadius * Math.cos(endAngle);
        const iy2 = cy + innerRadius * Math.sin(endAngle);
        pathD = `M ${ix1} ${iy1} L ${x1} ${y1} A ${outerR} ${outerR} 0 ${largeArc} 1 ${x2} ${y2} L ${ix2} ${iy2} A ${innerRadius} ${innerRadius} 0 ${largeArc} 0 ${ix1} ${iy1} Z`;
      } else {
        pathD = `M ${cx} ${cy} L ${x1} ${y1} A ${outerR} ${outerR} 0 ${largeArc} 1 ${x2} ${y2} Z`;
      }
      svg += `<path d="${pathD}" fill="${escapeXml(slice.color)}"/>`;
    }

    // Labels
    if (showLabels) {
      const midAngle = startAngle + sliceAngle / 2;
      const labelR = outerR + 12;
      const lx = cx + labelR * Math.cos(midAngle);
      const ly = cy + labelR * Math.sin(midAngle);
      const isRight = Math.cos(midAngle) >= 0;

      labels.push(
        React.createElement(Text, {
          key: `label-${i}`,
          style: {
            position: 'absolute' as const,
            top: ly - AXIS_LABEL_FONT / 2,
            left: isRight ? lx : lx - 50,
            width: 50,
            fontSize: AXIS_LABEL_FONT,
            textAlign: isRight ? 'left' as const : 'right' as const,
            color: '#2d3748',
          },
        }, slice.label)
      );
    }

    startAngle = endAngle;
  }

  const viewBox = `0 0 ${width} ${height}`;

  return React.createElement(View, {
    style: { width, height, position: 'relative' as const, ...props.style } as Style,
  },
    React.createElement(Svg, { width, height, viewBox, content: svg }),
    ...labels,
  );
}
