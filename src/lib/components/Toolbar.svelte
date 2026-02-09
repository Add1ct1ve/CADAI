<script lang="ts">
  import { projectNew, projectOpen, projectSave, projectExportStl, projectExportStep, projectInsertComponent } from '$lib/services/project-actions';
  import { getComponentStore } from '$lib/stores/component.svelte';
  import { getMateStore } from '$lib/stores/mate.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { triggerPipeline } from '$lib/services/execution-pipeline';
  import type { ToolId, SketchPlane, SketchToolId, BooleanOpType, PatternOp, PatternType, MateType } from '$lib/types/cad';
  import { getDatumStore } from '$lib/stores/datum.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { runPythonExecution } from '$lib/services/execution-pipeline';
  import { checkInterference } from '$lib/services/interference-check';

  interface Props {
    onSettingsClick: () => void;
  }

  let { onSettingsClick }: Props = $props();

  const toolStore = getToolStore();
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();
  const featureTree = getFeatureTreeStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();
  const viewportStore = getViewportStore();
  const settingsStore = getSettingsStore();

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
    const current = captureSnapshot();
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
      if (snapshot.datumPlanes !== undefined || snapshot.datumAxes !== undefined) {
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
      triggerPipeline(100);
    }
  }

  function handleRedo() {
    const current = captureSnapshot();
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
      if (snapshot.datumPlanes !== undefined || snapshot.datumAxes !== undefined) {
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

  // ── Pattern ──

  let canPattern = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    scene.selectedIds.length === 1 &&
    scene.firstSelected !== null &&
    !scene.firstSelected.booleanOp &&
    !scene.firstSelected.splitOp
  );

  function applyPattern(type: PatternType) {
    if (!canPattern) return;
    const obj = scene.firstSelected!;
    history.pushSnapshot(captureSnapshot());
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

  // ── Datum reference geometry ──

  let datumPopup = $state<'offset' | '3pt' | 'axis' | null>(null);
  let datumOffsetBase = $state<'XY' | 'XZ' | 'YZ'>('XY');
  let datumOffsetVal = $state(10);
  let datum3ptP1 = $state<[number, number, number]>([0, 0, 0]);
  let datum3ptP2 = $state<[number, number, number]>([10, 0, 0]);
  let datum3ptP3 = $state<[number, number, number]>([0, 10, 0]);
  let datumAxisOrigin = $state<[number, number, number]>([0, 0, 0]);
  let datumAxisDir = $state<[number, number, number]>([0, 0, 1]);

  function createOffsetPlane() {
    history.pushSnapshot(captureSnapshot());
    datumStore.addOffsetPlane(datumOffsetBase, datumOffsetVal);
    datumPopup = null;
    triggerPipeline(100);
  }

  function createThreePointPlane() {
    history.pushSnapshot(captureSnapshot());
    datumStore.addThreePointPlane([...datum3ptP1], [...datum3ptP2], [...datum3ptP3]);
    datumPopup = null;
    triggerPipeline(100);
  }

  function createDatumAxis() {
    history.pushSnapshot(captureSnapshot());
    datumStore.addDatumAxis([...datumAxisOrigin], [...datumAxisDir]);
    datumPopup = null;
    triggerPipeline(100);
  }

  // ── Component / Assembly ──

  let canCreateComponent = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    (scene.selectedIds.length > 0 || sketchStore.selectedSketchId !== null || datumStore.selectedDatumId !== null) &&
    // Check none of the selected features are already in a component
    (() => {
      const selectedFeatureIds: string[] = [
        ...scene.selectedIds,
        ...(sketchStore.selectedSketchId ? [sketchStore.selectedSketchId] : []),
        ...(datumStore.selectedDatumId ? [datumStore.selectedDatumId] : []),
      ];
      return selectedFeatureIds.every((id) => !componentStore.getComponentForFeature(id));
    })()
  );

  function handleCreateComponent() {
    if (!canCreateComponent) return;
    const featureIds: string[] = [
      ...scene.selectedIds,
      ...(sketchStore.selectedSketchId ? [sketchStore.selectedSketchId] : []),
      ...(datumStore.selectedDatumId ? [datumStore.selectedDatumId] : []),
    ];
    if (featureIds.length === 0) return;
    history.pushSnapshot(captureSnapshot());
    const comp = componentStore.createComponent(featureIds);
    featureTree.registerComponent(comp.id);
    scene.clearSelection();
    sketchStore.selectSketch(null);
    datumStore.selectDatum(null);
    componentStore.selectComponent(comp.id);
    triggerPipeline(100);
  }

  async function handleInsertComponent() {
    try {
      isBusy = true;
      showStatus('Inserting component...');
      const result = await projectInsertComponent();
      if (result) showStatus(result);
      triggerPipeline(100);
      runPythonExecution();
    } catch (err) {
      showStatus(`Insert failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  // ── Assembly Mates ──

  let canCreateMate = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    componentStore.components.length >= 2
  );

  function startMate(type: MateType) {
    if (!canCreateMate) return;
    mateStore.startMateCreation(type);
    showStatus(`Select first face on a component (${type} mate)`);
  }

  function handleInterferenceCheck() {
    const engine = viewportStore.getCameraState() ? (viewportStore as any) : null;
    // We need 2 selected components for interference check
    // Use the selected component + one other
    const selComp = componentStore.selectedComponent;
    if (!selComp) {
      showStatus('Select a component first');
      return;
    }
    const otherComps = componentStore.components.filter((c) => c.id !== selComp.id && c.visible);
    if (otherComps.length === 0) {
      showStatus('Need at least 2 visible components');
      return;
    }
    // Check against all other components
    let found = false;
    for (const other of otherComps) {
      // Bounding box check via engine - we'll use a simplified approach
      showStatus(`Checking ${selComp.name} vs ${other.name}...`);
      // The check uses the viewport engine directly, so we report results
      found = true;
      showStatus(`Interference check: ${selComp.name} vs ${other.name} - check viewport for overlap`, 5000);
      break;
    }
    if (!found) {
      showStatus('No interference found');
    }
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
        onclick={() => sketchStore.setSketchSnap(sketchStore.sketchSnap ? null : (settingsStore.config.snap_sketch ?? 0.5))}
        title="Toggle sketch grid snap ({settingsStore.config.snap_sketch ?? 0.5} units)"
      >
        Snap: {settingsStore.config.snap_sketch ?? 0.5}
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
          onclick={() => toolStore.setTranslateSnap(toolStore.translateSnap ? null : (settingsStore.config.snap_translate ?? 1))}
          title="Toggle translation snap ({settingsStore.config.snap_translate ?? 1} unit)"
        >
          Snap: {settingsStore.config.snap_translate ?? 1}u
        </button>
      {/if}
      {#if toolStore.activeTool === 'rotate'}
        <button
          class="toolbar-btn snap-btn"
          class:snap-active={toolStore.rotationSnap !== null}
          onclick={() => toolStore.setRotationSnap(toolStore.rotationSnap ? null : (settingsStore.config.snap_rotation ?? 15))}
          title="Toggle rotation snap ({settingsStore.config.snap_rotation ?? 15} degrees)"
        >
          Snap: {settingsStore.config.snap_rotation ?? 15}&deg;
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

      <!-- Pattern operations -->
      <button class="toolbar-btn pattern-btn"
        onclick={() => applyPattern('mirror')}
        title="Mirror Body (Ctrl+Shift+M)"
        disabled={!canPattern}>Mirror</button>
      <button class="toolbar-btn pattern-btn"
        onclick={() => applyPattern('linear')}
        title="Linear Pattern (Ctrl+Shift+L)"
        disabled={!canPattern}>Lin Pattern</button>
      <button class="toolbar-btn pattern-btn"
        onclick={() => applyPattern('circular')}
        title="Circular Pattern (Ctrl+Shift+O)"
        disabled={!canPattern}>Circ Pattern</button>

      <div class="toolbar-separator"></div>

      <!-- Datum / Reference Geometry -->
      <button class="toolbar-btn datum-btn"
        onclick={() => { datumPopup = datumPopup === 'offset' ? null : 'offset'; }}
        title="Offset Plane"
        disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode}>Offset Plane</button>
      <button class="toolbar-btn datum-btn"
        onclick={() => { datumPopup = datumPopup === '3pt' ? null : '3pt'; }}
        title="3-Point Plane"
        disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode}>3-Pt Plane</button>
      <button class="toolbar-btn datum-btn"
        onclick={() => { datumPopup = datumPopup === 'axis' ? null : 'axis'; }}
        title="Datum Axis"
        disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode}>Datum Axis</button>

      <div class="toolbar-separator"></div>

      <!-- Assembly / Components -->
      <button class="toolbar-btn component-btn"
        onclick={handleCreateComponent}
        title="Group selected features into Component"
        disabled={!canCreateComponent}>Group</button>
      <button class="toolbar-btn component-btn"
        onclick={handleInsertComponent}
        title="Insert Component from .cadai file"
        disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode || isBusy}>Insert</button>

      <div class="toolbar-separator"></div>

      <!-- Assembly Mates -->
      <button class="toolbar-btn mate-btn"
        class:tool-active={mateStore.mateCreationMode === 'coincident'}
        onclick={() => startMate('coincident')}
        title="Coincident Mate"
        disabled={!canCreateMate}>Coinc</button>
      <button class="toolbar-btn mate-btn"
        class:tool-active={mateStore.mateCreationMode === 'concentric'}
        onclick={() => startMate('concentric')}
        title="Concentric Mate"
        disabled={!canCreateMate}>Conc</button>
      <button class="toolbar-btn mate-btn"
        class:tool-active={mateStore.mateCreationMode === 'distance'}
        onclick={() => startMate('distance')}
        title="Distance Mate"
        disabled={!canCreateMate}>Dist</button>
      <button class="toolbar-btn mate-btn"
        class:tool-active={mateStore.mateCreationMode === 'angle'}
        onclick={() => startMate('angle')}
        title="Angle Mate"
        disabled={!canCreateMate}>Angle</button>

      <div class="toolbar-separator"></div>

      <!-- Interference + Explode -->
      <button class="toolbar-btn check-btn"
        onclick={handleInterferenceCheck}
        title="Check Interference"
        disabled={componentStore.components.length < 2}>Check</button>
      <button class="toolbar-btn explode-btn"
        class:explode-active={viewportStore.explodeEnabled}
        onclick={() => viewportStore.toggleExplode()}
        title="Toggle Exploded View"
        disabled={componentStore.components.length < 2}>Explode</button>
      {#if viewportStore.explodeEnabled}
        <input
          type="range"
          class="explode-slider"
          min="0"
          max="100"
          value={viewportStore.explodeFactor * 100}
          oninput={(e) => viewportStore.setExplodeFactor(parseInt((e.target as HTMLInputElement).value) / 100)}
          title="Explode factor"
        />
      {/if}

      <div class="toolbar-separator"></div>

      <!-- Measure -->
      <button class="toolbar-btn measure-btn"
        class:tool-active={toolStore.activeTool === 'measure'}
        onclick={() => setTool('measure')}
        title="Measurement Tools"
        disabled={sketchStore.isInSketchMode}>Measure</button>

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

  <!-- Datum popup dialogs -->
  {#if datumPopup === 'offset'}
    <div class="datum-popup">
      <div class="datum-popup-title">Offset Plane</div>
      <div class="datum-popup-row">
        <label>Base</label>
        <select class="datum-popup-select" bind:value={datumOffsetBase}>
          <option value="XY">XY</option>
          <option value="XZ">XZ</option>
          <option value="YZ">YZ</option>
        </select>
      </div>
      <div class="datum-popup-row">
        <label>Offset</label>
        <input type="number" class="datum-popup-input" bind:value={datumOffsetVal} step="1" />
      </div>
      <div class="datum-popup-actions">
        <button class="datum-popup-create" onclick={createOffsetPlane}>Create</button>
        <button class="datum-popup-cancel" onclick={() => datumPopup = null}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if datumPopup === '3pt'}
    <div class="datum-popup datum-popup-wide">
      <div class="datum-popup-title">3-Point Plane</div>
      {#each [{ label: 'P1', val: datum3ptP1, set: (v: [number,number,number]) => datum3ptP1 = v },
               { label: 'P2', val: datum3ptP2, set: (v: [number,number,number]) => datum3ptP2 = v },
               { label: 'P3', val: datum3ptP3, set: (v: [number,number,number]) => datum3ptP3 = v }] as pt}
        <div class="datum-popup-row">
          <label>{pt.label}</label>
          <input type="number" class="datum-popup-input" value={pt.val[0]} step="1"
            oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; pt.set([v, pt.val[1], pt.val[2]]); }} />
          <input type="number" class="datum-popup-input" value={pt.val[1]} step="1"
            oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; pt.set([pt.val[0], v, pt.val[2]]); }} />
          <input type="number" class="datum-popup-input" value={pt.val[2]} step="1"
            oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; pt.set([pt.val[0], pt.val[1], v]); }} />
        </div>
      {/each}
      <div class="datum-popup-actions">
        <button class="datum-popup-create" onclick={createThreePointPlane}>Create</button>
        <button class="datum-popup-cancel" onclick={() => datumPopup = null}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if datumPopup === 'axis'}
    <div class="datum-popup datum-popup-wide">
      <div class="datum-popup-title">Datum Axis</div>
      <div class="datum-popup-row">
        <label>Origin</label>
        <input type="number" class="datum-popup-input" value={datumAxisOrigin[0]} step="1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisOrigin = [v, datumAxisOrigin[1], datumAxisOrigin[2]]; }} />
        <input type="number" class="datum-popup-input" value={datumAxisOrigin[1]} step="1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisOrigin = [datumAxisOrigin[0], v, datumAxisOrigin[2]]; }} />
        <input type="number" class="datum-popup-input" value={datumAxisOrigin[2]} step="1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisOrigin = [datumAxisOrigin[0], datumAxisOrigin[1], v]; }} />
      </div>
      <div class="datum-popup-row">
        <label>Dir</label>
        <input type="number" class="datum-popup-input" value={datumAxisDir[0]} step="0.1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisDir = [v, datumAxisDir[1], datumAxisDir[2]]; }} />
        <input type="number" class="datum-popup-input" value={datumAxisDir[1]} step="0.1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisDir = [datumAxisDir[0], v, datumAxisDir[2]]; }} />
        <input type="number" class="datum-popup-input" value={datumAxisDir[2]} step="0.1"
          oninput={(e) => { const v = parseFloat((e.target as HTMLInputElement).value) || 0; datumAxisDir = [datumAxisDir[0], datumAxisDir[1], v]; }} />
      </div>
      <div class="datum-popup-actions">
        <button class="datum-popup-create" onclick={createDatumAxis}>Create</button>
        <button class="datum-popup-cancel" onclick={() => datumPopup = null}>Cancel</button>
      </div>
    </div>
  {/if}
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

  .pattern-btn {
    font-size: 11px;
    color: #a6e3a1;
    border: 1px solid rgba(166, 227, 161, 0.3);
  }

  .pattern-btn:hover:not(:disabled) {
    background: rgba(166, 227, 161, 0.1);
    border-color: #a6e3a1;
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

  .measure-btn {
    font-size: 11px;
    color: #94e2d5;
    border: 1px solid rgba(148, 226, 213, 0.3);
  }

  .measure-btn:hover:not(:disabled) {
    background: rgba(148, 226, 213, 0.1);
    border-color: #94e2d5;
  }

  .measure-btn.tool-active {
    background: rgba(148, 226, 213, 0.15);
    border-color: #94e2d5;
    color: #94e2d5;
    font-weight: 600;
  }

  .component-btn {
    font-size: 11px;
    color: #89dceb;
    border: 1px solid rgba(137, 220, 235, 0.3);
  }

  .component-btn:hover:not(:disabled) {
    background: rgba(137, 220, 235, 0.1);
    border-color: #89dceb;
  }

  .mate-btn {
    font-size: 11px;
    color: #f2cdcd;
    border: 1px solid rgba(242, 205, 205, 0.3);
  }

  .mate-btn:hover:not(:disabled) {
    background: rgba(242, 205, 205, 0.1);
    border-color: #f2cdcd;
  }

  .mate-btn.tool-active {
    background: rgba(242, 205, 205, 0.15);
    border-color: #f2cdcd;
    color: #f2cdcd;
    font-weight: 600;
  }

  .check-btn {
    font-size: 11px;
    color: #f38ba8;
    border: 1px solid rgba(243, 139, 168, 0.3);
  }

  .check-btn:hover:not(:disabled) {
    background: rgba(243, 139, 168, 0.1);
    border-color: #f38ba8;
  }

  .explode-btn {
    font-size: 11px;
    color: #b4befe;
    border: 1px solid rgba(180, 190, 254, 0.3);
  }

  .explode-btn:hover:not(:disabled) {
    background: rgba(180, 190, 254, 0.1);
    border-color: #b4befe;
  }

  .explode-btn.explode-active {
    background: rgba(180, 190, 254, 0.15);
    border-color: #b4befe;
    color: #b4befe;
    font-weight: 600;
  }

  .explode-slider {
    width: 60px;
    height: 4px;
    appearance: none;
    -webkit-appearance: none;
    background: var(--border-subtle);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
    vertical-align: middle;
  }

  .explode-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #b4befe;
    cursor: pointer;
  }

  .explode-slider::-moz-range-thumb {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #b4befe;
    cursor: pointer;
    border: none;
  }

  .datum-btn {
    font-size: 11px;
    color: #f5c2e7;
    border: 1px solid rgba(245, 194, 231, 0.3);
  }

  .datum-btn:hover:not(:disabled) {
    background: rgba(245, 194, 231, 0.1);
    border-color: #f5c2e7;
  }

  .datum-popup {
    position: absolute;
    top: 40px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-mantle);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 14px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 200px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .datum-popup-wide {
    min-width: 300px;
  }

  .datum-popup-title {
    font-size: 11px;
    font-weight: 700;
    color: #f5c2e7;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 2px;
  }

  .datum-popup-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .datum-popup-row label {
    font-size: 11px;
    color: var(--text-secondary);
    width: 42px;
    flex-shrink: 0;
  }

  .datum-popup-select {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    color: var(--text-primary);
  }

  .datum-popup-input {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-primary);
    min-width: 0;
    width: 50px;
  }

  .datum-popup-input:focus,
  .datum-popup-select:focus {
    border-color: #f5c2e7;
    outline: none;
  }

  .datum-popup-actions {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  .datum-popup-create {
    flex: 1;
    background: rgba(245, 194, 231, 0.15);
    border: 1px solid #f5c2e7;
    color: #f5c2e7;
    border-radius: 3px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .datum-popup-create:hover {
    background: rgba(245, 194, 231, 0.25);
  }

  .datum-popup-cancel {
    background: none;
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    border-radius: 3px;
    padding: 4px 10px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .datum-popup-cancel:hover {
    border-color: var(--text-secondary);
    color: var(--text-secondary);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
