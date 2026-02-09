import type {
  SketchEntity,
  SketchConstraint,
  ConstraintState,
  Point2D,
} from '$lib/types/cad';
import type { GcsWrapper } from '@salusoft89/planegcs';
import type { SketchPrimitive } from '@salusoft89/planegcs';
import { SolveStatus } from '@salusoft89/planegcs';

export interface SolveResult {
  updatedEntities: SketchEntity[];
  dof: number;
  status: ConstraintState;
}

let gcsWrapper: GcsWrapper | null = null;

export async function initSolver(): Promise<void> {
  if (gcsWrapper) return;
  // Dynamic import to support WASM lazy-loading
  const { make_gcs_wrapper } = await import('@salusoft89/planegcs');
  gcsWrapper = await make_gcs_wrapper();
}

export function destroySolver(): void {
  if (gcsWrapper) {
    gcsWrapper.destroy_gcs_module();
    gcsWrapper = null;
  }
}

export function isSolverReady(): boolean {
  return gcsWrapper !== null;
}

// ── Point coordinate helpers ──

export function getPointCoords(entity: SketchEntity, pointIndex: number): Point2D {
  switch (entity.type) {
    case 'line':
      return pointIndex === 0 ? entity.start : entity.end;
    case 'rectangle': {
      const c1 = entity.corner1;
      const c2 = entity.corner2;
      switch (pointIndex) {
        case 0: return c1;
        case 1: return [c2[0], c1[1]];
        case 2: return c2;
        case 3: return [c1[0], c2[1]];
        default: return c1;
      }
    }
    case 'circle':
      return entity.center;
    case 'arc':
      switch (pointIndex) {
        case 0: return entity.start;
        case 1: return entity.mid;
        case 2: return entity.end;
        default: return entity.start;
      }
  }
}

// ── Build planegcs primitives from entities ──

function buildEntityPrimitives(entity: SketchEntity): SketchPrimitive[] {
  const primitives: SketchPrimitive[] = [];
  const eid = entity.id;

  switch (entity.type) {
    case 'line': {
      primitives.push(
        { id: `${eid}_p0`, type: 'point', x: entity.start[0], y: entity.start[1], fixed: false },
        { id: `${eid}_p1`, type: 'point', x: entity.end[0], y: entity.end[1], fixed: false },
        { id: `${eid}_l`, type: 'line', p1_id: `${eid}_p0`, p2_id: `${eid}_p1` },
      );
      break;
    }
    case 'rectangle': {
      // 4 corner points + 4 line edges
      const c1 = entity.corner1;
      const c2 = entity.corner2;
      const corners: Point2D[] = [c1, [c2[0], c1[1]], c2, [c1[0], c2[1]]];
      for (let i = 0; i < 4; i++) {
        primitives.push({
          id: `${eid}_p${i}`, type: 'point',
          x: corners[i][0], y: corners[i][1], fixed: false,
        });
      }
      for (let i = 0; i < 4; i++) {
        primitives.push({
          id: `${eid}_l${i}`, type: 'line',
          p1_id: `${eid}_p${i}`, p2_id: `${eid}_p${(i + 1) % 4}`,
        });
      }
      // Implicit constraints for rectangle shape: opposite sides parallel, all perpendicular
      primitives.push(
        { id: `${eid}_para0`, type: 'parallel', l1_id: `${eid}_l0`, l2_id: `${eid}_l2` } as SketchPrimitive,
        { id: `${eid}_para1`, type: 'parallel', l1_id: `${eid}_l1`, l2_id: `${eid}_l3` } as SketchPrimitive,
        { id: `${eid}_perp0`, type: 'perpendicular_ll', l1_id: `${eid}_l0`, l2_id: `${eid}_l1` } as SketchPrimitive,
      );
      break;
    }
    case 'circle': {
      primitives.push(
        { id: `${eid}_p0`, type: 'point', x: entity.center[0], y: entity.center[1], fixed: false },
        { id: `${eid}_c`, type: 'circle', c_id: `${eid}_p0`, radius: entity.radius },
      );
      break;
    }
    case 'arc': {
      // Compute center and radius from 3 points
      const { center, radius, startAngle, endAngle } = computeArcCenterRadius(
        entity.start, entity.mid, entity.end,
      );
      primitives.push(
        { id: `${eid}_center`, type: 'point', x: center[0], y: center[1], fixed: false },
        { id: `${eid}_p0`, type: 'point', x: entity.start[0], y: entity.start[1], fixed: false },
        { id: `${eid}_p2`, type: 'point', x: entity.end[0], y: entity.end[1], fixed: false },
        {
          id: `${eid}_a`, type: 'arc',
          c_id: `${eid}_center`,
          radius,
          start_angle: startAngle,
          end_angle: endAngle,
          start_id: `${eid}_p0`,
          end_id: `${eid}_p2`,
        },
        { id: `${eid}_arules`, type: 'arc_rules', a_id: `${eid}_a` } as SketchPrimitive,
      );
      break;
    }
  }

  return primitives;
}

