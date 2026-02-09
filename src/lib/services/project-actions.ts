/**
 * Shared project actions used by both Toolbar and keyboard shortcuts.
 * These functions operate on the global stores and Tauri services.
 */
import { getProjectStore } from '$lib/stores/project.svelte';
import { getChatStore } from '$lib/stores/chat.svelte';
import { getViewportStore } from '$lib/stores/viewport.svelte';
import { getSceneStore } from '$lib/stores/scene.svelte';
import {
  saveProject,
  loadProject,
  exportStl,
  exportStep,
  showSaveDialog,
  showOpenDialog,
} from '$lib/services/tauri';
import { getHistoryStore } from '$lib/stores/history.svelte';
import { getSketchStore } from '$lib/stores/sketch.svelte';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';
import { clearDraft } from '$lib/services/autosave';
import type { RustChatMessage, ChatMessage } from '$lib/types';
import type { SceneObject, CodeMode, CameraState, Sketch, DatumPlane, DatumAxis, DisplayMode, Component, SketchEntity, SketchConstraint } from '$lib/types/cad';
import type { FeatureTreeSnapshot } from '$lib/stores/feature-tree.svelte';

/**
 * Convert chat messages to the Rust format for project saving.
 */
function toRustMessages(messages: ChatMessage[]): RustChatMessage[] {
  return messages
    .filter((m) => m.role === 'user' || m.role === 'assistant')
    .map((m) => ({ role: m.role, content: m.content }));
}

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

export async function projectNew(): Promise<string> {
  const project = getProjectStore();
  const chatStore = getChatStore();
  const scene = getSceneStore();
  const sketchStore = getSketchStore();

  if (project.modified) {
    const confirmed = window.confirm('You have unsaved changes. Create a new project anyway?');
    if (!confirmed) return '';
  }
  project.reset();
  chatStore.clear();
  scene.clearAll();
  sketchStore.clearAll();
  getDatumStore().clearAll();
  getComponentStore().clearAll();
  getFeatureTreeStore().clearAll();
  scene.setCodeMode('parametric');
  getHistoryStore().clear();
  const viewportStore = getViewportStore();
  viewportStore.setPendingClear(true);
  viewportStore.setDisplayMode('shaded');
  return 'New project created';
}

export async function projectOpen(): Promise<string> {
  const project = getProjectStore();
  const chatStore = getChatStore();
  const scene = getSceneStore();
  const viewportStore = getViewportStore();

  const path = await showOpenDialog('cadai');
  if (!path) return '';

  const file = await loadProject(path);
  project.setName(file.name);
  project.setCode(file.code);
  project.setFilePath(path);

  // Convert RustChatMessages back into ChatMessages for the chat store
  chatStore.clear();
  for (const msg of file.messages) {
    chatStore.addMessage({
      id: generateId(),
      role: msg.role as 'user' | 'assistant',
      content: msg.content,
      timestamp: Date.now(),
    });
  }

  // Restore scene state from v2 format
  const sketchStore = getSketchStore();
  const ftStore = getFeatureTreeStore();
  if (file.scene) {
    const sceneData = file.scene as { objects: SceneObject[]; codeMode: CodeMode; camera: CameraState; sketches?: Sketch[]; featureTree?: FeatureTreeSnapshot; datumPlanes?: DatumPlane[]; datumAxes?: DatumAxis[]; displayMode?: DisplayMode; components?: Component[]; componentNameCounter?: number };
    scene.restore({ objects: sceneData.objects, codeMode: sceneData.codeMode });
    // Restore sketches if present
    if (sceneData.sketches) {
      sketchStore.restore({ sketches: sceneData.sketches });
    } else {
      sketchStore.clearAll();
    }
    // Restore feature tree if present, else sync from stores (backward compat)
    if (sceneData.featureTree) {
      ftStore.restore(sceneData.featureTree);
    } else {
      ftStore.clearAll();
      ftStore.syncFromStores();
    }
    // Restore datum geometry if present
    if (sceneData.datumPlanes || sceneData.datumAxes) {
      getDatumStore().restore({ datumPlanes: sceneData.datumPlanes ?? [], datumAxes: sceneData.datumAxes ?? [] });
    } else {
      getDatumStore().clearAll();
    }
    // Restore components if present
    if (sceneData.components) {
      getComponentStore().restore({ components: sceneData.components, nameCounter: sceneData.componentNameCounter });
    } else {
      getComponentStore().clearAll();
    }
    // Clear viewport first so meshes get rebuilt from restored objects
    viewportStore.setPendingClear(true);
    // Restore camera after a tick to allow viewport to process
    if (sceneData.camera) {
      setTimeout(() => viewportStore.setCameraState(sceneData.camera), 50);
    }
    // Restore display mode after camera
    if (sceneData.displayMode) {
      setTimeout(() => viewportStore.setDisplayMode(sceneData.displayMode!), 60);
    }
  } else {
    // V1 file: just clear scene, keep manual mode
    scene.clearAll();
    sketchStore.clearAll();
    getDatumStore().clearAll();
    getComponentStore().clearAll();
    ftStore.clearAll();
    scene.setCodeMode('manual');
    viewportStore.setPendingClear(true);
  }

  getHistoryStore().clear();
  project.setModified(false);
  return `Opened: ${file.name}`;
}

