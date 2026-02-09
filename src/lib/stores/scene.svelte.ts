import { nanoid } from 'nanoid';
import type {
  ObjectId,
  SceneObject,
  PrimitiveParams,
  CadTransform,
  CodeMode,
  PrimitiveType,
  FilletParams,
  ChamferParams,
  ShellParams,
  HoleParams,
  BooleanOp,
  SplitOp,
  PatternOp,
} from '$lib/types/cad';
import { getDefaultParams, getDefaultTransform } from '$lib/types/cad';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';

let objects = $state<SceneObject[]>([]);
let selectedIds = $state<ObjectId[]>([]);
let hoveredId = $state<ObjectId | null>(null);
let codeMode = $state<CodeMode>('parametric');

// Counter for naming objects (box_1, box_2, etc.)
const nameCounts: Record<string, number> = {};

function nextName(type: PrimitiveType): string {
  nameCounts[type] = (nameCounts[type] ?? 0) + 1;
  return `${type}_${nameCounts[type]}`;
}

export function getSceneStore() {
  return {
    get objects() {
      return objects;
    },
    get selectedIds() {
      return selectedIds;
    },
    get hoveredId() {
      return hoveredId;
    },
    get codeMode() {
      return codeMode;
    },

    get selectedObjects(): SceneObject[] {
      return objects.filter((o) => selectedIds.includes(o.id));
    },

    get firstSelected(): SceneObject | null {
      if (selectedIds.length === 0) return null;
      return objects.find((o) => o.id === selectedIds[0]) ?? null;
    },

    addObject(
      type: PrimitiveType,
      position?: [number, number, number],
    ): SceneObject {
      const id = nanoid(10);
      const transform = getDefaultTransform();
      if (position) transform.position = position;

      const obj: SceneObject = {
        id,
        name: nextName(type),
        params: getDefaultParams(type),
        transform,
        color: '#89b4fa',
        visible: true,
        locked: false,
      };
      objects = [...objects, obj];
      getFeatureTreeStore().registerFeature(id);
      return obj;
    },

    /** Bulk insert pre-built objects (for component import — does not auto-name) */
    addObjects(objs: SceneObject[]) {
      objects = [...objects, ...objs];
      const ft = getFeatureTreeStore();
      for (const obj of objs) {
        ft.registerFeature(obj.id);
      }
    },

    removeObject(id: ObjectId) {
      // Orphan any boolean tools that reference this target
      objects = objects.map((o) =>
        o.booleanOp?.targetId === id ? { ...o, booleanOp: undefined } : o,
      );
      objects = objects.filter((o) => o.id !== id);
      selectedIds = selectedIds.filter((sid) => sid !== id);
      if (hoveredId === id) hoveredId = null;
      getFeatureTreeStore().unregisterFeature(id);
    },

    updateObject(id: ObjectId, partial: Partial<SceneObject>) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, ...partial } : o,
      );
    },

    updateParams(id: ObjectId, params: PrimitiveParams) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, params } : o,
      );
    },

    updateTransform(id: ObjectId, transform: CadTransform) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, transform } : o,
      );
    },

    setFillet(id: ObjectId, params: FilletParams | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, fillet: params } : o,
      );
    },

    setChamfer(id: ObjectId, params: ChamferParams | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, chamfer: params } : o,
      );
    },

    setShell(id: ObjectId, params: ShellParams | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, shell: params } : o,
      );
    },

    setHoles(id: ObjectId, holes: HoleParams[] | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, holes } : o,
      );
    },

    addHole(id: ObjectId, hole: HoleParams) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, holes: [...(o.holes ?? []), hole] } : o,
      );
    },

    removeHole(id: ObjectId, index: number) {
      objects = objects.map((o) => {
        if (o.id !== id) return o;
        const holes = [...(o.holes ?? [])];
        holes.splice(index, 1);
        return { ...o, holes: holes.length > 0 ? holes : undefined };
      });
    },

    updateHole(id: ObjectId, index: number, hole: HoleParams) {
      objects = objects.map((o) => {
        if (o.id !== id) return o;
        const holes = [...(o.holes ?? [])];
        holes[index] = hole;
        return { ...o, holes };
      });
    },

    setBooleanOp(id: ObjectId, op: BooleanOp | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, booleanOp: op, splitOp: op ? undefined : o.splitOp, patternOp: op ? undefined : o.patternOp } : o,
      );
    },

    setSplitOp(id: ObjectId, op: SplitOp | undefined) {
      objects = objects.map((o) =>
        o.id === id ? { ...o, splitOp: op, booleanOp: op ? undefined : o.booleanOp, patternOp: op ? undefined : o.patternOp } : o,
      );
    },

    setPatternOp(id: ObjectId, op: PatternOp | undefined) {
      objects = objects.map((o) =>
        o.id === id
          ? { ...o, patternOp: op, booleanOp: op ? undefined : o.booleanOp, splitOp: op ? undefined : o.splitOp }
          : o,
      );
    },

    isBooleanTool(id: ObjectId): boolean {
      const obj = objects.find((o) => o.id === id);
      return !!obj?.booleanOp;
    },

    isBooleanTarget(id: ObjectId): boolean {
      return objects.some((o) => o.booleanOp?.targetId === id);
    },

    select(id: ObjectId, additive = false) {
      if (additive) {
        if (selectedIds.includes(id)) {
          selectedIds = selectedIds.filter((sid) => sid !== id);
        } else {
          selectedIds = [...selectedIds, id];
        }
      } else {
        selectedIds = [id];
      }
    },

    clearSelection() {
      selectedIds = [];
    },

    setHovered(id: ObjectId | null) {
      hoveredId = id;
    },

    deleteSelected() {
      const toRemove = new Set(selectedIds);
      // Orphan any boolean tools that reference deleted targets
      objects = objects.map((o) =>
        o.booleanOp?.targetId && toRemove.has(o.booleanOp.targetId) && !toRemove.has(o.id)
          ? { ...o, booleanOp: undefined }
          : o,
      );
      objects = objects.filter((o) => !toRemove.has(o.id));
      for (const id of toRemove) {
        getFeatureTreeStore().unregisterFeature(id);
      }
      selectedIds = [];
    },

    setCodeMode(mode: CodeMode) {
      codeMode = mode;
    },

    clearAll() {
      objects = [];
      selectedIds = [];
      hoveredId = null;
      // Reset name counters
      for (const key of Object.keys(nameCounts)) {
        delete nameCounts[key];
      }
      // Note: feature tree clearAll is called by the caller (projectNew)
    },

    getObjectById(id: ObjectId): SceneObject | undefined {
      return objects.find((o) => o.id === id);
    },

    snapshot(): { objects: SceneObject[]; selectedIds: ObjectId[] } {
      return {
        objects: $state.snapshot(objects) as SceneObject[],
        selectedIds: [...selectedIds],
      };
    },

    restoreSnapshot(data: { objects: SceneObject[]; selectedIds: ObjectId[] }) {
      objects = data.objects;
      selectedIds = data.selectedIds;
      hoveredId = null;

      // Rebuild name counters from restored object names
      for (const key of Object.keys(nameCounts)) {
        delete nameCounts[key];
      }
      for (const obj of objects) {
        const match = obj.name.match(/^(\w+)_(\d+)$/);
        if (match) {
          const type = match[1];
          const num = parseInt(match[2], 10);
          nameCounts[type] = Math.max(nameCounts[type] ?? 0, num);
        }
      }
      // Note: feature tree sync is handled by the caller (restoreFullSnapshot)
    },

    serialize(): { objects: SceneObject[]; codeMode: CodeMode } {
      return {
        objects: $state.snapshot(objects) as SceneObject[],
        codeMode,
      };
    },

    restore(data: { objects: SceneObject[]; codeMode: CodeMode }) {
      objects = data.objects;
      selectedIds = [];
      hoveredId = null;
      codeMode = data.codeMode;

      // Rebuild name counters from restored object names
      for (const key of Object.keys(nameCounts)) {
        delete nameCounts[key];
      }
      for (const obj of objects) {
        const match = obj.name.match(/^(\w+)_(\d+)$/);
        if (match) {
          const type = match[1];
          const num = parseInt(match[2], 10);
          nameCounts[type] = Math.max(nameCounts[type] ?? 0, num);
        }
      }
      // Note: feature tree sync is handled by the caller (projectOpen)
    },
  };
}
