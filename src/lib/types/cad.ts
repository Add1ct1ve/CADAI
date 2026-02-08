export type ObjectId = string;
export type PrimitiveType = 'box' | 'cylinder' | 'sphere' | 'cone';

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
