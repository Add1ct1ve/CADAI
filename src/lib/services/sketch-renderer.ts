import * as THREE from 'three';
import type {
  Sketch,
  SketchEntity,
  SketchEntityId,
  SketchId,
  SketchToolId,
  Point2D,
} from '$lib/types/cad';
import {
  type SketchPlaneInfo,
  getSketchPlaneInfo,
  sketchToThreePos,
  threeToSketchPos,
} from '$lib/services/sketch-plane-utils';

const SKETCH_COLOR = 0xf9e2af; // yellow
const INACTIVE_SKETCH_COLOR = 0xa6935c; // dimmer yellow for inactive sketches
const SELECTED_COLOR = 0x89b4fa; // blue accent
const HOVERED_COLOR = 0x74c7ec; // teal
const PREVIEW_COLOR = 0x6c7086; // gray
const PLANE_COLOR = 0x89b4fa; // accent blue for plane overlay
const GRID_COLOR = 0x45475a; // surface2

const ARC_SEGMENTS = 64;
const CIRCLE_SEGMENTS = 64;

export class SketchRenderer {
  private scene: THREE.Scene;
  private container: THREE.Group;
  private inactiveContainer: THREE.Group; // for finished sketches
  private planeOverlay: THREE.Mesh | null = null;
  private planeGrid: THREE.Group | null = null;
  private entityMeshes: Map<SketchEntityId, THREE.Line> = new Map();
  private inactiveMeshes: Map<SketchId, THREE.Group> = new Map();
  private previewLine: THREE.Line | null = null;
  private planeInfo: SketchPlaneInfo | null = null;

  constructor(scene: THREE.Scene) {
    this.scene = scene;
    this.container = new THREE.Group();
    this.container.name = 'sketch-container';
    this.scene.add(this.container);
    this.inactiveContainer = new THREE.Group();
    this.inactiveContainer.name = 'sketch-inactive';
    this.scene.add(this.inactiveContainer);
  }

  enterSketch(sketch: Sketch): void {
    this.planeInfo = getSketchPlaneInfo(sketch.plane, sketch.origin);
    this.showPlaneOverlay();
    this.showPlaneGrid();
  }

  exitSketch(): void {
    this.hidePlaneOverlay();
    this.hidePlaneGrid();
    this.clearPreview();
    this.clearAllEntities();
    this.planeInfo = null;
  }

  getPlaneInfo(): SketchPlaneInfo | null {
    return this.planeInfo;
  }

  // ── Entity Sync ──────────────────────────────────

  syncEntities(
    sketch: Sketch,
    selectedIds: SketchEntityId[],
    hoveredId: SketchEntityId | null,
  ): void {
    if (!this.planeInfo) return;

    const currentEntityIds = new Set(sketch.entities.map((e) => e.id));
    const selectedSet = new Set(selectedIds);

    // Remove meshes for deleted entities
    for (const [id] of this.entityMeshes) {
      if (!currentEntityIds.has(id)) {
        this.removeEntityMesh(id);
      }
    }

    // Add/update entities
    for (const entity of sketch.entities) {
      const isSelected = selectedSet.has(entity.id);
      const isHovered = entity.id === hoveredId;
      const color = isSelected ? SELECTED_COLOR : isHovered ? HOVERED_COLOR : SKETCH_COLOR;

      // Always recreate for simplicity (entities are lightweight lines)
      this.removeEntityMesh(entity.id);
      const line = this.createEntityLine(entity, color);
      if (line) {
        this.entityMeshes.set(entity.id, line);
        this.container.add(line);
      }
    }
  }

  // ── Preview (rubber-band) ───────────────────────