export async function projectSave(): Promise<string> {
  const project = getProjectStore();
  const chatStore = getChatStore();
  const scene = getSceneStore();
  const viewportStore = getViewportStore();

  let path = project.filePath;
  if (!path) {
    path = await showSaveDialog(project.name + '.cadai', 'cadai');
    if (!path) return '';
  }

  const rustMessages = toRustMessages(chatStore.messages);

  // Build scene snapshot for v2 format
  const sceneData = scene.serialize();
  const sketchData = getSketchStore().serialize();
  const ftData = getFeatureTreeStore().serialize();
  const datumData = getDatumStore().serialize();
  const compData = getComponentStore().serialize();
  const camera = viewportStore.getCameraState();
  const scenePayload = camera
    ? { objects: sceneData.objects, codeMode: sceneData.codeMode, camera, sketches: sketchData.sketches, featureTree: ftData, datumPlanes: datumData.datumPlanes, datumAxes: datumData.datumAxes, displayMode: viewportStore.displayMode, components: compData.components, componentNameCounter: compData.nameCounter }
    : undefined;

  await saveProject(project.name, project.code, rustMessages, path, scenePayload);
  project.setFilePath(path);
  project.setModified(false);
  clearDraft().catch(() => {}); // Best-effort draft cleanup
  return 'Project saved';
}

// ─── Insert Component from .cadai file ─────────

function reIdImportData(
  objects: SceneObject[],
  sketches: Sketch[],
  datumPlanes: DatumPlane[],
  datumAxes: DatumAxis[],
) {
  const prefix = Date.now().toString(36) + Math.random().toString(36).slice(2, 5) + '_';
  const idMap = new Map<string, string>();

  // Build ID map for all entities
  for (const obj of objects) idMap.set(obj.id, prefix + obj.id);
  for (const sk of sketches) {
    idMap.set(sk.id, prefix + sk.id);
    for (const e of sk.entities) idMap.set(e.id, prefix + e.id);
    for (const c of (sk.constraints ?? [])) idMap.set(c.id, prefix + c.id);
  }
  for (const dp of datumPlanes) idMap.set(dp.id, prefix + dp.id);
  for (const da of datumAxes) idMap.set(da.id, prefix + da.id);

  const remap = (id: string) => idMap.get(id) ?? id;

  // Remap objects
  const newObjects: SceneObject[] = objects.map((obj) => ({
    ...structuredClone(obj),
    id: remap(obj.id),
    booleanOp: obj.booleanOp
      ? { ...obj.booleanOp, targetId: remap(obj.booleanOp.targetId) }
      : undefined,
  }));

  // Remap sketches
  const newSketches: Sketch[] = sketches.map((sk) => {
    const newSk = structuredClone(sk);
    newSk.id = remap(sk.id);
    newSk.entities = newSk.entities.map((e: SketchEntity) => ({ ...e, id: remap(e.id) }));
    newSk.constraints = (newSk.constraints ?? []).map((c: SketchConstraint) => {
      const nc = { ...c, id: remap(c.id) } as any;
      // Remap entity references in constraints
      if ('entityId' in nc) nc.entityId = remap(nc.entityId);
      if ('entityId1' in nc) nc.entityId1 = remap(nc.entityId1);
      if ('entityId2' in nc) nc.entityId2 = remap(nc.entityId2);
      if ('point1' in nc && nc.point1?.entityId) nc.point1 = { ...nc.point1, entityId: remap(nc.point1.entityId) };
      if ('point2' in nc && nc.point2?.entityId) nc.point2 = { ...nc.point2, entityId: remap(nc.point2.entityId) };
      return nc as SketchConstraint;
    });
    // Remap operation references
    if (newSk.operation) {
      if ('cutTargetId' in newSk.operation && newSk.operation.cutTargetId) {
        (newSk.operation as any).cutTargetId = remap(newSk.operation.cutTargetId!);
      }
      if ('pathSketchId' in newSk.operation && (newSk.operation as any).pathSketchId) {
        (newSk.operation as any).pathSketchId = remap((newSk.operation as any).pathSketchId);
      }
    }
    return newSk;
  });

  // Remap datums
  const newDatumPlanes: DatumPlane[] = datumPlanes.map((dp) => ({
    ...structuredClone(dp),
    id: remap(dp.id),
  }));

  const newDatumAxes: DatumAxis[] = datumAxes.map((da) => ({
    ...structuredClone(da),
    id: remap(da.id),
  }));

  const featureIds = [
    ...newObjects.map((o) => o.id),
    ...newSketches.map((s) => s.id),
    ...newDatumPlanes.map((d) => d.id),
    ...newDatumAxes.map((d) => d.id),
  ];

  return { objects: newObjects, sketches: newSketches, datumPlanes: newDatumPlanes, datumAxes: newDatumAxes, featureIds };
}

