import { nanoid } from 'nanoid';
import type {
  Drawing,
  DrawingId,
  DrawingViewId,
  DimensionId,
  NoteId,
  DrawingView,
  Dimension,
  DrawingNote,
  TitleBlock,
  PaperSize,
  PaperOrientation,
  DrawingTool,
} from '$lib/types/drawing';
import { defaultTitleBlock } from '$lib/types/drawing';

// ─── State ──────────────────────────────────────
let drawings = $state<Drawing[]>([]);
let activeDrawingId = $state<DrawingId | null>(null);
let selectedViewId = $state<DrawingViewId | null>(null);
let selectedDimensionId = $state<DimensionId | null>(null);
let selectedNoteId = $state<NoteId | null>(null);
let drawingTool = $state<DrawingTool>('select');
let isGenerating = $state(false);

let drawingNameCounter = 0;

// ─── Serialization types ────────────────────────
export interface DrawingSnapshot {
  drawings: Drawing[];
  activeDrawingId: DrawingId | null;
  selectedViewId: DrawingViewId | null;
  selectedDimensionId: DimensionId | null;
  selectedNoteId: NoteId | null;
}

export interface DrawingSerialData {
  drawings: Drawing[];
}

// ─── Store ──────────────────────────────────────
export function getDrawingStore() {
  return {
    get drawings() { return drawings; },
    get activeDrawingId() { return activeDrawingId; },
    get selectedViewId() { return selectedViewId; },
    get selectedDimensionId() { return selectedDimensionId; },
    get selectedNoteId() { return selectedNoteId; },
    get drawingTool() { return drawingTool; },
    get isGenerating() { return isGenerating; },

    get activeDrawing(): Drawing | null {
      if (!activeDrawingId) return null;
      return drawings.find((d) => d.id === activeDrawingId) ?? null;
    },

    // ── Drawing CRUD ──

    createDrawing(
      name?: string,
      paperSize: PaperSize = 'A4',
      orientation: PaperOrientation = 'landscape',
    ): Drawing {
      const id = nanoid(10);
      drawingNameCounter++;
      const drawing: Drawing = {
        id,
        name: name ?? `Drawing ${drawingNameCounter}`,
        paperSize,
        orientation,
        views: [],
        dimensions: [],
        notes: [],
        titleBlock: defaultTitleBlock(),
      };
      drawings = [...drawings, drawing];
      activeDrawingId = id;
      return drawing;
    },

    removeDrawing(id: DrawingId) {
      drawings = drawings.filter((d) => d.id !== id);
      if (activeDrawingId === id) {
        activeDrawingId = drawings.length > 0 ? drawings[0].id : null;
      }
    },

    setActiveDrawing(id: DrawingId | null) {
      activeDrawingId = id;
      selectedViewId = null;
      selectedDimensionId = null;
      selectedNoteId = null;
    },

    // ── View CRUD ──

    addView(drawingId: DrawingId, view: DrawingView) {
      drawings = drawings.map((d) =>
        d.id === drawingId ? { ...d, views: [...d.views, view] } : d,
      );
    },

    updateView(drawingId: DrawingId, viewId: DrawingViewId, partial: Partial<DrawingView>) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, views: d.views.map((v) => v.id === viewId ? { ...v, ...partial } : v) }
          : d,
      );
    },

    removeView(drawingId: DrawingId, viewId: DrawingViewId) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? {
              ...d,
              views: d.views.filter((v) => v.id !== viewId),
              dimensions: d.dimensions.filter((dim) => dim.viewId !== viewId),
            }
          : d,
      );
      if (selectedViewId === viewId) selectedViewId = null;
    },

    // ── Dimension CRUD ──

    addDimension(drawingId: DrawingId, dim: Dimension) {
      drawings = drawings.map((d) =>
        d.id === drawingId ? { ...d, dimensions: [...d.dimensions, dim] } : d,
      );
    },

    updateDimension(drawingId: DrawingId, dimId: DimensionId, partial: Partial<Dimension>) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, dimensions: d.dimensions.map((dim) => dim.id === dimId ? { ...dim, ...partial } : dim) }
          : d,
      );
    },

    removeDimension(drawingId: DrawingId, dimId: DimensionId) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, dimensions: d.dimensions.filter((dim) => dim.id !== dimId) }
          : d,
      );
      if (selectedDimensionId === dimId) selectedDimensionId = null;
    },

    // ── Note CRUD ──

    addNote(drawingId: DrawingId, note: DrawingNote) {
      drawings = drawings.map((d) =>
        d.id === drawingId ? { ...d, notes: [...d.notes, note] } : d,
      );
    },

    updateNote(drawingId: DrawingId, noteId: NoteId, partial: Partial<DrawingNote>) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, notes: d.notes.map((n) => n.id === noteId ? { ...n, ...partial } : n) }
          : d,
      );
    },

    removeNote(drawingId: DrawingId, noteId: NoteId) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, notes: d.notes.filter((n) => n.id !== noteId) }
          : d,
      );
      if (selectedNoteId === noteId) selectedNoteId = null;
    },

    // ── Title block ──

    updateDrawingMeta(drawingId: DrawingId, partial: Partial<Pick<Drawing, 'name' | 'paperSize' | 'orientation'>>) {
      drawings = drawings.map((d) =>
        d.id === drawingId ? { ...d, ...partial } : d,
      );
    },

    updateTitleBlock(drawingId: DrawingId, partial: Partial<TitleBlock>) {
      drawings = drawings.map((d) =>
        d.id === drawingId
          ? { ...d, titleBlock: { ...d.titleBlock, ...partial } }
          : d,
      );
    },

    // ── Selection ──

    selectView(id: DrawingViewId | null) {
      selectedViewId = id;
      selectedDimensionId = null;
      selectedNoteId = null;
    },

    selectDimension(id: DimensionId | null) {
      selectedDimensionId = id;
      selectedViewId = null;
      selectedNoteId = null;
    },

    selectNote(id: NoteId | null) {
      selectedNoteId = id;
      selectedViewId = null;
      selectedDimensionId = null;
    },

    clearSelection() {
      selectedViewId = null;
      selectedDimensionId = null;
      selectedNoteId = null;
    },

    // ── Tool ──

    setDrawingTool(tool: DrawingTool) {
      drawingTool = tool;
    },

    setGenerating(val: boolean) {
      isGenerating = val;
    },

    // ── Delete selected ──

    deleteSelected() {
      if (!activeDrawingId) return;
      if (selectedViewId) {
        this.removeView(activeDrawingId, selectedViewId);
      } else if (selectedDimensionId) {
        this.removeDimension(activeDrawingId, selectedDimensionId);
      } else if (selectedNoteId) {
        this.removeNote(activeDrawingId, selectedNoteId);
      }
    },

    // ── Snapshot / Restore (for undo/redo) ──

    snapshot(): DrawingSnapshot {
      return {
        drawings: $state.snapshot(drawings) as Drawing[],
        activeDrawingId,
        selectedViewId,
        selectedDimensionId,
        selectedNoteId,
      };
    },

    restoreSnapshot(data: DrawingSnapshot) {
      drawings = data.drawings;
      activeDrawingId = data.activeDrawingId;
      selectedViewId = data.selectedViewId;
      selectedDimensionId = data.selectedDimensionId;
      selectedNoteId = data.selectedNoteId;
    },

    // ── Serialize / Restore (for save/load) ──

    serialize(): DrawingSerialData {
      return {
        drawings: $state.snapshot(drawings) as Drawing[],
      };
    },

    restore(data: DrawingSerialData) {
      drawings = data.drawings ?? [];
      activeDrawingId = null;
      selectedViewId = null;
      selectedDimensionId = null;
      selectedNoteId = null;
      drawingTool = 'select';
      isGenerating = false;
      // Rebuild name counter
      drawingNameCounter = 0;
      for (const d of drawings) {
        const match = d.name.match(/^Drawing (\d+)$/);
        if (match) drawingNameCounter = Math.max(drawingNameCounter, parseInt(match[1], 10));
      }
    },

    clearAll() {
      drawings = [];
      activeDrawingId = null;
      selectedViewId = null;
      selectedDimensionId = null;
      selectedNoteId = null;
      drawingTool = 'select';
      isGenerating = false;
      drawingNameCounter = 0;
    },
  };
}
