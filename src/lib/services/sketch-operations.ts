import type {
  SketchToolId,
  SketchEntityId,
  SketchEntity,
  SketchLine,
  SketchCircle,
  SketchArc,
  SketchRectangle,
  Point2D,
} from '$lib/types/cad';

// ── Action types ─────────────────────────────────

export interface PendingSketchOp {
  opType: string;
  entityIds: string[];
  clickPoint?: Point2D;
}

export type SketchOpAction =
  | { type: 'replace'; removeIds: string[]; addEntities: SketchEntity[] }
  | { type: 'need-value'; prompt: string; defaultValue: number; pendingOp: PendingSketchOp }
  | { type: 'need-more'; message: string }
  | { type: 'invalid'; message: string };

// ── Main dispatch ────────────────────────────────

export function handleSketchOp(
  tool: SketchToolId,
  selectedEntityIds: SketchEntityId[],
  entities: SketchEntity[],
  newId: () => SketchEntityId,
  value?: number,
  clickPoint?: Point2D,
): SketchOpAction {
  const entityMap = new Map(entities.map(e => [e.id, e]));
  const selected = selectedEntityIds
    .map(id => entityMap.get(id))
    .filter(Boolean) as SketchEntity[];

  switch (tool) {
    case 'sketch-op-trim':
      return handleTrim(clickPoint ?? null, selectedEntityIds[0] ?? null, entities, newId);
    case 'sketch-op-extend':
      return handleExtend(selected, entities, newId);
    case 'sketch-op-offset':
      return handleOffset(selected, newId, value);
    case 'sketch-op-mirror':
      return handleMirror(selected, newId);
    case 'sketch-op-fillet':
      return handleSketchFillet(selected, entities, newId, value);
    case 'sketch-op-chamfer':
      return handleSketchChamfer(selected, entities, newId, value);
    default:
      return { type: 'invalid', message: 'Unknown sketch operation' };
  }
}

// ── Trim ─────────────────────────────────────────

export function handleTrim(
  clickPoint: Point2D | null,
  clickedEntityId: SketchEntityId | null,
  entities: SketchEntity[],
  newId: () => SketchEntityId,
): SketchOpAction {
  if (!clickedEntityId || !clickPoint) {
    return { type: 'need-more', message: 'Click on a segment to trim' };
  }

  const entity = entities.find(e => e.id === clickedEntityId);
  if (!entity) return { type: 'invalid', message: 'Entity not found' };

  const otherEntities = entities.filter(e => e.id !== clickedEntityId);

  // Decompose rectangle into 4 lines if needed
  if (entity.type === 'rectangle') {
    const lines = decomposeRectangle(entity, newId);
    // Find which edge the click is closest to
    let closestLine = lines[0];
    let closestDist = Infinity;
    for (const line of lines) {
      const d = pointToSegmentDist(clickPoint, line.start, line.end);
      if (d < closestDist) {
        closestDist = d;
        closestLine = line;
      }
    }
    // Find intersections for the clicked edge
    const intersections = findEntityIntersections(closestLine, otherEntities);
    if (intersections.length === 0) {
      // No intersections: just decompose and remove clicked edge
      const remaining = lines.filter(l => l !== closestLine);
      return { type: 'replace', removeIds: [entity.id], addEntities: remaining };
    }
    // Split the clicked edge at intersections, remove the clicked segment
    const result = trimLineAtIntersections(closestLine, intersections, clickPoint, newId);
    const remaining = lines.filter(l => l !== closestLine);
    return { type: 'replace', removeIds: [entity.id], addEntities: [...remaining, ...result] };
  }

  if (entity.type === 'line') {
    const intersections = findEntityIntersections(entity, otherEntities);
    if (intersections.length === 0) {
      return { type: 'invalid', message: 'No intersections found to trim at' };
    }
    const result = trimLineAtIntersections(entity, intersections, clickPoint, newId);
    return { type: 'replace', removeIds: [entity.id], addEntities: result };
  }

  if (entity.type === 'circle') {
    const intersections = findEntityIntersections(entity, otherEntities);
    if (intersections.length < 2) {
      return { type: 'invalid', message: 'Need at least 2 intersections to trim a circle' };
    }
    const result = trimCircleAtIntersections(entity, intersections, clickPoint, newId);
    return { type: 'replace', removeIds: [entity.id], addEntities: result };
  }

  if (entity.type === 'arc') {
    const intersections = findEntityIntersections(entity, otherEntities);
    if (intersections.length === 0) {
      return { type: 'invalid', message: 'No intersections found to trim at' };
    }
    const result = trimArcAtIntersections(entity, intersections, clickPoint, newId);
    return { type: 'replace', removeIds: [entity.id], addEntities: result };
  }

  return { type: 'invalid', message: 'Cannot trim this entity type' };
}

