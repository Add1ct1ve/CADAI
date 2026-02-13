import { nanoid } from 'nanoid';
import { getDrawingStore } from '$lib/stores/drawing.svelte';
import { getProjectStore } from '$lib/stores/project.svelte';
import {
  generateDrawingView,
  exportDrawingPdf,
  exportDrawingDxf,
  showSaveDialog,
} from '$lib/services/tauri';
import {
  VIEW_PROJECTION,
  getSheetDimensions,
  type DrawingId,
  type DrawingViewId,
  type DrawingView,
  type ViewDirection,
  type Drawing,
  type Dimension,
  type DrawingNote,
} from '$lib/types/drawing';

const VIEW_LABELS: Record<ViewDirection, string> = {
  front: 'Front View',
  back: 'Back View',
  top: 'Top View',
  bottom: 'Bottom View',
  left: 'Left View',
  right: 'Right View',
  iso: 'Isometric View',
  section: 'Section View',
};

/**
 * Generate a single view's SVG content via Build123d projection.
 */
export async function generateView(drawingId: DrawingId, viewId: DrawingViewId): Promise<void> {
  const store = getDrawingStore();
  const project = getProjectStore();
  const drawing = store.drawings.find((d) => d.id === drawingId);
  if (!drawing) return;
  const view = drawing.views.find((v) => v.id === viewId);
  if (!view) return;

  const code = project.code;
  if (!code) return;

  store.setGenerating(true);
  try {
    let projX: number, projY: number, projZ: number;
    if (view.direction === 'section') {
      // Default section is front view
      [projX, projY, projZ] = VIEW_PROJECTION.front;
    } else {
      [projX, projY, projZ] = VIEW_PROJECTION[view.direction];
    }

    const result = await generateDrawingView(
      code,
      projX, projY, projZ,
      view.showHidden,
      view.direction === 'section' ? view.sectionPlane : undefined,
      view.direction === 'section' ? view.sectionOffset : undefined,
    );

    store.updateView(drawingId, viewId, {
      svgContent: result.svgContent,
      width: result.width * view.scale,
      height: result.height * view.scale,
    });
  } finally {
    store.setGenerating(false);
  }
}

/**
 * Regenerate all views in a drawing.
 */
export async function regenerateAllViews(drawingId: DrawingId): Promise<void> {
  const store = getDrawingStore();
  const drawing = store.drawings.find((d) => d.id === drawingId);
  if (!drawing) return;

  await Promise.all(
    drawing.views.map((v) => generateView(drawingId, v.id)),
  );
}

/**
 * Add a new view to the drawing and generate its content.
 */
export async function addAndGenerateView(
  drawingId: DrawingId,
  direction: ViewDirection,
  options?: { sectionPlane?: 'XY' | 'XZ' | 'YZ'; sectionOffset?: number },
): Promise<void> {
  const store = getDrawingStore();
  const drawing = store.drawings.find((d) => d.id === drawingId);
  if (!drawing) return;

  // Calculate position based on existing views
  const [sheetW, sheetH] = getSheetDimensions(drawing.paperSize, drawing.orientation);
  const existingCount = drawing.views.length;
  const margin = 20;

  // Simple auto-layout: place views in a grid pattern
  const col = existingCount % 3;
  const row = Math.floor(existingCount / 3);
  const cellW = (sheetW - margin * 2 - 180) / 3; // 180 for title block area
  const cellH = (sheetH - margin * 2) / 2;

  const view: DrawingView = {
    id: nanoid(10),
    direction,
    label: VIEW_LABELS[direction],
    x: margin + col * cellW + cellW / 2,
    y: margin + row * cellH + cellH / 2,
    scale: 1,
    showHidden: true,
    svgContent: '',
    width: 0,
    height: 0,
    ...(direction === 'section' ? {
      sectionPlane: options?.sectionPlane ?? 'XY',
      sectionOffset: options?.sectionOffset ?? 0,
    } : {}),
  };

  store.addView(drawingId, view);
  await generateView(drawingId, view.id);
}

/**
 * Compose the full drawing sheet as a standalone SVG string.
 * Includes views, dimensions, notes, and title block.
 */
