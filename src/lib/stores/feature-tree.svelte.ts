import type { FeatureItem, FeatureKind, PrimitiveParams, SketchOperation, SceneObject } from '$lib/types/cad';
import { getSceneStore } from '$lib/stores/scene.svelte';
import { getSketchStore } from '$lib/stores/sketch.svelte';

// ─── State ──────────────────────────────────────
let featureOrder = $state<string[]>([]);
let suppressedIds = $state<Set<string>>(new Set());
let rollbackIndex = $state<number | null>(null);

// ─── Icon helpers ───────────────────────────────
function primitiveIcon(obj: SceneObject): string {
  if (obj.booleanOp) {
    switch (obj.booleanOp.type) {
      case 'union':     return '\u228C'; // ⊌
      case 'subtract':  return '\u2296'; // ⊖
      case 'intersect': return '\u2293'; // ⊓
    }
  }
  if (obj.splitOp) return '\u2702'; // ✂
  switch (obj.params.type) {
    case 'box': return '\u25FB'; // ◻
    case 'cylinder': return '\u25CB'; // ○
    case 'sphere': return '\u25CF'; // ●
    case 'cone': return '\u25B3'; // △
    default: return '\u25A0'; // ■
  }
}

function sketchIcon(op?: SketchOperation): string {
  if (!op) return '\u270E';              // ✎ plain sketch
  if (op.mode === 'cut') return '\u2702'; // ✂ cut
  if (op.type === 'revolve') return '\u27F3'; // ⟳
  if (op.type === 'sweep') return '\u2933';   // ⤳
  return '\u2B06';                        // ⬆ extrude add
}

function primitiveDetail(obj: SceneObject): string {
  if (obj.booleanOp) {
    const target = getSceneStore().getObjectById(obj.booleanOp.targetId);
    return `${obj.booleanOp.type} \u2192 ${target?.name ?? '?'}`;
  }
  if (obj.splitOp) return `split ${obj.splitOp.plane} (${obj.splitOp.keepSide})`;
  const params = obj.params;
  switch (params.type) {
    case 'box':
      return `Box ${params.width}\u00D7${params.depth}\u00D7${params.height}`;
    case 'cylinder':
      return `Cylinder r${params.radius} h${params.height}`;
    case 'sphere':
      return `Sphere r${params.radius}`;
    case 'cone':
      return `Cone r${params.bottomRadius}/${params.topRadius} h${params.height}`;
  }
}

function sketchDetail(sketch: { plane: string; entities: unknown[]; operation?: SketchOperation }): string {
  const entityCount = sketch.entities.length;
  let detail = `Sketch ${sketch.plane} (${entityCount} entit${entityCount === 1 ? 'y' : 'ies'})`;
  const op = sketch.operation;
  if (op) {
    if (op.type === 'extrude') {
      detail += ` \u2192 ${op.mode} ${op.distance}mm`;
      if (op.taper) detail += ` taper ${op.taper}\u00B0`;
    } else if (op.type === 'revolve') {
      detail += ` \u2192 revolve ${op.angle}\u00B0`;
    } else if (op.type === 'sweep') {
      detail += ` \u2192 sweep`;
    }
  }
  return detail;
}

// ─── Serialization types ────────────────────────
export interface FeatureTreeSnapshot {
  featureOrder: string[];
  suppressedIds: string[];
  rollbackIndex: number | null;
}

