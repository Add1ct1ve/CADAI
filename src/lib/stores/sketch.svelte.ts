import { nanoid } from 'nanoid';
import type {
  SketchId,
  SketchEntityId,
  SketchConstraintId,
  SketchPlane,
  Point2D,
  Sketch,
  SketchEntity,
  SketchConstraint,
  ConstraintState,
  SketchToolId,
  ExtrudeParams,
  FilletParams,
  ChamferParams,
} from '$lib/types/cad';
import {
  initSolver as initConstraintSolver,
  destroySolver as destroyConstraintSolver,
  solve as runConstraintSolve,
  isSolverReady,
} from '$lib/services/constraint-solver';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';

let sketches = $state<Sketch[]>([]);
let activeSketchId = $state<SketchId | null>(null);
let selectedSketchId = $state<SketchId | null>(null);
let activeSketchTool = $state<SketchToolId>('sketch-line');
let selectedEntityIds = $state<SketchEntityId[]>([]);
let hoveredEntityId = $state<SketchEntityId | null>(null);
let drawingPoints = $state<Point2D[]>([]);
let previewPoint = $state<Point2D | null>(null);
let sketchSnap = $state<number | null>(0.5);
let constraintState = $state<ConstraintState>('under-constrained');
let degreesOfFreedom = $state<number>(-1);

let sketchNameCounter = 0;

export interface SketchSnapshot {
  sketches: Sketch[];
  activeSketchId: SketchId | null;
  selectedSketchId: SketchId | null;
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
    get constraintState() {
      return constraintState;
    },
    get degreesOfFreedom() {
      return degreesOfFreedom;
    },

    get isInSketchMode(): boolean {
      return activeSketchId !== null;
    },

    get activeSketch(): Sketch | null {
      if (!activeSketchId) return null;
      return sketches.find((s) => s.id === activeSketchId) ?? null;
    },

    get selectedSketchId() {
      return selectedSketchId;
    },

    get selectedSketch(): Sketch | null {
      if (!selectedSketchId) return null;
      return sketches.find((s) => s.id === selectedSketchId) ?? null;
    },

    selectSketch(id: SketchId | null) {
      selectedSketchId = id;
    },

    getSketchById(id: SketchId): Sketch | null {
      return sketches.find((s) => s.id === id) ?? null;
    },

