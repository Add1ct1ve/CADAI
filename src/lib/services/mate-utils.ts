import type { FaceSelector } from '$lib/types/cad';
import * as THREE from 'three';

/**
 * Map a Three.js face normal to a CadQuery face selector string.
 * Three.js uses Y-up; CadQuery uses Z-up.
 * Coordinate mapping: Three(x, y, z) -> CadQuery(x, -z, y)
 *   i.e. Three +Y = CadQuery +Z, Three -Z = CadQuery +Y
 */
export function normalToFaceSelector(normal: THREE.Vector3): FaceSelector {
  const abs = [Math.abs(normal.x), Math.abs(normal.y), Math.abs(normal.z)];
  const maxIdx = abs.indexOf(Math.max(...abs));
  switch (maxIdx) {
    case 0: return normal.x > 0 ? '>X' : '<X';
    case 1: return normal.y > 0 ? '>Z' : '<Z';  // Three Y = CadQuery Z
    case 2: return normal.z > 0 ? '<Y' : '>Y';   // Three -Z = CadQuery Y
    default: return '>Z';
  }
}