  updatePreview(
    tool: SketchToolId,
    drawingPoints: Point2D[],
    preview: Point2D | null,
    sketch: Sketch,
  ): void {
    this.clearPreview();
    if (!this.planeInfo || !preview) return;

    let points: THREE.Vector3[] = [];

    switch (tool) {
      case 'sketch-line': {
        if (drawingPoints.length === 1) {
          points = [
            sketchToThreePos(drawingPoints[0], this.planeInfo),
            sketchToThreePos(preview, this.planeInfo),
          ];
        }
        break;
      }
      case 'sketch-rect': {
        if (drawingPoints.length === 1) {
          const c1 = drawingPoints[0];
          const c2 = preview;
          points = [
            sketchToThreePos([c1[0], c1[1]], this.planeInfo),
            sketchToThreePos([c2[0], c1[1]], this.planeInfo),
            sketchToThreePos([c2[0], c2[1]], this.planeInfo),
            sketchToThreePos([c1[0], c2[1]], this.planeInfo),
            sketchToThreePos([c1[0], c1[1]], this.planeInfo),
          ];
        }
        break;
      }
      case 'sketch-circle': {
        if (drawingPoints.length === 1) {
          const center = drawingPoints[0];
          const dx = preview[0] - center[0];
          const dy = preview[1] - center[1];
          const radius = Math.sqrt(dx * dx + dy * dy);
          points = this.circlePointsWithPlane(center, radius, this.planeInfo);
        }
        break;
      }
      case 'sketch-arc': {
        if (drawingPoints.length === 1) {
          // Show line from start to current position
          points = [
            sketchToThreePos(drawingPoints[0], this.planeInfo),
            sketchToThreePos(preview, this.planeInfo),
          ];
        } else if (drawingPoints.length === 2) {
          // Show arc preview through 3 points
          const arcPts = computeArcPoints(drawingPoints[0], preview, drawingPoints[1]);
          if (arcPts) {
            points = arcPts.map((p) => sketchToThreePos(p, this.planeInfo!));
          } else {
            // Fallback: straight line
            points = [
              sketchToThreePos(drawingPoints[0], this.planeInfo),
              sketchToThreePos(preview, this.planeInfo),
            ];
          }
        }
        break;
      }
    }

    if (points.length >= 2) {
      const geometry = new THREE.BufferGeometry().setFromPoints(points);
      const material = new THREE.LineDashedMaterial({
        color: PREVIEW_COLOR,
        dashSize: 0.3,
        gapSize: 0.15,
        linewidth: 1,
      });
      this.previewLine = new THREE.Line(geometry, material);
      this.previewLine.computeLineDistances();
      this.container.add(this.previewLine);
    }
  }

  clearPreview(): void {
    if (this.previewLine) {
      this.container.remove(this.previewLine);
      this.previewLine.geometry.dispose();
      (this.previewLine.material as THREE.Material).dispose();
      this.previewLine = null;
    }
  }

  // ── Raycast (2D distance-based hit-test) ────────

  raycastSketchEntities(
    worldPoint: THREE.Vector3,
    sketch: Sketch,
    threshold = 0.5,
  ): SketchEntityId | null {
    if (!this.planeInfo) return null;

    const sketchPt = threeToSketchPos(worldPoint, this.planeInfo);
    let closestId: SketchEntityId | null = null;
    let closestDist = threshold;

    for (const entity of sketch.entities) {
      const dist = entityDistance(sketchPt, entity);
      if (dist < closestDist) {
        closestDist = dist;
        closestId = entity.id;
      }
    }

    return closestId;
  }

  // ── Inactive Sketch Rendering ─────────────────────

  /**
   * Render all non-active sketches as static lines.
   * Called whenever sketches change or sketch mode is exited.
   */
  syncInactiveSketches(sketches: Sketch[], activeSketchId: SketchId | null): void {
    // Clear existing inactive meshes
    this.clearInactiveMeshes();

    for (const sketch of sketches) {
      // Skip the active sketch (it's rendered by syncEntities)
      if (sketch.id === activeSketchId) continue;
      if (sketch.entities.length === 0) continue;

      const pi = getSketchPlaneInfo(sketch.plane, sketch.origin);
      const group = new THREE.Group();
      group.name = `sketch-inactive-${sketch.id}`;

      for (const entity of sketch.entities) {
        const line = this.createEntityLineWithPlane(entity, INACTIVE_SKETCH_COLOR, pi);
        if (line) group.add(line);
      }

      this.inactiveContainer.add(group);
      this.inactiveMeshes.set(sketch.id, group);
    }
  }

  private clearInactiveMeshes(): void {
    for (const [, group] of this.inactiveMeshes) {
      group.traverse((child) => {
        if (child instanceof THREE.Line) {
          child.geometry.dispose();
          (child.material as THREE.Material).dispose();
        }
      });
      this.inactiveContainer.remove(group);
    }
    this.inactiveMeshes.clear();
  }

  // ── Dispose ─────────────────────────────────────

  dispose(): void {
    this.exitSketch();
    this.clearInactiveMeshes();
    this.scene.remove(this.container);
    this.scene.remove(this.inactiveContainer);
  }

  // ── Private helpers ─────────────────────────────

  private createEntityLine(entity: SketchEntity, color: number): THREE.Line | null {
    if (!this.planeInfo) return null;
    return this.createEntityLineWithPlane(entity, color, this.planeInfo);
  }