    setExtrude(sketchId: SketchId, params: ExtrudeParams | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, extrude: params } : s,
      );
    },

    setSketchFillet(sketchId: SketchId, params: FilletParams | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, fillet: params } : s,
      );
    },

    setSketchChamfer(sketchId: SketchId, params: ChamferParams | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, chamfer: params } : s,
      );
    },

    removeSketch(id: SketchId) {
      sketches = sketches.filter((s) => s.id !== id);
      if (selectedSketchId === id) selectedSketchId = null;
      if (activeSketchId === id) activeSketchId = null;
      getFeatureTreeStore().unregisterFeature(id);
    },

    editSketch(id: SketchId) {
      activeSketchId = id;
      selectedSketchId = null;
      activeSketchTool = 'sketch-line';
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
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
        constraints: [],
        closed: false,
      };
      sketches = [...sketches, sketch];
      activeSketchId = id;
      activeSketchTool = 'sketch-line';
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
      getFeatureTreeStore().registerFeature(id);
    },

    exitSketchMode() {
      // Remove sketch if empty
      if (activeSketchId) {
        const sketch = sketches.find((s) => s.id === activeSketchId);
        if (sketch && sketch.entities.length === 0) {
          sketches = sketches.filter((s) => s.id !== activeSketchId);
          getFeatureTreeStore().unregisterFeature(activeSketchId);
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
      sketches = sketches.map((s) => {
        if (s.id !== activeSketchId) return s;
        const newEntities = s.entities.filter((e) => !toRemove.has(e.id));
        // Remove constraints that reference any deleted entity
        const newConstraints = (s.constraints ?? []).filter((c) =>
          !constraintReferencesAny(c, toRemove),
        );
        return { ...s, entities: newEntities, constraints: newConstraints };
      });
      selectedEntityIds = [];
      this.runSolver();
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

    // ── Constraint solver lifecycle ──

    async initSolver() {
      await initConstraintSolver();
    },

    destroySolver() {
      destroyConstraintSolver();
    },

    // ── Constraint CRUD ──

    addConstraint(constraint: SketchConstraint) {
      if (!activeSketchId) return;
      sketches = sketches.map((s) =>
        s.id === activeSketchId
          ? { ...s, constraints: [...(s.constraints ?? []), constraint] }
          : s,
      );
      this.runSolver();
    },

    removeConstraint(constraintId: SketchConstraintId) {
      if (!activeSketchId) return;
      sketches = sketches.map((s) =>
        s.id === activeSketchId
          ? { ...s, constraints: (s.constraints ?? []).filter((c) => c.id !== constraintId) }
          : s,
      );
      this.runSolver();
    },

    getConstraintsForEntity(entityId: SketchEntityId): SketchConstraint[] {
      const sketch = activeSketchId ? sketches.find(s => s.id === activeSketchId) : null;
      if (!sketch) return [];
      return (sketch.constraints ?? []).filter(c => constraintReferencesEntity(c, entityId));
    },

    runSolver() {
      if (!isSolverReady()) return;
      const sketch = activeSketchId ? sketches.find(s => s.id === activeSketchId) : null;
      if (!sketch) return;

      const result = runConstraintSolve(sketch.entities, sketch.constraints ?? []);
      constraintState = result.status;
      degreesOfFreedom = result.dof;

      // Update entities with solved positions
      if (result.updatedEntities !== sketch.entities) {
        sketches = sketches.map((s) =>
          s.id === activeSketchId
            ? { ...s, entities: result.updatedEntities }
            : s,
        );
      }
    },

    // ── Snapshot / Restore (for undo/redo) ──
    snapshot(): SketchSnapshot {
      return {
        sketches: $state.snapshot(sketches) as Sketch[],
        activeSketchId,
        selectedSketchId,
      };
    },

    restoreSnapshot(data: SketchSnapshot) {
      sketches = data.sketches;
      activeSketchId = data.activeSketchId;
      selectedSketchId = data.selectedSketchId ?? null;
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
      // Ensure backward compatibility: add constraints field if missing
      sketches = data.sketches.map(s => ({
        ...s,
        constraints: s.constraints ?? [],
      }));
      activeSketchId = null;
      selectedSketchId = null;
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
      selectedSketchId = null;
      activeSketchTool = 'sketch-line';
      selectedEntityIds = [];
      hoveredEntityId = null;
      drawingPoints = [];
      previewPoint = null;
      sketchNameCounter = 0;
      constraintState = 'under-constrained';
      degreesOfFreedom = -1;
    },
  };
}

// ── Helper: check if a constraint references a given entity ──

function constraintReferencesEntity(c: SketchConstraint, entityId: string): boolean {
  switch (c.type) {
    case 'coincident':
      return c.point1.entityId === entityId || c.point2.entityId === entityId;
    case 'horizontal':
    case 'vertical':
      return c.entityId === entityId;
    case 'parallel':
    case 'perpendicular':
    case 'equal':
    case 'angle':
      return c.entityId1 === entityId || c.entityId2 === entityId;
    case 'distance':
      return c.point1.entityId === entityId || c.point2.entityId === entityId;
    case 'radius':
      return c.entityId === entityId;
  }
}

function constraintReferencesAny(c: SketchConstraint, ids: Set<string>): boolean {
  switch (c.type) {
    case 'coincident':
      return ids.has(c.point1.entityId) || ids.has(c.point2.entityId);
    case 'horizontal':
    case 'vertical':
      return ids.has(c.entityId);
    case 'parallel':
    case 'perpendicular':
    case 'equal':
    case 'angle':
      return ids.has(c.entityId1) || ids.has(c.entityId2);
    case 'distance':
      return ids.has(c.point1.entityId) || ids.has(c.point2.entityId);
    case 'radius':
      return ids.has(c.entityId);
  }
}
