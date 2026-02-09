import { writeTextFile, mkdir, remove, exists, BaseDirectory } from '@tauri-apps/plugin-fs';
import { getProjectStore } from '$lib/stores/project.svelte';
import { getChatStore } from '$lib/stores/chat.svelte';
import { getSceneStore } from '$lib/stores/scene.svelte';
import { getViewportStore } from '$lib/stores/viewport.svelte';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import type { RustChatMessage } from '$lib/types';

const AUTOSAVE_INTERVAL = 60_000; // 60 seconds
const DRAFTS_DIR = 'drafts';

let intervalId: ReturnType<typeof setInterval> | null = null;

function hashString(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash |= 0; // Convert to 32bit integer
  }
  return Math.abs(hash).toString(36);
}

function getDraftFilename(): string {
  const project = getProjectStore();
  const key = project.filePath ?? `unsaved-${document.title}`;
  return `${hashString(key)}.cadai.draft`;
}

function toRustMessages(): RustChatMessage[] {
  const chatStore = getChatStore();
  return chatStore.messages
    .filter((m) => m.role === 'user' || m.role === 'assistant')
    .map((m) => ({ role: m.role, content: m.content }));
}

async function performAutosave(): Promise<void> {
  const project = getProjectStore();
  if (!project.modified) return;

  try {
    // Ensure drafts directory exists
    const dirExists = await exists(DRAFTS_DIR, { baseDir: BaseDirectory.AppData });
    if (!dirExists) {
      await mkdir(DRAFTS_DIR, { baseDir: BaseDirectory.AppData, recursive: true });
    }

    const scene = getSceneStore();
    const viewportStore = getViewportStore();

    const sceneData = scene.serialize();
    const ftData = getFeatureTreeStore().serialize();
    const camera = viewportStore.getCameraState();

    const draftData = {
      name: project.name,
      code: project.code,
      messages: toRustMessages(),
      version: 2,
      scene: camera
        ? { objects: sceneData.objects, codeMode: sceneData.codeMode, camera, featureTree: ftData }
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
