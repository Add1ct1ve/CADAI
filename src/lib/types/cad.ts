export type ObjectId = string;
export type ComponentId = string;
export type PrimitiveType = 'box' | 'cylinder' | 'sphere' | 'cone';

// ─── Display modes ───────────────────────────────
export type DisplayMode = 'shaded' | 'wireframe' | 'shaded-edges' | 'transparent' | 'section';

export interface SectionPlaneConfig {
  enabled: boolean;
  normal: [number, number, number]; // Three.js Y-up coords
  offset: number;                   // distance along normal
}

// ─── Sketch types ────────────────────────────────
export type SketchId = string;
export type SketchEntityId = string;
export type SketchPlane = 'XY' | 'XZ' | 'YZ' | string; // string = DatumId referencing a datum plane
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
export type FaceSelector = '>Z' | '<Z' | '>X' | '<X' | '>Y' | '<Y' | '%Cylinder';

export interface ExtrudeParams {
  type: 'extrude';
  distance: number;
  mode: 'add' | 'cut';
  cutTargetId?: string;
  taper?: number;  // draft angle in degrees (0 = no taper)
}

export interface RevolveParams {
  type: 'revolve';
  angle: number;              // degrees (default 360)
  mode: 'add' | 'cut';
  cutTargetId?: string;
  axisDirection: 'X' | 'Y';  // axis relative to sketch plane
  axisOffset: number;         // perpendicular distance from origin to axis
}

export interface SweepParams {
  type: 'sweep';
  mode: 'add' | 'cut';
  cutTargetId?: string;
  pathSketchId: string;       // reference to path sketch
}

export type SketchOperation = ExtrudeParams | RevolveParams | SweepParams;

export interface FilletParams {
  radius: number;
  edges: EdgeSelector;
}

export interface ChamferParams {
  distance: number;
  edges: EdgeSelector;
}

export interface ShellParams {
  thickness: number;          // negative = inward
  face: FaceSelector;         // which face to remove/open
}

export type HoleType = 'through' | 'blind' | 'counterbore' | 'countersink';

export interface HoleParams {
  holeType: HoleType;
  diameter: number;
  depth?: number;             // blind only
  cboreDiameter?: number;     // counterbore
  cboreDepth?: number;        // counterbore
  cskDiameter?: number;       // countersink
  cskAngle?: number;          // countersink (default 82)
  position: [number, number]; // X,Y on target face
  face: FaceSelector;
}

// ─── Boolean / Split types ──────────────────────
export type BooleanOpType = 'union' | 'subtract' | 'intersect';

export interface BooleanOp {
  type: BooleanOpType;
  targetId: ObjectId;
}

export type SplitPlane = 'XY' | 'XZ' | 'YZ';

export interface SplitOp {
  plane: SplitPlane;
  offset: number;
  keepSide: 'positive' | 'negative' | 'both';
}

// ─── Pattern types ─────────────────────────────
export type PatternType = 'mirror' | 'linear' | 'circular';

export interface MirrorPattern {
  type: 'mirror';
  plane: 'XY' | 'XZ' | 'YZ';
  offset: number;
  keepOriginal: boolean;
}

export interface LinearPattern {
  type: 'linear';
  direction: 'X' | 'Y' | 'Z';
  spacing: number;
  count: number;
}

export interface CircularPattern {
  type: 'circular';
  axis: 'X' | 'Y' | 'Z';
  count: number;
  fullAngle: number;
}

export type PatternOp = MirrorPattern | LinearPattern | CircularPattern;

// ─── Datum / Reference geometry types ───────────
export type DatumId = string;

export type DatumPlaneDefinition =
  | { type: 'offset'; basePlane: 'XY' | 'XZ' | 'YZ'; offset: number }
  | { type: 'threePoint'; p1: [number, number, number]; p2: [number, number, number]; p3: [number, number, number] };

export interface DatumPlane {
  id: DatumId;
  name: string;
  definition: DatumPlaneDefinition;
  color: string;
  visible: boolean;
}

export interface DatumAxis {
  id: DatumId;
  name: string;
  origin: [number, number, number];
  direction: [number, number, number]; // unit vector
  color: string;
  visible: boolean;
}

export function isDatumPlane(d: DatumPlane | DatumAxis): d is DatumPlane {
  return 'definition' in d;
}
export function isDatumAxis(d: DatumPlane | DatumAxis): d is DatumAxis {
  return 'direction' in d && !('definition' in d);
}

export interface Sketch {
  id: SketchId;
  name: string;
  plane: SketchPlane;
  origin: [number, number, number];
  entities: SketchEntity[];
  constraints: SketchConstraint[];
  closed: boolean;
  operation?: SketchOperation;  // replaces extrude?: ExtrudeParams
  fillet?: FilletParams;
  chamfer?: ChamferParams;
  shell?: ShellParams;
  holes?: HoleParams[];
  suppressed?: boolean;
}