// ─── Store ──────────────────────────────────────
export function getFeatureTreeStore() {
  return {
    get featureOrder() {
      return featureOrder;
    },
    get suppressedIds() {
      return suppressedIds;
    },
    get rollbackIndex() {
      return rollbackIndex;
    },

    /** Build display items from scene/sketch stores + ordering */
    get features(): FeatureItem[] {
      const scene = getSceneStore();
      const sketchStore = getSketchStore();
      const items: FeatureItem[] = [];

      for (const id of featureOrder) {
        const obj = scene.getObjectById(id);
        if (obj) {
          items.push({
            id: obj.id,
            kind: 'primitive' as FeatureKind,
            name: obj.name,
            icon: primitiveIcon(obj),
            suppressed: suppressedIds.has(obj.id),
            detail: primitiveDetail(obj),
          });
          continue;
        }
        const sketch = sketchStore.getSketchById(id);
        if (sketch) {
          items.push({
            id: sketch.id,
            kind: 'sketch' as FeatureKind,
            name: sketch.name,
            icon: sketchIcon(sketch.operation),
            suppressed: suppressedIds.has(sketch.id),
            detail: sketchDetail(sketch),
          });
        }
      }

      return items;
    },

    /** Rollback index clamped to valid range */
    get effectiveRollbackIndex(): number {
      if (rollbackIndex === null) return featureOrder.length - 1;
      return Math.min(rollbackIndex, featureOrder.length - 1);
    },

    /** Feature IDs within rollback, not suppressed — for code gen ordering */
    get activeFeatureIds(): string[] {
      const maxIdx = this.effectiveRollbackIndex;
      const active: string[] = [];
      for (let i = 0; i <= maxIdx && i < featureOrder.length; i++) {
        const id = featureOrder[i];
        if (!suppressedIds.has(id)) {
          active.push(id);
        }
      }
      return active;
    },

    // ── Mutations ──

    registerFeature(id: string) {
      if (!featureOrder.includes(id)) {
        featureOrder = [...featureOrder, id];
      }
    },

    unregisterFeature(id: string) {
      featureOrder = featureOrder.filter((fid) => fid !== id);
      if (suppressedIds.has(id)) {
        const next = new Set(suppressedIds);
        next.delete(id);
        suppressedIds = next;
      }
      // Adjust rollback if needed
      if (rollbackIndex !== null && rollbackIndex >= featureOrder.length) {
        rollbackIndex = featureOrder.length > 0 ? featureOrder.length - 1 : null;
      }
    },

    reorder(fromIndex: number, toIndex: number) {
      if (fromIndex === toIndex) return;
      if (fromIndex < 0 || fromIndex >= featureOrder.length) return;
      if (toIndex < 0 || toIndex >= featureOrder.length) return;

      const newOrder = [...featureOrder];
      const [moved] = newOrder.splice(fromIndex, 1);
      newOrder.splice(toIndex, 0, moved);
      featureOrder = newOrder;
    },

    toggleSuppressed(id: string) {
      const next = new Set(suppressedIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      suppressedIds = next;
    },

    setRollbackIndex(index: number | null) {
      rollbackIndex = index;
    },

    /** Reconcile after load/restore: add missing IDs, remove orphans */
    syncFromStores() {
      const scene = getSceneStore();
      const sketchStore = getSketchStore();

      const validIds = new Set<string>();
      for (const obj of scene.objects) validIds.add(obj.id);
      for (const sk of sketchStore.sketches) validIds.add(sk.id);

      // Remove orphans from order
      const filtered = featureOrder.filter((id) => validIds.has(id));
      const ordered = new Set(filtered);

      // Add missing IDs (in store order)
      const missing: string[] = [];
      for (const obj of scene.objects) {
        if (!ordered.has(obj.id)) missing.push(obj.id);
      }
      for (const sk of sketchStore.sketches) {
        if (!ordered.has(sk.id)) missing.push(sk.id);
      }

      featureOrder = [...filtered, ...missing];

      // Clean suppressed of orphans
      const cleanSuppressed = new Set<string>();
      for (const id of suppressedIds) {
        if (validIds.has(id)) cleanSuppressed.add(id);
      }
      suppressedIds = cleanSuppressed;

      // Clamp rollback
      if (rollbackIndex !== null && rollbackIndex >= featureOrder.length) {
        rollbackIndex = featureOrder.length > 0 ? featureOrder.length - 1 : null;
      }
    },

    // ── Serialization (for undo/redo snapshots) ──
    snapshot(): FeatureTreeSnapshot {
      return {
        featureOrder: [...featureOrder],
        suppressedIds: [...suppressedIds],
        rollbackIndex,
      };
    },

    restoreSnapshot(data: FeatureTreeSnapshot) {
      featureOrder = data.featureOrder;
      suppressedIds = new Set(data.suppressedIds);
      rollbackIndex = data.rollbackIndex;
    },

    // ── Serialization (for save/load) ──
    serialize(): FeatureTreeSnapshot {
      return {
        featureOrder: [...featureOrder],
        suppressedIds: [...suppressedIds],
        rollbackIndex,
      };
    },

    restore(data: FeatureTreeSnapshot) {
      featureOrder = data.featureOrder;
      suppressedIds = new Set(data.suppressedIds);
      rollbackIndex = data.rollbackIndex;
    },

    clearAll() {
      featureOrder = [];
      suppressedIds = new Set();
      rollbackIndex = null;
    },
  };
}
