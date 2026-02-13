<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import Chat from '$lib/components/Chat.svelte';
  import FeatureTree from '$lib/components/FeatureTree.svelte';
  import Viewport from '$lib/components/Viewport.svelte';
  import DrawingViewer from '$lib/components/DrawingViewer.svelte';
  import RightPanel from '$lib/components/RightPanel.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import ShortcutsPanel from '$lib/components/ShortcutsPanel.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { applyTheme } from '$lib/services/theme';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { projectNew, projectSave } from '$lib/services/project-actions';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { startAutosave, stopAutosave, restoreDraftIfPresent } from '$lib/services/autosave';
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { getDatumStore } from '$lib/stores/datum.svelte';
  import { getComponentStore } from '$lib/stores/component.svelte';
  import { getMateStore } from '$lib/stores/mate.svelte';
  import { getDrawingStore } from '$lib/stores/drawing.svelte';
  import type { ToolId, SketchToolId, BooleanOpType, PatternOp, PatternType } from '$lib/types/cad';
  import type { SceneSnapshot } from '$lib/stores/history.svelte';
  import { onMount } from 'svelte';

  const settings = getSettingsStore();
  const project = getProjectStore();
  const scene = getSceneStore();
  const tools = getToolStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();
  const viewport = getViewportStore();
  const featureTree = getFeatureTreeStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();
  const drawingStore = getDrawingStore();

  let settingsOpen = $state(false);
  let shortcutsOpen = $state(false);

  onMount(() => {
    settings.load().then(() => {
      // Apply persisted theme
      applyTheme((settings.config.theme as 'dark' | 'light') || 'dark');

      // Apply persisted snap values
      tools.setTranslateSnap(settings.config.snap_translate ?? 1);
      tools.setRotationSnap(settings.config.snap_rotation ?? 15);
      sketchStore.setSketchSnap(settings.config.snap_sketch ?? 0.5);
    });

    restoreDraftIfPresent().catch((err) => {
      console.warn('Draft restore failed:', err);
    });
    startAutosave();
    sketchStore.initSolver().catch(console.error);

    return () => {
      stopAutosave();
      sketchStore.destroySolver();
    };
  });

  // ── Full snapshot helpers (scene + sketch + feature tree) ──
  function captureFullSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const ftSnap = featureTree.snapshot();
    const datumSnap = datumStore.snapshot();
    const compSnap = componentStore.snapshot();
    const mateSnap = mateStore.snapshot();
    return {
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
      mates: mateSnap.mates,
      selectedMateId: mateSnap.selectedMateId,
    };
  }

  function restoreFullSnapshot(snapshot: SceneSnapshot) {
    scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
    if (snapshot.sketches) {
      sketchStore.restoreSnapshot({
        sketches: snapshot.sketches,
        activeSketchId: snapshot.activeSketchId ?? null,
        selectedSketchId: snapshot.selectedSketchId ?? null,
      });
    }
    if (snapshot.datumPlanes || snapshot.datumAxes) {
      datumStore.restoreSnapshot({
        datumPlanes: snapshot.datumPlanes ?? [],
        datumAxes: snapshot.datumAxes ?? [],
        selectedDatumId: snapshot.selectedDatumId ?? null,
      });
    }
    if (snapshot.components) {
      componentStore.restoreSnapshot({
        components: snapshot.components,
        nameCounter: snapshot.componentNameCounter ?? 0,
        selectedComponentId: snapshot.selectedComponentId ?? null,
      });
    }
    if (snapshot.mates) {
      mateStore.restoreSnapshot({
        mates: snapshot.mates,
        selectedMateId: snapshot.selectedMateId ?? null,
      });
    }
    if (snapshot.featureTree) {
      featureTree.restoreSnapshot(snapshot.featureTree);
    } else {
      featureTree.syncFromStores();
    }
  }

  function applyBooleanFromPage(type: BooleanOpType) {
    if (scene.codeMode !== 'parametric' || sketchStore.isInSketchMode) return;
    if (scene.selectedIds.length !== 2) return;
    if (scene.selectedObjects.some((o) => !!o.booleanOp)) return;
    const targetId = scene.selectedIds[0];
    const toolId = scene.selectedIds[1];
    history.pushSnapshot(captureFullSnapshot());
    scene.setBooleanOp(toolId, { type, targetId });
    scene.select(targetId);
    triggerPipeline(100);
    runPythonExecution();
  }

  function applySplitFromPage() {
    if (scene.codeMode !== 'parametric' || sketchStore.isInSketchMode) return;
    if (scene.selectedIds.length !== 1) return;
    const obj = scene.firstSelected;
    if (!obj || obj.booleanOp) return;
    history.pushSnapshot(captureFullSnapshot());
    scene.setSplitOp(obj.id, { plane: 'XY', offset: 0, keepSide: 'both' });
    triggerPipeline(100);
    runPythonExecution();
  }

  function applyPatternFromPage(type: PatternType) {
    if (scene.codeMode !== 'parametric' || sketchStore.isInSketchMode) return;
    if (scene.selectedIds.length !== 1) return;
    const obj = scene.firstSelected;
    if (!obj || obj.booleanOp || obj.splitOp) return;
    history.pushSnapshot(captureFullSnapshot());
    let op: PatternOp;
    switch (type) {
      case 'mirror': op = { type: 'mirror', plane: 'XY', offset: 0, keepOriginal: true }; break;
      case 'linear': op = { type: 'linear', direction: 'X', spacing: 20, count: 3 }; break;
      case 'circular': op = { type: 'circular', axis: 'Z', count: 6, fullAngle: 360 }; break;
    }
    scene.setPatternOp(obj.id, op);
    triggerPipeline(100);
    runPythonExecution();
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

    // Boolean / Split shortcuts (Ctrl+Shift+U/D/I/P)
    if (ctrl && e.shiftKey && scene.codeMode === 'parametric') {
      const key = e.key.toUpperCase();
      if (key === 'U') { e.preventDefault(); applyBooleanFromPage('union'); return; }
      if (key === 'D') { e.preventDefault(); applyBooleanFromPage('subtract'); return; }
      if (key === 'I') { e.preventDefault(); applyBooleanFromPage('intersect'); return; }
      if (key === 'P') { e.preventDefault(); applySplitFromPage(); return; }
      if (key === 'M') { e.preventDefault(); applyPatternFromPage('mirror'); return; }
      if (key === 'L') { e.preventDefault(); applyPatternFromPage('linear'); return; }
      if (key === 'O') { e.preventDefault(); applyPatternFromPage('circular'); return; }
    }

    // ── Drawing mode shortcuts ──
    if (scene.drawingMode) {
      if (e.key === 'Escape') {
        e.preventDefault();
        if (drawingStore.selectedViewId || drawingStore.selectedDimensionId || drawingStore.selectedNoteId) {
          drawingStore.clearSelection();
        } else {
          scene.setDrawingMode(false);
          drawingStore.clearSelection();
          drawingStore.setDrawingTool('select');
        }
        return;
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && !isInput) {
        e.preventDefault();
        drawingStore.deleteSelected();
        return;
      }
      if (ctrl && e.shiftKey && e.key.toUpperCase() === 'E') {
        e.preventDefault();
        // Export PDF shortcut - handled by toolbar
        return;
      }
      // Block other shortcuts in drawing mode
      return;
    }

    // Tool shortcuts (only when not focused on an input)
    if (isInput) return;

    // ? key: open shortcuts panel
    if (e.key === '?' || (e.shiftKey && e.code === 'Slash')) {
      e.preventDefault();
      shortcutsOpen = !shortcutsOpen;
      return;
    }

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
        o: 'sketch-constraint-coincident',
        h: 'sketch-constraint-horizontal',
        i: 'sketch-constraint-vertical',
        p: 'sketch-constraint-parallel',
        t: 'sketch-constraint-perpendicular',
        e: 'sketch-constraint-equal',
        d: 'sketch-constraint-distance',
        q: 'sketch-constraint-radius',
        n: 'sketch-constraint-angle',
        x: 'sketch-op-trim',
        w: 'sketch-op-extend',
        f: 'sketch-op-offset',
        m: 'sketch-op-mirror',
        g: 'sketch-op-fillet',
        j: 'sketch-op-chamfer',
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
      // E key: extrude selected sketch
      if (e.key.toLowerCase() === 'e') {
        const selectedSketch = sketchStore.selectedSketch;
        if (selectedSketch && !selectedSketch.operation && selectedSketch.entities.length > 0) {
          e.preventDefault();
          history.pushSnapshot(captureFullSnapshot());
          sketchStore.setOperation(selectedSketch.id, { type: 'extrude', distance: 10, mode: 'add' });
          triggerPipeline(100);
          runPythonExecution();
          return;
        }
      }

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
          <SplitPane direction="vertical" sizes={[35, 65]} minSize={80}>
            {#snippet panes(subIndex)}
              {#if subIndex === 0}
                <FeatureTree />
              {:else}
                <Chat />
              {/if}
            {/snippet}
          </SplitPane>
        {:else if index === 1}
          {#if scene.drawingMode}
            <DrawingViewer />
          {:else}
            <Viewport />
          {/if}
        {:else}
          <RightPanel />
        {/if}
      {/snippet}
    </SplitPane>
  </div>

  <StatusBar />
  <Settings open={settingsOpen} onClose={() => { settingsOpen = false; }} />
  <ShortcutsPanel open={shortcutsOpen} onClose={() => { shortcutsOpen = false; }} />
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
