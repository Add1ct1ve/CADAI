import { nanoid } from 'nanoid';
import type {
  SketchId,
  SketchEntityId,
  SketchPlane,
  Point2D,
  Sketch,
  SketchEntity,
  SketchToolId,
} from '$lib/types/cad';

let sketches = $state<Sketch[]>([]);
let activeSketchId = $state<SketchId | null>(null);
let activeSketchTool = $state<SketchToolId>('sketch-line');
let selectedEntityIds = $state<SketchEntityId[]>([]);
let hoveredEntityId = $state<SketchEntityId | null>(null);
let drawingPoints = $state<Point2D[]>([]);
let previewPoint = $state<Point2D | null>(null);
let sketchSnap = $state<number | null>(0.5);

let sketchNameCounter = 0;

export interface SketchSnapshot {
  sketches: Sketch[];
  activeSketchId: SketchId | null;
}

export function getSketchStore() {
  return {
    get sketches() {
      return sketches;
    },
    get activeSketchId() {
      return activeSketchId;
    },
    get activeSketchTool() {
      return activeSketchTool;
    },
    get selectedEntityIds() {
      return selectedEntityIds;
    },
    get hoveredEntityId() {
      return hoveredEntityId;
    },
    get drawingPoints() {
      return drawingPoints;
    },
    get previewPoint() {
      return previewPoint;
    },
    get sketchSnap() {
      return sketchSnap;
    },

    get isInSketchMode(): boolean {
      return activeSketchId !== null;
    },

    get activeSketch(): Sketch | null {
      if (!activeSketchId) return null;
      return sketches.find((s) => s.id === activeSketchId) ?? null;
    },

    enterSketchMode(plane: SketchPlane) {
      const id = nanoid(10);
      sketchNameCounter++;
      const sketch: Sketch = {
        id,
        name: `sketch_${sketchNameCounter}`,
        plane,
        origin: [0, 0, 0],
        entities: [],
        closed: false,
      };
      sketches = [...sketches, sketch];
      activeSketchId = id;
      activeSketchTool = 'sketch-line';
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
    },

    exitSketchMode() {
      // Remove sketch if empty
      if (activeSketchId) {
        const sketch = sketches.find((s) => s.id === activeSketchId);
        if (sketch && sketch.entities.length === 0) {
          sketches = sketches.filter((s) => s.id !== activeSketchId);
        }
      }
      activeSketchId = null;
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
    },

    setSketchTool(tool: SketchToolId) {
      activeSketchTool = tool;
      drawingPoints = [];
      previewPoint = null;
    },

    addEntity(entity: SketchEntity) {
      if (!activeSketchId) return;
      sketches = sketches.map((s) =>
        s.id === activeSketchId
          ? { ...s, entities: [...s.entities, entity] }
          : s,
      );
    },

    removeEntity(entityId: SketchEntityId) {
      if (!activeSketchId) return;
      sketches = sketches.map((s) =>
        s.id === activeSketchId
          ? { ...s, entities: s.entities.filter((e) => e.id !== entityId) }
          : s,
      );
      selectedEntityIds = selectedEntityIds.filter((id) => id !== entityId);
    },

    selectEntity(entityId: SketchEntityId | null, additive = false) {
      if (!entityId) {
        selectedEntityIds = [];
        return;
      }
      if (additive) {
        if (selectedEntityIds.includes(entityId)) {
          selectedEntityIds = selectedEntityIds.filter((id) => id !== entityId);
        } else {
          selectedEntityIds = [...selectedEntityIds, entityId];
        }
      } else {
        selectedEntityIds = [entityId];
      }
    },

    setHoveredEntity(entityId: SketchEntityId | null) {
      hoveredEntityId = entityId;
    },

    deleteSelectedEntities() {
      if (!activeSketchId || selectedEntityIds.length === 0) return;
      const toRemove = new Set(selectedEntityIds);
      sketches = sketches.map((s) =>
        s.id === activeSketchId
          ? { ...s, entities: s.entities.filter((e) => !toRemove.has(e.id)) }
          : s,
      );
      selectedEntityIds = [];
    },

    addDrawingPoint(point: Point2D) {
      drawingPoints = [...drawingPoints, point];
    },

    setPreviewPoint(point: Point2D | null) {
      previewPoint = point;
    },

    clearDrawingState() {
      drawingPoints = [];
      previewPoint = null;
    },

    setSketchSnap(value: number | null) {
      sketchSnap = value;
    },

    // Generate a unique entity ID
    newEntityId(): SketchEntityId {
      return nanoid(10);
    },

    // ── Snapshot / Restore (for undo/redo) ──
    snapshot(): SketchSnapshot {
      return {
        sketches: $state.snapshot(sketches) as Sketch[],
        activeSketchId,
      };
    },

    restoreSnapshot(data: SketchSnapshot) {
      sketches = data.sketches;
      activeSketchId = data.activeSketchId;
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
    },

    // ── Serialize / Restore (for save/load) ──
    serialize(): { sketches: Sketch[] } {
      return {
        sketches: $state.snapshot(sketches) as Sketch[],
      };
    },

    restore(data: { sketches: Sketch[] }) {
      sketches = data.sketches;
      activeSketchId = null;
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
      // Rebuild name counter
      sketchNameCounter = 0;
      for (const s of sketches) {
        const match = s.name.match(/^sketch_(\d+)$/);
        if (match) {
          sketchNameCounter = Math.max(sketchNameCounter, parseInt(match[1], 10));
        }
      }
    },

    clearAll() {
      sketches = [];
      activeSketchId = null;
      activeSketchTool = 'sketch-line';
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
      sketchNameCounter = 0;
    },
  };
}