// ── Extend ───────────────────────────────────────

function handleExtend(
  selected: SketchEntity[],
  allEntities: SketchEntity[],
  newId: () => SketchEntityId,
): SketchOpAction {
  if (selected.length < 1) return { type: 'need-more', message: 'Click a line to extend' };

  const entity = selected[0];
  if (entity.type !== 'line') {
    return { type: 'invalid', message: 'Extend only works on lines' };
  }

  const others = allEntities.filter(e => e.id !== entity.id);
  if (others.length === 0) {
    return { type: 'invalid', message: 'No other entities to extend to' };
  }

  // Try extending from both ends and pick the closer intersection
  const dx = entity.end[0] - entity.start[0];
  const dy = entity.end[1] - entity.start[1];

  let bestResult: { point: Point2D; fromStart: boolean; dist: number } | null = null;

  // Extend from start (backwards)
  const extStart: Point2D = [entity.start[0] - dx * 100, entity.start[1] - dy * 100];
  const extLineStart: SketchLine = { type: 'line', id: 'tmp', start: extStart, end: entity.start };
  const intStart = findEntityIntersections(extLineStart, others);
  for (const int of intStart) {
    const d = dist2D(int.point, entity.start);
    if (d > 0.01 && (!bestResult || d < bestResult.dist)) {
      bestResult = { point: int.point, fromStart: true, dist: d };
    }
  }

  // Extend from end (forwards)
  const extEnd: Point2D = [entity.end[0] + dx * 100, entity.end[1] + dy * 100];
  const extLineEnd: SketchLine = { type: 'line', id: 'tmp', start: entity.end, end: extEnd };
  const intEnd = findEntityIntersections(extLineEnd, others);
  for (const int of intEnd) {
    const d = dist2D(int.point, entity.end);
    if (d > 0.01 && (!bestResult || d < bestResult.dist)) {
      bestResult = { point: int.point, fromStart: false, dist: d };
    }
  }

  if (!bestResult) {
    return { type: 'invalid', message: 'No intersection found to extend to' };
  }

  const newLine: SketchLine = {
    type: 'line',
    id: newId(),
    start: bestResult.fromStart ? bestResult.point : entity.start,
    end: bestResult.fromStart ? entity.end : bestResult.point,
  };

  return { type: 'replace', removeIds: [entity.id], addEntities: [newLine] };
}

// ── Offset ───────────────────────────────────────

function handleOffset(
  selected: SketchEntity[],
  newId: () => SketchEntityId,
  distance?: number,
): SketchOpAction {
  if (selected.length < 1) return { type: 'need-more', message: 'Select an entity to offset' };

  const entity = selected[0];

  if (distance === undefined) {
    return {
      type: 'need-value',
      prompt: 'Offset distance',
      defaultValue: 1,
      pendingOp: { opType: 'sketch-op-offset', entityIds: [entity.id] },
    };
  }

  if (distance === 0) return { type: 'invalid', message: 'Offset distance must be non-zero' };

  const result = offsetEntity(entity, distance, newId);
  if (!result) return { type: 'invalid', message: 'Cannot offset this entity type' };

  return { type: 'replace', removeIds: [], addEntities: [result] };
}

function offsetEntity(entity: SketchEntity, distance: number, newId: () => string): SketchEntity | null {
  switch (entity.type) {
    case 'line': {
      const dx = entity.end[0] - entity.start[0];
      const dy = entity.end[1] - entity.start[1];
      const len = Math.sqrt(dx * dx + dy * dy);
      if (len < 1e-10) return null;
      // Normal perpendicular to line
      const nx = -dy / len * distance;
      const ny = dx / len * distance;
      return {
        type: 'line',
        id: newId(),
        start: [entity.start[0] + nx, entity.start[1] + ny],
        end: [entity.end[0] + nx, entity.end[1] + ny],
      };
    }
    case 'circle': {
      const newRadius = entity.radius + distance;
      if (newRadius <= 0) return null;
      return {
        type: 'circle',
        id: newId(),
        center: [...entity.center] as Point2D,
        radius: newRadius,
      };
    }
    case 'arc': {
      // Compute arc center and radius
      const arc = computeArcCenterRadius(entity.start, entity.mid, entity.end);
      if (!arc) return null;
      const newRadius = arc.radius + distance;
      if (newRadius <= 0) return null;
      // Scale points from center
      const scale = newRadius / arc.radius;
      const offsetPt = (p: Point2D): Point2D => [
        arc.center[0] + (p[0] - arc.center[0]) * scale,
        arc.center[1] + (p[1] - arc.center[1]) * scale,
      ];
      return {
        type: 'arc',
        id: newId(),
        start: offsetPt(entity.start),
        mid: offsetPt(entity.mid),
        end: offsetPt(entity.end),
      };
    }
    case 'rectangle': {
      // Offset each edge inward/outward
      const c1 = entity.corner1;
      const c2 = entity.corner2;
      const minX = Math.min(c1[0], c2[0]);
      const maxX = Math.max(c1[0], c2[0]);
      const minY = Math.min(c1[1], c2[1]);
      const maxY = Math.max(c1[1], c2[1]);
      return {
        type: 'rectangle',
        id: newId(),
        corner1: [minX - distance, minY - distance],
        corner2: [maxX + distance, maxY + distance],
      };
    }
    default:
      return null;
  }
}

