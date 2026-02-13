export type DrawingId = string;
export type DrawingViewId = string;
export type DimensionId = string;
export type NoteId = string;

export type PaperSize = 'A4' | 'A3' | 'A2' | 'A1' | 'Letter' | 'Tabloid';
export type PaperOrientation = 'landscape' | 'portrait';

/** Paper dimensions in mm [width, height] in portrait orientation */
export const PAPER_SIZES: Record<PaperSize, [number, number]> = {
  A4: [210, 297],
  A3: [297, 420],
  A2: [420, 594],
  A1: [594, 841],
  Letter: [216, 279],
  Tabloid: [279, 432],
};

export type ViewDirection = 'front' | 'back' | 'top' | 'bottom' | 'left' | 'right' | 'iso' | 'section';

/** Maps view direction to CAD projection vector (Z-up coordinate system) */
export const VIEW_PROJECTION: Record<Exclude<ViewDirection, 'section'>, [number, number, number]> = {
  front:  [0, -1, 0],
  back:   [0, 1, 0],
  top:    [0, 0, 1],
  bottom: [0, 0, -1],
  left:   [-1, 0, 0],
  right:  [1, 0, 0],
  iso:    [1, -1, 1],
};

export interface DrawingView {
  id: DrawingViewId;
  direction: ViewDirection;
  label: string;
  x: number;              // Position on sheet (mm from left)
  y: number;              // Position on sheet (mm from top)
  scale: number;          // e.g. 1 = 1:1, 0.5 = 1:2
  showHidden: boolean;
  svgContent: string;     // Generated SVG from Build123d
  width: number;          // Bounding box width in mm
  height: number;         // Bounding box height in mm
  // Section view specific
  sectionPlane?: 'XY' | 'XZ' | 'YZ';
  sectionOffset?: number;
}

export type DimensionType = 'linear' | 'angular' | 'radial' | 'diameter';

export interface Dimension {
  id: DimensionId;
  type: DimensionType;
  viewId: DrawingViewId;
  // Start/end points relative to view origin (mm in sheet space)
  x1: number; y1: number;
  x2: number; y2: number;
  value: number;
  text?: string;          // Override text
  offsetDistance: number;  // How far the dimension line is from the geometry
}

export interface DrawingNote {
  id: NoteId;
  text: string;
  x: number;
  y: number;
  fontSize: number;       // in points
  bold: boolean;
}

export interface TitleBlock {
  title: string;
  author: string;
  date: string;
  scale: string;
  sheetNumber: string;
  revision: string;
  material: string;
  company: string;
}

export interface Drawing {
  id: DrawingId;
  name: string;
  paperSize: PaperSize;
  orientation: PaperOrientation;
  views: DrawingView[];
  dimensions: Dimension[];
  notes: DrawingNote[];
  titleBlock: TitleBlock;
}

export type DrawingTool =
  | 'select'
  | 'add-view'
  | 'dimension-linear'
  | 'dimension-angular'
  | 'dimension-radial'
  | 'add-note';

/** Get sheet dimensions in mm, accounting for orientation */
export function getSheetDimensions(
  paperSize: PaperSize,
  orientation: PaperOrientation,
): [number, number] {
  const [w, h] = PAPER_SIZES[paperSize];
  return orientation === 'landscape' ? [h, w] : [w, h];
}

/** Create a default title block */
export function defaultTitleBlock(): TitleBlock {
  return {
    title: 'Untitled Drawing',
    author: '',
    date: new Date().toISOString().slice(0, 10),
    scale: '1:1',
    sheetNumber: '1 of 1',
    revision: 'A',
    material: '',
    company: '',
  };
}
