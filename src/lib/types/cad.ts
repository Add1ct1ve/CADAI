export type ObjectId = string;
export type PrimitiveType = 'box' | 'cylinder' | 'sphere' | 'cone';

// ─── Sketch types ────────────────────────────────
export type SketchId = string;
export type SketchEntityId = string;
export type SketchPlane = 'XY' | 'XZ' | 'YZ';
export type Point2D = [number, number];

export interface SketchLine {
  type: 'line';
  id: SketchEntityId;
  start: Point2D;
  end: Point2D;
}

export interface SketchRectangle {
  type: 'rectangle';
  id: SketchEntityId;
  corner1: Point2D;
  corner2: Point2D;
}

export interface SketchCircle {
  type: 'circle';
  id: SketchEntityId;
  center: Point2D;
  radius: number;
}

export interface SketchArc {
  type: 'arc';
  id: SketchEntityId;
  start: Point2D;
  mid: Point2D;
  end: Point2D;
}

export type SketchEntity = SketchLine | SketchRectangle | SketchCircle | SketchArc;

// ─── Sketch constraint types ─────────────────────
export type SketchConstraintId = string;

export interface PointRef {
  entityId: SketchEntityId;
  pointIndex: number;
  // Line: 0=start, 1=end
  // Rectangle: 0=corner1, 1=(c2x,c1y), 2=corner2, 3=(c1x,c2y)
  // Circle: 0=center
  // Arc: 0=start, 1=mid, 2=end
}

export type SketchConstraint =
  | { type: 'coincident';     id: SketchConstraintId; point1: PointRef; point2: PointRef }
  | { type: 'horizontal';     id: SketchConstraintId; entityId: SketchEntityId }
  | { type: 'vertical';       id: SketchConstraintId; entityId: SketchEntityId }
  | { type: 'parallel';       id: SketchConstraintId; entityId1: SketchEntityId; entityId2: SketchEntityId }
  | { type: 'perpendicular';  id: SketchConstraintId; entityId1: SketchEntityId; entityId2: SketchEntityId }
  | { type: 'equal';          id: SketchConstraintId; entityId1: SketchEntityId; entityId2: SketchEntityId }
  | { type: 'distance';       id: SketchConstraintId; point1: PointRef; point2: PointRef; value: number }
  | { type: 'radius';         id: SketchConstraintId; entityId: SketchEntityId; value: number }
  | { type: 'angle';          id: SketchConstraintId; entityId1: SketchEntityId; entityId2: SketchEntityId; value: number };

export type ConstraintState = 'under-constrained' | 'well-constrained' | 'over-constrained';

export type EdgeSelector = 'all' | 'top' | 'bottom' | 'vertical';

export interface ExtrudeParams {
  distance: number;
  mode: 'add' | 'cut';
  cutTargetId?: string;
}

export interface FilletParams {
  radius: number;
  edges: EdgeSelector;
}

export interface ChamferParams {
  distance: number;
  edges: EdgeSelector;
}

export interface Sketch {
  id: SketchId;
  name: string;
  plane: SketchPlane;
  origin: [number, number, number];
  entities: SketchEntity[];
  constraints: SketchConstraint[];
  closed: boolean;
  extrude?: ExtrudeParams;
  fillet?: FilletParams;
  chamfer?: ChamferParams;
  suppressed?: boolean;
}

export type SketchToolId =
  | 'sketch-select' | 'sketch-line' | 'sketch-rect' | 'sketch-circle' | 'sketch-arc'
  | 'sketch-constraint-coincident' | 'sketch-constraint-horizontal' | 'sketch-constraint-vertical'
  | 'sketch-constraint-parallel'   | 'sketch-constraint-perpendicular' | 'sketch-constraint-equal'
  | 'sketch-constraint-distance'   | 'sketch-constraint-radius'    | 'sketch-constraint-angle';

export interface BoxParams {
  type: 'box';
  width: number;
  depth: number;
  height: number;
}

export interface CylinderParams {
  type: 'cylinder';
  radius: number;
  height: number;
}

export interface SphereParams {
  type: 'sphere';
  radius: number;
}

export interface ConeParams {
  type: 'cone';
  bottomRadius: number;
  topRadius: number;
  height: number;
}

export type PrimitiveParams = BoxParams | CylinderParams | SphereParams | ConeParams;

export interface CadTransform {
  position: [number, number, number]; // CadQuery coords (Z-up)
  rotation: [number, number, number]; // degrees
}

export interface SceneObject {
  id: ObjectId;
  name: string;
  params: PrimitiveParams;
  transform: CadTransform;
  color: string;
  visible: boolean;
  locked: boolean;
  fillet?: FilletParams;
  chamfer?: ChamferParams;
  suppressed?: boolean;
}

// ─── Feature tree types ─────────────────────────
export type FeatureKind = 'primitive' | 'sketch';

export interface FeatureItem {
  id: string;
  kind: FeatureKind;
  name: string;
  icon: string;
  suppressed: boolean;
  detail: string;
}

export type CodeMode = 'parametric' | 'manual';

export interface CameraState {
  position: [number, number, number];
  target: [number, number, number];
  zoom: number;
}

export type ToolId =
  | 'select'
  | 'translate'
  | 'rotate'
  | 'scale'
  | 'add-box'
  | 'add-cylinder'
  | 'add-sphere'
  | 'add-cone';

export function getDefaultParams(type: PrimitiveType): PrimitiveParams {
  switch (type) {
    case 'box':
      return { type: 'box', width: 10, depth: 10, height: 10 };
    case 'cylinder':
      return { type: 'cylinder', radius: 5, height: 10 };
    case 'sphere':
      return { type: 'sphere', radius: 5 };
    case 'cone':
      return { type: 'cone', bottomRadius: 5, topRadius: 0, height: 10 };
  }
}

export function getDefaultTransform(): CadTransform {
  return { position: [0, 0, 0], rotation: [0, 0, 0] };
}