export function composeSheetSvg(drawing: Drawing): string {
  const [sheetW, sheetH] = getSheetDimensions(drawing.paperSize, drawing.orientation);

  let svg = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  svg += `<svg xmlns="http://www.w3.org/2000/svg" width="${sheetW}mm" height="${sheetH}mm" viewBox="0 0 ${sheetW} ${sheetH}">\n`;

  // Paper background
  svg += `  <rect x="0" y="0" width="${sheetW}" height="${sheetH}" fill="white"/>\n`;

  // Border
  const m = 10;
  svg += `  <rect x="${m}" y="${m}" width="${sheetW - m * 2}" height="${sheetH - m * 2}" fill="none" stroke="black" stroke-width="0.5"/>\n`;

  // Views
  for (const view of drawing.views) {
    if (!view.svgContent) continue;
    svg += `  <g transform="translate(${view.x}, ${view.y}) scale(${view.scale})">\n`;
    // Strip the outer <svg> wrapper from the Build123d SVG and insert the inner content
    const inner = extractSvgInner(view.svgContent);
    svg += `    ${inner}\n`;
    svg += `  </g>\n`;
    // View label
    svg += `  <text x="${view.x}" y="${view.y + view.height / 2 + 5}" font-size="3" font-family="Arial, sans-serif" text-anchor="middle">${view.label}</text>\n`;
  }

  // Dimensions
  for (const dim of drawing.dimensions) {
    svg += renderDimensionSvg(dim);
  }

  // Notes
  for (const note of drawing.notes) {
    const weight = note.bold ? 'bold' : 'normal';
    svg += `  <text x="${note.x}" y="${note.y}" font-size="${note.fontSize * 0.35}" font-family="Arial, sans-serif" font-weight="${weight}">${escapeXml(note.text)}</text>\n`;
  }

  // Title block
  svg += renderTitleBlockSvg(drawing, sheetW, sheetH);

  svg += `</svg>`;
  return svg;
}