export type SketchToolId =
  | 'sketch-select' | 'sketch-line' | 'sketch-rect' | 'sketch-circle' | 'sketch-arc'
  | 'sketch-constraint-coincident' | 'sketch-constraint-horizontal' | 'sketch-constraint-vertical'
  | 'sketch-constraint-parallel'   | 'sketch-constraint-perpendicular' | 'sketch-constraint-equal'
  | 'sketch-constraint-distance'   | 'sketch-constraint-radius'    | 'sketch-constraint-angle'
  | 'sketch-op-trim' | 'sketch-op-extend' | 'sketch-op-offset'
  | 'sketch-op-mirror' | 'sketch-op-fillet' | 'sketch-op-chamfer';

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

export type MaterialId = string;

export interface SceneObject {
  id: ObjectId;
  name: string;
  params: PrimitiveParams;
  transform: CadTransform;
  color: string;
  materialId?: MaterialId;
  metalness?: number;
  roughness?: number;
  opacity?: number;
  visible: boolean;
  locked: boolean;
  fillet?: FilletParams;
  chamfer?: ChamferParams;
  shell?: ShellParams;
  holes?: HoleParams[];
  booleanOp?: BooleanOp;
  splitOp?: SplitOp;
  patternOp?: PatternOp;
  suppressed?: boolean;
}

// ─── Component types ────────────────────────────
export interface Component {
  id: ComponentId;
  name: string;
  featureIds: string[];           // IDs of objects, sketches, datums in this component
  transform: CadTransform;        // component-level position in assembly
  visible: boolean;               // hide/show all children
  grounded: boolean;              // lock transform (cannot be moved)
  color: string;                  // badge color in tree
  sourceFile?: string;            // path if imported from .cadai file
}

// ─── Assembly Mate types ────────────────────────
export type MateId = string;
export type MateType = 'coincident' | 'concentric' | 'distance' | 'angle';

export interface MateReference {
  componentId: ComponentId;
  featureId: string;          // ObjectId within the component
  faceSelector: FaceSelector;
}

export interface CoincidentMate {
  type: 'coincident';
  id: MateId;
  name: string;
  ref1: MateReference;
  ref2: MateReference;
  flipped: boolean;
}

export interface ConcentricMate {
  type: 'concentric';
  id: MateId;
  name: string;
  ref1: MateReference;
  ref2: MateReference;
}

export interface DistanceMate {
  type: 'distance';
  id: MateId;
  name: string;
  ref1: MateReference;
  ref2: MateReference;
  distance: number;
}

export interface AngleMate {
  type: 'angle';
  id: MateId;
  name: string;
  ref1: MateReference;
  ref2: MateReference;
  angle: number;
}

export type AssemblyMate = CoincidentMate | ConcentricMate | DistanceMate | AngleMate;

export interface InterferenceResult {
  componentA: ComponentId;
  componentB: ComponentId;
  hasInterference: boolean;
  volume?: number;
}

// ─── Feature tree types ─────────────────────────
export type FeatureKind = 'primitive' | 'sketch' | 'datum-plane' | 'datum-axis' | 'component' | 'mate';

export interface FeatureItem {
  id: string;
  kind: FeatureKind;
  name: string;
  icon: string;
  suppressed: boolean;
  detail: string;
  componentId?: ComponentId;  // which component this belongs to (undefined = root)
  depth: number;              // 0 = root, 1 = child of component
  color?: string;             // component badge color
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
  | 'add-cone'
  | 'measure';

// ─── Measurement types ──────────────────────────
export type MeasureToolId = 'measure-distance' | 'measure-angle' | 'measure-radius' | 'measure-bbox';
export type MeasurementId = string;

export interface MeasurePoint {
  worldPos: [number, number, number]; // Three.js Y-up coords
  objectId?: ObjectId;
}

export interface DistanceMeasurement {
  type: 'distance';
  id: MeasurementId;
  point1: MeasurePoint;
  point2: MeasurePoint;
  distance: number;
}

export interface AngleMeasurement {
  type: 'angle';
  id: MeasurementId;
  vertex: MeasurePoint;
  arm1: MeasurePoint;
  arm2: MeasurePoint;
  angleDegrees: number;
}

export interface RadiusMeasurement {
  type: 'radius';
  id: MeasurementId;
  center: [number, number, number];
  objectId: ObjectId;
  radius: number;
}

export interface BBoxMeasurement {
  type: 'bbox';
  id: MeasurementId;
  objectId: ObjectId;
  min: [number, number, number];
  max: [number, number, number];
  sizeX: number;
  sizeY: number;
  sizeZ: number;
}

export type Measurement = DistanceMeasurement | AngleMeasurement | RadiusMeasurement | BBoxMeasurement;

export interface MassProperties {
  volume: number;
  surfaceArea: number;
  centerOfMass: [number, number, number]; // CadQuery Z-up
  density?: number;    // g/cm³
  mass?: number;       // density × volume
}

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
