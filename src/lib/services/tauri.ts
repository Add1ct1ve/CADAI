import { invoke, Channel } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import type { AppConfig, ExecuteResult, PythonStatus, StreamEvent, RustChatMessage, AutoRetryResult, ProjectFile, ProviderInfo, MultiPartEvent, TokenUsageData, SkippedStepInfo, DesignPlanResult, PartSpec } from '$lib/types';

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
  onTokenUsage?: (usage: TokenUsageData) => void,
): Promise<string> {
  try {
    const onEvent = new Channel<StreamEvent>();
    onEvent.onmessage = (event) => {
      if (event.event_type === 'token_usage' && event.token_usage && onTokenUsage) {
        onTokenUsage(event.token_usage);
      } else {
        onDelta(event.delta, event.done);
      }
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
  onTokenUsage?: (usage: TokenUsageData) => void,
): Promise<AutoRetryResult> {
  try {
    const onEvent = new Channel<StreamEvent>();
    onEvent.onmessage = (event) => {
      if (event.event_type === 'token_usage' && event.token_usage && onTokenUsage) {
        onTokenUsage(event.token_usage);
      } else {
        onDelta(event.delta, event.done);
      }
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
  existingCode?: string | null,
): Promise<string> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    const result = await invoke<string>('generate_parallel', {
      message,
      history,
      existingCode: existingCode ?? null,
      onEvent: channel,
    });

    return result;
  } catch (err) {
    console.error('generate_parallel failed:', err);
    throw new Error(`Generate parallel failed: ${err}`);
  }
}

/**
 * Retry skipped steps from an iterative build.
 * Sends the current code and skipped step info to the backend, which
 * re-runs the iterative build loop for only the skipped steps.
 */
export async function retrySkippedSteps(
  currentCode: string,
  skippedSteps: SkippedStepInfo[],
  designPlanText: string,
  userRequest: string,
  onEvent: (event: MultiPartEvent) => void,
): Promise<string> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    const result = await invoke<string>('retry_skipped_steps', {
      currentCode,
      skippedSteps,
      designPlanText,
      userRequest,
      onEvent: channel,
    });

    return result;
  } catch (err) {
    console.error('retry_skipped_steps failed:', err);
    throw new Error(`Retry skipped steps failed: ${err}`);
  }
}

/**
 * Retry generation of a single failed part in a multi-part assembly.
 */
export async function retryPart(
  partIndex: number,
  partSpec: PartSpec,
  designPlanText: string,
  userRequest: string,
  onEvent: (event: MultiPartEvent) => void,
): Promise<string> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    const result = await invoke<string>('retry_part', {
      partIndex,
      partSpec,
      designPlanText,
      userRequest,
      onEvent: channel,
    });

    return result;
  } catch (err) {
    console.error('retry_part failed:', err);
    throw new Error(`Retry part failed: ${err}`);
  }
}

/**
 * Generate only the design plan (Phase 0). Returns the plan result
 * for the user to review/edit before proceeding to code generation.
 */
export async function generateDesignPlan(
  message: string,
  history: RustChatMessage[],
  onEvent: (event: MultiPartEvent) => void,
): Promise<DesignPlanResult> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    const result = await invoke<DesignPlanResult>('generate_design_plan', {
      message,
      history,
      onEvent: channel,
    });
    return result;
  } catch (err) {
    console.error('generate_design_plan failed:', err);
    throw new Error(`Generate design plan failed: ${err}`);
  }
}

/**
 * Generate code from a (possibly user-edited) design plan.
 * Runs Phase 1+ (planner decomposition, code gen, review, validation).
 */
export async function generateFromPlan(
  planText: string,
  userRequest: string,
  history: RustChatMessage[],
  onEvent: (event: MultiPartEvent) => void,
  existingCode?: string | null,
): Promise<string> {
  try {
    const channel = new Channel<MultiPartEvent>();
    channel.onmessage = (event) => {
      onEvent(event);
    };

    return await invoke<string>('generate_from_plan', {
      planText,
      userRequest,
      history,
      existingCode: existingCode ?? null,
      onEvent: channel,
    });
  } catch (err) {
    console.error('generate_from_plan failed:', err);
    throw new Error(`Generate from plan failed: ${err}`);
  }
}

/**
 * Extract Python code from an AI response using a 3-tier cascade:
 * 1. <CODE>...</CODE> XML tags (case-insensitive)
 * 2. ```python ... ``` markdown fence
 * 3. Any ``` block containing CadQuery markers (import cadquery / cq.)
 * Returns the first matched code block content, or null if none found.
 */
