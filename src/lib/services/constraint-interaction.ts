import type {
  SketchToolId,
  SketchEntityId,
  SketchConstraintId,
  SketchEntity,
  SketchConstraint,
  PointRef,
  Point2D,
} from '$lib/types/cad';
import { getPointCoords } from '$lib/services/constraint-solver';

export type ConstraintAction =
  | { type: 'need-more'; message: string }
  | { type: 'need-value'; partial: Partial<SketchConstraint>; prompt: string; defaultValue: number }
  | { type: 'create'; constraint: SketchConstraint }
  | { type: 'invalid'; message: string };

export function handleConstraintSelection(
  tool: SketchToolId,
  selectedEntityIds: SketchEntityId[],
  entities: SketchEntity[],
  newId: () => SketchConstraintId,
): ConstraintAction {
  const entityMap = new Map(entities.map(e => [e.id, e]));
  const selected = selectedEntityIds.map(id => entityMap.get(id)).filter(Boolean) as SketchEntity[];

  switch (tool) {
    case 'sketch-constraint-coincident':
      return handleCoincident(selected, newId);
    case 'sketch-constraint-horizontal':
      return handleHorizontal(selected, newId);
    case 'sketch-constraint-vertical':
      return handleVertical(selected, newId);
    case 'sketch-constraint-parallel':
      return handleParallel(selected, newId);
    case 'sketch-constraint-perpendicular':
      return handlePerpendicular(selected, newId);
    case 'sketch-constraint-equal':
      return handleEqual(selected, newId);
    case 'sketch-constraint-distance':
      return handleDistance(selected, newId);
    case 'sketch-constraint-radius':
      return handleRadius(selected, newId);
    case 'sketch-constraint-angle':
      return handleAngle(selected, newId);
    default:
      return { type: 'invalid', message: 'Unknown constraint tool' };
  }
}

// ── Coincident ──

function handleCoincident(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 entities for coincident constraint' };

  const e1 = selected[0];
  const e2 = selected[1];
  const [p1, p2] = getClosestEndpoints(e1, e2);

  return {
    type: 'create',
    constraint: { type: 'coincident', id: newId(), point1: p1, point2: p2 },
  };
}

// ── Horizontal ──

function handleHorizontal(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 1) return { type: 'need-more', message: 'Select a line for horizontal constraint' };
  const e = selected[0];
  if (e.type !== 'line') return { type: 'invalid', message: 'Horizontal constraint requires a line' };
  return {
    type: 'create',
    constraint: { type: 'horizontal', id: newId(), entityId: e.id },
  };
}

// ── Vertical ──

function handleVertical(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 1) return { type: 'need-more', message: 'Select a line for vertical constraint' };
  const e = selected[0];
  if (e.type !== 'line') return { type: 'invalid', message: 'Vertical constraint requires a line' };
  return {
    type: 'create',
    constraint: { type: 'vertical', id: newId(), entityId: e.id },
  };
}

// ── Parallel ──

function handleParallel(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 lines for parallel constraint' };
  const e1 = selected[0];
  const e2 = selected[1];
  if (e1.type !== 'line' || e2.type !== 'line') {
    return { type: 'invalid', message: 'Parallel constraint requires two lines' };
  }
  return {
    type: 'create',
    constraint: { type: 'parallel', id: newId(), entityId1: e1.id, entityId2: e2.id },
  };
}

// ── Perpendicular ──

function handlePerpendicular(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 lines for perpendicular constraint' };
  const e1 = selected[0];
  const e2 = selected[1];
  if (e1.type !== 'line' || e2.type !== 'line') {
    return { type: 'invalid', message: 'Perpendicular constraint requires two lines' };
  }
  return {
    type: 'create',
    constraint: { type: 'perpendicular', id: newId(), entityId1: e1.id, entityId2: e2.id },
  };
}

// ── Equal ──

function handleEqual(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 entities for equal constraint' };
  const e1 = selected[0];
  const e2 = selected[1];
  const bothLines = e1.type === 'line' && e2.type === 'line';
  const bothCircular = (e1.type === 'circle' || e1.type === 'arc') && (e2.type === 'circle' || e2.type === 'arc');
  if (!bothLines && !bothCircular) {
    return { type: 'invalid', message: 'Equal constraint requires two lines or two circles/arcs' };
  }
  return {
    type: 'create',
    constraint: { type: 'equal', id: newId(), entityId1: e1.id, entityId2: e2.id },
  };
}

// ── Distance ──

