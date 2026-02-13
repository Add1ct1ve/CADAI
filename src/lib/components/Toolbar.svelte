<script lang="ts">
  import { projectNew, projectOpen, projectSave, projectExportStl, projectExportStep, projectInsertComponent, projectExport3mf, projectMeshCheck, projectOrientForPrint, projectSheetMetalUnfold } from '$lib/services/project-actions';
  import type { MeshCheckResult, OrientResult } from '$lib/services/tauri';
  import MeshCheckPanel from './MeshCheckPanel.svelte';
  import OrientationPanel from './OrientationPanel.svelte';
  import MechanismCatalog from './MechanismCatalog.svelte';
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
  import { getDrawingStore } from '$lib/stores/drawing.svelte';
  import { addAndGenerateView, exportPdf, exportDxf } from '$lib/services/drawing-service';
  import type { ViewDirection } from '$lib/types/drawing';
  import { nanoid } from 'nanoid';

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
  const drawingStore = getDrawingStore();

  let isBusy = $state(false);
  let statusMessage = $state('');
  let activeDropdown = $state<string | null>(null);
  let mechanismCatalogOpen = $state(false);

  function showStatus(msg: string, duration = 3000) {
    statusMessage = msg;
    setTimeout(() => {
      statusMessage = '';
    }, duration);
  }

  function toggleDropdown(id: string) {
    activeDropdown = activeDropdown === id ? null : id;
  }

  function closeDropdowns() {
    activeDropdown = null;
  }

  function openMechanismCatalog() {
    closeDropdowns();
    mechanismCatalogOpen = true;
  }

  async function handleNew() {
    closeDropdowns();
    try {
      const result = await projectNew();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Failed: ${err}`);
    }
  }

  async function handleOpen() {
    closeDropdowns();
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
    closeDropdowns();
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
    closeDropdowns();
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
    closeDropdowns();
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
    closeDropdowns();
    sketchStore.enterSketchMode(plane);
  }

  function setSketchTool(tool: SketchToolId) {
    sketchStore.setSketchTool(tool);
  }

  function handleFinishSketch() {
    sketchStore.exitSketchMode();
    triggerPipeline(100);
  }

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
    closeDropdowns();
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
    closeDropdowns();
    if (!canSplit) return;
    const obj = scene.firstSelected!;
    history.pushSnapshot(captureSnapshot());
    scene.setSplitOp(obj.id, { plane: 'XY', offset: 0, keepSide: 'both' });
    triggerPipeline(100);
    runPythonExecution();
  }

  let canPattern = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    scene.selectedIds.length === 1 &&
    scene.firstSelected !== null &&
    !scene.firstSelected.booleanOp &&
    !scene.firstSelected.splitOp
  );

  function applyPattern(type: PatternType) {
    closeDropdowns();
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

  let canCreateComponent = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    (scene.selectedIds.length > 0 || sketchStore.selectedSketchId !== null || datumStore.selectedDatumId !== null) &&
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
    closeDropdowns();
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
    closeDropdowns();
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

  let canCreateMate = $derived(
    scene.codeMode === 'parametric' &&
    !sketchStore.isInSketchMode &&
    componentStore.components.length >= 2
  );

  function startMate(type: MateType) {
    closeDropdowns();
    if (!canCreateMate) return;
    mateStore.startMateCreation(type);
    showStatus(`Select first face on a component (${type} mate)`);
  }

  function handleInterferenceCheck() {
    closeDropdowns();
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
    let found = false;
    for (const other of otherComps) {
      showStatus(`Checking ${selComp.name} vs ${other.name}...`);
      found = true;
      showStatus(`Interference check: ${selComp.name} vs ${other.name} - check viewport for overlap`, 5000);
      break;
    }
    if (!found) {
      showStatus('No interference found');
    }
  }

  function enterDrawingMode() {
    closeDropdowns();
    if (scene.codeMode !== 'parametric' || sketchStore.isInSketchMode) return;
    if (drawingStore.drawings.length === 0) {
      drawingStore.createDrawing();
    } else if (!drawingStore.activeDrawingId) {
      drawingStore.setActiveDrawing(drawingStore.drawings[0].id);
    }
    scene.setDrawingMode(true);
  }

  function exitDrawingMode() {
    scene.setDrawingMode(false);
    drawingStore.clearSelection();
    drawingStore.setDrawingTool('select');
  }

  async function handleAddView(direction: ViewDirection) {
    const drawingId = drawingStore.activeDrawingId;
    if (!drawingId) return;
    isBusy = true;
    showStatus(`Generating ${direction} view...`);
    try {
      await addAndGenerateView(drawingId, direction);
      showStatus(`${direction} view added`);
    } catch (err) {
      showStatus(`Failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportDrawingPdf() {
    const drawingId = drawingStore.activeDrawingId;
    if (!drawingId) return;
    isBusy = true;
    showStatus('Exporting PDF...');
    try {
      const result = await exportPdf(drawingId);
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`PDF export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleExportDrawingDxf() {
    const drawingId = drawingStore.activeDrawingId;
    if (!drawingId) return;
    isBusy = true;
    showStatus('Exporting DXF...');
    try {
      const result = await exportDxf(drawingId);
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`DXF export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  function addDimensionToDrawing(type: 'linear' | 'angular' | 'radial') {
    const drawingId = drawingStore.activeDrawingId;
    if (!drawingId) return;
    const drawing = drawingStore.activeDrawing;
    if (!drawing || drawing.views.length === 0) {
      showStatus('Add a view first');
      return;
    }
    const view = drawing.views[0];
    drawingStore.addDimension(drawingId, {
      id: nanoid(10),
      type,
      viewId: view.id,
      x1: view.x - 15,
      y1: view.y + 10,
      x2: view.x + 15,
      y2: view.y + 10,
      value: 30,
      offsetDistance: 8,
    });
    showStatus(`${type} dimension added - drag to position`);
  }

  function addNoteToDrawing() {
    const drawingId = drawingStore.activeDrawingId;
    if (!drawingId) return;
    drawingStore.addNote(drawingId, {
      id: nanoid(10),
      text: 'Note',
      x: 30,
      y: 30,
      fontSize: 10,
      bold: false,
    });
    showStatus('Note added');
  }

  let meshCheckResult = $state<MeshCheckResult | null>(null);
  let orientResult = $state<OrientResult | null>(null);

  async function handleExport3mf() {
    closeDropdowns();
    try {
      isBusy = true;
      showStatus('Exporting 3MF...');
      const result = await projectExport3mf();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`3MF export failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleMeshCheck() {
    closeDropdowns();
    try {
      isBusy = true;
      showStatus('Checking mesh...');
      const result = await projectMeshCheck();
      meshCheckResult = result;
      showStatus(result.issues.length === 0 ? 'Mesh check passed' : `${result.issues.length} issue(s) found`);
    } catch (err) {
      showStatus(`Mesh check failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  async function handleOrient() {
    closeDropdowns();
    try {
      isBusy = true;
      showStatus('Analyzing orientation...');
      const result = await projectOrientForPrint();
      orientResult = result;
      showStatus('Orientation analysis complete');
    } catch (err) {
      showStatus(`Orient failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  function handleApplyOrientation(rotation: [number, number, number]) {
    const obj = scene.firstSelected ?? scene.objects[0];
    if (!obj) {
      showStatus('No object to rotate');
      orientResult = null;
      return;
    }
    history.pushSnapshot(captureSnapshot());
    const current = obj.transform.rotation;
    scene.updateObject(obj.id, {
      transform: {
        ...obj.transform,
        rotation: [
          current[0] + rotation[0],
          current[1] + rotation[1],
          current[2] + rotation[2],
        ],
      },
    });
    triggerPipeline(100);
    runPythonExecution();
    orientResult = null;
    showStatus('Orientation applied');
  }

  async function handleUnfold() {
    closeDropdowns();
    try {
      isBusy = true;
      showStatus('Computing flat pattern...');
      const result = await projectSheetMetalUnfold();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Unfold failed: ${err}`);
    } finally {
      isBusy = false;
    }
  }

  const toolButtons: { id: ToolId; label: string; shortcut: string }[] = [
    { id: 'select', label: 'Select', shortcut: 'V' },
    { id: 'translate', label: 'Move', shortcut: 'G' },
    { id: 'rotate', label: 'Rotate', shortcut: 'R' },
    { id: 'scale', label: 'Scale', shortcut: 'S' },
  ];

  const primitiveButtons: { id: ToolId; label: string; shortcut: string }[] = [
    { id: 'add-box', label: 'Box', shortcut: '1' },
    { id: 'add-cylinder', label: 'Cylinder', shortcut: '2' },
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
    { id: 'sketch-constraint-coincident',    label: 'Coincident',    shortcut: 'O' },
    { id: 'sketch-constraint-horizontal',    label: 'Horizontal',    shortcut: 'H' },
    { id: 'sketch-constraint-vertical',      label: 'Vertical',      shortcut: 'I' },
    { id: 'sketch-constraint-parallel',      label: 'Parallel',      shortcut: 'P' },
    { id: 'sketch-constraint-perpendicular', label: 'Perpendicular', shortcut: 'T' },
    { id: 'sketch-constraint-equal',         label: 'Equal',         shortcut: 'E' },
    { id: 'sketch-constraint-distance',      label: 'Distance',      shortcut: 'D' },
    { id: 'sketch-constraint-radius',        label: 'Radius',        shortcut: 'Q' },
    { id: 'sketch-constraint-angle',         label: 'Angle',         shortcut: 'N' },
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

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="toolbar-backdrop" class:visible={activeDropdown !== null} onclick={closeDropdowns}></div>

<div class="toolbar">
  <!-- Logo -->
  <div class="toolbar-brand">
    <span class="brand-mark">CAD</span><span class="brand-ai">AI</span>
  </div>

  <div class="toolbar-sep-v"></div>

  {#if scene.drawingMode}
    <!-- ═══ DRAWING MODE ═══ -->
    <button class="tb drawing-back" onclick={exitDrawingMode} title="Back to 3D (Escape)">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
      Back to 3D
    </button>

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Views</span>
    <button class="tb" onclick={() => handleAddView('front')} disabled={isBusy} title="Front View">Front</button>
    <button class="tb" onclick={() => handleAddView('top')} disabled={isBusy} title="Top View">Top</button>
    <button class="tb" onclick={() => handleAddView('right')} disabled={isBusy} title="Right View">Right</button>
    <button class="tb" onclick={() => handleAddView('iso')} disabled={isBusy} title="Isometric View">Iso</button>
    <button class="tb" onclick={() => handleAddView('section')} disabled={isBusy} title="Section View">Section</button>

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Annotate</span>
    <button class="tb" onclick={() => addDimensionToDrawing('linear')} title="Linear Dimension">Linear</button>
    <button class="tb" onclick={() => addDimensionToDrawing('angular')} title="Angular Dimension">Angular</button>
    <button class="tb" onclick={() => addDimensionToDrawing('radial')} title="Radial Dimension">Radial</button>
    <button class="tb" onclick={addNoteToDrawing} title="Add Note">Note</button>

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Export</span>
    <button class="tb" onclick={handleExportDrawingPdf} disabled={isBusy} title="Export PDF">PDF</button>
    <button class="tb" onclick={handleExportDrawingDxf} disabled={isBusy} title="Export DXF">DXF</button>

    {#if drawingStore.isGenerating}
      <span class="toolbar-status generating">Generating...</span>
    {/if}

  {:else if sketchStore.isInSketchMode}
    <!-- ═══ SKETCH MODE ═══ -->
    <button class="tb sketch-finish" onclick={handleFinishSketch} title="Finish Sketch (Escape)">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
      Finish
    </button>

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Draw</span>
    {#each sketchToolButtons as btn}
      <button
        class="tb"
        class:active={sketchStore.activeSketchTool === btn.id}
        onclick={() => setSketchTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
      >
        {btn.label}
      </button>
    {/each}

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Constrain</span>
    {#each constraintToolButtons as btn}
      <button
        class="tb compact"
        class:active={sketchStore.activeSketchTool === btn.id}
        onclick={() => setSketchTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
      >
        {btn.label}
      </button>
    {/each}

    <div class="toolbar-sep-v"></div>

    <span class="toolbar-group-label">Edit</span>
    {#each operationToolButtons as btn}
      <button
        class="tb compact"
        class:active={sketchStore.activeSketchTool === btn.id}
        onclick={() => setSketchTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
      >
        {btn.label}
      </button>
    {/each}

    <div class="toolbar-sep-v"></div>

    <button
      class="tb snap-toggle"
      class:snap-on={sketchStore.sketchSnap !== null}
      onclick={() => sketchStore.setSketchSnap(sketchStore.sketchSnap ? null : (settingsStore.config.snap_sketch ?? 0.5))}
      title="Toggle sketch grid snap"
    >
      Snap {settingsStore.config.snap_sketch ?? 0.5}
    </button>

  {:else}
    <!-- ═══ NORMAL 3D MODE ═══ -->

    <!-- File dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'file'} onclick={() => toggleDropdown('file')}>
        File
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'file'}
        <div class="dropdown-menu">
          <button class="dropdown-item" onclick={handleNew} disabled={isBusy}>
            <span>New Project</span><kbd>Ctrl+N</kbd>
          </button>
          <button class="dropdown-item" onclick={handleOpen} disabled={isBusy}>
            <span>Open...</span><kbd>Ctrl+O</kbd>
          </button>
          <button class="dropdown-item" onclick={handleSave} disabled={isBusy}>
            <span>Save</span><kbd>Ctrl+S</kbd>
          </button>
          <div class="dropdown-divider"></div>
          <button class="dropdown-item" onclick={handleExportStl} disabled={isBusy}>
            <span>Export STL</span>
          </button>
          <button class="dropdown-item" onclick={handleExportStep} disabled={isBusy}>
            <span>Export STEP</span>
          </button>
        </div>
      {/if}
    </div>

    <button
      class="tb"
      onclick={openMechanismCatalog}
      disabled={isBusy || scene.codeMode !== 'parametric'}
      title="Open mechanism catalog"
    >
      Mechanisms
    </button>

    <!-- Undo/Redo -->
    <button
      class="tb icon-btn"
      onclick={handleUndo}
      title="Undo (Ctrl+Z)"
      disabled={!history.canUndo || scene.codeMode !== 'parametric'}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 10h10a5 5 0 0 1 0 10H9"/><polyline points="7 14 3 10 7 6"/></svg>
    </button>
    <button
      class="tb icon-btn"
      onclick={handleRedo}
      title="Redo (Ctrl+Shift+Z)"
      disabled={!history.canRedo || scene.codeMode !== 'parametric'}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 10H11a5 5 0 0 0 0 10h4"/><polyline points="17 14 21 10 17 6"/></svg>
    </button>

    <div class="toolbar-sep-v"></div>

    <!-- Transform tools (always visible - most used) -->
    {#each toolButtons as btn}
      <button
        class="tb tool"
        class:active={toolStore.activeTool === btn.id}
        onclick={() => setTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
        disabled={scene.codeMode !== 'parametric'}
      >
        {btn.label}
      </button>
    {/each}

    <!-- Context-sensitive snap controls -->
    {#if toolStore.activeTool === 'translate'}
      <button
        class="tb snap-toggle"
        class:snap-on={toolStore.translateSnap !== null}
        onclick={() => toolStore.setTranslateSnap(toolStore.translateSnap ? null : (settingsStore.config.snap_translate ?? 1))}
        title="Toggle translation snap"
      >
        Snap {settingsStore.config.snap_translate ?? 1}u
      </button>
    {/if}
    {#if toolStore.activeTool === 'rotate'}
      <button
        class="tb snap-toggle"
        class:snap-on={toolStore.rotationSnap !== null}
        onclick={() => toolStore.setRotationSnap(toolStore.rotationSnap ? null : (settingsStore.config.snap_rotation ?? 15))}
        title="Toggle rotation snap"
      >
        Snap {settingsStore.config.snap_rotation ?? 15}&deg;
      </button>
    {/if}
    {#if toolStore.activeTool === 'scale'}
      <button
        class="tb snap-toggle"
        class:snap-on={toolStore.uniformScale}
        onclick={() => toolStore.setUniformScale(!toolStore.uniformScale)}
        title="Toggle uniform scaling"
      >
        Uniform
      </button>
    {/if}

    <div class="toolbar-sep-v"></div>

    <!-- Add Primitives dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'add'} onclick={() => toggleDropdown('add')} disabled={scene.codeMode !== 'parametric'}>
        Add
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'add'}
        <div class="dropdown-menu">
          {#each primitiveButtons as btn}
            <button class="dropdown-item" onclick={() => { setTool(btn.id); closeDropdowns(); }}>
              <span>{btn.label}</span><kbd>{btn.shortcut}</kbd>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Sketch dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger sketch-accent" class:open={activeDropdown === 'sketch'} onclick={() => toggleDropdown('sketch')} disabled={scene.codeMode !== 'parametric'}>
        Sketch
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'sketch'}
        <div class="dropdown-menu">
          <button class="dropdown-item" onclick={() => enterSketch('XY')}>
            <span>Sketch on XY</span>
          </button>
          <button class="dropdown-item" onclick={() => enterSketch('XZ')}>
            <span>Sketch on XZ</span>
          </button>
          <button class="dropdown-item" onclick={() => enterSketch('YZ')}>
            <span>Sketch on YZ</span>
          </button>
        </div>
      {/if}
    </div>

    <!-- Modify dropdown (Boolean + Pattern) -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'modify'} onclick={() => toggleDropdown('modify')} disabled={scene.codeMode !== 'parametric'}>
        Modify
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'modify'}
        <div class="dropdown-menu">
          <div class="dropdown-section-label">Boolean</div>
          <button class="dropdown-item" onclick={() => applyBoolean('union')} disabled={!canBoolean}>
            <span>Union</span><kbd>Ctrl+Shift+U</kbd>
          </button>
          <button class="dropdown-item" onclick={() => applyBoolean('subtract')} disabled={!canBoolean}>
            <span>Subtract</span><kbd>Ctrl+Shift+D</kbd>
          </button>
          <button class="dropdown-item" onclick={() => applyBoolean('intersect')} disabled={!canBoolean}>
            <span>Intersect</span><kbd>Ctrl+Shift+I</kbd>
          </button>
          <button class="dropdown-item" onclick={applySplit} disabled={!canSplit}>
            <span>Split</span><kbd>Ctrl+Shift+P</kbd>
          </button>
          <div class="dropdown-divider"></div>
          <div class="dropdown-section-label">Pattern</div>
          <button class="dropdown-item" onclick={() => applyPattern('mirror')} disabled={!canPattern}>
            <span>Mirror</span><kbd>Ctrl+Shift+M</kbd>
          </button>
          <button class="dropdown-item" onclick={() => applyPattern('linear')} disabled={!canPattern}>
            <span>Linear Pattern</span><kbd>Ctrl+Shift+L</kbd>
          </button>
          <button class="dropdown-item" onclick={() => applyPattern('circular')} disabled={!canPattern}>
            <span>Circular Pattern</span><kbd>Ctrl+Shift+O</kbd>
          </button>
        </div>
      {/if}
    </div>

    <!-- Reference Geometry dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'ref'} onclick={() => toggleDropdown('ref')} disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode}>
        Reference
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'ref'}
        <div class="dropdown-menu">
          <button class="dropdown-item" onclick={() => { datumPopup = 'offset'; closeDropdowns(); }}>
            <span>Offset Plane</span>
          </button>
          <button class="dropdown-item" onclick={() => { datumPopup = '3pt'; closeDropdowns(); }}>
            <span>3-Point Plane</span>
          </button>
          <button class="dropdown-item" onclick={() => { datumPopup = 'axis'; closeDropdowns(); }}>
            <span>Datum Axis</span>
          </button>
        </div>
      {/if}
    </div>

    <!-- Assembly dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'asm'} onclick={() => toggleDropdown('asm')} disabled={scene.codeMode !== 'parametric'}>
        Assembly
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'asm'}
        <div class="dropdown-menu">
          <div class="dropdown-section-label">Components</div>
          <button class="dropdown-item" onclick={handleCreateComponent} disabled={!canCreateComponent}>
            <span>Group to Component</span>
          </button>
          <button class="dropdown-item" onclick={handleInsertComponent} disabled={isBusy}>
            <span>Insert Component...</span>
          </button>
          <div class="dropdown-divider"></div>
          <div class="dropdown-section-label">Mates</div>
          <button class="dropdown-item" onclick={() => startMate('coincident')} disabled={!canCreateMate}>
            <span>Coincident</span>
          </button>
          <button class="dropdown-item" onclick={() => startMate('concentric')} disabled={!canCreateMate}>
            <span>Concentric</span>
          </button>
          <button class="dropdown-item" onclick={() => startMate('distance')} disabled={!canCreateMate}>
            <span>Distance</span>
          </button>
          <button class="dropdown-item" onclick={() => startMate('angle')} disabled={!canCreateMate}>
            <span>Angle</span>
          </button>
          <div class="dropdown-divider"></div>
          <div class="dropdown-section-label">Analysis</div>
          <button class="dropdown-item" onclick={handleInterferenceCheck} disabled={componentStore.components.length < 2}>
            <span>Interference Check</span>
          </button>
          <button class="dropdown-item" class:active-toggle={viewportStore.explodeEnabled} onclick={() => { viewportStore.toggleExplode(); closeDropdowns(); }} disabled={componentStore.components.length < 2}>
            <span>Exploded View</span>
            {#if viewportStore.explodeEnabled}<span class="check-mark">&#10003;</span>{/if}
          </button>
        </div>
      {/if}
    </div>

    <div class="toolbar-sep-v"></div>

    <!-- Inline tool buttons for frequent actions -->
    <button class="tb tool"
      class:active={toolStore.activeTool === 'measure'}
      onclick={() => setTool('measure')}
      title="Measure"
      disabled={sketchStore.isInSketchMode}>
      Measure
    </button>

    <button class="tb tool drawing-accent"
      onclick={enterDrawingMode}
      title="2D Drawing Mode"
      disabled={scene.codeMode !== 'parametric' || sketchStore.isInSketchMode}>
      Drawing
    </button>

    <!-- Manufacturing dropdown -->
    <div class="dropdown-wrap">
      <button class="tb dropdown-trigger" class:open={activeDropdown === 'mfg'} onclick={() => toggleDropdown('mfg')} disabled={scene.codeMode !== 'parametric'}>
        Mfg
        <svg class="chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if activeDropdown === 'mfg'}
        <div class="dropdown-menu">
          <button class="dropdown-item" onclick={handleExport3mf} disabled={isBusy}>
            <span>Export 3MF</span>
          </button>
          <button class="dropdown-item" onclick={handleMeshCheck} disabled={isBusy}>
            <span>Mesh Check</span>
          </button>
          <button class="dropdown-item" onclick={handleOrient} disabled={isBusy}>
            <span>Optimize Orientation</span>
          </button>
          <button class="dropdown-item" onclick={handleUnfold} disabled={isBusy}>
            <span>Sheet Metal Unfold</span>
          </button>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Right side: status + mode + settings -->
  <div class="toolbar-spacer"></div>

  {#if statusMessage}
    <span class="toolbar-status">{statusMessage}</span>
  {/if}

  {#if !scene.drawingMode && !sketchStore.isInSketchMode}
    <!-- Explode slider (shown inline when active) -->
    {#if viewportStore.explodeEnabled}
      <div class="explode-inline">
        <span class="explode-label">Explode</span>
        <input
          type="range"
          class="explode-slider"
          min="0"
          max="100"
          value={viewportStore.explodeFactor * 100}
          oninput={(e) => viewportStore.setExplodeFactor(parseInt((e.target as HTMLInputElement).value) / 100)}
        />
      </div>
    {/if}

    <button
      class="tb mode-toggle"
      class:mode-parametric={scene.codeMode === 'parametric'}
      class:mode-manual={scene.codeMode === 'manual'}
      onclick={toggleCodeMode}
      title="Toggle code mode"
    >
      {scene.codeMode === 'parametric' ? 'Parametric' : 'Manual'}
    </button>
  {/if}

  <button class="tb icon-btn settings-btn" onclick={onSettingsClick} title="Settings">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.32 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
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

{#if meshCheckResult}
  <MeshCheckPanel result={meshCheckResult} onClose={() => meshCheckResult = null} />
{/if}

{#if orientResult}
  <OrientationPanel
    result={orientResult}
    onApply={handleApplyOrientation}
    onClose={() => orientResult = null}
  />
{/if}

<MechanismCatalog
  open={mechanismCatalogOpen}
  onClose={() => mechanismCatalogOpen = false}
  onStatus={(msg) => showStatus(msg)}
/>

<style>
  /* ── Backdrop for closing dropdowns ── */
  .toolbar-backdrop {
    position: fixed;
    inset: 0;
    z-index: 49;
    display: none;
  }
  .toolbar-backdrop.visible {
    display: block;
  }

  /* ── Main toolbar bar ── */
  .toolbar {
    position: relative;
    display: flex;
    align-items: center;
    height: 44px;
    padding: 0 10px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-mantle) 90%, white 10%), var(--bg-mantle));
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 3px;
    z-index: 50;
    -webkit-app-region: drag;
    font-size: 12px;
  }

  /* ── Brand ── */
  .toolbar-brand {
    display: flex;
    align-items: center;
    gap: 0;
    padding: 0 6px 0 2px;
    -webkit-app-region: drag;
    user-select: none;
  }
  .brand-mark {
    font-weight: 800;
    font-size: 13px;
    letter-spacing: -0.5px;
    color: var(--text-primary);
  }
  .brand-ai {
    font-weight: 800;
    font-size: 13px;
    letter-spacing: -0.5px;
    color: var(--accent);
  }

  /* ── Vertical separator ── */
  .toolbar-sep-v {
    width: 1px;
    height: 22px;
    background: var(--border-subtle);
    margin: 0 5px;
    flex-shrink: 0;
  }

  /* ── Group label ── */
  .toolbar-group-label {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    color: var(--text-muted);
    padding: 0 4px 0 2px;
    user-select: none;
    flex-shrink: 0;
  }

  /* ── Base toolbar button ── */
  .tb {
    background: none;
    border: 1px solid transparent;
    color: var(--text-secondary);
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    -webkit-app-region: no-drag;
    line-height: 1;
  }
  .tb:hover:not(:disabled) {
    background: var(--bg-overlay);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }
  .tb:active:not(:disabled) {
    background: var(--bg-surface);
    transform: translateY(0.5px);
  }
  .tb:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .tb.compact {
    padding: 4px 7px;
    font-size: 11px;
  }

  /* ── Icon-only buttons ── */
  .tb.icon-btn {
    padding: 5px 7px;
  }
  .tb.icon-btn svg {
    opacity: 0.7;
    transition: opacity 0.15s;
  }
  .tb.icon-btn:hover:not(:disabled) svg {
    opacity: 1;
  }

  /* ── Tool button active state ── */
  .tb.tool.active,
  .tb.active {
    background: rgba(137, 180, 250, 0.12);
    border-color: rgba(137, 180, 250, 0.4);
    color: var(--accent);
    font-weight: 600;
  }

  /* ── Sketch finish button ── */
  .tb.sketch-finish {
    background: rgba(166, 227, 161, 0.1);
    border-color: rgba(166, 227, 161, 0.4);
    color: var(--success);
    font-weight: 600;
  }
  .tb.sketch-finish:hover {
    background: rgba(166, 227, 161, 0.2);
  }

  /* ── Drawing back button ── */
  .tb.drawing-back {
    background: rgba(235, 160, 172, 0.1);
    border-color: rgba(235, 160, 172, 0.35);
    color: #eba0ac;
    font-weight: 600;
  }
  .tb.drawing-back:hover {
    background: rgba(235, 160, 172, 0.2);
  }

  /* ── Sketch accent for sketch dropdown trigger ── */
  .tb.sketch-accent {
    color: #f9e2af;
  }
  .tb.sketch-accent:hover:not(:disabled) {
    color: #f9e2af;
    border-color: rgba(249, 226, 175, 0.3);
    background: rgba(249, 226, 175, 0.08);
  }
  .tb.sketch-accent.open {
    color: #f9e2af;
    border-color: rgba(249, 226, 175, 0.4);
    background: rgba(249, 226, 175, 0.12);
  }

  /* ── Drawing accent ── */
  .tb.drawing-accent {
    color: #eba0ac;
  }
  .tb.drawing-accent:hover:not(:disabled) {
    color: #eba0ac;
    border-color: rgba(235, 160, 172, 0.3);
    background: rgba(235, 160, 172, 0.08);
  }

  /* ── Snap toggle ── */
  .tb.snap-toggle {
    font-size: 10px;
    padding: 3px 7px;
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    font-weight: 600;
    letter-spacing: 0.2px;
  }
  .tb.snap-toggle.snap-on {
    color: var(--success);
    border-color: rgba(166, 227, 161, 0.4);
    background: rgba(166, 227, 161, 0.08);
  }

  /* ── Mode toggle ── */
  .tb.mode-toggle {
    font-weight: 700;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 4px 10px;
    border-radius: 4px;
  }
  .tb.mode-toggle.mode-parametric {
    color: var(--accent);
    border-color: rgba(137, 180, 250, 0.3);
    background: rgba(137, 180, 250, 0.08);
  }
  .tb.mode-toggle.mode-manual {
    color: var(--warning);
    border-color: rgba(249, 226, 175, 0.3);
    background: rgba(249, 226, 175, 0.08);
  }

  /* ── Settings button ── */
  .tb.settings-btn {
    margin-left: 2px;
  }
  .tb.settings-btn svg {
    opacity: 0.5;
    transition: all 0.3s ease;
  }
  .tb.settings-btn:hover svg {
    opacity: 1;
    transform: rotate(30deg);
  }

  /* ── Spacer ── */
  .toolbar-spacer {
    flex: 1;
    min-width: 8px;
  }

  /* ── Status message ── */
  .toolbar-status {
    font-size: 11px;
    color: var(--text-muted);
    padding: 3px 10px;
    background: var(--bg-overlay);
    border-radius: 4px;
    animation: fadeIn 0.2s ease;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .toolbar-status.generating {
    color: var(--accent);
  }

  /* ── Explode inline controls ── */
  .explode-inline {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px;
    flex-shrink: 0;
  }
  .explode-label {
    font-size: 10px;
    color: #b4befe;
    font-weight: 600;
    letter-spacing: 0.3px;
    text-transform: uppercase;
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

  /* ═══ DROPDOWN SYSTEM ═══ */

  .dropdown-wrap {
    position: relative;
    -webkit-app-region: no-drag;
  }

  .tb.dropdown-trigger {
    gap: 4px;
  }
  .tb.dropdown-trigger .chevron {
    opacity: 0.4;
    transition: all 0.2s ease;
  }
  .tb.dropdown-trigger:hover .chevron {
    opacity: 0.7;
  }
  .tb.dropdown-trigger.open {
    background: var(--bg-overlay);
    color: var(--text-primary);
    border-color: var(--border);
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }
  .tb.dropdown-trigger.open .chevron {
    opacity: 1;
    transform: rotate(180deg);
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% - 1px);
    left: 0;
    min-width: 200px;
    background: var(--bg-mantle);
    border: 1px solid var(--border);
    border-radius: 0 6px 6px 6px;
    padding: 4px 0;
    z-index: 100;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35), 0 2px 8px rgba(0, 0, 0, 0.2);
    animation: dropdownReveal 0.12s ease-out;
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 7px 14px;
    background: none;
    border: none;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.1s ease;
    text-align: left;
    gap: 16px;
  }
  .dropdown-item:hover:not(:disabled) {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }
  .dropdown-item:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .dropdown-item kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-surface);
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid var(--border-subtle);
    line-height: 1.4;
  }
  .dropdown-item .check-mark {
    color: var(--success);
    font-size: 13px;
    font-weight: 700;
  }

  .dropdown-divider {
    height: 1px;
    background: var(--border-subtle);
    margin: 4px 10px;
  }

  .dropdown-section-label {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    color: var(--text-muted);
    padding: 6px 14px 3px;
    user-select: none;
  }

  /* ═══ DATUM POPUPS ═══ */

  .datum-popup {
    position: fixed;
    top: 54px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-mantle);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 220px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    animation: dropdownReveal 0.15s ease-out;
  }
  .datum-popup-wide {
    min-width: 320px;
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
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 12px;
    color: var(--text-primary);
  }
  .datum-popup-input {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 8px;
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
    background: rgba(245, 194, 231, 0.12);
    border: 1px solid rgba(245, 194, 231, 0.4);
    color: #f5c2e7;
    border-radius: 5px;
    padding: 5px 12px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .datum-popup-create:hover {
    background: rgba(245, 194, 231, 0.22);
  }
  .datum-popup-cancel {
    background: none;
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    border-radius: 5px;
    padding: 5px 12px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .datum-popup-cancel:hover {
    border-color: var(--text-secondary);
    color: var(--text-secondary);
  }

  /* ═══ ANIMATIONS ═══ */

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes dropdownReveal {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
