import * as THREE from 'three';
import type {
  Sketch,
  SketchEntity,
  SketchEntityId,
  SketchId,
  SketchToolId,
  SketchConstraint,
  ConstraintState,
  Point2D,
} from '$lib/types/cad';
import {
  type SketchPlaneInfo,
  getSketchPlaneInfo,
  sketchToThreePos,
  threeToSketchPos,
} from '$lib/services/sketch-plane-utils';
import { getPointCoords } from '$lib/services/constraint-solver';
import { findEntityIntersections, reflectPoint } from '$lib/services/sketch-operations';

const SKETCH_COLOR = 0xf9e2af; // yellow
const INACTIVE_SKETCH_COLOR = 0xa6935c; // dimmer yellow for inactive sketches
const SELECTED_COLOR = 0x89b4fa; // blue accent
const HOVERED_COLOR = 0x74c7ec; // teal
const PREVIEW_COLOR = 0x6c7086; // gray
const TRIM_HIGHLIGHT_COLOR = 0xf38ba8; // red for trim preview
const OP_PREVIEW_COLOR = 0x94e2d5; // teal for operation preview
const PLANE_COLOR = 0x89b4fa; // accent blue for plane overlay
const GRID_COLOR = 0x45475a; // surface2
const WELL_CONSTRAINED_COLOR = 0xa6e3a1; // green
const OVER_CONSTRAINED_COLOR = 0xf38ba8; // red
const CONSTRAINT_ICON_COLOR = '#cba6f7'; // mauve for constraint icons
const DIMENSION_TEXT_COLOR = '#94e2d5'; // teal for dimension text

const ARC_SEGMENTS = 64;
const CIRCLE_SEGMENTS = 64;
const SKETCH_MIN_SPAN = 200;
const SKETCH_TARGET_GRID_LINES = 90;
const SKETCH_CENTER_SNAP_MULTIPLIER = 10;

export class SketchRenderer {
  private scene: THREE.Scene;
  private container: THREE.Group;
  private inactiveContainer: THREE.Group; // for finished sketches
  private planeOverlay: THREE.Mesh | null = null;
  private planeGrid: THREE.Group | null = null;
  private entityMeshes: Map<SketchEntityId, THREE.Line> = new Map();
  private inactiveMeshes: Map<SketchId, THREE.Group> = new Map();
  private constraintGroup: THREE.Group;
  private previewLine: THREE.Line | null = null;
  private opPreviewLines: THREE.Line[] = [];
  private planeInfo: SketchPlaneInfo | null = null;
  private planeVisualCenter: Point2D | null = null;
  private planeVisualSpan = 0;
  private planeVisualStep = 1;

  constructor(scene: THREE.Scene) {
    this.scene = scene;
    this.container = new THREE.Group();
    this.container.name = 'sketch-container';
    this.scene.add(this.container);
    this.inactiveContainer = new THREE.Group();
    this.inactiveContainer.name = 'sketch-inactive';
    this.scene.add(this.inactiveContainer);
    this.constraintGroup = new THREE.Group();
    this.constraintGroup.name = 'sketch-constraints';
    this.container.add(this.constraintGroup);
  }

  enterSketch(sketch: Sketch): void {
    this.planeInfo = getSketchPlaneInfo(sketch.plane, sketch.origin);
    this.planeVisualCenter = null;
    this.planeVisualSpan = 0;
    this.planeVisualStep = 1;
    this.rebuildPlaneVisuals([0, 0], SKETCH_MIN_SPAN, 2);
  }

  exitSketch(): void {
    this.hidePlaneOverlay();
    this.hidePlaneGrid();
    this.clearPreview();
    this.clearAllEntities();
    this.clearConstraintVisuals();
    this.planeInfo = null;
    this.planeVisualCenter = null;
    this.planeVisualSpan = 0;
    this.planeVisualStep = 1;
  }

  getPlaneInfo(): SketchPlaneInfo | null {
    return this.planeInfo;
  }