function handleDistance(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 entities for distance constraint' };
  const e1 = selected[0];
  const e2 = selected[1];
  const [p1, p2] = getClosestEndpoints(e1, e2);
  const c1 = getPointCoords(e1, p1.pointIndex);
  const c2 = getPointCoords(e2, p2.pointIndex);
  const currentDist = Math.sqrt((c2[0] - c1[0]) ** 2 + (c2[1] - c1[1]) ** 2);
  const rounded = Math.round(currentDist * 100) / 100;

  return {
    type: 'need-value',
    partial: { type: 'distance', id: newId(), point1: p1, point2: p2 },
    prompt: 'Distance',
    defaultValue: rounded,
  };
}

// ── Radius ──

function handleRadius(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 1) return { type: 'need-more', message: 'Select a circle or arc for radius constraint' };
  const e = selected[0];
  if (e.type !== 'circle' && e.type !== 'arc') {
    return { type: 'invalid', message: 'Radius constraint requires a circle or arc' };
  }
  const currentRadius = e.type === 'circle' ? e.radius : computeArcRadius(e.start, e.mid, e.end);
  const rounded = Math.round(currentRadius * 100) / 100;

  return {
    type: 'need-value',
    partial: { type: 'radius', id: newId(), entityId: e.id },
    prompt: 'Radius',
    defaultValue: rounded,
  };
}

// ── Angle ──

function handleAngle(selected: SketchEntity[], newId: () => string): ConstraintAction {
  if (selected.length < 2) return { type: 'need-more', message: 'Select 2 lines for angle constraint' };
  const e1 = selected[0];
  const e2 = selected[1];
  if (e1.type !== 'line' || e2.type !== 'line') {
    return { type: 'invalid', message: 'Angle constraint requires two lines' };
  }
  const currentAngle = computeAngleBetweenLines(e1, e2);
  const rounded = Math.round(currentAngle * 100) / 100;

  return {
    type: 'need-value',
    partial: { type: 'angle', id: newId(), entityId1: e1.id, entityId2: e2.id },
    prompt: 'Angle (degrees)',
    defaultValue: rounded,
  };
}

// ── Helpers ──

function getClosestEndpoints(e1: SketchEntity, e2: SketchEntity): [PointRef, PointRef] {
  const points1 = getEntityEndpoints(e1);
  const points2 = getEntityEndpoints(e2);

  let bestDist = Infinity;
  let bestP1 = points1[0];
  let bestP2 = points2[0];

  for (const p1 of points1) {
    const c1 = getPointCoords(e1, p1.pointIndex);
    for (const p2 of points2) {
      const c2 = getPointCoords(e2, p2.pointIndex);
      const d = (c2[0] - c1[0]) ** 2 + (c2[1] - c1[1]) ** 2;
      if (d < bestDist) {
        bestDist = d;
        bestP1 = p1;
        bestP2 = p2;
      }
    }
  }

  return [bestP1, bestP2];
}

function getEntityEndpoints(entity: SketchEntity): PointRef[] {
  switch (entity.type) {
    case 'line':
      return [
        { entityId: entity.id, pointIndex: 0 },
        { entityId: entity.id, pointIndex: 1 },
      ];
    case 'rectangle':
      return [0, 1, 2, 3].map(i => ({ entityId: entity.id, pointIndex: i }));
    case 'circle':
      return [{ entityId: entity.id, pointIndex: 0 }];
    case 'arc':
      return [
        { entityId: entity.id, pointIndex: 0 },
        { entityId: entity.id, pointIndex: 2 },
      ];
  }
}

function computeArcRadius(start: Point2D, mid: Point2D, end: Point2D): number {
  const ax = start[0], ay = start[1];
  const bx = mid[0], by = mid[1];
  const cx = end[0], cy = end[1];
  const D = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
  if (Math.abs(D) < 1e-10) return 1;
  const ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / D;
  const uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / D;
  return Math.sqrt((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy));
}

function computeAngleBetweenLines(
  l1: { start: Point2D; end: Point2D },
  l2: { start: Point2D; end: Point2D },
): number {
  const dx1 = l1.end[0] - l1.start[0];
  const dy1 = l1.end[1] - l1.start[1];
  const dx2 = l2.end[0] - l2.start[0];
  const dy2 = l2.end[1] - l2.start[1];

  const dot = dx1 * dx2 + dy1 * dy2;
  const cross = dx1 * dy2 - dy1 * dx2;
  const angle = Math.atan2(Math.abs(cross), dot);
  return (angle * 180) / Math.PI;
}
