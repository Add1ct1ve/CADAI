import { nanoid } from 'nanoid';
import type { MateId, MateType, MateReference, AssemblyMate } from '$lib/types/cad';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';

// ─── State ──────────────────────────────────────
let mates = $state<AssemblyMate[]>([]);
let selectedMateId = $state<MateId | null>(null);
let mateCreationMode = $state<MateType | null>(null);
let pendingRef1 = $state<MateReference | null>(null);
let nameCounter = 0;

// ─── Serialization types ────────────────────────
export interface MateSnapshot {
  mates: AssemblyMate[];
  selectedMateId: MateId | null;
}

export interface MateSerialData {
  mates: AssemblyMate[];
}

// ─── Store ──────────────────────────────────────
export function getMateStore() {
  return {
    get mates() {
      return mates;
    },
    get selectedMateId() {
      return selectedMateId;
    },
    get mateCreationMode() {
      return mateCreationMode;
    },
    get pendingRef1() {
      return pendingRef1;
    },

    get selectedMate(): AssemblyMate | null {
      if (!selectedMateId) return null;
      return mates.find((m) => m.id === selectedMateId) ?? null;
    },

    // ── CRUD ──

    addMate(mate: AssemblyMate) {
      mates = [...mates, mate];
      getFeatureTreeStore().registerFeature(mate.id);
    },

    removeMate(id: MateId) {
      mates = mates.filter((m) => m.id !== id);
      if (selectedMateId === id) selectedMateId = null;
      getFeatureTreeStore().unregisterFeature(id);
    },

    updateMate(id: MateId, partial: Partial<AssemblyMate>) {
      mates = mates.map((m) =>
        m.id === id ? { ...m, ...partial } as AssemblyMate : m,
      );
    },

    getMateById(id: MateId): AssemblyMate | null {
      return mates.find((m) => m.id === id) ?? null;
    },

    getMatesForComponent(compId: string): AssemblyMate[] {
      return mates.filter(
        (m) => m.ref1.componentId === compId || m.ref2.componentId === compId,
      );
    },

    // ── Selection ──

    selectMate(id: MateId | null) {
      selectedMateId = id;
    },

    // ── Creation mode ──

    startMateCreation(type: MateType) {
      mateCreationMode = type;
      pendingRef1 = null;
    },

    setPendingRef1(ref: MateReference) {
      pendingRef1 = ref;
    },

    completeMateCreation(ref2: MateReference, params?: { distance?: number; angle?: number }) {
      if (!mateCreationMode || !pendingRef1) return;

      const id = nanoid(10);
      nameCounter++;
      const compStore = getComponentStore();
      const c1 = compStore.getComponentById(pendingRef1.componentId)?.name ?? '?';
      const c2 = compStore.getComponentById(ref2.componentId)?.name ?? '?';

      let mate: AssemblyMate;
      switch (mateCreationMode) {
        case 'coincident':
          mate = {
            type: 'coincident',
            id,
            name: `mate_${nameCounter}`,
            ref1: pendingRef1,
            ref2,
            flipped: false,
          };
          break;
        case 'concentric':
          mate = {
            type: 'concentric',
            id,
            name: `mate_${nameCounter}`,
            ref1: pendingRef1,
            ref2,
          };
          break;
        case 'distance':
          mate = {
            type: 'distance',
            id,
            name: `mate_${nameCounter}`,
            ref1: pendingRef1,
            ref2,
            distance: params?.distance ?? 10,
          };
          break;
        case 'angle':
          mate = {
            type: 'angle',
            id,
            name: `mate_${nameCounter}`,
            ref1: pendingRef1,
            ref2,
            angle: params?.angle ?? 90,
          };
          break;
      }

      this.addMate(mate);
      mateCreationMode = null;
      pendingRef1 = null;
      selectedMateId = mate.id;
    },

    cancelMateCreation() {
      mateCreationMode = null;
      pendingRef1 = null;
    },

    // ── Snapshot / Restore (for undo/redo) ──

    snapshot(): MateSnapshot {
      return {
        mates: $state.snapshot(mates) as AssemblyMate[],
        selectedMateId,
      };
    },

    restoreSnapshot(data: MateSnapshot) {
      mates = data.mates;
      selectedMateId = data.selectedMateId;
    },

    // ── Serialize / Restore (for save/load) ──

    serialize(): MateSerialData {
      return {
        mates: $state.snapshot(mates) as AssemblyMate[],
      };
    },

    restore(data: MateSerialData) {
      mates = data.mates ?? [];
      selectedMateId = null;
      mateCreationMode = null;
      pendingRef1 = null;
      // Rebuild name counter
      nameCounter = 0;
      for (const m of mates) {
        const match = m.name.match(/^mate_(\d+)$/);
        if (match) nameCounter = Math.max(nameCounter, parseInt(match[1], 10));
      }
    },

    clearAll() {
      mates = [];
      selectedMateId = null;
      mateCreationMode = null;
      pendingRef1 = null;
      nameCounter = 0;
    },
  };
}
