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
} from '$lib/types/cad';
import { getDefaultParams, getDefaultTransform } from '$lib/types/cad';

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
      return obj;
    },

    removeObject(id: ObjectId) {
      objects = objects.filter((o) => o.id !== id);
      selectedIds = selectedIds.filter((sid) => sid !== id);
      if (hoveredId === id) hoveredId = null;
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
      objects = objects.filter((o) => !toRemove.has(o.id));
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
    },
  };
}
