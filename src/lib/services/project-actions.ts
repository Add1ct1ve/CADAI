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
import { clearDraft } from '$lib/services/autosave';
import type { RustChatMessage, ChatMessage } from '$lib/types';
import type { SceneObject, CodeMode, CameraState } from '$lib/types/cad';

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

  if (project.modified) {
    const confirmed = window.confirm('You have unsaved changes. Create a new project anyway?');
    if (!confirmed) return '';
  }
  project.reset();
  chatStore.clear();
  scene.clearAll();
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
  if (file.scene) {
    const sceneData = file.scene as { objects: SceneObject[]; codeMode: CodeMode; camera: CameraState };
    scene.restore({ objects: sceneData.objects, codeMode: sceneData.codeMode });
    // Clear viewport first so meshes get rebuilt from restored objects
    viewportStore.setPendingClear(true);
    // Restore camera after a tick to allow viewport to process
    if (sceneData.camera) {
      setTimeout(() => viewportStore.setCameraState(sceneData.camera), 50);
    }
  } else {
    // V1 file: just clear scene, keep manual mode
    scene.clearAll();
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
  const camera = viewportStore.getCameraState();
  const scenePayload = camera
    ? { objects: sceneData.objects, codeMode: sceneData.codeMode, camera }
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
