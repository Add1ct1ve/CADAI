/**
 * Shared project actions used by both Toolbar and keyboard shortcuts.
 * These functions operate on the global stores and Tauri services.
 */
import { getProjectStore } from '$lib/stores/project.svelte';
import { getChatStore } from '$lib/stores/chat.svelte';
import {
  saveProject,
  loadProject,
  exportStl,
  showSaveDialog,
  showOpenDialog,
} from '$lib/services/tauri';
import type { RustChatMessage, ChatMessage } from '$lib/types';

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

  if (project.modified) {
    const confirmed = window.confirm('You have unsaved changes. Create a new project anyway?');
    if (!confirmed) return '';
  }
  project.reset();
  chatStore.clear();
  return 'New project created';
}

export async function projectOpen(): Promise<string> {
  const project = getProjectStore();
  const chatStore = getChatStore();

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

  project.setModified(false);
  return `Opened: ${file.name}`;
}

export async function projectSave(): Promise<string> {
  const project = getProjectStore();
  const chatStore = getChatStore();

  let path = project.filePath;
  if (!path) {
    path = await showSaveDialog(project.name + '.cadai', 'cadai');
    if (!path) return '';
  }

  const rustMessages = toRustMessages(chatStore.messages);
  await saveProject(project.name, project.code, rustMessages, path);
  project.setFilePath(path);
  project.setModified(false);
  return 'Project saved';
}

export async function projectExportStl(): Promise<string> {
  const project = getProjectStore();

  const path = await showSaveDialog('model.stl', 'stl');
  if (!path) return '';

  const result = await exportStl(project.code, path);
  return result || 'STL exported successfully';
}
