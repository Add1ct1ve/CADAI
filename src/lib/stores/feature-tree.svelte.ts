import type { FeatureItem, FeatureKind, PrimitiveParams, SketchOperation, SceneObject, DatumPlane, DatumAxis, Component } from '$lib/types/cad';
import { isDatumPlane, isDatumAxis } from '$lib/types/cad';
import { getSceneStore } from '$lib/stores/scene.svelte';
import { getSketchStore } from '$lib/stores/sketch.svelte';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';
import { getMateStore } from '$lib/stores/mate.svelte';
import { getSettingsStore } from '$lib/stores/settings.svelte';
import { unitSuffix } from '$lib/services/units';

function currentUnitSuffix(): string {
  return unitSuffix(getSettingsStore().config.display_units);
}

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
  if (obj.patternOp) {
    switch (obj.patternOp.type) {
      case 'mirror':   return '\u2194'; // ↔
      case 'linear':   return '\u22EF'; // ⋯
      case 'circular': return '\u21BB'; // ↻
    }
  }
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
  if (obj.patternOp) {
    switch (obj.patternOp.type) {
      case 'mirror':
        return `mirror ${obj.patternOp.plane}${obj.patternOp.offset ? ` +${obj.patternOp.offset}` : ''}`;
      case 'linear':
        return `${obj.patternOp.count}\u00D7 linear ${obj.patternOp.direction} @${obj.patternOp.spacing}${currentUnitSuffix()}`;
      case 'circular':
        return `${obj.patternOp.count}\u00D7 circular ${obj.patternOp.axis} ${obj.patternOp.fullAngle}\u00B0`;
    }
  }
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
      detail += ` \u2192 ${op.mode} ${op.distance}${currentUnitSuffix()}`;
      if (op.taper) detail += ` taper ${op.taper}\u00B0`;
    } else if (op.type === 'revolve') {
      detail += ` \u2192 revolve ${op.angle}\u00B0`;
    } else if (op.type === 'sweep') {
      detail += ` \u2192 sweep`;
    }
  }
  return detail;
}

function datumPlaneDetail(datum: DatumPlane): string {
  if (datum.definition.type === 'offset') {
    const sign = datum.definition.offset >= 0 ? '+' : '';
    return `Offset ${datum.definition.basePlane} ${sign}${datum.definition.offset}${currentUnitSuffix()}`;
  }
  const { p1 } = datum.definition;
  return `3-Point (${p1[0]},${p1[1]},${p1[2]})...`;
}

function datumAxisDetail(datum: DatumAxis): string {
  const [dx, dy, dz] = datum.direction;
  const [ox, oy, oz] = datum.origin;
  return `Axis [${dx},${dy},${dz}] at (${ox},${oy},${oz})`;
}

