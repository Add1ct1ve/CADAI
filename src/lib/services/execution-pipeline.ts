import { getSceneStore } from '$lib/stores/scene.svelte';
import { getSketchStore } from '$lib/stores/sketch.svelte';
import { getProjectStore } from '$lib/stores/project.svelte';
import { getViewportStore } from '$lib/stores/viewport.svelte';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import { generateCode } from '$lib/services/code-generator';
import { executeCode } from '$lib/services/tauri';

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let generation = 0;

/**
 * Trigger code generation only — updates the code editor but does NOT run Python.
 * Used whenever scene objects change in parametric mode.
 */
export function triggerPipeline(delay = 500) {
  if (debounceTimer) clearTimeout(debounceTimer);

  debounceTimer = setTimeout(() => {
    const scene = getSceneStore();
    const sketchStore = getSketchStore();
    const project = getProjectStore();

    if (scene.codeMode !== 'parametric') return;
    const codegenObjects = scene.objects.filter((o) => !o.importedMeshBase64);
    const hasSketchGeometry = sketchStore.sketches.some((s) => s.entities.length > 0);
    // Imported STL parts are preview/edit references, not codegen primitives.
    // If they are the only content, keep the existing generated code untouched.
    if (codegenObjects.length === 0 && hasSketchGeometry === false) return;

    const featureTree = getFeatureTreeStore();
    const code = generateCode(scene.objects, sketchStore.sketches, featureTree.activeFeatureIds);
    project.setCode(code);
  }, delay);
}

/**
 * Run Python execution explicitly (Ctrl+R / Run button / export).
 * Generates code from scene first, then executes via Python subprocess.
 */
export async function runPythonExecution() {
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const project = getProjectStore();
  const viewport = getViewportStore();

  const gen = ++generation;

  // Always generate fresh code from scene if in parametric mode
  if (scene.codeMode === 'parametric') {
    const codegenObjects = scene.objects.filter((o) => !o.importedMeshBase64);
    const hasSketchGeometry = sketchStore.sketches.some((s) => s.entities.length > 0);
    const featureTree = getFeatureTreeStore();
    if (codegenObjects.length > 0 || hasSketchGeometry) {
      const code = generateCode(scene.objects, sketchStore.sketches, featureTree.activeFeatureIds);
      project.setCode(code);
    }
  }

  const code = project.code;

  if (!code.trim()) return;

  try {
    viewport.setLoading(true);
    const result = await executeCode(code);

    // Discard stale result
    if (gen !== generation) return;

    if (result.success && result.stl_base64) {
      viewport.setPendingStl(result.stl_base64);
      viewport.setHasModel(true);
    } else {
      console.warn('Python execution failed:', result.stderr);
    }
  } catch (err) {
    if (gen !== generation) return;
    console.error('Python execution error:', err);
  } finally {
    if (gen === generation) {
      viewport.setLoading(false);
    }
  }
}

/**
 * Cancel any pending pipeline execution.
 */
export function cancelPipeline() {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  generation++;
}