function computeArcCenterRadius(start: Point2D, mid: Point2D, end: Point2D) {
  const ax = start[0], ay = start[1];
  const bx = mid[0], by = mid[1];
  const cx = end[0], cy = end[1];

  const D = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
  if (Math.abs(D) < 1e-10) {
    // Degenerate: points are collinear
    return { center: mid, radius: 1, startAngle: 0, endAngle: Math.PI };
  }

  const ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / D;
  const uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / D;

  const radius = Math.sqrt((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy));
  const startAngle = Math.atan2(ay - uy, ax - ux);
  const endAngle = Math.atan2(cy - uy, cx - ux);

  return { center: [ux, uy] as Point2D, radius, startAngle, endAngle };
}

// ── Build planegcs constraint primitives ──

function buildConstraintPrimitives(
  constraint: SketchConstraint,
  entities: SketchEntity[],
): SketchPrimitive[] {
  const entityMap = new Map(entities.map(e => [e.id, e]));

  switch (constraint.type) {
    case 'coincident': {
      const p1Id = pointRefToGcsId(constraint.point1);
      const p2Id = pointRefToGcsId(constraint.point2);
      return [{ id: constraint.id, type: 'p2p_coincident', p1_id: p1Id, p2_id: p2Id } as SketchPrimitive];
    }
    case 'horizontal': {
      const lineId = getEntityLineId(constraint.entityId, entityMap);
      if (!lineId) return [];
      return [{ id: constraint.id, type: 'horizontal_l', l_id: lineId } as SketchPrimitive];
    }
    case 'vertical': {
      const lineId = getEntityLineId(constraint.entityId, entityMap);
      if (!lineId) return [];
      return [{ id: constraint.id, type: 'vertical_l', l_id: lineId } as SketchPrimitive];
    }
    case 'parallel': {
      const l1 = getEntityLineId(constraint.entityId1, entityMap);
      const l2 = getEntityLineId(constraint.entityId2, entityMap);
      if (!l1 || !l2) return [];
      return [{ id: constraint.id, type: 'parallel', l1_id: l1, l2_id: l2 } as SketchPrimitive];
    }
    case 'perpendicular': {
      const l1 = getEntityLineId(constraint.entityId1, entityMap);
      const l2 = getEntityLineId(constraint.entityId2, entityMap);
      if (!l1 || !l2) return [];
      return [{ id: constraint.id, type: 'perpendicular_ll', l1_id: l1, l2_id: l2 } as SketchPrimitive];
    }
    case 'equal': {
      const e1 = entityMap.get(constraint.entityId1);
      const e2 = entityMap.get(constraint.entityId2);
      if (!e1 || !e2) return [];
      if (e1.type === 'line' && e2.type === 'line') {
        return [{ id: constraint.id, type: 'equal_length', l1_id: `${e1.id}_l`, l2_id: `${e2.id}_l` } as SketchPrimitive];
      }
      if ((e1.type === 'circle' || e1.type === 'arc') && (e2.type === 'circle' || e2.type === 'arc')) {
        const c1Id = e1.type === 'circle' ? `${e1.id}_c` : `${e1.id}_a`;
        const c2Id = e2.type === 'circle' ? `${e2.id}_c` : `${e2.id}_a`;
        return [{ id: constraint.id, type: 'equal_radius_cc', c1_id: c1Id, c2_id: c2Id } as SketchPrimitive];
      }
      return [];
    }
    case 'distance': {
      const p1Id = pointRefToGcsId(constraint.point1);
      const p2Id = pointRefToGcsId(constraint.point2);
      return [{ id: constraint.id, type: 'p2p_distance', p1_id: p1Id, p2_id: p2Id, distance: constraint.value } as SketchPrimitive];
    }
    case 'radius': {
      const entity = entityMap.get(constraint.entityId);
      if (!entity) return [];
      if (entity.type === 'circle') {
        return [{ id: constraint.id, type: 'circle_radius', c_id: `${entity.id}_c`, radius: constraint.value } as SketchPrimitive];
      }
      if (entity.type === 'arc') {
        return [{ id: constraint.id, type: 'arc_radius', a_id: `${entity.id}_a`, radius: constraint.value } as SketchPrimitive];
      }
      return [];
    }
    case 'angle': {
      const l1 = getEntityLineId(constraint.entityId1, entityMap);
      const l2 = getEntityLineId(constraint.entityId2, entityMap);
      if (!l1 || !l2) return [];
      // planegcs expects angle in radians
      const angleRad = (constraint.value * Math.PI) / 180;
      return [{ id: constraint.id, type: 'l2l_angle_ll', l1_id: l1, l2_id: l2, angle: angleRad } as SketchPrimitive];
    }
  }
}

function pointRefToGcsId(ref: { entityId: string; pointIndex: number }): string {
  return `${ref.entityId}_p${ref.pointIndex}`;
}

