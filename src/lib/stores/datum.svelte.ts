import { nanoid } from 'nanoid';
import type { DatumId, DatumPlane, DatumAxis, DatumPlaneDefinition } from '$lib/types/cad';
import { isDatumPlane, isDatumAxis } from '$lib/types/cad';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';

// ─── State ──────────────────────────────────────
let datumPlanes = $state<DatumPlane[]>([]);
let datumAxes = $state<DatumAxis[]>([]);
let selectedDatumId = $state<DatumId | null>(null);

let planeNameCounter = 0;
let axisNameCounter = 0;

// ─── Default colors ─────────────────────────────
const COLOR_OFFSET = '#89b4fa';    // blue
const COLOR_THREE_POINT = '#cba6f7'; // mauve
const COLOR_AXIS = '#a6e3a1';      // green

// ─── Serialization types ────────────────────────
export interface DatumSnapshot {
  datumPlanes: DatumPlane[];
  datumAxes: DatumAxis[];
  selectedDatumId: DatumId | null;
}

export interface DatumSerialData {
  datumPlanes: DatumPlane[];
  datumAxes: DatumAxis[];
}

// ─── Store ──────────────────────────────────────
export function getDatumStore() {
  return {
    get datumPlanes() {
      return datumPlanes;
    },
    get datumAxes() {
      return datumAxes;
    },
    get selectedDatumId() {
      return selectedDatumId;
    },

    get selectedDatum(): DatumPlane | DatumAxis | null {
      if (!selectedDatumId) return null;
      const plane = datumPlanes.find((p) => p.id === selectedDatumId);
      if (plane) return plane;
      return datumAxes.find((a) => a.id === selectedDatumId) ?? null;
    },

    /** Bulk insert pre-built datum planes and axes (for component import) */
    addDatums(planes: DatumPlane[], axes: DatumAxis[]) {
      const ft = getFeatureTreeStore();
      if (planes.length > 0) {
        datumPlanes = [...datumPlanes, ...planes];
        for (const dp of planes) ft.registerFeature(dp.id);
      }
      if (axes.length > 0) {
        datumAxes = [...datumAxes, ...axes];
        for (const da of axes) ft.registerFeature(da.id);
      }
    },

    // ── CRUD: Datum Planes ──

    addOffsetPlane(basePlane: 'XY' | 'XZ' | 'YZ', offset: number): DatumPlane {
      const id = nanoid(10);
      planeNameCounter++;
      const plane: DatumPlane = {
        id,
        name: `datum_plane_${planeNameCounter}`,
        definition: { type: 'offset', basePlane, offset },
        color: COLOR_OFFSET,
        visible: true,
      };
      datumPlanes = [...datumPlanes, plane];
      getFeatureTreeStore().registerFeature(id);
      return plane;
    },

    addThreePointPlane(
      p1: [number, number, number],
      p2: [number, number, number],
      p3: [number, number, number],
    ): DatumPlane {
      const id = nanoid(10);
      planeNameCounter++;
      const plane: DatumPlane = {
        id,
        name: `datum_plane_${planeNameCounter}`,
        definition: { type: 'threePoint', p1, p2, p3 },
        color: COLOR_THREE_POINT,
        visible: true,
      };
      datumPlanes = [...datumPlanes, plane];
      getFeatureTreeStore().registerFeature(id);
      return plane;
    },

    updateDatumPlane(id: DatumId, partial: Partial<DatumPlane>) {
      datumPlanes = datumPlanes.map((p) =>
        p.id === id ? { ...p, ...partial } : p,
      );
    },

    removeDatumPlane(id: DatumId) {
      datumPlanes = datumPlanes.filter((p) => p.id !== id);
      if (selectedDatumId === id) selectedDatumId = null;
      getFeatureTreeStore().unregisterFeature(id);
    },

    getDatumPlaneById(id: DatumId): DatumPlane | null {
      return datumPlanes.find((p) => p.id === id) ?? null;
    },

    // ── CRUD: Datum Axes ──

    addDatumAxis(
      origin: [number, number, number],
      direction: [number, number, number],
    ): DatumAxis {
      const id = nanoid(10);
      axisNameCounter++;
      const axis: DatumAxis = {
        id,
        name: `datum_axis_${axisNameCounter}`,
        origin,
        direction,
        color: COLOR_AXIS,
        visible: true,
      };
      datumAxes = [...datumAxes, axis];
      getFeatureTreeStore().registerFeature(id);
      return axis;
    },

    updateDatumAxis(id: DatumId, partial: Partial<DatumAxis>) {
      datumAxes = datumAxes.map((a) =>
        a.id === id ? { ...a, ...partial } : a,
      );
    },

    removeDatumAxis(id: DatumId) {
      datumAxes = datumAxes.filter((a) => a.id !== id);
      if (selectedDatumId === id) selectedDatumId = null;
      getFeatureTreeStore().unregisterFeature(id);
    },

    getDatumAxisById(id: DatumId): DatumAxis | null {
      return datumAxes.find((a) => a.id === id) ?? null;
    },

    // ── Generic lookup ──

    getDatumById(id: DatumId): DatumPlane | DatumAxis | null {
      const plane = datumPlanes.find((p) => p.id === id);
      if (plane) return plane;
      return datumAxes.find((a) => a.id === id) ?? null;
    },

    // ── Selection ──

    selectDatum(id: DatumId | null) {
      selectedDatumId = id;
    },

    // ── Snapshot / Restore (for undo/redo) ──

    snapshot(): DatumSnapshot {
      return {
        datumPlanes: $state.snapshot(datumPlanes) as DatumPlane[],
        datumAxes: $state.snapshot(datumAxes) as DatumAxis[],
        selectedDatumId,
      };
    },

    restoreSnapshot(data: DatumSnapshot) {
      datumPlanes = data.datumPlanes;
      datumAxes = data.datumAxes;
      selectedDatumId = data.selectedDatumId;
    },

    // ── Serialize / Restore (for save/load) ──

    serialize(): DatumSerialData {
      return {
        datumPlanes: $state.snapshot(datumPlanes) as DatumPlane[],
        datumAxes: $state.snapshot(datumAxes) as DatumAxis[],
      };
    },

    restore(data: DatumSerialData) {
      datumPlanes = data.datumPlanes ?? [];
      datumAxes = data.datumAxes ?? [];
      selectedDatumId = null;
      // Rebuild name counters
      planeNameCounter = 0;
      for (const p of datumPlanes) {
        const match = p.name.match(/^datum_plane_(\d+)$/);
        if (match) planeNameCounter = Math.max(planeNameCounter, parseInt(match[1], 10));
      }
      axisNameCounter = 0;
      for (const a of datumAxes) {
        const match = a.name.match(/^datum_axis_(\d+)$/);
        if (match) axisNameCounter = Math.max(axisNameCounter, parseInt(match[1], 10));
      }
    },

    clearAll() {
      datumPlanes = [];
      datumAxes = [];
      selectedDatumId = null;
      planeNameCounter = 0;
      axisNameCounter = 0;
    },
  };
}
