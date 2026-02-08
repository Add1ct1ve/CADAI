import * as THREE from 'three';

/**
 * CadQuery uses Z-up coordinate system, Three.js uses Y-up.
 * CadQuery (X, Y, Z) -> Three.js (X, Z, -Y)
 */

export function cadToThreePos(cadPos: [number, number, number]): THREE.Vector3 {
  const [x, y, z] = cadPos;
  return new THREE.Vector3(x, z, -y);
}

export function threeToCadPos(vec: THREE.Vector3): [number, number, number] {
  return [vec.x, -vec.z, vec.y];
}

/**
 * Convert CadQuery rotation (degrees around X, Y, Z in Z-up)
 * to Three.js Euler (Y-up).
 * Remaps: cadX -> threeX, cadY -> -threeZ, cadZ -> threeY
 */
export function cadToThreeRot(cadRot: [number, number, number]): THREE.Euler {
  const [rx, ry, rz] = cadRot;
  const deg2rad = Math.PI / 180;
  return new THREE.Euler(
    rx * deg2rad,   // X stays X
    rz * deg2rad,   // CadQuery Z-rot -> Three.js Y-rot
    -ry * deg2rad,  // CadQuery Y-rot -> Three.js -Z-rot
    'XYZ',
  );
}

export function threeToCadRot(euler: THREE.Euler): [number, number, number] {
  const rad2deg = 180 / Math.PI;
  return [
    euler.x * rad2deg,
    -euler.z * rad2deg,
    euler.y * rad2deg,
  ];
}

/**
 * Intersect a screen-space mouse event with the XZ ground plane (Y=0 in Three.js = Z=0 in CadQuery).
 * Returns the CadQuery position [x, y, z] or null if no intersection.
 */
export function raycastGroundPlane(
  event: PointerEvent,
  container: HTMLElement,
  camera: THREE.Camera,
): [number, number, number] | null {
  const rect = container.getBoundingClientRect();
  const ndcX = ((event.clientX - rect.left) / rect.width) * 2 - 1;
  const ndcY = -((event.clientY - rect.top) / rect.height) * 2 + 1;

  const raycaster = new THREE.Raycaster();
  raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), camera);

  const groundPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  const intersection = new THREE.Vector3();
  const hit = raycaster.ray.intersectPlane(groundPlane, intersection);

  if (!hit) return null;

  // Snap to grid (1 unit)
  const snapped = new THREE.Vector3(
    Math.round(intersection.x),
    Math.round(intersection.y),
    Math.round(intersection.z),
  );

  return threeToCadPos(snapped);
}