/** Build a single FeatureItem from a feature ID (shared by root and component children) */
function buildFeatureItem(
  id: string,
  scene: ReturnType<typeof getSceneStore>,
  sketchStore: ReturnType<typeof getSketchStore>,
): FeatureItem | null {
  const obj = scene.getObjectById(id);
  if (obj) {
    return {
      id: obj.id,
      kind: 'primitive' as FeatureKind,
      name: obj.name,
      icon: primitiveIcon(obj),
      suppressed: suppressedIds.has(obj.id),
      detail: primitiveDetail(obj),
      depth: 0,
    };
  }
  const sketch = sketchStore.getSketchById(id);
  if (sketch) {
    return {
      id: sketch.id,
      kind: 'sketch' as FeatureKind,
      name: sketch.name,
      icon: sketchIcon(sketch.operation),
      suppressed: suppressedIds.has(sketch.id),
      detail: sketchDetail(sketch),
      depth: 0,
    };
  }
  const datum = getDatumStore().getDatumById(id);
  if (datum) {
    return {
      id: datum.id,
      kind: isDatumPlane(datum) ? 'datum-plane' : 'datum-axis',
      name: datum.name,
      icon: isDatumPlane(datum) ? '\u25C7' : '\u2195',
      suppressed: suppressedIds.has(datum.id),
      detail: isDatumPlane(datum) ? datumPlaneDetail(datum) : datumAxisDetail(datum as DatumAxis),
      depth: 0,
    };
  }
  const mate = getMateStore().getMateById(id);
  if (mate) {
    const compStore = getComponentStore();
    const c1 = compStore.getComponentById(mate.ref1.componentId)?.name ?? '?';
    const c2 = compStore.getComponentById(mate.ref2.componentId)?.name ?? '?';
    let detail: string;
    if (mate.type === 'distance') {
      detail = `Distance ${mate.distance}: ${c1} \u2194 ${c2}`;
    } else if (mate.type === 'angle') {
      detail = `Angle ${mate.angle}\u00B0: ${c1} \u2194 ${c2}`;
    } else {
      detail = `${mate.type}: ${c1} \u2194 ${c2}`;
    }
    return {
      id: mate.id,
      kind: 'mate' as FeatureKind,
      name: mate.name,
      icon: '\u26D3', // ⛓
      suppressed: suppressedIds.has(mate.id),
      detail,
      depth: 0,
    };
  }
  return null;
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

    /** Build display items from scene/sketch/component stores + ordering */
    get features(): FeatureItem[] {
      const scene = getSceneStore();
      const sketchStore = getSketchStore();
      const componentStore = getComponentStore();
      const items: FeatureItem[] = [];

      // Build set of IDs belonging to any component
      const componentFeatureIds = new Set<string>();
      for (const comp of componentStore.components) {
        for (const fid of comp.featureIds) {
          componentFeatureIds.add(fid);
        }
      }

      for (const id of featureOrder) {
        // Check if this is a component ID
        const comp = componentStore.getComponentById(id);
        if (comp) {
          const count = comp.featureIds.length;
          let detail = `${count} feature${count !== 1 ? 's' : ''}`;
          if (comp.grounded) detail += ' [grounded]';
          if (!comp.visible) detail += ' [hidden]';
          if (comp.sourceFile) detail += ' [imported]';

          items.push({
            id: comp.id,
            kind: 'component' as FeatureKind,
            name: comp.name,
            icon: '\u229E', // ⊞ squared plus
            suppressed: suppressedIds.has(comp.id),
            detail,
            depth: 0,
            color: comp.color,
          });

          // Append child features
          for (const fid of comp.featureIds) {
            const childItem = buildFeatureItem(fid, scene, sketchStore);
            if (childItem) {
              items.push({
                ...childItem,
                componentId: comp.id,
                depth: 1,
              });
            }
          }
          continue;
        }

        // Skip features that belong to a component (rendered as children above)
        if (componentFeatureIds.has(id)) continue;

        // Root-level feature
        const item = buildFeatureItem(id, scene, sketchStore);
        if (item) {
          items.push(item);
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

    registerComponent(id: string) {
      if (!featureOrder.includes(id)) {
        featureOrder = [...featureOrder, id];
      }
    },

    unregisterComponent(id: string) {
      featureOrder = featureOrder.filter((fid) => fid !== id);
      if (suppressedIds.has(id)) {
        const next = new Set(suppressedIds);
        next.delete(id);
        suppressedIds = next;
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
      const datumStore = getDatumStore();
      const componentStore = getComponentStore();
      const mateStore = getMateStore();

      const validIds = new Set<string>();
      for (const obj of scene.objects) validIds.add(obj.id);
      for (const sk of sketchStore.sketches) validIds.add(sk.id);
      for (const dp of datumStore.datumPlanes) validIds.add(dp.id);
      for (const da of datumStore.datumAxes) validIds.add(da.id);
      // Component IDs are also valid in featureOrder
      for (const comp of componentStore.components) validIds.add(comp.id);
      // Mate IDs
      for (const m of mateStore.mates) validIds.add(m.id);

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
      for (const dp of datumStore.datumPlanes) {
        if (!ordered.has(dp.id)) missing.push(dp.id);
      }
      for (const da of datumStore.datumAxes) {
        if (!ordered.has(da.id)) missing.push(da.id);
      }
      // Add missing component IDs
      for (const comp of componentStore.components) {
        if (!ordered.has(comp.id)) missing.push(comp.id);
      }
      // Add missing mate IDs
      for (const m of mateStore.mates) {
        if (!ordered.has(m.id)) missing.push(m.id);
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
