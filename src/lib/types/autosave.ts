import type { RustChatMessage } from '$lib/types';
import type {
  CameraState,
  CodeMode,
  Component,
  DatumAxis,
  DatumPlane,
  DisplayMode,
  SceneObject,
  Sketch,
  AssemblyMate,
} from '$lib/types/cad';
import type { Drawing } from '$lib/types/drawing';
import type { FeatureTreeSnapshot } from '$lib/stores/feature-tree.svelte';

export interface AutosaveSceneSnapshotV2 {
  objects: SceneObject[];
  codeMode: CodeMode;
  camera: CameraState;
  featureTree: FeatureTreeSnapshot;
  sketches: Sketch[];
  datumPlanes: DatumPlane[];
  datumAxes: DatumAxis[];
  displayMode: DisplayMode;
  components: Component[];
  componentNameCounter: number;
  mates: AssemblyMate[];
  drawings: Drawing[];
}

export interface AutosaveSnapshotV2 {
  version: 2;
  savedAt: number;
  name: string;
  code: string;
  messages: RustChatMessage[];
  scene?: AutosaveSceneSnapshotV2;
}

interface AutosaveSnapshotLegacy {
  version?: number;
  name?: string;
  code?: string;
  messages?: RustChatMessage[];
  scene?: {
    objects?: SceneObject[];
    codeMode?: CodeMode;
    camera?: CameraState;
    featureTree?: FeatureTreeSnapshot;
    sketches?: Sketch[];
    datumPlanes?: DatumPlane[];
    datumAxes?: DatumAxis[];
    displayMode?: DisplayMode;
    components?: Component[];
    componentNameCounter?: number;
    mates?: AssemblyMate[];
    drawings?: Drawing[];
  };
}

export function migrateAutosaveSnapshot(raw: unknown): AutosaveSnapshotV2 | null {
  if (!raw || typeof raw !== 'object') return null;

  const legacy = raw as AutosaveSnapshotLegacy;
  const version = legacy.version ?? 1;

  if (version >= 2) {
    return legacy as AutosaveSnapshotV2;
  }

  return {
    version: 2,
    savedAt: Date.now(),
    name: legacy.name ?? 'Untitled',
    code: legacy.code ?? '',
    messages: Array.isArray(legacy.messages) ? legacy.messages : [],
    scene: legacy.scene && legacy.scene.camera
      ? {
          objects: legacy.scene.objects ?? [],
          codeMode: legacy.scene.codeMode ?? 'manual',
          camera: legacy.scene.camera,
          featureTree: legacy.scene.featureTree ?? {
            featureOrder: [],
            suppressedIds: [],
            rollbackIndex: null,
          },
          sketches: legacy.scene.sketches ?? [],
          datumPlanes: legacy.scene.datumPlanes ?? [],
          datumAxes: legacy.scene.datumAxes ?? [],
          displayMode: legacy.scene.displayMode ?? 'shaded',
          components: legacy.scene.components ?? [],
          componentNameCounter: legacy.scene.componentNameCounter ?? 0,
          mates: legacy.scene.mates ?? [],
          drawings: legacy.scene.drawings ?? [],
        }
      : undefined,
  };
}

