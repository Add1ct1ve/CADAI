import type {
  SketchToolId,
  Point2D,
  SketchEntity,
  SketchEntityId,
} from '$lib/types/cad';

export type SketchAction =
  | { type: 'advance'; points: Point2D[] }
  | { type: 'create'; entity: SketchEntity; chainPoints: Point2D[] }
  | { type: 'none' };

/**
 * Pure-function state machine for sketch drawing tools.
 * Returns an action describing what to do next.
 */
export function handleSketchClick(
  tool: SketchToolId,
  clickPoint: Point2D,
  drawingPoints: Point2D[],
  newEntityId: () => SketchEntityId,
): SketchAction {
  switch (tool) {
    case 'sketch-line': {
      if (drawingPoints.length === 0) {
        // First click: set start point
        return { type: 'advance', points: [clickPoint] };
      }
      // Second click: create line, chain (endpoint becomes next start)
      const entity: SketchEntity = {
        type: 'line',
        id: newEntityId(),
        start: drawingPoints[0],
        end: clickPoint,
      };
      return { type: 'create', entity, chainPoints: [clickPoint] };
    }

    case 'sketch-rect': {
      if (drawingPoints.length === 0) {
        // First click: set corner1
        return { type: 'advance', points: [clickPoint] };
      }
      // Second click: create rectangle
      const entity: SketchEntity = {
        type: 'rectangle',
        id: newEntityId(),
        corner1: drawingPoints[0],
        corner2: clickPoint,
      };
      return { type: 'create', entity, chainPoints: [] };
    }

    case 'sketch-circle': {
      if (drawingPoints.length === 0) {
        // First click: set center
        return { type: 'advance', points: [clickPoint] };
      }
      // Second click: create circle (radius = distance)
      const dx = clickPoint[0] - drawingPoints[0][0];
      const dy = clickPoint[1] - drawingPoints[0][1];
      const radius = Math.sqrt(dx * dx + dy * dy);
      if (radius < 0.01) return { type: 'none' };
      const entity: SketchEntity = {
        type: 'circle',
        id: newEntityId(),
        center: drawingPoints[0],
        radius,
      };
      return { type: 'create', entity, chainPoints: [] };
    }

    case 'sketch-arc': {
      if (drawingPoints.length === 0) {
        // First click: start point
        return { type: 'advance', points: [clickPoint] };
      }
      if (drawingPoints.length === 1) {
        // Second click: end point (we'll ask for mid next via preview)
        return { type: 'advance', points: [...drawingPoints, clickPoint] };
      }
      // Third click: mid point -> create arc (start, mid=click, end=point[1])
      const entity: SketchEntity = {
        type: 'arc',
        id: newEntityId(),
        start: drawingPoints[0],
        mid: clickPoint,
        end: drawingPoints[1],
      };
      return { type: 'create', entity, chainPoints: [] };
    }

    default:
      return { type: 'none' };
  }
}

/**
 * Whether the tool should continue drawing after creating an entity
 * (i.e., the line tool chains: endpoint becomes next start).
 */
export function shouldChainDraw(tool: SketchToolId): boolean {
  return tool === 'sketch-line';
}
