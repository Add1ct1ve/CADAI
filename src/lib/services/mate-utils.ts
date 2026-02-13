import type { FaceSelector } from '$lib/types/cad';
import * as THREE from 'three';

/**
 * Map a Three.js face normal to a Build123d face selector string.
 * Three.js uses Y-up; Build123d uses Z-up.
 * Coordinate mapping: Three(x, y, z) -> Build123d(x, -z, y)
 *   i.e. Three +Y = Build123d +Z, Three -Z = Build123d +Y
 */
export function normalToFaceSelector(normal: THREE.Vector3): FaceSelector {
  const abs = [Math.abs(normal.x), Math.abs(normal.y), Math.abs(normal.z)];
  const maxIdx = abs.indexOf(Math.max(...abs));
  switch (maxIdx) {
    case 0: return normal.x > 0 ? '>X' : '<X';
    case 1: return normal.y > 0 ? '>Z' : '<Z';  // Three Y = CAD Z
    case 2: return normal.z > 0 ? '<Y' : '>Y';   // Three -Z = CAD Y
    default: return '>Z';
  }
}
