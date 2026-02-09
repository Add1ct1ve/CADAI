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
import { clearDraft } from '$lib/services/autosave';
import type { RustChatMessage, ChatMessage } from '$lib/types';
import type { SceneObject, CodeMode, CameraState, Sketch, DatumPlane, DatumAxis } from '$lib/types/cad';
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
  getFeatureTreeStore().clearAll();
  scene.setCodeMode('parametric');
  getHistoryStore().clear();
  const viewportStore = getViewportStore();
  viewportStore.setPendingClear(true);
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
    const sceneData = file.scene as { objects: SceneObject[]; codeMode: CodeMode; camera: CameraState; sketches?: Sketch[]; featureTree?: FeatureTreeSnapshot; datumPlanes?: DatumPlane[]; datumAxes?: DatumAxis[] };
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
    // Clear viewport first so meshes get rebuilt from restored objects
    viewportStore.setPendingClear(true);
    // Restore camera after a tick to allow viewport to process
    if (sceneData.camera) {
      setTimeout(() => viewportStore.setCameraState(sceneData.camera), 50);
    }
  } else {
    // V1 file: just clear scene, keep manual mode
    scene.clearAll();
    sketchStore.clearAll();
    getDatumStore().clearAll();
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
  const camera = viewportStore.getCameraState();
  const scenePayload = camera
    ? { objects: sceneData.objects, codeMode: sceneData.codeMode, camera, sketches: sketchData.sketches, featureTree: ftData, datumPlanes: datumData.datumPlanes, datumAxes: datumData.datumAxes }
    : undefined;

  await saveProject(project.name, project.code, rustMessages, path, scenePayload);
  project.setFilePath(path);
  project.setModified(false);
  clearDraft().catch(() => {}); // Best-effort draft cleanup
  return 'Project saved';
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