function getEntityLineId(entityId: string, entityMap: Map<string, SketchEntity>): string | null {
  const entity = entityMap.get(entityId);
  if (!entity) return null;
  if (entity.type === 'line') return `${entityId}_l`;
  // For rectangle, use first edge as representative line
  if (entity.type === 'rectangle') return `${entityId}_l0`;
  return null;
}

// ── DOF calculation ──

function calcEntityDof(entity: SketchEntity): number {
  switch (entity.type) {
    case 'line': return 4;      // 2 points × 2 coords
    case 'rectangle': return 4; // effectively 2 corner points (other 2 determined by rectangle shape)
    case 'circle': return 3;    // center (2) + radius (1)
    case 'arc': return 5;       // center (2) + radius (1) + 2 angles
  }
}

function calcConstraintReduction(constraint: SketchConstraint): number {
  switch (constraint.type) {
    case 'coincident': return 2;
    case 'horizontal': return 1;
    case 'vertical': return 1;
    case 'parallel': return 1;
    case 'perpendicular': return 1;
    case 'equal': return 1;
    case 'distance': return 1;
    case 'radius': return 1;
    case 'angle': return 1;
  }
}

// ── Main solve function ──

export function solve(
  entities: SketchEntity[],
  constraints: SketchConstraint[],
): SolveResult {
  if (!gcsWrapper || entities.length === 0) {
    return {
      updatedEntities: entities,
      dof: entities.reduce((sum, e) => sum + calcEntityDof(e), 0),
      status: 'under-constrained',
    };
  }

  // Clear previous data
  gcsWrapper.clear_data();

  // Build all primitives
  const allPrimitives: SketchPrimitive[] = [];
  for (const entity of entities) {
    allPrimitives.push(...buildEntityPrimitives(entity));
  }
  for (const constraint of constraints) {
    allPrimitives.push(...buildConstraintPrimitives(constraint, entities));
  }

  // Push into solver
  try {
    gcsWrapper.push_primitives_and_params(allPrimitives);
  } catch (e) {
    console.warn('Constraint solver push failed:', e);
    return {
      updatedEntities: entities,
      dof: -1,
      status: 'over-constrained',
    };
  }

  // Solve
  const solveStatus = gcsWrapper.solve();

  if (solveStatus === SolveStatus.Failed) {
    // Check for over-constrained
    const hasConflicts = gcsWrapper.has_gcs_conflicting_constraints();
    return {
      updatedEntities: entities,
      dof: -1,
      status: hasConflicts ? 'over-constrained' : 'under-constrained',
    };
  }

  // Apply solution
  gcsWrapper.apply_solution();

  // Read DOF
  const dof = gcsWrapper.gcs.dof();

  // Read solved values back into entities
  const updatedEntities = entities.map(entity => readBackEntity(entity));

  const status: ConstraintState = dof === 0 ? 'well-constrained'
    : dof < 0 ? 'over-constrained'
    : 'under-constrained';

  return { updatedEntities, dof, status };
}

function readBackEntity(entity: SketchEntity): SketchEntity {
  if (!gcsWrapper) return entity;
  const idx = gcsWrapper.sketch_index;
  const eid = entity.id;

  switch (entity.type) {
    case 'line': {
      const p0 = idx.get_primitive(`${eid}_p0`);
      const p1 = idx.get_primitive(`${eid}_p1`);
      if (p0?.type === 'point' && p1?.type === 'point') {
        return { ...entity, start: [p0.x, p0.y], end: [p1.x, p1.y] };
      }
      return entity;
    }
    case 'rectangle': {
      const p0 = idx.get_primitive(`${eid}_p0`);
      const p2 = idx.get_primitive(`${eid}_p2`);
      if (p0?.type === 'point' && p2?.type === 'point') {
        return { ...entity, corner1: [p0.x, p0.y], corner2: [p2.x, p2.y] };
      }
      return entity;
    }
    case 'circle': {
      const center = idx.get_primitive(`${eid}_p0`);
      const circle = idx.get_primitive(`${eid}_c`);
      if (center?.type === 'point' && circle?.type === 'circle') {
        return { ...entity, center: [center.x, center.y], radius: circle.radius };
      }
      return entity;
    }
    case 'arc': {
      const centerPt = idx.get_primitive(`${eid}_center`);
      const startPt = idx.get_primitive(`${eid}_p0`);
      const endPt = idx.get_primitive(`${eid}_p2`);
      const arcObj = idx.get_primitive(`${eid}_a`);
      if (centerPt?.type === 'point' && startPt?.type === 'point' &&
          endPt?.type === 'point' && arcObj?.type === 'arc') {
        // Recompute mid from center, radius, and mid-angle
        const startAngle = arcObj.start_angle;
        const endAngle = arcObj.end_angle;
        const midAngle = (startAngle + endAngle) / 2;
        const r = arcObj.radius;
        const cx = centerPt.x, cy = centerPt.y;
        const mid: Point2D = [cx + Math.cos(midAngle) * r, cy + Math.sin(midAngle) * r];
        return {
          ...entity,
          start: [startPt.x, startPt.y],
          mid,
          end: [endPt.x, endPt.y],
        };
      }
      return entity;
    }
  }
}
