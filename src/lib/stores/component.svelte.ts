import type { Component, ComponentId, CadTransform } from '$lib/types/cad';

// ─── State ──────────────────────────────────────
let components = $state<Component[]>([]);
let nameCounter = $state(0);
let selectedComponentId = $state<ComponentId | null>(null);

// ─── Derived reverse lookup ─────────────────────
// Rebuilt whenever components changes for O(1) feature→component lookups
function buildFeatureMap(): Map<string, ComponentId> {
  const map = new Map<string, ComponentId>();
  for (const comp of components) {
    for (const fid of comp.featureIds) {
      map.set(fid, comp.id);
    }
  }
  return map;
}

const COMPONENT_COLORS = [
  '#f2cdcd', '#f5c2e7', '#cba6f7', '#f38ba8',
  '#eba0ac', '#fab387', '#f9e2af', '#a6e3a1',
  '#94e2d5', '#89dceb', '#74c7ec', '#89b4fa',
  '#b4befe',
];

function generateId(): string {
  return 'comp_' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

// ─── Serialization types ────────────────────────
export interface ComponentStoreSnapshot {
  components: Component[];
  nameCounter: number;
  selectedComponentId: ComponentId | null;
}

// ─── Store ──────────────────────────────────────
export function getComponentStore() {
  return {
    get components() {
      return components;
    },
    get selectedComponentId() {
      return selectedComponentId;
    },
    get selectedComponent(): Component | null {
      if (!selectedComponentId) return null;
      return components.find((c) => c.id === selectedComponentId) ?? null;
    },

    createComponent(featureIds: string[], name?: string): Component {
      nameCounter++;
      const comp: Component = {
        id: generateId(),
        name: name ?? `Component ${nameCounter}`,
        featureIds: [...featureIds],
        transform: { position: [0, 0, 0], rotation: [0, 0, 0] },
        visible: true,
        grounded: false,
        color: COMPONENT_COLORS[(nameCounter - 1) % COMPONENT_COLORS.length],
      };
      components = [...components, comp];
      return comp;
    },

    removeComponent(id: ComponentId) {
      components = components.filter((c) => c.id !== id);
      if (selectedComponentId === id) selectedComponentId = null;
    },

    updateComponent(id: ComponentId, partial: Partial<Component>) {
      components = components.map((c) =>
        c.id === id ? { ...c, ...partial } : c,
      );
    },

    addFeatureToComponent(componentId: ComponentId, featureId: string) {
      components = components.map((c) =>
        c.id === componentId && !c.featureIds.includes(featureId)
          ? { ...c, featureIds: [...c.featureIds, featureId] }
          : c,
      );
    },

    removeFeatureFromComponent(componentId: ComponentId, featureId: string) {
      components = components.map((c) =>
        c.id === componentId
          ? { ...c, featureIds: c.featureIds.filter((fid) => fid !== featureId) }
          : c,
      );
    },

    getComponentById(id: ComponentId): Component | null {
      return components.find((c) => c.id === id) ?? null;
    },

    getComponentForFeature(featureId: string): Component | null {
      const map = buildFeatureMap();
      const compId = map.get(featureId);
      if (!compId) return null;
      return components.find((c) => c.id === compId) ?? null;
    },

    /** Returns the feature→componentId map for efficient batch lookups */
    getFeatureComponentMap(): Map<string, ComponentId> {
      return buildFeatureMap();
    },

    setVisible(id: ComponentId, visible: boolean) {
      components = components.map((c) =>
        c.id === id ? { ...c, visible } : c,
      );
    },

    setGrounded(id: ComponentId, grounded: boolean) {
      components = components.map((c) =>
        c.id === id ? { ...c, grounded } : c,
      );
    },

    selectComponent(id: ComponentId | null) {
      selectedComponentId = id;
    },

    // ── Serialization (for undo/redo snapshots) ──
    snapshot(): ComponentStoreSnapshot {
      return {
        components: $state.snapshot(components) as Component[],
        nameCounter,
        selectedComponentId,
      };
    },

    restoreSnapshot(data: ComponentStoreSnapshot) {
      components = data.components;
      nameCounter = data.nameCounter;
      selectedComponentId = data.selectedComponentId;
    },

    // ── Serialization (for save/load) ──
    serialize(): { components: Component[]; nameCounter: number } {
      return {
        components: $state.snapshot(components) as Component[],
        nameCounter,
      };
    },

    restore(data: { components: Component[]; nameCounter?: number }) {
      components = data.components;
      nameCounter = data.nameCounter ?? data.components.length;
      selectedComponentId = null;
    },

    clearAll() {
      components = [];
      nameCounter = 0;
      selectedComponentId = null;
    },
  };
}