// ── Mirror ───────────────────────────────────────

function handleMirror(
  selected: SketchEntity[],
  newId: () => SketchEntityId,
): SketchOpAction {
  if (selected.length < 2) {
    return { type: 'need-more', message: 'Select entities to mirror, then click the mirror line (last selection)' };
  }

  // Last selected entity is the mirror line
  const mirrorEntity = selected[selected.length - 1];
  if (mirrorEntity.type !== 'line') {
    return { type: 'invalid', message: 'Mirror axis must be a line' };
  }

  const toMirror = selected.slice(0, -1);
  const mirrored: SketchEntity[] = [];

  for (const entity of toMirror) {
    const result = mirrorEntity2D(entity, mirrorEntity.start, mirrorEntity.end, newId);
    if (result) mirrored.push(result);
  }

  if (mirrored.length === 0) {
    return { type: 'invalid', message: 'No entities could be mirrored' };
  }

  return { type: 'replace', removeIds: [], addEntities: mirrored };
}

function mirrorEntity2D(
  entity: SketchEntity,
  lineStart: Point2D,
  lineEnd: Point2D,
  newId: () => string,
): SketchEntity | null {
  const reflect = (p: Point2D) => reflectPoint(p, lineStart, lineEnd);

  switch (entity.type) {
    case 'line':
      return {
        type: 'line',
        id: newId(),
        start: reflect(entity.start),
        end: reflect(entity.end),
      };
    case 'circle':
      return {
        type: 'circle',
        id: newId(),
        center: reflect(entity.center),
        radius: entity.radius,
      };
    case 'arc':
      return {
        type: 'arc',
        id: newId(),
        start: reflect(entity.start),
        mid: reflect(entity.mid),
        end: reflect(entity.end),
      };
    case 'rectangle':
      return {
        type: 'rectangle',
        id: newId(),
        corner1: reflect(entity.corner1),
        corner2: reflect(entity.corner2),
      };
    case 'spline':
    case 'bezier':
      return {
        ...entity,
        id: newId(),
        points: entity.points.map(reflect),
      };
  }
}

// ── Sketch Fillet ────────────────────────────────

function handleSketchFillet(
  selected: SketchEntity[],
  allEntities: SketchEntity[],
  newId: () => SketchEntityId,
  radius?: number,
): SketchOpAction {
  if (selected.length < 2) {
    return { type: 'need-more', message: 'Select 2 connected lines for fillet' };
  }

  const e1 = selected[0];
  const e2 = selected[1];
  if (e1.type !== 'line' || e2.type !== 'line') {
    return { type: 'invalid', message: 'Fillet requires two lines' };
  }

  // Find shared endpoint
  const shared = findSharedEndpoint(e1, e2);
  if (!shared) {
    return { type: 'invalid', message: 'Lines must share an endpoint (coincident or close)' };
  }

  if (radius === undefined) {
    return {
      type: 'need-value',
      prompt: 'Fillet radius',
      defaultValue: 1,
      pendingOp: { opType: 'sketch-op-fillet', entityIds: [e1.id, e2.id] },
    };
  }

  if (radius <= 0) return { type: 'invalid', message: 'Fillet radius must be positive' };

  // Compute fillet
  const result = computeFillet(e1, e2, shared, radius, newId);
  if (!result) return { type: 'invalid', message: 'Cannot compute fillet for these lines' };

  return {
    type: 'replace',
    removeIds: [e1.id, e2.id],
    addEntities: [result.line1, result.line2, result.arc],
  };
}

// ── Sketch Chamfer ───────────────────────────────

