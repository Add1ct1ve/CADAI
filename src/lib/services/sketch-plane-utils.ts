import * as THREE from 'three';
import type { SketchPlane, Point2D } from '$lib/types/cad';

export interface SketchPlaneInfo {
  plane: THREE.Plane;
  normal: THREE.Vector3;
  u: THREE.Vector3; // sketch X axis in Three.js world
  v: THREE.Vector3; // sketch Y axis in Three.js world
  origin: THREE.Vector3;
}

/**
 * CadQuery uses Z-up, Three.js uses Y-up.
 * CadQuery (X, Y, Z) -> Three.js (X, Z, -Y)
 *
 * Sketch plane mappings (in Three.js world coords):
 * - XY (CadQuery: Z=0 plane) -> Three.js: Y=0 ground plane
 *   Normal: (0, 1, 0), U: (1, 0, 0), V: (0, 0, -1)
 * - XZ (CadQuery: Y=0 plane) -> Three.js: Z=0 plane
 *   Normal: (0, 0, -1), U: (1, 0, 0), V: (0, 1, 0)
 * - YZ (CadQuery: X=0 plane) -> Three.js: X=0 plane
 *   Normal: (-1, 0, 0), U: (0, 0, -1), V: (0, 1, 0)
 *
 * Note: We negate some normals to orient the camera "facing" the plane
 * from the positive side.
 */
export function getSketchPlaneInfo(
  plane: SketchPlane,
  origin: [number, number, number],
): SketchPlaneInfo {
  // Convert CadQuery origin to Three.js
  const threeOrigin = new THREE.Vector3(origin[0], origin[2], -origin[1]);

  let normal: THREE.Vector3;
  let u: THREE.Vector3;
  let v: THREE.Vector3;

  switch (plane) {
    case 'XY':
      // CadQuery XY plane (Z=0) -> Three.js ground plane (Y=0)
      normal = new THREE.Vector3(0, 1, 0);
      u = new THREE.Vector3(1, 0, 0);
      v = new THREE.Vector3(0, 0, -1);
      break;
    case 'XZ':
      // CadQuery XZ plane (Y=0) -> Three.js Z=0 plane
      normal = new THREE.Vector3(0, 0, 1);
      u = new THREE.Vector3(1, 0, 0);
      v = new THREE.Vector3(0, 1, 0);
      break;
    case 'YZ':
      // CadQuery YZ plane (X=0) -> Three.js X=0 plane
      normal = new THREE.Vector3(1, 0, 0);
      u = new THREE.Vector3(0, 0, -1);
      v = new THREE.Vector3(0, 1, 0);
      break;
  }

  const threePlane = new THREE.Plane();
  threePlane.setFromNormalAndCoplanarPoint(normal, threeOrigin);

  return { plane: threePlane, normal, u, v, origin: threeOrigin };
}

/**
 * Convert a 2D sketch coordinate to a 3D Three.js world position.
 */
export function sketchToThreePos(
  point: Point2D,
  planeInfo: SketchPlaneInfo,
): THREE.Vector3 {
  const [su, sv] = point;
  return new THREE.Vector3()
    .copy(planeInfo.origin)
    .addScaledVector(planeInfo.u, su)
    .addScaledVector(planeInfo.v, sv);
}

/**
 * Convert a 3D Three.js world position to 2D sketch coordinates.
 * Projects the point onto the sketch plane.
 */
export function threeToSketchPos(
  worldPoint: THREE.Vector3,
  planeInfo: SketchPlaneInfo,
): Point2D {
  const delta = new THREE.Vector3().subVectors(worldPoint, planeInfo.origin);
  const su = delta.dot(planeInfo.u);
  const sv = delta.dot(planeInfo.v);
  return [su, sv];
}

/**
 * Snap a 2D sketch point to the grid.
 */
export function snapToSketchGrid(point: Point2D, gridSize: number): Point2D {
  return [
    Math.round(point[0] / gridSize) * gridSize,
    Math.round(point[1] / gridSize) * gridSize,
  ];
}

/**
 * Get camera position and target for viewing a sketch plane.
 */
export function getSketchViewCamera(
  planeInfo: SketchPlaneInfo,
  distance = 30,
): { position: THREE.Vector3; target: THREE.Vector3 } {
  const position = new THREE.Vector3()
    .copy(planeInfo.origin)
    .addScaledVector(planeInfo.normal, distance);
  return { position, target: planeInfo.origin.clone() };
}