export async function projectInsertComponent(): Promise<string> {
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const featureTree = getFeatureTreeStore();
  const history = getHistoryStore();

  const path = await showOpenDialog('cadai');
  if (!path) return '';

  const file = await loadProject(path);
  if (!file.scene) return 'File has no scene data';

  const sceneData = file.scene as {
    objects?: SceneObject[];
    sketches?: Sketch[];
    datumPlanes?: DatumPlane[];
    datumAxes?: DatumAxis[];
  };

  // Re-ID everything to avoid collisions
  const { objects, sketches, datumPlanes, datumAxes, featureIds } = reIdImportData(
    sceneData.objects ?? [],
    sceneData.sketches ?? [],
    sceneData.datumPlanes ?? [],
    sceneData.datumAxes ?? [],
  );

  if (featureIds.length === 0) return 'File has no features to import';

  // Push undo snapshot
  const sceneSnap = scene.snapshot();
  const sketchSnap = sketchStore.snapshot();
  const ftSnap = featureTree.snapshot();
  const datumSnap = datumStore.snapshot();
  const compSnap = componentStore.snapshot();
  history.pushSnapshot({
    ...sceneSnap,
    sketches: sketchSnap.sketches,
    activeSketchId: sketchSnap.activeSketchId,
    selectedSketchId: sketchSnap.selectedSketchId,
    featureTree: ftSnap,
    datumPlanes: datumSnap.datumPlanes,
    datumAxes: datumSnap.datumAxes,
    selectedDatumId: datumSnap.selectedDatumId,
    components: compSnap.components,
    componentNameCounter: compSnap.nameCounter,
    selectedComponentId: compSnap.selectedComponentId,
  });

  // Add all entities to their respective stores
  if (objects.length > 0) scene.addObjects(objects);
  if (sketches.length > 0) sketchStore.addSketches(sketches);
  if (datumPlanes.length > 0 || datumAxes.length > 0) datumStore.addDatums(datumPlanes, datumAxes);

  // Create component with featureIds
  const fileName = path.split(/[\\/]/).pop()?.replace('.cadai', '') ?? 'Imported';
  const comp = componentStore.createComponent(featureIds, fileName);
  comp.sourceFile = path;
  componentStore.updateComponent(comp.id, { sourceFile: path });
  featureTree.registerComponent(comp.id);

  return `Inserted component: ${comp.name} (${featureIds.length} features)`;
}

export async function projectExportStl(): Promise<string> {
  const project = getProjectStore();

  const path = await showSaveDialog('model.stl', 'stl');
  if (!path) return '';

  const result = await exportStl(project.code, path);
  return result || 'STL exported successfully';
}

export async function projectExportStep(): Promise<string> {
  const project = getProjectStore();

  const path = await showSaveDialog('model.step', 'step');
  if (!path) return '';

  const result = await exportStep(project.code, path);
  return result || 'STEP exported successfully';
}