function extractSvgInner(svgString: string): string {
  // Remove outer <svg ...> and </svg> tags, keeping inner content
  return svgString
    .replace(/<svg[^>]*>/, '')
    .replace(/<\/svg>\s*$/, '')
    .trim();
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function renderDimensionSvg(dim: Dimension): string {
  const offset = dim.offsetDistance;
  let svg = '';

  if (dim.type === 'linear') {
    // Determine if horizontal or vertical
    const dx = Math.abs(dim.x2 - dim.x1);
    const dy = Math.abs(dim.y2 - dim.y1);
    const isHorizontal = dx >= dy;

    if (isHorizontal) {
      const y = Math.min(dim.y1, dim.y2) - offset;
      // Extension lines
      svg += `  <line x1="${dim.x1}" y1="${dim.y1}" x2="${dim.x1}" y2="${y}" stroke="black" stroke-width="0.15"/>\n`;
      svg += `  <line x1="${dim.x2}" y1="${dim.y2}" x2="${dim.x2}" y2="${y}" stroke="black" stroke-width="0.15"/>\n`;
      // Dimension line
      svg += `  <line x1="${dim.x1}" y1="${y}" x2="${dim.x2}" y2="${y}" stroke="black" stroke-width="0.2" marker-start="url(#arrow-start)" marker-end="url(#arrow-end)"/>\n`;
      // Text
      const mx = (dim.x1 + dim.x2) / 2;
      const text = dim.text ?? dim.value.toFixed(2);
      svg += `  <text x="${mx}" y="${y - 1}" font-size="2.5" font-family="Arial, sans-serif" text-anchor="middle">${text}</text>\n`;
    } else {
      const x = Math.max(dim.x1, dim.x2) + offset;
      svg += `  <line x1="${dim.x1}" y1="${dim.y1}" x2="${x}" y2="${dim.y1}" stroke="black" stroke-width="0.15"/>\n`;
      svg += `  <line x1="${dim.x2}" y1="${dim.y2}" x2="${x}" y2="${dim.y2}" stroke="black" stroke-width="0.15"/>\n`;
      svg += `  <line x1="${x}" y1="${dim.y1}" x2="${x}" y2="${dim.y2}" stroke="black" stroke-width="0.2"/>\n`;
      const my = (dim.y1 + dim.y2) / 2;
      const text = dim.text ?? dim.value.toFixed(2);
      svg += `  <text x="${x + 1}" y="${my}" font-size="2.5" font-family="Arial, sans-serif" dominant-baseline="middle">${text}</text>\n`;
    }
  } else if (dim.type === 'radial' || dim.type === 'diameter') {
    const prefix = dim.type === 'diameter' ? '\u00D8' : 'R';
    const text = dim.text ?? `${prefix}${dim.value.toFixed(2)}`;
    svg += `  <line x1="${dim.x1}" y1="${dim.y1}" x2="${dim.x2}" y2="${dim.y2}" stroke="black" stroke-width="0.2"/>\n`;
    const mx = (dim.x1 + dim.x2) / 2;
    const my = (dim.y1 + dim.y2) / 2;
    svg += `  <text x="${mx}" y="${my - 1}" font-size="2.5" font-family="Arial, sans-serif" text-anchor="middle">${text}</text>\n`;
  }

  return svg;
}

function renderTitleBlockSvg(drawing: Drawing, sheetW: number, sheetH: number): string {
  const tb = drawing.titleBlock;
  const m = 10;
  const tbW = 170;
  const tbH = 40;
  const x0 = sheetW - m - tbW;
  const y0 = sheetH - m - tbH;

  let svg = '';
  // Title block border
  svg += `  <rect x="${x0}" y="${y0}" width="${tbW}" height="${tbH}" fill="none" stroke="black" stroke-width="0.4"/>\n`;

  // Horizontal dividers
  svg += `  <line x1="${x0}" y1="${y0 + 10}" x2="${x0 + tbW}" y2="${y0 + 10}" stroke="black" stroke-width="0.2"/>\n`;
  svg += `  <line x1="${x0}" y1="${y0 + 20}" x2="${x0 + tbW}" y2="${y0 + 20}" stroke="black" stroke-width="0.2"/>\n`;
  svg += `  <line x1="${x0}" y1="${y0 + 30}" x2="${x0 + tbW}" y2="${y0 + 30}" stroke="black" stroke-width="0.2"/>\n`;

  // Vertical divider (title block split into 2 columns)
  const xMid = x0 + tbW / 2;
  svg += `  <line x1="${xMid}" y1="${y0 + 10}" x2="${xMid}" y2="${y0 + tbH}" stroke="black" stroke-width="0.2"/>\n`;

  // Title (full width row)
  svg += `  <text x="${x0 + tbW / 2}" y="${y0 + 7}" font-size="4" font-family="Arial, sans-serif" text-anchor="middle" font-weight="bold">${escapeXml(tb.title)}</text>\n`;

  // Row 2: Author / Date
  svg += `  <text x="${x0 + 2}" y="${y0 + 17}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Author: ${tb.author}`)}</text>\n`;
  svg += `  <text x="${xMid + 2}" y="${y0 + 17}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Date: ${tb.date}`)}</text>\n`;

  // Row 3: Scale / Sheet
  svg += `  <text x="${x0 + 2}" y="${y0 + 27}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Scale: ${tb.scale}`)}</text>\n`;
  svg += `  <text x="${xMid + 2}" y="${y0 + 27}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Sheet: ${tb.sheetNumber}`)}</text>\n`;

  // Row 4: Material / Rev
  svg += `  <text x="${x0 + 2}" y="${y0 + 37}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Material: ${tb.material}`)}</text>\n`;
  svg += `  <text x="${xMid + 2}" y="${y0 + 37}" font-size="2.5" font-family="Arial, sans-serif">${escapeXml(`Rev: ${tb.revision}`)}</text>\n`;

  return svg;
}

/**
 * Export drawing to PDF via save dialog.
 */
export async function exportPdf(drawingId: DrawingId): Promise<string> {
  const store = getDrawingStore();
  const drawing = store.drawings.find((d) => d.id === drawingId);
  if (!drawing) return '';

  const path = await showSaveDialog('drawing.pdf', 'pdf');
  if (!path) return '';

  const svgContent = composeSheetSvg(drawing);
  return await exportDrawingPdf(svgContent, path);
}

/**
 * Export drawing to DXF via save dialog.
 */
export async function exportDxf(drawingId: DrawingId): Promise<string> {
  const store = getDrawingStore();
  const drawing = store.drawings.find((d) => d.id === drawingId);
  if (!drawing) return '';

  const path = await showSaveDialog('drawing.dxf', 'dxf');
  if (!path) return '';

  const svgContent = composeSheetSvg(drawing);
  return await exportDrawingDxf(svgContent, path);
}