  private createEntityLineWithPlane(entity: SketchEntity, color: number, pi: SketchPlaneInfo): THREE.Line | null {
    let points: THREE.Vector3[] = [];

    switch (entity.type) {
      case 'line':
        points = [
          sketchToThreePos(entity.start, pi),
          sketchToThreePos(entity.end, pi),
        ];
        break;
      case 'rectangle': {
        const c1 = entity.corner1;
        const c2 = entity.corner2;
        points = [
          sketchToThreePos([c1[0], c1[1]], pi),
          sketchToThreePos([c2[0], c1[1]], pi),
          sketchToThreePos([c2[0], c2[1]], pi),
          sketchToThreePos([c1[0], c2[1]], pi),
          sketchToThreePos([c1[0], c1[1]], pi),
        ];
        break;
      }
      case 'circle':
        points = this.circlePointsWithPlane(entity.center, entity.radius, pi);
        break;
      case 'arc': {
        const arcPts = computeArcPoints(entity.start, entity.mid, entity.end);
        if (arcPts) {
          points = arcPts.map((p) => sketchToThreePos(p, pi));
        }
        break;
      }
    }

    if (points.length < 2) return null;

    const geometry = new THREE.BufferGeometry().setFromPoints(points);
    const material = new THREE.LineBasicMaterial({ color, linewidth: 1 });
    const line = new THREE.Line(geometry, material);
    line.userData.sketchEntityId = entity.id;
    return line;
  }

  private circlePointsWithPlane(center: Point2D, radius: number, pi: SketchPlaneInfo): THREE.Vector3[] {
    const pts: THREE.Vector3[] = [];
    for (let i = 0; i <= CIRCLE_SEGMENTS; i++) {
      const angle = (i / CIRCLE_SEGMENTS) * Math.PI * 2;
      const x = center[0] + Math.cos(angle) * radius;
      const y = center[1] + Math.sin(angle) * radius;
      pts.push(sketchToThreePos([x, y], pi));
    }
    return pts;
  }

  private removeEntityMesh(id: SketchEntityId): void {
    const mesh = this.entityMeshes.get(id);
    if (mesh) {
      this.container.remove(mesh);
      mesh.geometry.dispose();
      (mesh.material as THREE.Material).dispose();
      this.entityMeshes.delete(id);
    }
  }

  private clearAllEntities(): void {
    for (const id of this.entityMeshes.keys()) {
      this.removeEntityMesh(id);
    }
  }

  private showPlaneOverlay(): void {
    if (!this.planeInfo) return;
    this.hidePlaneOverlay();

    const size = 100;
    const geometry = new THREE.PlaneGeometry(size, size);
    const material = new THREE.MeshBasicMaterial({
      color: PLANE_COLOR,
      transparent: true,
      opacity: 0.05,
      side: THREE.DoubleSide,
      depthWrite: false,
    });
    this.planeOverlay = new THREE.Mesh(geometry, material);

    // Orient plane to match sketch plane
    const quaternion = new THREE.Quaternion();
    quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), this.planeInfo.normal);
    this.planeOverlay.quaternion.copy(quaternion);
    this.planeOverlay.position.copy(this.planeInfo.origin);
    this.planeOverlay.renderOrder = -1;

    this.container.add(this.planeOverlay);
  }

  private hidePlaneOverlay(): void {
    if (this.planeOverlay) {
      this.container.remove(this.planeOverlay);
      this.planeOverlay.geometry.dispose();
      (this.planeOverlay.material as THREE.Material).dispose();
      this.planeOverlay = null;
    }
  }

  private showPlaneGrid(): void {
    if (!this.planeInfo) return;
    this.hidePlaneGrid();

    this.planeGrid = new THREE.Group();
    const halfSize = 50;
    const step = 1;
    const material = new THREE.LineBasicMaterial({
      color: GRID_COLOR,
      transparent: true,
      opacity: 0.3,
    });

    // Generate grid lines along U and V axes
    for (let i = -halfSize; i <= halfSize; i += step) {
      // Lines along U
      const startU = sketchToThreePos([i, -halfSize], this.planeInfo);
      const endU = sketchToThreePos([i, halfSize], this.planeInfo);
      const geoU = new THREE.BufferGeometry().setFromPoints([startU, endU]);
      this.planeGrid.add(new THREE.Line(geoU, material));

      // Lines along V
      const startV = sketchToThreePos([-halfSize, i], this.planeInfo);
      const endV = sketchToThreePos([halfSize, i], this.planeInfo);
      const geoV = new THREE.BufferGeometry().setFromPoints([startV, endV]);
      this.planeGrid.add(new THREE.Line(geoV, material));
    }

    this.container.add(this.planeGrid);
  }

  private hidePlaneGrid(): void {
    if (this.planeGrid) {
      this.planeGrid.traverse((child) => {
        if (child instanceof THREE.Line) {
          child.geometry.dispose();
          (child.material as THREE.Material).dispose();
        }
      });
      this.container.remove(this.planeGrid);
      this.planeGrid = null;
    }
  }
}