  updatePlaneVisuals(camera: THREE.PerspectiveCamera, cameraTarget: THREE.Vector3): void {
    if (!this.planeInfo) return;

    const projectedTarget = new THREE.Vector3();
    this.planeInfo.plane.projectPoint(cameraTarget, projectedTarget);
    const projectedSketch = threeToSketchPos(projectedTarget, this.planeInfo);

    const cameraDistanceToPlane = Math.max(Math.abs(this.planeInfo.plane.distanceToPoint(camera.position)), 1);
    const targetSpan = Math.max(SKETCH_MIN_SPAN, cameraDistanceToPlane * 6);
    const step = this.computeAdaptiveGridStep(targetSpan);

    const snappedCenter: Point2D = [
      Math.round(projectedSketch[0] / (step * SKETCH_CENTER_SNAP_MULTIPLIER)) * step * SKETCH_CENTER_SNAP_MULTIPLIER,
      Math.round(projectedSketch[1] / (step * SKETCH_CENTER_SNAP_MULTIPLIER)) * step * SKETCH_CENTER_SNAP_MULTIPLIER,
    ];
    const normalizedSpan = Math.ceil(targetSpan / (step * 2)) * step * 2;

    const centerChanged = !this.planeVisualCenter
      || this.planeVisualCenter[0] !== snappedCenter[0]
      || this.planeVisualCenter[1] !== snappedCenter[1];
    const spanChanged = Math.abs(this.planeVisualSpan - normalizedSpan) > 1e-6;
    const stepChanged = Math.abs(this.planeVisualStep - step) > 1e-6;

    if (centerChanged || spanChanged || stepChanged || !this.planeOverlay || !this.planeGrid) {
      this.rebuildPlaneVisuals(snappedCenter, normalizedSpan, step);
    }
  }

  // ── Entity Sync ──────────────────────────────────

