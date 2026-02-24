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
  SketchOperation,
  FilletParams,
  ChamferParams,
  ShellParams,
  HoleParams,
} from '$lib/types/cad';
import {
  initSolver as initConstraintSolver,
  destroySolver as destroyConstraintSolver,
  solve as runConstraintSolve,
  isSolverReady,
} from '$lib/services/constraint-solver';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { computeOffsetOriginCQ } from '$lib/services/sketch-plane-utils';

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

    setOperation(sketchId: SketchId, params: SketchOperation | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, operation: params } : s,
      );
    },

    setSketchShell(sketchId: SketchId, params: ShellParams | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, shell: params } : s,
      );
    },

    setSketchHoles(sketchId: SketchId, holes: HoleParams[] | undefined) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, holes } : s,
      );
    },

    addSketchHole(sketchId: SketchId, hole: HoleParams) {
      sketches = sketches.map((s) =>
        s.id === sketchId ? { ...s, holes: [...(s.holes ?? []), hole] } : s,
      );
    },

    removeSketchHole(sketchId: SketchId, index: number) {
      sketches = sketches.map((s) => {
        if (s.id !== sketchId) return s;
        const holes = [...(s.holes ?? [])];
        holes.splice(index, 1);
        return { ...s, holes: holes.length > 0 ? holes : undefined };
      });
    },

    updateSketchHole(sketchId: SketchId, index: number, hole: HoleParams) {
      sketches = sketches.map((s) => {
        if (s.id !== sketchId) return s;
        const holes = [...(s.holes ?? [])];
        holes[index] = hole;
        return { ...s, holes };
      });
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

    /** Bulk insert pre-built sketches (for component import — does not auto-name) */
    addSketches(newSketches: Sketch[]) {
      sketches = [...sketches, ...newSketches];
      const ft = getFeatureTreeStore();
      for (const sk of newSketches) {
        ft.registerFeature(sk.id);
      }
    },

    enterSketchMode(plane: SketchPlane) {
      const id = nanoid(10);
      sketchNameCounter++;

      // Resolve origin for datum planes and face references
      let origin: [number, number, number] = [0, 0, 0];
      if (plane.startsWith('face:')) {
        // Face reference plane: face:objectId:faceId:nx:ny:nz
        // Origin will be set from face data externally or defaults to [0,0,0]
        const parts = plane.split(':');
        if (parts.length >= 9) {
          // Extended format: face:objectId:faceId:nx:ny:nz:ox:oy:oz
          origin = [parseFloat(parts[6]), parseFloat(parts[7]), parseFloat(parts[8])];
        }
      } else if (plane !== 'XY' && plane !== 'XZ' && plane !== 'YZ') {
        const datumPlane = getDatumStore().getDatumPlaneById(plane);
        if (datumPlane) {
          if (datumPlane.definition.type === 'offset') {
            origin = computeOffsetOriginCQ(datumPlane.definition.basePlane, datumPlane.definition.offset);
          } else {
            origin = datumPlane.definition.p1;
          }
        }
      }

      const sketch: Sketch = {
        id,
        name: `sketch_${sketchNameCounter}`,
        plane,
        origin,
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

    applySketchOp(removeIds: string[], addEntities: SketchEntity[]) {
      if (!activeSketchId) return;
      const removeSet = new Set(removeIds);
      sketches = sketches.map((s) => {
        if (s.id !== activeSketchId) return s;
        const filteredEntities = s.entities.filter((e) => !removeSet.has(e.id));
        const filteredConstraints = (s.constraints ?? []).filter((c) =>
          !constraintReferencesAny(c, removeSet),
        );
        return {
          ...s,
          entities: [...filteredEntities, ...addEntities],
          constraints: filteredConstraints,
        };
      });
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
      // Migrate old extrude field → operation
      sketches = data.sketches.map(s => ({
        ...s,
        constraints: s.constraints ?? [],
        operation: s.operation ?? ((s as any).extrude
          ? { type: 'extrude' as const, ...(s as any).extrude }
          : undefined),
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
    case 'tangent':
      return c.entityId1 === entityId || c.entityId2 === entityId;
    case 'fix':
      return c.entityId === entityId;
    case 'midpoint':
      return c.pointEntityId === entityId || c.lineEntityId === entityId;
    case 'symmetric':
      return c.entityId1 === entityId || c.entityId2 === entityId || c.axisEntityId === entityId;
    case 'collinear':
      return c.entityId1 === entityId || c.entityId2 === entityId;
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
    case 'tangent':
      return ids.has(c.entityId1) || ids.has(c.entityId2);
    case 'fix':
      return ids.has(c.entityId);
    case 'midpoint':
      return ids.has(c.pointEntityId) || ids.has(c.lineEntityId);
    case 'symmetric':
      return ids.has(c.entityId1) || ids.has(c.entityId2) || ids.has(c.axisEntityId);
    case 'collinear':
      return ids.has(c.entityId1) || ids.has(c.entityId2);
  }
}