// ── Geometry helpers ──────────────────────────────

/**
 * Compute a smooth arc through 3 points (start, mid, end).
 * Returns an array of 2D points along the arc.
 */
function computeArcPoints(
  start: Point2D,
  mid: Point2D,
  end: Point2D,
): Point2D[] | null {
  // Find circumcenter of the three points
  const ax = start[0], ay = start[1];
  const bx = mid[0], by = mid[1];
  const cx = end[0], cy = end[1];

  const D = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
  if (Math.abs(D) < 1e-10) return null; // Collinear

  const ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / D;
  const uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / D;

  const radius = Math.sqrt((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy));

  // Compute angles
  let angleStart = Math.atan2(ay - uy, ax - ux);
  let angleMid = Math.atan2(by - uy, bx - ux);
  let angleEnd = Math.atan2(cy - uy, cx - ux);

  // Ensure the arc goes through mid point by choosing the correct sweep direction
  // Normalize angles to [0, 2pi]
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  angleStart = normalize(angleStart);
  angleMid = normalize(angleMid);
  angleEnd = normalize(angleEnd);

  // Determine if we need to go clockwise or counter-clockwise
  let sweep = angleEnd - angleStart;
  const midSweep = angleMid - angleStart;

  // Normalize both to same range
  const normSweep = ((sweep % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const normMidSweep = ((midSweep % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);

  // If mid is not within the forward sweep, go the other way
  if (normMidSweep > normSweep) {
    sweep = normSweep - 2 * Math.PI;
  } else {
    sweep = normSweep;
  }

  const points: Point2D[] = [];
  for (let i = 0; i <= ARC_SEGMENTS; i++) {
    const t = i / ARC_SEGMENTS;
    const angle = angleStart + sweep * t;
    points.push([ux + Math.cos(angle) * radius, uy + Math.sin(angle) * radius]);
  }

  return points;
}

/**
 * Compute the minimum distance from a 2D point to a sketch entity.
 */
function entityDistance(point: Point2D, entity: SketchEntity): number {
  switch (entity.type) {
    case 'line':
      return pointToSegmentDist(point, entity.start, entity.end);
    case 'rectangle': {
      const c1 = entity.corner1;
      const c2 = entity.corner2;
      const edges: [Point2D, Point2D][] = [
        [[c1[0], c1[1]], [c2[0], c1[1]]],
        [[c2[0], c1[1]], [c2[0], c2[1]]],
        [[c2[0], c2[1]], [c1[0], c2[1]]],
        [[c1[0], c2[1]], [c1[0], c1[1]]],
      ];
      return Math.min(...edges.map(([a, b]) => pointToSegmentDist(point, a, b)));
    }
    case 'circle': {
      const dx = point[0] - entity.center[0];
      const dy = point[1] - entity.center[1];
      const distToCenter = Math.sqrt(dx * dx + dy * dy);
      return Math.abs(distToCenter - entity.radius);
    }
    case 'arc': {
      const arcPts = computeArcPoints(entity.start, entity.mid, entity.end);
      if (!arcPts || arcPts.length < 2) return Infinity;
      let minDist = Infinity;
      for (let i = 0; i < arcPts.length - 1; i++) {
        minDist = Math.min(minDist, pointToSegmentDist(point, arcPts[i], arcPts[i + 1]));
      }
      return minDist;
    }
  }
}

function pointToSegmentDist(p: Point2D, a: Point2D, b: Point2D): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const lenSq = dx * dx + dy * dy;

  if (lenSq === 0) {
    // a and b are the same point
    const ex = p[0] - a[0];
    const ey = p[1] - a[1];
    return Math.sqrt(ex * ex + ey * ey);
  }

  let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  const projX = a[0] + t * dx;
  const projY = a[1] + t * dy;
  const ex = p[0] - projX;
  const ey = p[1] - projY;
  return Math.sqrt(ex * ex + ey * ey);
}