function handleSketchChamfer(
  selected: SketchEntity[],
  allEntities: SketchEntity[],
  newId: () => SketchEntityId,
  distance?: number,
): SketchOpAction {
  if (selected.length < 2) {
    return { type: 'need-more', message: 'Select 2 connected lines for chamfer' };
  }

  const e1 = selected[0];
  const e2 = selected[1];
  if (e1.type !== 'line' || e2.type !== 'line') {
    return { type: 'invalid', message: 'Chamfer requires two lines' };
  }

  const shared = findSharedEndpoint(e1, e2);
  if (!shared) {
    return { type: 'invalid', message: 'Lines must share an endpoint (coincident or close)' };
  }

  if (distance === undefined) {
    return {
      type: 'need-value',
      prompt: 'Chamfer distance',
      defaultValue: 1,
      pendingOp: { opType: 'sketch-op-chamfer', entityIds: [e1.id, e2.id] },
    };
  }

  if (distance <= 0) return { type: 'invalid', message: 'Chamfer distance must be positive' };

  const result = computeChamfer(e1, e2, shared, distance, newId);
  if (!result) return { type: 'invalid', message: 'Cannot compute chamfer for these lines' };

  return {
    type: 'replace',
    removeIds: [e1.id, e2.id],
    addEntities: [result.line1, result.line2, result.chamferLine],
  };
}

// ── Geometry helpers ─────────────────────────────

export function lineLineIntersection(
  p1: Point2D, p2: Point2D,
  p3: Point2D, p4: Point2D,
): Point2D | null {
  const d1x = p2[0] - p1[0];
  const d1y = p2[1] - p1[1];
  const d2x = p4[0] - p3[0];
  const d2y = p4[1] - p3[1];

  const denom = d1x * d2y - d1y * d2x;
  if (Math.abs(denom) < 1e-10) return null; // parallel

  const t = ((p3[0] - p1[0]) * d2y - (p3[1] - p1[1]) * d2x) / denom;
  const u = ((p3[0] - p1[0]) * d1y - (p3[1] - p1[1]) * d1x) / denom;

  // Check both are within [0, 1] for segment-segment intersection
  if (t < -1e-10 || t > 1 + 1e-10 || u < -1e-10 || u > 1 + 1e-10) return null;

  return [p1[0] + t * d1x, p1[1] + t * d1y];
}

export function lineCircleIntersection(
  p1: Point2D, p2: Point2D,
  center: Point2D, radius: number,
): Point2D[] {
  const dx = p2[0] - p1[0];
  const dy = p2[1] - p1[1];
  const fx = p1[0] - center[0];
  const fy = p1[1] - center[1];

  const a = dx * dx + dy * dy;
  const b = 2 * (fx * dx + fy * dy);
  const c = fx * fx + fy * fy - radius * radius;

  let discriminant = b * b - 4 * a * c;
  if (discriminant < 0) return [];

  discriminant = Math.sqrt(discriminant);
  const results: Point2D[] = [];

  for (const sign of [-1, 1]) {
    const t = (-b + sign * discriminant) / (2 * a);
    if (t >= -1e-10 && t <= 1 + 1e-10) {
      results.push([p1[0] + t * dx, p1[1] + t * dy]);
    }
  }

  return results;
}

export function reflectPoint(point: Point2D, lineStart: Point2D, lineEnd: Point2D): Point2D {
  const dx = lineEnd[0] - lineStart[0];
  const dy = lineEnd[1] - lineStart[1];
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1e-10) return [...point] as Point2D;

  // Project point onto the line
  const t = ((point[0] - lineStart[0]) * dx + (point[1] - lineStart[1]) * dy) / lenSq;
  const projX = lineStart[0] + t * dx;
  const projY = lineStart[1] + t * dy;

  // Reflect: P' = 2 * proj - P
  return [2 * projX - point[0], 2 * projY - point[1]];
}

export function projectPointOnLine(
  point: Point2D, lineStart: Point2D, lineEnd: Point2D,
): { t: number; point: Point2D } {
  const dx = lineEnd[0] - lineStart[0];
  const dy = lineEnd[1] - lineStart[1];
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1e-10) return { t: 0, point: [...lineStart] as Point2D };

  const t = ((point[0] - lineStart[0]) * dx + (point[1] - lineStart[1]) * dy) / lenSq;
  return {
    t,
    point: [lineStart[0] + t * dx, lineStart[1] + t * dy],
  };
}

interface IntersectionResult {
  point: Point2D;
  entityId: string;
  t: number;  // parametric position on the source entity
}

export function findEntityIntersections(
  entity: SketchEntity,
  otherEntities: SketchEntity[],
): IntersectionResult[] {
  const results: IntersectionResult[] = [];

  for (const other of otherEntities) {
    const points = intersectEntities(entity, other);
    for (const pt of points) {
      const t = getParametricT(entity, pt);
      results.push({ point: pt, entityId: other.id, t });
    }
  }

  // Sort by parametric position
  results.sort((a, b) => a.t - b.t);
  return results;
}

