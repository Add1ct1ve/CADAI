<script lang="ts">
  import { projectNew, projectOpen, projectSave, projectExportStl, projectExportStep } from '$lib/services/project-actions';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { triggerPipeline } from '$lib/services/execution-pipeline';
  import type { ToolId, SketchPlane, SketchToolId, BooleanOpType } from '$lib/types/cad';
  import { runPythonExecution } from '$lib/services/execution-pipeline';

  interface Props {
    onSettingsClick: () => void;
  }

  let { onSettingsClick }: Props = $props();

  const toolStore = getToolStore();
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();
  const featureTree = getFeatureTreeStore();

  let isBusy = $state(false);
  let statusMessage = $state('');

  function showStatus(msg: string, duration = 3000) {
    statusMessage = msg;
    setTimeout(() => {
      statusMessage = '';
    }, duration);
  }

  async function handleNew() {
    try {
      const result = await projectNew();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Failed: ${err}`);
    }
  }

  async function handleOpen() {
    try {
      isBusy = true;
      const result = await projectOpen();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Failed to open: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleSave() {
    try {
      isBusy = true;
      const result = await projectSave();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Failed to save: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportStl() {
    try {
      isBusy = true;
      showStatus('Exporting STL...');
      const result = await projectExportStl();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportStep() {
    try {
      isBusy = true;
      showStatus('Exporting STEP...');
      const result = await projectExportStep();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  function handleUndo() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const ftSnap = featureTree.snapshot();
    const current = {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
      featureTree: ftSnap,
    };
    const snapshot = history.undo(current);
    if (snapshot) {
      scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
      if (snapshot.sketches !== undefined) {
        sketchStore.restoreSnapshot({
          sketches: snapshot.sketches,
          activeSketchId: snapshot.activeSketchId ?? null,
          selectedSketchId: snapshot.selectedSketchId ?? null,
        });
      }
      if (snapshot.featureTree) {
        featureTree.restoreSnapshot(snapshot.featureTree);
      } else {
        featureTree.syncFromStores();
      }
      triggerPipeline(100);
    }
  }

  function handleRedo() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const ftSnap = featureTree.snapshot();
    const current = {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
      featureTree: ftSnap,
    };
    const snapshot = history.redo(current);
    if (snapshot) {
      scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
      if (snapshot.sketches !== undefined) {
        sketchStore.restoreSnapshot({
          sketches: snapshot.sketches,
          activeSketchId: snapshot.activeSketchId ?? null,
          selectedSketchId: snapshot.selectedSketchId ?? null,
        });
      }
      if (snapshot.featureTree) {
        featureTree.restoreSnapshot(snapshot.featureTree);
      } else {
        featureTree.syncFromStores();
      }
      triggerPipeline(100);
    }
  }

  function setTool(tool: ToolId) {
    toolStore.setTool(tool);
  }

  function toggleCodeMode() {
    scene.setCodeMode(scene.codeMode === 'parametric' ? 'manual' : 'parametric');
  }

  function enterSketch(plane: SketchPlane) {
    sketchStore.enterSketchMode(plane);
  }

  function setSketchTool(tool: SketchToolId) {
    sketchStore.setSketchTool(tool);
  }

  function handleFinishSketch() {
    sketchStore.exitSketchMode();
    triggerPipeline(100);
  }

  // ── Boolean / Split ──

  function captureSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const ftSnap = featureTree.snapshot();
    return {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
      featureTree: ftSnap,
    };
  }

  let canBoolean = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    scene.selectedIds.length === 2 &&
    !scene.selectedObjects.some((o) => !!o.booleanOp)
  );

  let canSplit = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    scene.selectedIds.length === 1 &&
    scene.firstSelected !== null &&
    !scene.firstSelected.booleanOp
  );

  function applyBoolean(type: BooleanOpType) {
    if (!canBoolean) return;
    const ids = scene.selectedIds;
    const targetId = ids[0];
    const toolId = ids[1];
    history.pushSnapshot(captureSnapshot());
    scene.setBooleanOp(toolId, { type, targetId });
    scene.select(targetId);
    triggerPipeline(100);
    runPythonExecution();
  }

  function applySplit() {
    if (!canSplit) return;
    const obj = scene.firstSelected!;
    history.pushSnapshot(captureSnapshot());
    scene.setSplitOp(obj.id, { plane: 'XY', offset: 0, keepSide: 'both' });
    triggerPipeline(100);
    runPythonExecution();
  }

  const toolButtons: { id: ToolId; label: string; shortcut: string; group: 'select' | 'primitive' }[] = [
    { id: 'select', label: 'Select', shortcut: 'V', group: 'select' },
    { id: 'translate', label: 'Move', shortcut: 'G', group: 'select' },
    { id: 'rotate', label: 'Rotate', shortcut: 'R', group: 'select' },
    { id: 'scale', label: 'Scale', shortcut: 'S', group: 'select' },
  ];

  const primitiveButtons: { id: ToolId; label: string; shortcut: string }[] = [
    { id: 'add-box', label: 'Box', shortcut: '1' },
    { id: 'add-cylinder', label: 'Cyl', shortcut: '2' },
    { id: 'add-sphere', label: 'Sphere', shortcut: '3' },
    { id: 'add-cone', label: 'Cone', shortcut: '4' },
  ];

  const sketchToolButtons: { id: SketchToolId; label: string; shortcut: string }[] = [
    { id: 'sketch-select', label: 'Select', shortcut: 'V' },
    { id: 'sketch-line', label: 'Line', shortcut: 'L' },
    { id: 'sketch-rect', label: 'Rect', shortcut: 'R' },
    { id: 'sketch-circle', label: 'Circle', shortcut: 'C' },
    { id: 'sketch-arc', label: 'Arc', shortcut: 'A' },
  ];

  const constraintToolButtons: { id: SketchToolId; label: string; shortcut: string }[] = [
    { id: 'sketch-constraint-coincident',    label: 'Coinc',  shortcut: 'O' },
    { id: 'sketch-constraint-horizontal',    label: 'Horiz',  shortcut: 'H' },
    { id: 'sketch-constraint-vertical',      label: 'Vert',   shortcut: 'I' },
    { id: 'sketch-constraint-parallel',      label: 'Para',   shortcut: 'P' },
    { id: 'sketch-constraint-perpendicular', label: 'Perp',   shortcut: 'T' },
    { id: 'sketch-constraint-equal',         label: 'Equal',  shortcut: 'E' },
    { id: 'sketch-constraint-distance',      label: 'Dist',   shortcut: 'D' },
    { id: 'sketch-constraint-radius',        label: 'Rad',    shortcut: 'Q' },
    { id: 'sketch-constraint-angle',         label: 'Angle',  shortcut: 'N' },
  ];

  const operationToolButtons: { id: SketchToolId; label: string; shortcut: string }[] = [
    { id: 'sketch-op-trim',    label: 'Trim',    shortcut: 'X' },
    { id: 'sketch-op-extend',  label: 'Extend',  shortcut: 'W' },
    { id: 'sketch-op-offset',  label: 'Offset',  shortcut: 'F' },
    { id: 'sketch-op-mirror',  label: 'Mirror',  shortcut: 'M' },
    { id: 'sketch-op-fillet',  label: 'Fillet',  shortcut: 'G' },
    { id: 'sketch-op-chamfer', label: 'Chamfer', shortcut: 'J' },
  ];
</script>

<div class="toolbar">
  <div class="toolbar-left">
    <span class="app-title">CAD AI Studio</span>
  </div>

  <div class="toolbar-center">
    <!-- File actions -->
    <button class="toolbar-btn" onclick={handleNew} title="New Project (Ctrl+N)" disabled={isBusy}>
      New
    </button>
    <button class="toolbar-btn" onclick={handleOpen} title="Open Project (Ctrl+O)" disabled={isBusy}>
      Open
    </button>
    <button class="toolbar-btn" onclick={handleSave} title="Save Project (Ctrl+S)" disabled={isBusy}>
      Save
    </button>
    <button class="toolbar-btn" onclick={handleExportStl} title="Export STL" disabled={isBusy}>
      STL
    </button>
    <button class="toolbar-btn" onclick={handleExportStep} title="Export STEP" disabled={isBusy}>
      STEP
    </button>

    <div class="toolbar-separator"></div>

    <!-- Undo / Redo -->
    <button
      class="toolbar-btn"
      onclick={handleUndo}
      title="Undo (Ctrl+Z)"
      disabled={!history.canUndo || scene.codeMode !== 'parametric'}
    >
      Undo
    </button>
    <button
      class="toolbar-btn"
      onclick={handleRedo}
      title="Redo (Ctrl+Shift+Z)"
      disabled={!history.canRedo || scene.codeMode !== 'parametric'}
    >
      Redo
    </button>

    <div class="toolbar-separator"></div>

    {#if sketchStore.isInSketchMode}
      <!-- Sketch mode tools -->
      <button class="toolbar-btn sketch-finish-btn" onclick={handleFinishSketch} title="Finish Sketch (Escape)">
        Finish
      </button>

      <div class="toolbar-separator"></div>

      {#each sketchToolButtons as btn}
        <button
          class="toolbar-btn tool-btn"
          class:tool-active-sketch={sketchStore.activeSketchTool === btn.id}
          onclick={() => setSketchTool(btn.id)}
          title="{btn.label} ({btn.shortcut})"
        >
          {btn.label}
        </button>
      {/each}

      <div class="toolbar-separator"></div>

      <!-- Constraint tools -->
      {#each constraintToolButtons as btn}
        <button
          class="toolbar-btn tool-btn constraint-btn"
          class:tool-active-sketch={sketchStore.activeSketchTool === btn.id}
          onclick={() => setSketchTool(btn.id)}
          title="{btn.label} ({btn.shortcut})"
        >
          {btn.label}
        </button>
      {/each}

      <div class="toolbar-separator"></div>

      <!-- Operation tools -->
      {#each operationToolButtons as btn}
        <button
          class="toolbar-btn tool-btn operation-btn"
          class:tool-active-sketch={sketchStore.activeSketchTool === btn.id}
          onclick={() => setSketchTool(btn.id)}
          title="{btn.label} ({btn.shortcut})"
        >
          {btn.label}
        </button>
      {/each}

      <div class="toolbar-separator"></div>

      <!-- Snap toggle -->
      <button
        class="toolbar-btn snap-btn"
        class:snap-active={sketchStore.sketchSnap !== null}
        onclick={() => sketchStore.setSketchSnap(sketchStore.sketchSnap ? null : 0.5)}
        title="Toggle sketch grid snap (0.5 units)"
      >
        Snap: 0.5
      </button>
    {:else}
      <!-- Normal 3D tools -->

      <!-- Selection / Transform tools -->
      {#each toolButtons as btn}
        <button
          class="toolbar-btn tool-btn"
          class:tool-active={toolStore.activeTool === btn.id}
          onclick={() => setTool(btn.id)}
          title="{btn.label} ({btn.shortcut})"
          disabled={scene.codeMode !== 'parametric'}
        >
          {btn.label}
        </button>
      {/each}

      <!-- Snap controls (shown when a transform tool is active) -->
      {#if toolStore.activeTool === 'translate'}
        <button
          class="toolbar-btn snap-btn"
          class:snap-active={toolStore.translateSnap !== null}
          onclick={() => toolStore.setTranslateSnap(toolStore.translateSnap ? null : 1)}
          title="Toggle translation snap (1 unit)"
        >
          Snap: 1u
        </button>
      {/if}
      {#if toolStore.activeTool === 'rotate'}
        <button
          class="toolbar-btn snap-btn"
          class:snap-active={toolStore.rotationSnap !== null}
          onclick={() => toolStore.setRotationSnap(toolStore.rotationSnap ? null : 15)}
          title="Toggle rotation snap (15 degrees)"
        >
          Snap: 15°
        </button>
      {/if}
      {#if toolStore.activeTool === 'scale'}
        <button
          class="toolbar-btn snap-btn"
          class:snap-active={toolStore.uniformScale}
          onclick={() => toolStore.setUniformScale(!toolStore.uniformScale)}
          title="Toggle uniform scaling (all axes equal)"
        >
          Uniform
        </button>
      {/if}

      <div class="toolbar-separator"></div>

      <!-- Primitive tools -->
      {#each primitiveButtons as btn}
        <button
          class="toolbar-btn tool-btn"
          class:tool-active={toolStore.activeTool === btn.id}
          onclick={() => setTool(btn.id)}
          title="{btn.label} ({btn.shortcut})"
          disabled={scene.codeMode !== 'parametric'}
        >
          {btn.label}
        </button>
      {/each}

      <div class="toolbar-separator"></div>

      <!-- Sketch plane buttons -->
      <button
        class="toolbar-btn sketch-btn"
        onclick={() => enterSketch('XY')}
        title="Start sketch on XY plane"
        disabled={scene.codeMode !== 'parametric'}
      >
        Sketch XY
      </button>
      <button
        class="toolbar-btn sketch-btn"
        onclick={() => enterSketch('XZ')}
        title="Start sketch on XZ plane"
        disabled={scene.codeMode !== 'parametric'}
      >
        Sketch XZ
      </button>
      <button
        class="toolbar-btn sketch-btn"
        onclick={() => enterSketch('YZ')}
        title="Start sketch on YZ plane"
        disabled={scene.codeMode !== 'parametric'}
      >
        Sketch YZ
      </button>

      <div class="toolbar-separator"></div>

      <!-- Boolean operations -->
      <button class="toolbar-btn boolean-btn"
        onclick={() => applyBoolean('union')}
        title="Union (Ctrl+Shift+U)"
        disabled={!canBoolean}>Union</button>
      <button class="toolbar-btn boolean-btn"
        onclick={() => applyBoolean('subtract')}
        title="Subtract (Ctrl+Shift+D)"
        disabled={!canBoolean}>Subtract</button>
      <button class="toolbar-btn boolean-btn"
        onclick={() => applyBoolean('intersect')}
        title="Intersect (Ctrl+Shift+I)"
        disabled={!canBoolean}>Intersect</button>
      <button class="toolbar-btn boolean-btn"
        onclick={applySplit}
        title="Split (Ctrl+Shift+P)"
        disabled={!canSplit}>Split</button>

      <div class="toolbar-separator"></div>

      <!-- Code mode toggle -->
      <button
        class="toolbar-btn mode-btn"
        class:mode-parametric={scene.codeMode === 'parametric'}
        class:mode-manual={scene.codeMode === 'manual'}
        onclick={toggleCodeMode}
        title="Toggle between parametric and manual code mode"
      >
        {scene.codeMode === 'parametric' ? 'Parametric' : 'Manual'}
      </button>
    {/if}

    {#if statusMessage}
      <span class="toolbar-status">{statusMessage}</span>
    {/if}
  </div>

  <div class="toolbar-right">
    <button class="toolbar-btn settings-btn" onclick={onSettingsClick} title="Settings">
      &#9881;
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 12px;
    background: var(--bg-mantle);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 12px;
    -webkit-app-region: drag;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .app-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.3px;
    -webkit-app-region: drag;
  }

  .toolbar-center {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 1;
    -webkit-app-region: no-drag;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
  }

  .toolbar-btn {
    background: none;
    border: 1px solid transparent;
    color: var(--text-secondary);
    padding: 4px 10px;
    border-radius: 3px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.12s ease;
    white-space: nowrap;
  }

  .toolbar-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }

  .toolbar-btn:active {
    background: var(--bg-surface);
  }

  .toolbar-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .tool-btn.tool-active {
    background: rgba(137, 180, 250, 0.15);
    border-color: var(--accent);
    color: var(--accent);
  }

  .tool-btn.tool-active-sketch {
    background: rgba(249, 226, 175, 0.15);
    border-color: #f9e2af;
    color: #f9e2af;
  }

  .snap-btn {
    font-size: 10px;
    padding: 2px 6px;
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .snap-btn.snap-active {
    color: var(--success);
    border-color: var(--success);
    background: rgba(166, 227, 161, 0.1);
  }

  .boolean-btn {
    font-size: 11px;
    color: #fab387;
    border: 1px solid rgba(250, 179, 135, 0.3);
  }

  .boolean-btn:hover:not(:disabled) {
    background: rgba(250, 179, 135, 0.1);
    border-color: #fab387;
  }

  .sketch-btn {
    font-size: 11px;
    color: #f9e2af;
    border: 1px solid rgba(249, 226, 175, 0.3);
  }

  .sketch-btn:hover {
    background: rgba(249, 226, 175, 0.1);
    border-color: #f9e2af;
  }

  .constraint-btn {
    font-size: 10px;
    padding: 3px 6px;
    color: #cba6f7;
    border: 1px solid rgba(203, 166, 247, 0.3);
  }

  .constraint-btn:hover {
    background: rgba(203, 166, 247, 0.1);
    border-color: #cba6f7;
  }

  .operation-btn {
    font-size: 10px;
    padding: 3px 6px;
    color: #94e2d5;
    border: 1px solid rgba(148, 226, 213, 0.3);
  }

  .operation-btn:hover {
    background: rgba(148, 226, 213, 0.1);
    border-color: #94e2d5;
  }

  .sketch-finish-btn {
    font-weight: 600;
    font-size: 11px;
    color: #f9e2af;
    border: 1px solid #f9e2af;
    background: rgba(249, 226, 175, 0.1);
  }

  .sketch-finish-btn:hover {
    background: rgba(249, 226, 175, 0.2);
  }

  .mode-btn {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    padding: 3px 8px;
  }

  .mode-btn.mode-parametric {
    color: var(--accent);
    border-color: var(--accent);
    background: rgba(137, 180, 250, 0.1);
  }

  .mode-btn.mode-manual {
    color: var(--warning);
    border-color: var(--warning);
    background: rgba(249, 226, 175, 0.1);
  }

  .settings-btn {
    font-size: 16px;
    padding: 2px 8px;
  }

  .toolbar-separator {
    width: 1px;
    height: 20px;
    background: var(--border-subtle);
    margin: 0 4px;
  }

  .toolbar-status {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: 8px;
    padding: 2px 8px;
    background: var(--bg-overlay);
    border-radius: 3px;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
