<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import Chat from '$lib/components/Chat.svelte';
  import Viewport from '$lib/components/Viewport.svelte';
  import RightPanel from '$lib/components/RightPanel.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { projectNew, projectSave } from '$lib/services/project-actions';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { startAutosave, stopAutosave } from '$lib/services/autosave';
  import type { ToolId, SketchToolId } from '$lib/types/cad';
  import type { SceneSnapshot } from '$lib/stores/history.svelte';
  import { onMount } from 'svelte';

  const settings = getSettingsStore();
  const project = getProjectStore();
  const scene = getSceneStore();
  const tools = getToolStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();
  const viewport = getViewportStore();

  let settingsOpen = $state(false);

  onMount(() => {
    settings.load();
    startAutosave();

    return () => {
      stopAutosave();
    };
  });

  // ── Full snapshot helpers (scene + sketch) ──
  function captureFullSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    return {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
    };
  }

  function restoreFullSnapshot(snapshot: SceneSnapshot) {
    scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
    if (snapshot.sketches) {
      sketchStore.restoreSnapshot({
        sketches: snapshot.sketches,
        activeSketchId: snapshot.activeSketchId ?? null,
      });
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    const target = e.target as HTMLElement;
    const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

    // Global shortcuts (always active)
    if (ctrl && e.key === 'n') {
      e.preventDefault();
      projectNew();
      return;
    }
    if (ctrl && e.key === 's') {
      e.preventDefault();
      projectSave();
      return;
    }
    if (ctrl && e.key === 'r') {
      e.preventDefault();
      runCurrentCode();
      return;
    }

    // Undo/Redo (always active, parametric mode only)
    if (ctrl && e.key === 'z' && !e.shiftKey && scene.codeMode === 'parametric') {
      e.preventDefault();
      performUndo();
      return;
    }
    if (ctrl && (e.key === 'y' || (e.key === 'z' && e.shiftKey)) && scene.codeMode === 'parametric') {
      e.preventDefault();
      performRedo();
      return;
    }

    // Tool shortcuts (only when not focused on an input)
    if (isInput) return;

    // ── Sketch mode shortcuts (intercept before 3D shortcuts) ──
    if (sketchStore.isInSketchMode) {
      // Escape: cancel drawing or exit sketch mode
      if (e.key === 'Escape') {
        e.preventDefault();
        if (sketchStore.drawingPoints.length > 0) {
          sketchStore.clearDrawingState();
        } else {
          sketchStore.exitSketchMode();
          triggerPipeline(100);
        }
        return;
      }

      // Delete/Backspace: delete selected entities
      if ((e.key === 'Delete' || e.key === 'Backspace') && sketchStore.selectedEntityIds.length > 0) {
        e.preventDefault();
        history.pushSnapshot(captureFullSnapshot());
        sketchStore.deleteSelectedEntities();
        triggerPipeline(100);
        return;
      }

      // Sketch tool shortcuts
      const sketchToolMap: Record<string, SketchToolId> = {
        v: 'sketch-select',
        l: 'sketch-line',
        r: 'sketch-rect',
        c: 'sketch-circle',
        a: 'sketch-arc',
      };
      const sketchTool = sketchToolMap[e.key.toLowerCase()];
      if (sketchTool) {
        e.preventDefault();
        sketchStore.setSketchTool(sketchTool);
        return;
      }

      // Block all other single-key shortcuts while in sketch mode
      return;
    }

    // View shortcuts (work in all modes)
    if (e.key === 'Home') {
      e.preventDefault();
      viewport.fitAll();
      return;
    }
    if (e.code === 'Numpad7') {
      e.preventDefault();
      viewport.animateToView('top');
      return;
    }
    if (e.code === 'Numpad1') {
      e.preventDefault();
      viewport.animateToView('front');
      return;
    }
    if (e.code === 'Numpad3') {
      e.preventDefault();
      viewport.animateToView('right');
      return;
    }
    if (e.code === 'Numpad0') {
      e.preventDefault();
      viewport.animateToView('iso');
      return;
    }

    // Only allow tool shortcuts in parametric mode
    if (scene.codeMode === 'parametric') {
      const toolMap: Record<string, ToolId> = {
        v: 'select',
        g: 'translate',
        r: 'rotate',
        s: 'scale',
        '1': 'add-box',
        '2': 'add-cylinder',
        '3': 'add-sphere',
        '4': 'add-cone',
      };

      const tool = toolMap[e.key.toLowerCase()];
      if (tool) {
        e.preventDefault();
        tools.setTool(tool);
        return;
      }
    }

    // Delete selected objects
    if ((e.key === 'Delete' || e.key === 'Backspace') && scene.codeMode === 'parametric') {
      if (scene.selectedIds.length > 0) {
        e.preventDefault();
        history.pushSnapshot(captureFullSnapshot());
        scene.deleteSelected();
        triggerPipeline(100);
      }
      return;
    }

    // Escape: deselect or revert tool
    if (e.key === 'Escape') {
      if (tools.activeTool !== 'select') {
        tools.revertToSelect();
      } else {
        scene.clearSelection();
      }
    }
  }

  function performUndo() {
    const snapshot = history.undo(captureFullSnapshot());
    if (snapshot) {
      restoreFullSnapshot(snapshot);
      triggerPipeline(100);
    }
  }

  function performRedo() {
    const snapshot = history.redo(captureFullSnapshot());
    if (snapshot) {
      restoreFullSnapshot(snapshot);
      triggerPipeline(100);
    }
  }

  async function runCurrentCode() {
    try {
      await runPythonExecution();
    } catch (err) {
      console.error('Run code failed:', err);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell">
  <Toolbar onSettingsClick={() => { settingsOpen = true; }} />

  <div class="main-area">
    <SplitPane direction="horizontal" sizes={[25, 50, 25]}>
      {#snippet panes(index)}
        {#if index === 0}
          <Chat />
        {:else if index === 1}
          <Viewport />
        {:else}
          <RightPanel />
        {/if}
      {/snippet}
    </SplitPane>
  </div>

  <StatusBar />
  <Settings open={settingsOpen} onClose={() => { settingsOpen = false; }} />
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