function intersectEntities(e1: SketchEntity, e2: SketchEntity): Point2D[] {
  // Line-Line
  if (e1.type === 'line' && e2.type === 'line') {
    const pt = lineLineIntersection(e1.start, e1.end, e2.start, e2.end);
    return pt ? [pt] : [];
  }

  // Line-Circle
  if (e1.type === 'line' && e2.type === 'circle') {
    return lineCircleIntersection(e1.start, e1.end, e2.center, e2.radius);
  }
  if (e1.type === 'circle' && e2.type === 'line') {
    return lineCircleIntersection(e2.start, e2.end, e1.center, e1.radius);
  }

  // Line-Rectangle (decompose rect into 4 edges)
  if (e1.type === 'line' && e2.type === 'rectangle') {
    return rectLineIntersections(e2, e1);
  }
  if (e1.type === 'rectangle' && e2.type === 'line') {
    return rectLineIntersections(e1, e2);
  }

  // Line-Arc (approximate: use line-circle then filter by arc bounds)
  if (e1.type === 'line' && e2.type === 'arc') {
    return lineArcIntersection(e1, e2);
  }
  if (e1.type === 'arc' && e2.type === 'line') {
    return lineArcIntersection(e2, e1);
  }

  // Circle-Circle
  if (e1.type === 'circle' && e2.type === 'circle') {
    return circleCircleIntersection(e1.center, e1.radius, e2.center, e2.radius);
  }

  return [];
}

function rectLineIntersections(rect: SketchRectangle, line: SketchLine): Point2D[] {
  const c1 = rect.corner1;
  const c2 = rect.corner2;
  const edges: [Point2D, Point2D][] = [
    [[c1[0], c1[1]], [c2[0], c1[1]]],
    [[c2[0], c1[1]], [c2[0], c2[1]]],
    [[c2[0], c2[1]], [c1[0], c2[1]]],
    [[c1[0], c2[1]], [c1[0], c1[1]]],
  ];
  const results: Point2D[] = [];
  for (const [a, b] of edges) {
    const pt = lineLineIntersection(line.start, line.end, a, b);
    if (pt) results.push(pt);
  }
  return results;
}

function lineArcIntersection(line: SketchLine, arc: SketchArc): Point2D[] {
  const arcInfo = computeArcCenterRadius(arc.start, arc.mid, arc.end);
  if (!arcInfo) return [];

  const circleIntersections = lineCircleIntersection(line.start, line.end, arcInfo.center, arcInfo.radius);

  // Filter to points that are actually on the arc
  return circleIntersections.filter(pt => isPointOnArc(pt, arc, arcInfo));
}

function circleCircleIntersection(c1: Point2D, r1: number, c2: Point2D, r2: number): Point2D[] {
  const dx = c2[0] - c1[0];
  const dy = c2[1] - c1[1];
  const d = Math.sqrt(dx * dx + dy * dy);

  if (d > r1 + r2 + 1e-10 || d < Math.abs(r1 - r2) - 1e-10 || d < 1e-10) {
    return [];
  }

  const a = (r1 * r1 - r2 * r2 + d * d) / (2 * d);
  const h = Math.sqrt(Math.max(0, r1 * r1 - a * a));

  const mx = c1[0] + a * dx / d;
  const my = c1[1] + a * dy / d;

  if (h < 1e-10) return [[mx, my]];

  return [
    [mx + h * dy / d, my - h * dx / d],
    [mx - h * dy / d, my + h * dx / d],
  ];
}

function getParametricT(entity: SketchEntity, point: Point2D): number {
  if (entity.type === 'line') {
    const dx = entity.end[0] - entity.start[0];
    const dy = entity.end[1] - entity.start[1];
    const lenSq = dx * dx + dy * dy;
    if (lenSq < 1e-10) return 0;
    return ((point[0] - entity.start[0]) * dx + (point[1] - entity.start[1]) * dy) / lenSq;
  }

  if (entity.type === 'circle') {
    return Math.atan2(point[1] - entity.center[1], point[0] - entity.center[0]);
  }

  // For other types, use distance-based approximation
  return 0;
}

// ── Trim helpers ─────────────────────────────────

