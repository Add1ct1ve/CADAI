import type { ComponentId, InterferenceResult } from '$lib/types/cad';
import type { ViewportEngine } from '$lib/services/viewport-engine';
import { getComponentStore } from '$lib/stores/component.svelte';
import * as THREE from 'three';

/**
 * Quick interference check using Three.js axis-aligned bounding boxes.
 * Returns true if the AABBs of two components overlap.
 */
export function quickInterferenceCheck(
  engine: ViewportEngine,
  compAId: ComponentId,
  compBId: ComponentId,
): boolean {
  const compStore = getComponentStore();
  const compA = compStore.getComponentById(compAId);
  const compB = compStore.getComponentById(compBId);
  if (!compA || !compB) return false;

  // Compute combined AABB for each component's features
  const bboxA = computeComponentBBox(engine, compA.featureIds);
  const bboxB = computeComponentBBox(engine, compB.featureIds);

  if (!bboxA || !bboxB) return false;

  return bboxA.intersectsBox(bboxB);
}

function computeComponentBBox(engine: ViewportEngine, featureIds: string[]): THREE.Box3 | null {
  const combined = new THREE.Box3();
  let hasAny = false;

  for (const fid of featureIds) {
    const bbox = engine.getObjectBBox(fid);
    if (bbox) {
      if (!hasAny) {
        combined.copy(bbox);
        hasAny = true;
      } else {
        combined.union(bbox);
      }
    }
  }

  return hasAny ? combined : null;
}

/**
 * Run interference check between two components.
 * Uses bounding box quick-check first, then reports result.
 */
export function checkInterference(
  engine: ViewportEngine,
  compAId: ComponentId,
  compBId: ComponentId,
): InterferenceResult {
  const hasOverlap = quickInterferenceCheck(engine, compAId, compBId);
  return {
    componentA: compAId,
    componentB: compBId,
    hasInterference: hasOverlap,
  };
}