  syncEntities(
    sketch: Sketch,
    selectedIds: SketchEntityId[],
    hoveredId: SketchEntityId | null,
    cState: ConstraintState = 'under-constrained',
  ): void {
    if (!this.planeInfo) return;

    const currentEntityIds = new Set(sketch.entities.map((e) => e.id));
    const selectedSet = new Set(selectedIds);

    // Choose default color based on constraint state
    const defaultColor = cState === 'well-constrained' ? WELL_CONSTRAINED_COLOR
      : cState === 'over-constrained' ? OVER_CONSTRAINED_COLOR
      : SKETCH_COLOR;

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
      const color = isSelected ? SELECTED_COLOR : isHovered ? HOVERED_COLOR : defaultColor;

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

      case 'sketch-spline': {
        if (drawingPoints.length >= 1) {
          const allPts = [...drawingPoints, preview].map(p => sketchToThreePos(p, this.planeInfo!));
          if (allPts.length >= 2) {
            const curve = new THREE.CatmullRomCurve3(allPts, false, 'catmullrom', 0.5);
            points = curve.getPoints(Math.max(allPts.length * 16, 50));
          }
        }
        break;
      }
      case 'sketch-bezier': {
        if (drawingPoints.length >= 1) {
          const allPts = [...drawingPoints, preview].map(p => sketchToThreePos(p, this.planeInfo!));
          if (allPts.length === 3) {
            const curve = new THREE.QuadraticBezierCurve3(allPts[0], allPts[1], allPts[2]);
            points = curve.getPoints(50);
          } else if (allPts.length === 4) {
            const curve = new THREE.CubicBezierCurve3(allPts[0], allPts[1], allPts[2], allPts[3]);
            points = curve.getPoints(50);
          } else if (allPts.length >= 2) {
            const curve = new THREE.CatmullRomCurve3(allPts, false);
            points = curve.getPoints(50);
          }
        }
        break;
      }

      // ── Operation previews ──
      case 'sketch-op-trim': {
        // Highlight the hovered entity in red to show which segment would be removed
        const hoveredEntity = sketch.entities.find(e => {
          const d = entityDistance(preview, e);
          return d < 0.5;
        });
        if (hoveredEntity) {
          const entityLine = this.createEntityLineWithPlane(hoveredEntity, TRIM_HIGHLIGHT_COLOR, this.planeInfo);
          if (entityLine) {
            this.opPreviewLines.push(entityLine);
            this.container.add(entityLine);
          }
        }
        break;
      }
      case 'sketch-op-mirror': {
        // Show mirror preview of selected entities when hovering over the mirror line
        // (We'd need selectedEntityIds here; skip for now - mirror preview is complex)
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
    for (const line of this.opPreviewLines) {
      this.container.remove(line);
      line.geometry.dispose();
      (line.material as THREE.Material).dispose();
    }
    this.opPreviewLines = [];
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

  // ── Inactive Sketch Selection ────────────────────

  getInactiveMeshes(): Map<SketchId, THREE.Group> {
    return this.inactiveMeshes;
  }

  highlightInactiveSketch(selectedId: SketchId | null): void {
    for (const [id, group] of this.inactiveMeshes) {
      const color = id === selectedId ? SELECTED_COLOR : INACTIVE_SKETCH_COLOR;
      group.traverse((child) => {
        if (child instanceof THREE.Line && child.material instanceof THREE.LineBasicMaterial) {
          child.material.color.setHex(color);
        }
      });
    }
  }

  // ── Constraint Rendering ────────────────────────

  syncConstraints(sketch: Sketch, cState: ConstraintState): void {
    this.clearConstraintVisuals();
    if (!this.planeInfo) return;

    const entityMap = new Map(sketch.entities.map(e => [e.id, e]));

    for (const constraint of (sketch.constraints ?? [])) {
      const sprite = this.createConstraintSprite(constraint, entityMap);
      if (sprite) {
        this.constraintGroup.add(sprite);
      }
    }
  }

  private clearConstraintVisuals(): void {
    while (this.constraintGroup.children.length > 0) {
      const child = this.constraintGroup.children[0];
      this.constraintGroup.remove(child);
      if (child instanceof THREE.Sprite) {
        child.material.map?.dispose();
        child.material.dispose();
      }
      if (child instanceof THREE.Line) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
    }
  }

  private createConstraintSprite(
    constraint: SketchConstraint,
    entityMap: Map<string, SketchEntity>,
  ): THREE.Object3D | null {
    if (!this.planeInfo) return null;

    switch (constraint.type) {
      case 'coincident': {
        const e1 = entityMap.get(constraint.point1.entityId);
        if (!e1) return null;
        const pt = getPointCoords(e1, constraint.point1.pointIndex);
        const pos = sketchToThreePos(pt, this.planeInfo);
        return this.makeIconSprite('\u25CF', pos, CONSTRAINT_ICON_COLOR, 0.3);
      }
      case 'horizontal': {
        const entity = entityMap.get(constraint.entityId);
        if (!entity || entity.type !== 'line') return null;
        const mid = midpoint(entity.start, entity.end);
        const offset: Point2D = [mid[0], mid[1] + 0.6];
        const pos = sketchToThreePos(offset, this.planeInfo);
        return this.makeIconSprite('H', pos, CONSTRAINT_ICON_COLOR);
      }
      case 'vertical': {
        const entity = entityMap.get(constraint.entityId);
        if (!entity || entity.type !== 'line') return null;
        const mid = midpoint(entity.start, entity.end);
        const offset: Point2D = [mid[0] + 0.6, mid[1]];
        const pos = sketchToThreePos(offset, this.planeInfo);
        return this.makeIconSprite('V', pos, CONSTRAINT_ICON_COLOR);
      }
      case 'parallel': {
        const e1 = entityMap.get(constraint.entityId1);
        const e2 = entityMap.get(constraint.entityId2);
        if (!e1 || !e2) return null;
        const mid1 = getEntityMidpoint(e1);
        const mid2 = getEntityMidpoint(e2);
        const between: Point2D = [(mid1[0] + mid2[0]) / 2, (mid1[1] + mid2[1]) / 2];
        const pos = sketchToThreePos(between, this.planeInfo);
        return this.makeIconSprite('//', pos, CONSTRAINT_ICON_COLOR);
      }
      case 'perpendicular': {
        const e1 = entityMap.get(constraint.entityId1);
        const e2 = entityMap.get(constraint.entityId2);
        if (!e1 || !e2) return null;
        const mid1 = getEntityMidpoint(e1);
        const mid2 = getEntityMidpoint(e2);
        const between: Point2D = [(mid1[0] + mid2[0]) / 2, (mid1[1] + mid2[1]) / 2];
        const pos = sketchToThreePos(between, this.planeInfo);
        return this.makeIconSprite('\u22A5', pos, CONSTRAINT_ICON_COLOR);
      }
      case 'equal': {
        const e1 = entityMap.get(constraint.entityId1);
        const e2 = entityMap.get(constraint.entityId2);
        if (!e1 || !e2) return null;
        const group = new THREE.Group();
        const mid1 = getEntityMidpoint(e1);
        const mid2 = getEntityMidpoint(e2);
        const s1 = this.makeIconSprite('=', sketchToThreePos([mid1[0], mid1[1] + 0.5], this.planeInfo), CONSTRAINT_ICON_COLOR);
        const s2 = this.makeIconSprite('=', sketchToThreePos([mid2[0], mid2[1] + 0.5], this.planeInfo), CONSTRAINT_ICON_COLOR);
        if (s1) group.add(s1);
        if (s2) group.add(s2);
        return group;
      }
      case 'distance': {
        const e1 = entityMap.get(constraint.point1.entityId);
        const e2 = entityMap.get(constraint.point2.entityId);
        if (!e1 || !e2) return null;
        const c1 = getPointCoords(e1, constraint.point1.pointIndex);
        const c2 = getPointCoords(e2, constraint.point2.pointIndex);
        const mid: Point2D = [(c1[0] + c2[0]) / 2, (c1[1] + c2[1]) / 2 + 0.5];
        const pos = sketchToThreePos(mid, this.planeInfo);
        const text = constraint.value.toFixed(1);
        const group = new THREE.Group();
        // Dashed leader line
        const pts = [sketchToThreePos(c1, this.planeInfo), sketchToThreePos(c2, this.planeInfo)];
        const geo = new THREE.BufferGeometry().setFromPoints(pts);
        const mat = new THREE.LineDashedMaterial({ color: 0x94e2d5, dashSize: 0.2, gapSize: 0.1 });
        const line = new THREE.Line(geo, mat);
        line.computeLineDistances();
        group.add(line);
        const sprite = this.makeIconSprite(text, pos, DIMENSION_TEXT_COLOR, 0.6);
        if (sprite) group.add(sprite);
        return group;
      }
      case 'radius': {
        const entity = entityMap.get(constraint.entityId);
        if (!entity) return null;
        const center = entity.type === 'circle' ? entity.center
          : entity.type === 'arc' ? entity.start : null;
        if (!center) return null;
        const offset: Point2D = [center[0] + 0.5, center[1] + 0.5];
        const pos = sketchToThreePos(offset, this.planeInfo);
        return this.makeIconSprite(`R=${constraint.value.toFixed(1)}`, pos, DIMENSION_TEXT_COLOR, 0.6);
      }
      case 'angle': {
        const e1 = entityMap.get(constraint.entityId1);
        const e2 = entityMap.get(constraint.entityId2);
        if (!e1 || !e2) return null;
        const mid1 = getEntityMidpoint(e1);
        const mid2 = getEntityMidpoint(e2);
        const between: Point2D = [(mid1[0] + mid2[0]) / 2, (mid1[1] + mid2[1]) / 2 + 0.5];
        const pos = sketchToThreePos(between, this.planeInfo);
        return this.makeIconSprite(`${constraint.value.toFixed(1)}\u00B0`, pos, DIMENSION_TEXT_COLOR, 0.6);
      }
      default:
        return null;
    }
  }

  private makeIconSprite(
    text: string,
    position: THREE.Vector3,
    color: string,
    scale = 0.5,
  ): THREE.Sprite {
    const canvas = document.createElement('canvas');
    const size = 128;
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext('2d')!;

    // Background
    ctx.fillStyle = 'rgba(30, 30, 46, 0.85)';
    const pad = 8;
    const textWidth = ctx.measureText(text).width;
    ctx.beginPath();
    ctx.roundRect(pad, pad, size - pad * 2, size - pad * 2, 8);
    ctx.fill();

    // Text
    ctx.font = 'bold 48px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = color;
    ctx.fillText(text, size / 2, size / 2);

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;
    const material = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthWrite: false,
      depthTest: false,
    });
    const sprite = new THREE.Sprite(material);
    sprite.position.copy(position);
    sprite.scale.set(scale, scale, 1);
    sprite.renderOrder = 10;
    return sprite;
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
      case 'spline': {
        if (entity.points.length >= 2) {
          const curvePoints = entity.points.map(p => sketchToThreePos([p[0], p[1]], pi));
          const curve = new THREE.CatmullRomCurve3(curvePoints, false, 'catmullrom', 0.5);
          points = curve.getPoints(Math.max(entity.points.length * 16, 50));
        }
        break;
      }
      case 'bezier': {
        if (entity.points.length >= 2) {
          const pts = entity.points.map(p => sketchToThreePos([p[0], p[1]], pi));
          if (pts.length === 3) {
            const curve = new THREE.QuadraticBezierCurve3(pts[0], pts[1], pts[2]);
            points = curve.getPoints(50);
          } else if (pts.length === 4) {
            const curve = new THREE.CubicBezierCurve3(pts[0], pts[1], pts[2], pts[3]);
            points = curve.getPoints(50);
          } else {
            // Fallback: use CatmullRom for arbitrary number of points
            const curve = new THREE.CatmullRomCurve3(pts, false);
            points = curve.getPoints(50);
          }
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

  private rebuildPlaneVisuals(center: Point2D, span: number, step: number): void {
    this.showPlaneOverlay(center, span);
    this.showPlaneGrid(center, span, step);
    this.planeVisualCenter = center;
    this.planeVisualSpan = span;
    this.planeVisualStep = step;
  }

  private computeAdaptiveGridStep(span: number): number {
    const rawStep = Math.max(span / SKETCH_TARGET_GRID_LINES, 0.1);
    const magnitude = Math.pow(10, Math.floor(Math.log10(rawStep)));
    const normalized = rawStep / magnitude;

    const stepMultiplier = normalized <= 1
      ? 1
      : normalized <= 2
        ? 2
        : normalized <= 5
          ? 5
          : 10;
    return stepMultiplier * magnitude;
  }

  private showPlaneOverlay(center: Point2D, span: number): void {
    if (!this.planeInfo) return;
    this.hidePlaneOverlay();

    const geometry = new THREE.PlaneGeometry(span, span);
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
    this.planeOverlay.position.copy(sketchToThreePos(center, this.planeInfo));
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

  private showPlaneGrid(center: Point2D, span: number, step: number): void {
    if (!this.planeInfo) return;
    this.hidePlaneGrid();

    this.planeGrid = new THREE.Group();
    const halfSpan = span / 2;
    const minU = center[0] - halfSpan;
    const maxU = center[0] + halfSpan;
    const minV = center[1] - halfSpan;
    const maxV = center[1] + halfSpan;
    const material = new THREE.LineBasicMaterial({
      color: GRID_COLOR,
      transparent: true,
      opacity: 0.3,
    });

    // Generate grid lines along U axis
    for (let u = minU; u <= maxU + step * 0.5; u += step) {
      const startU = sketchToThreePos([u, minV], this.planeInfo);
      const endU = sketchToThreePos([u, maxV], this.planeInfo);
      const geoU = new THREE.BufferGeometry().setFromPoints([startU, endU]);
      this.planeGrid.add(new THREE.Line(geoU, material));
    }

    // Generate grid lines along V axis
    for (let v = minV; v <= maxV + step * 0.5; v += step) {
      const startV = sketchToThreePos([minU, v], this.planeInfo);
      const endV = sketchToThreePos([maxU, v], this.planeInfo);
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
    case 'spline':
    case 'bezier': {
      if (entity.points.length < 2) return Infinity;
      let minDist = Infinity;
      for (let i = 0; i < entity.points.length - 1; i++) {
        minDist = Math.min(minDist, pointToSegmentDist(point, entity.points[i], entity.points[i + 1]));
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

function midpoint(a: Point2D, b: Point2D): Point2D {
  return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
}

function getEntityMidpoint(entity: SketchEntity): Point2D {
  switch (entity.type) {
    case 'line':
      return midpoint(entity.start, entity.end);
    case 'rectangle':
      return midpoint(entity.corner1, entity.corner2);
    case 'circle':
      return entity.center;
    case 'arc':
      return entity.mid;
    case 'spline':
    case 'bezier': {
      if (entity.points.length === 0) return [0, 0];
      const midIdx = Math.floor(entity.points.length / 2);
      return entity.points[midIdx];
    }
  }
}
