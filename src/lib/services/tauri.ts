import { invoke, Channel } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import type { AppConfig, ExecuteResult, PythonStatus, StreamEvent, RustChatMessage, AutoRetryResult, ProjectFile, ProviderInfo, MultiPartEvent } from '$lib/types';

/**
 * Test IPC with a greeting
 */
export async function greet(name: string): Promise<string> {
  try {
    return await invoke<string>('greet', { name });
  } catch (err) {
    console.error('greet failed:', err);
    throw new Error(`Greet failed: ${err}`);
  }
}

/**
 * Send a chat message (non-streaming fallback)
 */
export async function sendMessage(message: string): Promise<string> {
  try {
    return await invoke<string>('send_message', { message });
  } catch (err) {
    console.error('send_message failed:', err);
    throw new Error(`Send message failed: ${err}`);
  }
}

/**
 * Send a chat message with streaming response via Tauri Channel API.
 * Streams delta events as they arrive, then returns the full response.
 */
export async function sendMessageStreaming(
  message: string,
  history: RustChatMessage[],
  onDelta: (delta: string, done: boolean) => void,
): Promise<string> {
  try {
    const onEvent = new Channel<StreamEvent>();
    onEvent.onmessage = (event) => {
      onDelta(event.delta, event.done);
    };

    const result = await invoke<string>('send_message', {
      message,
      history,
      onEvent,
    });

    return result;
  } catch (err) {
    console.error('send_message streaming failed:', err);
    throw new Error(`Send message failed: ${err}`);
  }
}

/**
 * Ask the AI to fix code that failed execution, with streaming response.
 * Returns the AutoRetryResult with the new code (if any) and AI explanation.
 */
export async function autoRetry(
  failedCode: string,
  errorMessage: string,
  history: RustChatMessage[],
  attempt: number,
  onDelta: (delta: string, done: boolean) => void,
): Promise<AutoRetryResult> {
  try {
    const onEvent = new Channel<StreamEvent>();
    onEvent.onmessage = (event) => {
      onDelta(event.delta, event.done);
    };

    const result = await invoke<AutoRetryResult>('auto_retry', {
      failedCode,
      errorMessage,
      history,
      attempt,
      onEvent,
    });

    return result;
  } catch (err) {
    console.error('auto_retry failed:', err);
    throw new Error(`Auto retry failed: ${err}`);
  }
}

/**
 * Send a chat message through the parallel generation pipeline.
 * The planner decides whether to use single or multi-part generation.
 * Events are forwarded via the onEvent callback.
 */
export async function generateParallel(
  message: string,
  history: RustChatMessage[],
  onEvent: (event: MultiPartEvent) => void,
): Promise<string> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    const result = await invoke<string>('generate_parallel', {
      message,
      history,
      onEvent: channel,
    });

    return result;
  } catch (err) {
    console.error('generate_parallel failed:', err);
    throw new Error(`Generate parallel failed: ${err}`);
  }
}

/**
 * Extract Python code from a markdown response containing ```python code blocks.
 * Returns the first matched code block content, or null if none found.
 */
export function extractPythonCode(text: string): string | null {
  const match = text.match(/```python\s*\n([\s\S]*?)```/);
  return match ? match[1].trim() : null;
}

/**
 * Execute CadQuery code via the Python backend
 */
export async function executeCode(code: string): Promise<ExecuteResult> {
  try {
    return await invoke<ExecuteResult>('execute_code', { code });
  } catch (err) {
    console.error('execute_code failed:', err);
    throw new Error(`Execute code failed: ${err}`);
  }
}

/**
 * Check Python environment status (python, venv, cadquery)
 */
export async function checkPython(): Promise<PythonStatus> {
  try {
    return await invoke<PythonStatus>('check_python');
  } catch (err) {
    console.error('check_python failed:', err);
    throw new Error(`Check Python failed: ${err}`);
  }
}

/**
 * Set up Python virtual environment and install CadQuery
 */
export async function setupPython(): Promise<string> {
  try {
    return await invoke<string>('setup_python');
  } catch (err) {
    console.error('setup_python failed:', err);
    throw new Error(`Setup Python failed: ${err}`);
  }
}

/**
 * Get the provider/model registry
 */
export async function getProviderRegistry(): Promise<ProviderInfo[]> {
  try {
    return await invoke<ProviderInfo[]>('get_provider_registry');
  } catch (err) {
    console.error('get_provider_registry failed:', err);
    throw new Error(`Get provider registry failed: ${err}`);
  }
}

/**
 * Get application settings
 */
export async function getSettings(): Promise<AppConfig> {
  try {
    return await invoke<AppConfig>('get_settings');
  } catch (err) {
    console.error('get_settings failed:', err);
    throw new Error(`Get settings failed: ${err}`);
  }
}

/**
 * Update application settings
 */
export async function updateSettings(config: AppConfig): Promise<void> {
  try {
    await invoke('update_settings', { config });
  } catch (err) {
    console.error('update_settings failed:', err);
    throw new Error(`Update settings failed: ${err}`);
  }
}

/**
 * Save project to a file
 */
export async function saveProject(name: string, code: string, messages: RustChatMessage[], path: string, scene?: unknown): Promise<void> {
  try {
    await invoke('save_project', { name, code, messages, path, scene: scene ?? null });
  } catch (err) {
    console.error('save_project failed:', err);
    throw new Error(`Save project failed: ${err}`);
  }
}

/**
 * Load project from a file
 */
export async function loadProject(path: string): Promise<ProjectFile> {
  try {
    return await invoke<ProjectFile>('load_project', { path });
  } catch (err) {
    console.error('load_project failed:', err);
    throw new Error(`Load project failed: ${err}`);
  }
}

/**
 * Export STL: run CadQuery code and save the resulting STL to a file
 */
export async function exportStl(code: string, outputPath: string): Promise<string> {
  try {
    return await invoke<string>('export_stl', { code, outputPath });
  } catch (err) {
    console.error('export_stl failed:', err);
    throw new Error(`Export STL failed: ${err}`);
  }
}

/**
 * Export STEP: run CadQuery code and save the resulting STEP to a file
 */
export async function exportStep(code: string, outputPath: string): Promise<string> {
  try {
    return await invoke<string>('export_step', { code, outputPath });
  } catch (err) {
    console.error('export_step failed:', err);
    throw new Error(`Export STEP failed: ${err}`);
  }
}

/**
 * Show a native save file dialog
 */
export async function showSaveDialog(defaultName: string, extension: string): Promise<string | null> {
  try {
    const result = await save({
      title: `Save ${extension.toUpperCase()} File`,
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      defaultPath: defaultName,
    });
    return result;
  } catch (err) {
    console.error('showSaveDialog failed:', err);
    return null;
  }
}

/**
 * Show a native open file dialog
 */
export async function showOpenDialog(extension: string): Promise<string | null> {
  try {
    const result = await open({
      title: 'Open File',
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      multiple: false,
    });
    return typeof result === 'string' ? result : null;
  } catch (err) {
    console.error('showOpenDialog failed:', err);
    return null;
  }
}
