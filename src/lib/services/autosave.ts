import {
  BaseDirectory,
  exists,
  mkdir,
  readTextFile,
  remove,
  writeTextFile,
} from '@tauri-apps/plugin-fs';

import { getProjectStore } from '$lib/stores/project.svelte';
import { getChatStore } from '$lib/stores/chat.svelte';
import { getSceneStore } from '$lib/stores/scene.svelte';
import { getViewportStore } from '$lib/stores/viewport.svelte';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import { getSketchStore } from '$lib/stores/sketch.svelte';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';
import { getMateStore } from '$lib/stores/mate.svelte';
import { getDrawingStore } from '$lib/stores/drawing.svelte';
import type { ChatMessage, RustChatMessage } from '$lib/types';
import type { AutosaveSnapshotV2 } from '$lib/types/autosave';
import { migrateAutosaveSnapshot } from '$lib/types/autosave';

const AUTOSAVE_INTERVAL = 60_000; // 60 seconds
const DRAFTS_DIR = 'drafts';
const AUTOSAVE_VERSION = 2;

let intervalId: ReturnType<typeof setInterval> | null = null;

function hashString(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash |= 0;
  }
  return Math.abs(hash).toString(36);
}

function getDraftFilename(): string {
  const project = getProjectStore();
  const key = project.filePath ?? `unsaved-${document.title}`;
  return `${hashString(key)}.cadai.draft`;
}

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

function toRustMessages(): RustChatMessage[] {
  const chatStore = getChatStore();
  return chatStore.messages
    .filter((m) => m.role === 'user' || m.role === 'assistant')
    .map((m) => ({ role: m.role, content: m.content }));
}

function toChatMessages(messages: RustChatMessage[]): ChatMessage[] {
  return messages.map((msg) => ({
    id: generateId(),
    role: msg.role as 'user' | 'assistant',
    content: msg.content,
    timestamp: Date.now(),
  }));
}

function applyDraftSnapshot(snapshot: AutosaveSnapshotV2): void {
  const project = getProjectStore();
  const chatStore = getChatStore();
  const scene = getSceneStore();
  const viewportStore = getViewportStore();
  const sketchStore = getSketchStore();
  const featureTreeStore = getFeatureTreeStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();
  const drawingStore = getDrawingStore();

  project.setName(snapshot.name);
  project.setCode(snapshot.code);
  project.setModified(true);

  chatStore.clear();
  for (const message of toChatMessages(snapshot.messages)) {
    chatStore.addMessage(message);
  }

  if (!snapshot.scene) {
    scene.clearAll();
    sketchStore.clearAll();
    featureTreeStore.clearAll();
    datumStore.clearAll();
    componentStore.clearAll();
    mateStore.clearAll();
    drawingStore.clearAll();
    viewportStore.setPendingClear(true);
    return;
  }

  const sceneData = snapshot.scene;
  scene.restore({
    objects: sceneData.objects,
    codeMode: sceneData.codeMode,
  });
  sketchStore.restore({ sketches: sceneData.sketches });
  featureTreeStore.restore(sceneData.featureTree);
  datumStore.restore({
    datumPlanes: sceneData.datumPlanes,
    datumAxes: sceneData.datumAxes,
  });
  componentStore.restore({
    components: sceneData.components,
    nameCounter: sceneData.componentNameCounter,
  });
  mateStore.restore({ mates: sceneData.mates });
  drawingStore.restore({ drawings: sceneData.drawings });

  viewportStore.setPendingClear(true);
  setTimeout(() => {
    viewportStore.setCameraState(sceneData.camera);
  }, 50);
  setTimeout(() => {
    viewportStore.setDisplayMode(sceneData.displayMode);
  }, 60);
}

async function ensureDraftDir(): Promise<void> {
  const dirExists = await exists(DRAFTS_DIR, { baseDir: BaseDirectory.AppData });
  if (!dirExists) {
    await mkdir(DRAFTS_DIR, { baseDir: BaseDirectory.AppData, recursive: true });
  }
}

async function performAutosave(): Promise<void> {
  const project = getProjectStore();
  if (!project.modified) return;

  try {
    await ensureDraftDir();

    const scene = getSceneStore();
    const viewportStore = getViewportStore();
    const sketchData = getSketchStore().serialize();
    const featureTreeData = getFeatureTreeStore().serialize();
    const datumData = getDatumStore().serialize();
    const componentData = getComponentStore().serialize();
    const mateData = getMateStore().serialize();
    const drawingData = getDrawingStore().serialize();

    const sceneData = scene.serialize();
    const camera = viewportStore.getCameraState();

    const draftData: AutosaveSnapshotV2 = {
      version: AUTOSAVE_VERSION,
      savedAt: Date.now(),
      name: project.name,
      code: project.code,
      messages: toRustMessages(),
      scene: camera
        ? {
            objects: sceneData.objects,
            codeMode: sceneData.codeMode,
            camera,
            featureTree: featureTreeData,
            sketches: sketchData.sketches,
            datumPlanes: datumData.datumPlanes,
            datumAxes: datumData.datumAxes,
            displayMode: viewportStore.displayMode,
            components: componentData.components,
            componentNameCounter: componentData.nameCounter,
            mates: mateData.mates,
            drawings: drawingData.drawings,
          }
        : undefined,
    };

    const filename = getDraftFilename();
    await writeTextFile(
      `${DRAFTS_DIR}/${filename}`,
      JSON.stringify(draftData),
      { baseDir: BaseDirectory.AppData },
    );

    console.log(`[autosave] Draft saved: ${filename}`);
  } catch (err) {
    console.warn('[autosave] Failed to save draft:', err);
  }
}

async function readCurrentDraft(): Promise<AutosaveSnapshotV2 | null> {
  try {
    const filename = getDraftFilename();
    const path = `${DRAFTS_DIR}/${filename}`;
    const fileExists = await exists(path, { baseDir: BaseDirectory.AppData });
    if (!fileExists) return null;

    const rawText = await readTextFile(path, { baseDir: BaseDirectory.AppData });
    const parsed = JSON.parse(rawText);
    return migrateAutosaveSnapshot(parsed);
  } catch (err) {
    console.warn('[autosave] Failed to read draft:', err);
    return null;
  }
}

export async function restoreDraftIfPresent(): Promise<boolean> {
  const project = getProjectStore();
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const drawingStore = getDrawingStore();

  // Only auto-restore into a fresh session.
  if (project.modified) return false;
  if (scene.objects.length > 0) return false;
  if (sketchStore.sketches.length > 0) return false;
  if (drawingStore.drawings.length > 0) return false;

  const snapshot = await readCurrentDraft();
  if (!snapshot) return false;

  applyDraftSnapshot(snapshot);
  console.log('[autosave] Draft restored');
  return true;
}

export async function clearDraft(): Promise<void> {
  try {
    const filename = getDraftFilename();
    const path = `${DRAFTS_DIR}/${filename}`;
    const fileExists = await exists(path, { baseDir: BaseDirectory.AppData });
    if (fileExists) {
      await remove(path, { baseDir: BaseDirectory.AppData });
      console.log(`[autosave] Draft cleared: ${filename}`);
    }
  } catch (err) {
    console.warn('[autosave] Failed to clear draft:', err);
  }
}

export function startAutosave(): void {
  if (intervalId !== null) return;
  intervalId = setInterval(performAutosave, AUTOSAVE_INTERVAL);
  console.log('[autosave] Started (60s interval)');
}

export function stopAutosave(): void {
  if (intervalId !== null) {
    clearInterval(intervalId);
    intervalId = null;
    console.log('[autosave] Stopped');
  }
}