function trimLineAtIntersections(
  line: SketchLine,
  intersections: IntersectionResult[],
  clickPoint: Point2D,
  newId: () => string,
): SketchLine[] {
  // Sort intersections by parametric position
  const sorted = [...intersections].sort((a, b) => a.t - b.t);

  // Get click point's parametric position
  const clickT = getParametricT(line, clickPoint);

  // Build segment boundaries: [0, t1, t2, ..., 1]
  const boundaries = [0, ...sorted.map(i => Math.max(0, Math.min(1, i.t))), 1];

  // Find which segment the click falls in
  let clickSegIdx = 0;
  for (let i = 0; i < boundaries.length - 1; i++) {
    if (clickT >= boundaries[i] - 1e-6 && clickT <= boundaries[i + 1] + 1e-6) {
      clickSegIdx = i;
      break;
    }
  }

  // Create all segments except the clicked one
  const result: SketchLine[] = [];
  for (let i = 0; i < boundaries.length - 1; i++) {
    if (i === clickSegIdx) continue;
    const t0 = boundaries[i];
    const t1 = boundaries[i + 1];
    if (t1 - t0 < 1e-6) continue;

    const start: Point2D = [
      line.start[0] + t0 * (line.end[0] - line.start[0]),
      line.start[1] + t0 * (line.end[1] - line.start[1]),
    ];
    const end: Point2D = [
      line.start[0] + t1 * (line.end[0] - line.start[0]),
      line.start[1] + t1 * (line.end[1] - line.start[1]),
    ];
    result.push({ type: 'line', id: newId(), start, end });
  }

  return result;
}

function trimCircleAtIntersections(
  circle: SketchCircle,
  intersections: IntersectionResult[],
  clickPoint: Point2D,
  newId: () => string,
): SketchArc[] {
  // Convert intersection points to angles
  const angles = intersections.map(i =>
    Math.atan2(i.point[1] - circle.center[1], i.point[0] - circle.center[0])
  );
  const clickAngle = Math.atan2(clickPoint[1] - circle.center[1], clickPoint[0] - circle.center[0]);

  // Sort angles
  const sorted = [...angles].sort((a, b) => a - b);

  // Find which arc segment the click falls in and create all others
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const clickNorm = normalize(clickAngle);

  const normalizedSorted = sorted.map(normalize).sort((a, b) => a - b);

  // Create arcs between consecutive intersection points, excluding the one containing click
  const result: SketchArc[] = [];
  for (let i = 0; i < normalizedSorted.length; i++) {
    const startAngle = normalizedSorted[i];
    const endAngle = normalizedSorted[(i + 1) % normalizedSorted.length];
    const midAngle = startAngle < endAngle
      ? (startAngle + endAngle) / 2
      : normalize((startAngle + endAngle + 2 * Math.PI) / 2);

    // Check if click is in this arc
    const inArc = isAngleInRange(clickNorm, startAngle, endAngle);
    if (inArc) continue;

    const start: Point2D = [
      circle.center[0] + Math.cos(startAngle) * circle.radius,
      circle.center[1] + Math.sin(startAngle) * circle.radius,
    ];
    const mid: Point2D = [
      circle.center[0] + Math.cos(midAngle) * circle.radius,
      circle.center[1] + Math.sin(midAngle) * circle.radius,
    ];
    const end: Point2D = [
      circle.center[0] + Math.cos(endAngle) * circle.radius,
      circle.center[1] + Math.sin(endAngle) * circle.radius,
    ];

    result.push({ type: 'arc', id: newId(), start, mid, end });
  }

  return result;
}

function trimArcAtIntersections(
  arc: SketchArc,
  intersections: IntersectionResult[],
  clickPoint: Point2D,
  newId: () => string,
): SketchArc[] {
  const arcInfo = computeArcCenterRadius(arc.start, arc.mid, arc.end);
  if (!arcInfo) return [];

  const { center, radius } = arcInfo;

  // Get angles for arc endpoints and intersection points
  const startAngle = Math.atan2(arc.start[1] - center[1], arc.start[0] - center[0]);
  const endAngle = Math.atan2(arc.end[1] - center[1], arc.end[0] - center[0]);
  const clickAngle = Math.atan2(clickPoint[1] - center[1], clickPoint[0] - center[0]);

  // Filter intersections that are actually on the arc
  const onArc = intersections.filter(i => isPointOnArc(i.point, arc, arcInfo));
  if (onArc.length === 0) return [];

  const intAngles = onArc.map(i =>
    Math.atan2(i.point[1] - center[1], i.point[0] - center[0])
  );

  // Build sub-arcs between arc start, intersection points, and arc end
  const allAngles = [startAngle, ...intAngles, endAngle];

  // Determine sweep direction from original arc
  const midAngle = Math.atan2(arc.mid[1] - center[1], arc.mid[0] - center[0]);
  // Just create sub-arcs and exclude the one containing click
  const result: SketchArc[] = [];
  for (let i = 0; i < allAngles.length - 1; i++) {
    const a0 = allAngles[i];
    const a1 = allAngles[i + 1];
    const aMid = (a0 + a1) / 2;

    // Check if click is closest to this segment
    const diff = Math.abs(normalizeAngle(clickAngle - aMid));
    const segSpan = Math.abs(normalizeAngle(a1 - a0));
    if (diff < segSpan / 2 + 0.1) continue;

    const s: Point2D = [center[0] + Math.cos(a0) * radius, center[1] + Math.sin(a0) * radius];
    const m: Point2D = [center[0] + Math.cos(aMid) * radius, center[1] + Math.sin(aMid) * radius];
    const e: Point2D = [center[0] + Math.cos(a1) * radius, center[1] + Math.sin(a1) * radius];

    result.push({ type: 'arc', id: newId(), start: s, mid: m, end: e });
  }

  return result;
}