export function extractPythonCode(text: string): string | null {
  // Tier 1: <CODE>...</CODE> XML tags
  const xmlMatch = text.match(/<CODE>([\s\S]*?)<\/CODE>/i);
  if (xmlMatch && xmlMatch[1].trim()) return xmlMatch[1].trim();

  // Tier 2: ```python ... ``` markdown fence
  const fenceMatch = text.match(/```python\s*\n([\s\S]*?)```/);
  if (fenceMatch && fenceMatch[1].trim()) return fenceMatch[1].trim();

  // Tier 3: Any ``` block with CadQuery markers
  const heuristicRe = /```\w*\s*\n([\s\S]*?)```/g;
  let m;
  while ((m = heuristicRe.exec(text)) !== null) {
    const code = m[1].trim();
    if (code.includes('import cadquery') || code.includes('cq.')) return code;
  }

  return null;
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
 * Generate an SVG orthographic projection view of a CadQuery model
 */
export async function generateDrawingView(
  code: string,
  projX: number,
  projY: number,
  projZ: number,
  showHidden: boolean,
  sectionPlane?: string,
  sectionOffset?: number,
): Promise<{ svgContent: string; width: number; height: number }> {
  try {
    return await invoke<{ svgContent: string; width: number; height: number }>(
      'generate_drawing_view',
      { code, projX, projY, projZ, showHidden, sectionPlane: sectionPlane ?? null, sectionOffset: sectionOffset ?? null },
    );
  } catch (err) {
    console.error('generate_drawing_view failed:', err);
    throw new Error(`Generate drawing view failed: ${err}`);
  }
}

/**
 * Export a composed drawing SVG to PDF
 */
export async function exportDrawingPdf(svgContent: string, outputPath: string): Promise<string> {
  try {
    return await invoke<string>('export_drawing_pdf', { svgContent, outputPath });
  } catch (err) {
    console.error('export_drawing_pdf failed:', err);
    throw new Error(`Export PDF failed: ${err}`);
  }
}

/**
 * Export a composed drawing SVG to DXF
 */
export async function exportDrawingDxf(svgContent: string, outputPath: string): Promise<string> {
  try {
    return await invoke<string>('export_drawing_dxf', { svgContent, outputPath });
  } catch (err) {
    console.error('export_drawing_dxf failed:', err);
    throw new Error(`Export DXF failed: ${err}`);
  }
}

// ── Manufacturing types ──

export interface MeshCheckResult {
  watertight: boolean;
  winding_consistent: boolean;
  degenerate_faces: number;
  euler_number: number;
  volume: number;
  triangle_count: number;
  issues: string[];
}

export interface OrientResult {
  rotation: [number, number, number];
  height: number;
  overhang_pct: number;
  base_area: number;
  candidates_evaluated: number;
}

export interface UnfoldResult {
  path: string;
  face_count: number;
  bend_count: number;
  flat_width: number;
  flat_height: number;
}

export interface ColorInfo {
  r: number;
  g: number;
  b: number;
  a: number;
}

/**
 * Export model as 3MF with optional colors
 */
export async function export3mf(code: string, outputPath: string, colors?: ColorInfo[]): Promise<string> {
  try {
    const result = await invoke<{ path: string; triangles: number }>('export_3mf', {
      code,
      outputPath,
      colors: colors ?? null,
    });
    return `3MF exported (${result.triangles} triangles)`;
  } catch (err) {
    console.error('export_3mf failed:', err);
    throw new Error(`Export 3MF failed: ${err}`);
  }
}

/**
 * Check mesh quality for manufacturing
 */
export async function meshCheck(code: string): Promise<MeshCheckResult> {
  try {
    return await invoke<MeshCheckResult>('mesh_check', { code });
  } catch (err) {
    console.error('mesh_check failed:', err);
    throw new Error(`Mesh check failed: ${err}`);
  }
}

/**
 * Analyze optimal print orientation
 */
export async function orientForPrint(code: string): Promise<OrientResult> {
  try {
    return await invoke<OrientResult>('orient_for_print', { code });
  } catch (err) {
    console.error('orient_for_print failed:', err);
    throw new Error(`Orient analysis failed: ${err}`);
  }
}

/**
 * Compute sheet metal flat pattern and export as DXF
 */
export async function sheetMetalUnfold(code: string, outputPath: string, thickness?: number): Promise<UnfoldResult> {
  try {
    return await invoke<UnfoldResult>('sheet_metal_unfold', {
      code,
      outputPath,
      thickness: thickness ?? null,
    });
  } catch (err) {
    console.error('sheet_metal_unfold failed:', err);
    throw new Error(`Sheet metal unfold failed: ${err}`);
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