// ── Fillet/Chamfer helpers ───────────────────────

interface SharedEndpointInfo {
  point: Point2D;
  line1Shared: 'start' | 'end';
  line2Shared: 'start' | 'end';
}

function findSharedEndpoint(l1: SketchLine, l2: SketchLine): SharedEndpointInfo | null {
  const threshold = 0.5;
  const pairs: [Point2D, 'start' | 'end', Point2D, 'start' | 'end'][] = [
    [l1.start, 'start', l2.start, 'start'],
    [l1.start, 'start', l2.end, 'end'],
    [l1.end, 'end', l2.start, 'start'],
    [l1.end, 'end', l2.end, 'end'],
  ];

  for (const [p1, w1, p2, w2] of pairs) {
    if (dist2D(p1, p2) < threshold) {
      const avg: Point2D = [(p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2];
      return { point: avg, line1Shared: w1, line2Shared: w2 };
    }
  }
  return null;
}

function computeFillet(
  l1: SketchLine, l2: SketchLine,
  shared: SharedEndpointInfo,
  radius: number,
  newId: () => string,
): { line1: SketchLine; line2: SketchLine; arc: SketchArc } | null {
  // Direction vectors pointing away from the shared point
  const d1 = getDirectionAway(l1, shared.line1Shared);
  const d2 = getDirectionAway(l2, shared.line2Shared);

  const len1 = lineLength(l1);
  const len2 = lineLength(l2);

  // Compute angle between lines
  const dot = d1[0] * d2[0] + d1[1] * d2[1];
  const cross = d1[0] * d2[1] - d1[1] * d2[0];
  const halfAngle = Math.acos(Math.max(-1, Math.min(1, dot))) / 2;
  if (halfAngle < 1e-6) return null;

  const trimDist = radius / Math.tan(halfAngle);
  if (trimDist > len1 * 0.99 || trimDist > len2 * 0.99) return null;

  // Trim points on each line
  const t1: Point2D = [shared.point[0] + d1[0] * trimDist, shared.point[1] + d1[1] * trimDist];
  const t2: Point2D = [shared.point[0] + d2[0] * trimDist, shared.point[1] + d2[1] * trimDist];

  // Arc center: offset along bisector
  const bisector: Point2D = [d1[0] + d2[0], d1[1] + d2[1]];
  const bisLen = Math.sqrt(bisector[0] * bisector[0] + bisector[1] * bisector[1]);
  if (bisLen < 1e-10) return null;

  const centerDist = radius / Math.sin(halfAngle);
  const center: Point2D = [
    shared.point[0] + (bisector[0] / bisLen) * centerDist,
    shared.point[1] + (bisector[1] / bisLen) * centerDist,
  ];

  // Arc midpoint
  const midAngle = Math.atan2(
    (t1[1] + t2[1]) / 2 - center[1],
    (t1[0] + t2[0]) / 2 - center[0],
  );
  const arcMid: Point2D = [
    center[0] + Math.cos(midAngle) * radius,
    center[1] + Math.sin(midAngle) * radius,
  ];

  // Build trimmed lines
  const newL1 = buildTrimmedLine(l1, shared.line1Shared, t1, newId);
  const newL2 = buildTrimmedLine(l2, shared.line2Shared, t2, newId);

  return {
    line1: newL1,
    line2: newL2,
    arc: { type: 'arc', id: newId(), start: t1, mid: arcMid, end: t2 },
  };
}

function computeChamfer(
  l1: SketchLine, l2: SketchLine,
  shared: SharedEndpointInfo,
  distance: number,
  newId: () => string,
): { line1: SketchLine; line2: SketchLine; chamferLine: SketchLine } | null {
  const d1 = getDirectionAway(l1, shared.line1Shared);
  const d2 = getDirectionAway(l2, shared.line2Shared);

  const len1 = lineLength(l1);
  const len2 = lineLength(l2);

  if (distance > len1 * 0.99 || distance > len2 * 0.99) return null;

  const t1: Point2D = [shared.point[0] + d1[0] * distance, shared.point[1] + d1[1] * distance];
  const t2: Point2D = [shared.point[0] + d2[0] * distance, shared.point[1] + d2[1] * distance];

  const newL1 = buildTrimmedLine(l1, shared.line1Shared, t1, newId);
  const newL2 = buildTrimmedLine(l2, shared.line2Shared, t2, newId);

  return {
    line1: newL1,
    line2: newL2,
    chamferLine: { type: 'line', id: newId(), start: t1, end: t2 },
  };
}

function getDirectionAway(line: SketchLine, whichEnd: 'start' | 'end'): Point2D {
  let dx: number, dy: number;
  if (whichEnd === 'start') {
    dx = line.end[0] - line.start[0];
    dy = line.end[1] - line.start[1];
  } else {
    dx = line.start[0] - line.end[0];
    dy = line.start[1] - line.end[1];
  }
  const len = Math.sqrt(dx * dx + dy * dy);
  if (len < 1e-10) return [1, 0];
  return [dx / len, dy / len];
}

function buildTrimmedLine(
  line: SketchLine,
  sharedEnd: 'start' | 'end',
  trimPoint: Point2D,
  newId: () => string,
): SketchLine {
  if (sharedEnd === 'start') {
    return { type: 'line', id: newId(), start: trimPoint, end: line.end };
  } else {
    return { type: 'line', id: newId(), start: line.start, end: trimPoint };
  }
}

// ── Rectangle decomposition ─────────────────────

function decomposeRectangle(rect: SketchRectangle, newId: () => string): SketchLine[] {
  const c1 = rect.corner1;
  const c2 = rect.corner2;
  return [
    { type: 'line', id: newId(), start: [c1[0], c1[1]], end: [c2[0], c1[1]] },
    { type: 'line', id: newId(), start: [c2[0], c1[1]], end: [c2[0], c2[1]] },
    { type: 'line', id: newId(), start: [c2[0], c2[1]], end: [c1[0], c2[1]] },
    { type: 'line', id: newId(), start: [c1[0], c2[1]], end: [c1[0], c1[1]] },
  ];
}

// ── Arc helpers ──────────────────────────────────

function computeArcCenterRadius(
  start: Point2D, mid: Point2D, end: Point2D,
): { center: Point2D; radius: number } | null {
  const ax = start[0], ay = start[1];
  const bx = mid[0], by = mid[1];
  const cx = end[0], cy = end[1];

  const D = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
  if (Math.abs(D) < 1e-10) return null;

  const ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / D;
  const uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / D;

  const radius = Math.sqrt((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy));
  return { center: [ux, uy], radius };
}

function isPointOnArc(
  point: Point2D,
  arc: SketchArc,
  arcInfo: { center: Point2D; radius: number },
): boolean {
  const { center } = arcInfo;
  const startAngle = Math.atan2(arc.start[1] - center[1], arc.start[0] - center[0]);
  const midAngle = Math.atan2(arc.mid[1] - center[1], arc.mid[0] - center[0]);
  const endAngle = Math.atan2(arc.end[1] - center[1], arc.end[0] - center[0]);
  const pointAngle = Math.atan2(point[1] - center[1], point[0] - center[0]);

  // Check if point angle is between start and end (going through mid)
  return isAngleBetween(pointAngle, startAngle, midAngle, endAngle);
}

function isAngleBetween(angle: number, start: number, mid: number, end: number): boolean {
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const a = normalize(angle - start);
  const m = normalize(mid - start);
  const e = normalize(end - start);

  if (m <= e) {
    // Counter-clockwise: angle should be between 0 and e
    return a <= e + 0.1;
  } else {
    // Clockwise: angle should be between e (wrapped) and 2pi
    return a >= e - 0.1 || a <= 0.1;
  }
}

function isAngleInRange(angle: number, start: number, end: number): boolean {
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const a = normalize(angle);
  const s = normalize(start);
  const e = normalize(end);

  if (s <= e) {
    return a >= s && a <= e;
  } else {
    return a >= s || a <= e;
  }
}

function normalizeAngle(a: number): number {
  while (a > Math.PI) a -= 2 * Math.PI;
  while (a < -Math.PI) a += 2 * Math.PI;
  return a;
}

// ── Generic helpers ──────────────────────────────

function dist2D(a: Point2D, b: Point2D): number {
  return Math.sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2);
}

function lineLength(line: SketchLine): number {
  return dist2D(line.start, line.end);
}

function pointToSegmentDist(p: Point2D, a: Point2D, b: Point2D): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return dist2D(p, a);
  let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  const projX = a[0] + t * dx;
  const projY = a[1] + t * dy;
  return Math.sqrt((p[0] - projX) ** 2 + (p[1] - projY) ** 2);
}
