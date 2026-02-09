<script lang="ts">
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import type { PrimitiveParams, EdgeSelector, ExtrudeParams, FilletParams, ChamferParams, SketchConstraint } from '$lib/types/cad';

  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Move-by-delta state
  let deltaX = $state(0);
  let deltaY = $state(0);
  let deltaZ = $state(0);

  // Scale factor state
  let scaleFactor = $state(1.0);

  const edgeSelectorOptions: { value: EdgeSelector; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'top', label: 'Top' },
    { value: 'bottom', label: 'Bottom' },
    { value: 'vertical', label: 'Vertical' },
  ];

  function debounced(fn: () => void, ms = 300) {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(fn, ms);
  }

  // Full snapshot helper for undo
  function captureSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    return {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
    };
  }

  function updateName(e: Event) {
    const obj = scene.firstSelected;
    if (!obj) return;
    const value = (e.target as HTMLInputElement).value;
    scene.updateObject(obj.id, { name: value });
  }

  // Capture snapshot before property panel mutations (debounced to avoid excessive snapshots)
  let snapshotCaptured = false;
  function captureOnce() {
    if (!snapshotCaptured) {
      history.pushSnapshot(captureSnapshot());
      snapshotCaptured = true;
      setTimeout(() => { snapshotCaptured = false; }, 500);
    }
  }

  function triggerAndRun() {
    triggerPipeline(100);
    debounced(() => runPythonExecution());
  }

  function updateParam(key: string, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const newParams = { ...obj.params, [key]: value } as PrimitiveParams;
    scene.updateParams(obj.id, newParams);
    debounced(() => triggerPipeline());
  }

  function updatePosition(axis: 0 | 1 | 2, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const pos = [...obj.transform.position] as [number, number, number];
    pos[axis] = value;
    scene.updateTransform(obj.id, { ...obj.transform, position: pos });
    debounced(() => triggerPipeline());
  }

  function updateRotation(axis: 0 | 1 | 2, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const rot = [...obj.transform.rotation] as [number, number, number];
    rot[axis] = value;
    scene.updateTransform(obj.id, { ...obj.transform, rotation: rot });
    debounced(() => triggerPipeline());
  }

  function updateColor(e: Event) {
    const obj = scene.firstSelected;
    if (!obj) return;
    const value = (e.target as HTMLInputElement).value;
    scene.updateObject(obj.id, { color: value });
  }

  function toggleVisible() {
    const obj = scene.firstSelected;
    if (!obj) return;
    scene.updateObject(obj.id, { visible: !obj.visible });
    debounced(() => triggerPipeline());
  }

  function deleteObject() {
    history.pushSnapshot(captureSnapshot());
    scene.deleteSelected();
    triggerPipeline(100);
  }

  function applyMoveDelta() {
    const obj = scene.firstSelected;
    if (!obj) return;
    history.pushSnapshot(captureSnapshot());
    const pos = obj.transform.position;
    const newPos: [number, number, number] = [pos[0] + deltaX, pos[1] + deltaY, pos[2] + deltaZ];
    scene.updateTransform(obj.id, { ...obj.transform, position: newPos });
    deltaX = 0;
    deltaY = 0;
    deltaZ = 0;
    triggerPipeline(100);
  }

  function applyScaleFactor() {
    const obj = scene.firstSelected;
    if (!obj || scaleFactor <= 0) return;
    history.pushSnapshot(captureSnapshot());
    const p = obj.params;
    let newParams: PrimitiveParams;
    switch (p.type) {
      case 'box':
        newParams = { ...p, width: p.width * scaleFactor, depth: p.depth * scaleFactor, height: p.height * scaleFactor };
        break;
      case 'cylinder':
        newParams = { ...p, radius: p.radius * scaleFactor, height: p.height * scaleFactor };
        break;
      case 'sphere':
        newParams = { ...p, radius: p.radius * scaleFactor };
        break;
      case 'cone':
        newParams = { ...p, bottomRadius: p.bottomRadius * scaleFactor, topRadius: p.topRadius * scaleFactor, height: p.height * scaleFactor };
        break;
    }
    scene.updateParams(obj.id, newParams);
    scaleFactor = 1.0;
    triggerPipeline(100);
  }

  function numInput(e: Event, callback: (v: number) => void) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(value)) callback(value);
  }

  // ── Object Fillet/Chamfer ──

  function addObjectFillet() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setFillet(obj.id, { radius: 1, edges: 'all' });
    triggerAndRun();
  }

  function updateObjectFillet(key: 'radius' | 'edges', value: number | EdgeSelector) {
    const obj = scene.firstSelected;
    if (!obj || !obj.fillet) return;
    captureOnce();
    scene.setFillet(obj.id, { ...obj.fillet, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectFillet() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setFillet(obj.id, undefined);
    triggerAndRun();
  }

  function addObjectChamfer() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setChamfer(obj.id, { distance: 0.5, edges: 'all' });
    triggerAndRun();
  }

  function updateObjectChamfer(key: 'distance' | 'edges', value: number | EdgeSelector) {
    const obj = scene.firstSelected;
    if (!obj || !obj.chamfer) return;
    captureOnce();
    scene.setChamfer(obj.id, { ...obj.chamfer, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectChamfer() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setChamfer(obj.id, undefined);
    triggerAndRun();
  }

  // ── Sketch Extrude/Fillet/Chamfer ──

  function addSketchExtrude() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setExtrude(sketch.id, { distance: 10, mode: 'add' });
    triggerAndRun();
  }

  function updateSketchExtrudeDistance(value: number) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.extrude) return;
    captureOnce();
    sketchStore.setExtrude(sketch.id, { ...sketch.extrude, distance: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function updateSketchExtrudeMode(mode: 'add' | 'cut') {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.extrude) return;
    captureOnce();
    sketchStore.setExtrude(sketch.id, { ...sketch.extrude, mode });
    triggerAndRun();
  }

  function updateSketchCutTarget(targetId: string) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.extrude) return;
    captureOnce();
    sketchStore.setExtrude(sketch.id, { ...sketch.extrude, cutTargetId: targetId || undefined });
    triggerAndRun();
  }

  function removeSketchExtrude() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setExtrude(sketch.id, undefined);
    sketchStore.setSketchFillet(sketch.id, undefined);
    sketchStore.setSketchChamfer(sketch.id, undefined);
    triggerAndRun();
  }

  function addSketchFillet() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, { radius: 1, edges: 'all' });
    triggerAndRun();
  }

  function updateSketchFillet(key: 'radius' | 'edges', value: number | EdgeSelector) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.fillet) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, { ...sketch.fillet, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchFillet() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, undefined);
    triggerAndRun();
  }

  function addSketchChamfer() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, { distance: 0.5, edges: 'all' });
    triggerAndRun();
  }

  function updateSketchChamfer(key: 'distance' | 'edges', value: number | EdgeSelector) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.chamfer) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, { ...sketch.chamfer, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchChamfer() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, undefined);
    triggerAndRun();
  }

  function removeConstraint(constraintId: string) {
    captureOnce();
    sketchStore.removeConstraint(constraintId);
    triggerPipeline(100);
  }

  function constraintLabel(c: SketchConstraint): string {
    switch (c.type) {
      case 'coincident': return 'Coincident';
      case 'horizontal': return 'Horizontal';
      case 'vertical': return 'Vertical';
      case 'parallel': return 'Parallel';
      case 'perpendicular': return 'Perpendicular';
      case 'equal': return 'Equal';
      case 'distance': return `Distance: ${c.value}`;
      case 'radius': return `Radius: ${c.value}`;
      case 'angle': return `Angle: ${c.value}\u00B0`;
    }
  }

  function editSketch() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    sketchStore.editSketch(sketch.id);
  }

  function deleteSketch() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    history.pushSnapshot(captureSnapshot());
    sketchStore.removeSketch(sketch.id);
    triggerAndRun();
  }

  // Build list of possible cut targets (other extruded sketches + visible primitives)
  function getCutTargets() {
    const sketch = sketchStore.selectedSketch;
    const targets: { id: string; name: string }[] = [];

    // Extruded add-mode sketches (excluding current)
    for (const s of sketchStore.sketches) {
      if (s.id === sketch?.id) continue;
      if (s.extrude && s.extrude.mode === 'add' && s.entities.length > 0) {
        targets.push({ id: s.id, name: s.name });
      }
    }

    // Visible scene objects
    for (const obj of scene.objects) {
      if (obj.visible) {
        targets.push({ id: obj.id, name: obj.name });
      }
    }

    return targets;
  }
</script>

{#if scene.firstSelected}
  {@const obj = scene.firstSelected}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge">{obj.params.type}</span>
      <input
        class="prop-name-input"
        type="text"
        value={obj.name}
        oninput={updateName}
      />
    </div>

    <!-- Dimensions -->
    <div class="prop-section">
      <div class="prop-section-title">Dimensions</div>

      {#if obj.params.type === 'box'}
        <div class="prop-row">
          <label>Width</label>
          <input type="number" value={obj.params.width} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('width', v))} />
        </div>
        <div class="prop-row">
          <label>Depth</label>
          <input type="number" value={obj.params.depth} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('depth', v))} />
        </div>
        <div class="prop-row">
          <label>Height</label>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {:else if obj.params.type === 'cylinder'}
        <div class="prop-row">
          <label>Radius</label>
          <input type="number" value={obj.params.radius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('radius', v))} />
        </div>
        <div class="prop-row">
          <label>Height</label>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {:else if obj.params.type === 'sphere'}
        <div class="prop-row">
          <label>Radius</label>
          <input type="number" value={obj.params.radius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('radius', v))} />
        </div>
      {:else if obj.params.type === 'cone'}
        <div class="prop-row">
          <label>Bottom R</label>
          <input type="number" value={obj.params.bottomRadius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('bottomRadius', v))} />
        </div>
        <div class="prop-row">
          <label>Top R</label>
          <input type="number" value={obj.params.topRadius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('topRadius', v))} />
        </div>
        <div class="prop-row">
          <label>Height</label>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {/if}
    </div>

    <!-- Scale Factor -->
    <div class="prop-section">
      <div class="prop-section-title">Scale</div>
      <div class="prop-row">
        <label>Factor</label>
        <input type="number" bind:value={scaleFactor} step="0.1" min="0.01" />
        <button class="apply-btn" onclick={applyScaleFactor}>Apply</button>
      </div>
    </div>

    <!-- Position -->
    <div class="prop-section">
      <div class="prop-section-title">Position</div>
      <div class="prop-row">
        <label>X</label>
        <input type="number" value={obj.transform.position[0]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(0, v))} />
      </div>
      <div class="prop-row">
        <label>Y</label>
        <input type="number" value={obj.transform.position[1]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(1, v))} />
      </div>
      <div class="prop-row">
        <label>Z</label>
        <input type="number" value={obj.transform.position[2]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(2, v))} />
      </div>
    </div>

    <!-- Move By -->
    <div class="prop-section">
      <div class="prop-section-title">Move By</div>
      <div class="prop-row">
        <label>dX</label>
        <input type="number" bind:value={deltaX} step="1" />
      </div>
      <div class="prop-row">
        <label>dY</label>
        <input type="number" bind:value={deltaY} step="1" />
      </div>
      <div class="prop-row">
        <label>dZ</label>
        <input type="number" bind:value={deltaZ} step="1" />
      </div>
      <button class="apply-btn full-width" onclick={applyMoveDelta}>Apply Move</button>
    </div>

    <!-- Rotation -->
    <div class="prop-section">
      <div class="prop-section-title">Rotation</div>
      <div class="prop-row">
        <label>X</label>
        <input type="number" value={obj.transform.rotation[0]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(0, v))} />
      </div>
      <div class="prop-row">
        <label>Y</label>
        <input type="number" value={obj.transform.rotation[1]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(1, v))} />
      </div>
      <div class="prop-row">
        <label>Z</label>
        <input type="number" value={obj.transform.rotation[2]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(2, v))} />
      </div>
    </div>

    <!-- Fillet (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Fillet</div>
      {#if obj.fillet}
        <div class="prop-row">
          <label>Radius</label>
          <input type="number" value={obj.fillet.radius} step="0.1" min="0.01"
            oninput={(e) => numInput(e, (v) => updateObjectFillet('radius', v))} />
        </div>
        <div class="prop-row">
          <label>Edges</label>
          <select class="prop-select" value={obj.fillet.edges}
            onchange={(e) => updateObjectFillet('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
            {#each edgeSelectorOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeObjectFillet}>Remove Fillet</button>
      {:else}
        <button class="apply-btn full-width" onclick={addObjectFillet}>Add Fillet</button>
      {/if}
    </div>

    <!-- Chamfer (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Chamfer</div>
      {#if obj.chamfer}
        <div class="prop-row">
          <label>Distance</label>
          <input type="number" value={obj.chamfer.distance} step="0.1" min="0.01"
            oninput={(e) => numInput(e, (v) => updateObjectChamfer('distance', v))} />
        </div>
        <div class="prop-row">
          <label>Edges</label>
          <select class="prop-select" value={obj.chamfer.edges}
            onchange={(e) => updateObjectChamfer('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
            {#each edgeSelectorOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeObjectChamfer}>Remove Chamfer</button>
      {:else}
        <button class="apply-btn full-width" onclick={addObjectChamfer}>Add Chamfer</button>
      {/if}
    </div>

    <!-- Appearance -->
    <div class="prop-section">
      <div class="prop-section-title">Appearance</div>
      <div class="prop-row">
        <label>Color</label>
        <input type="color" value={obj.color} oninput={updateColor} class="color-picker" />
      </div>
      <div class="prop-row">
        <label>Visible</label>
        <button class="toggle-btn" class:active={obj.visible} onclick={toggleVisible}>
          {obj.visible ? 'Yes' : 'No'}
        </button>
      </div>
    </div>

    <!-- Actions -->
    <div class="prop-actions">
      <button class="delete-btn" onclick={deleteObject}>
        Delete
      </button>
    </div>
  </div>

{:else if sketchStore.selectedSketch}
  {@const sketch = sketchStore.selectedSketch}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge sketch-badge">sketch</span>
      <span class="prop-name-text">{sketch.name}</span>
    </div>

    <!-- Sketch Info -->
    <div class="prop-section">
      <div class="prop-section-title">Info</div>
      <div class="prop-row">
        <label>Plane</label>
        <span class="prop-value">{sketch.plane}</span>
      </div>
      <div class="prop-row">
        <label>Entities</label>
        <span class="prop-value">{sketch.entities.length}</span>
      </div>
    </div>

    <!-- Extrude -->
    <div class="prop-section">
      <div class="prop-section-title">Extrude</div>
      {#if sketch.extrude}
        <div class="prop-row">
          <label>Distance</label>
          <input type="number" value={sketch.extrude.distance} step="1" min="0.1"
            oninput={(e) => numInput(e, updateSketchExtrudeDistance)} />
        </div>
        <div class="prop-row">
          <label>Mode</label>
          <select class="prop-select" value={sketch.extrude.mode}
            onchange={(e) => updateSketchExtrudeMode((e.target as HTMLSelectElement).value as 'add' | 'cut')}>
            <option value="add">Add</option>
            <option value="cut">Cut</option>
          </select>
        </div>
        {#if sketch.extrude.mode === 'cut'}
          <div class="prop-row">
            <label>Target</label>
            <select class="prop-select" value={sketch.extrude.cutTargetId ?? ''}
              onchange={(e) => updateSketchCutTarget((e.target as HTMLSelectElement).value)}>
              <option value="">None</option>
              {#each getCutTargets() as target}
                <option value={target.id}>{target.name}</option>
              {/each}
            </select>
          </div>
        {/if}
        <button class="remove-btn" onclick={removeSketchExtrude}>Remove Extrude</button>
      {:else}
        <button class="apply-btn full-width" onclick={addSketchExtrude}>Extrude</button>
      {/if}
    </div>

    <!-- Fillet (only when extruded) -->
    {#if sketch.extrude}
      <div class="prop-section">
        <div class="prop-section-title">Fillet</div>
        {#if sketch.fillet}
          <div class="prop-row">
            <label>Radius</label>
            <input type="number" value={sketch.fillet.radius} step="0.1" min="0.01"
              oninput={(e) => numInput(e, (v) => updateSketchFillet('radius', v))} />
          </div>
          <div class="prop-row">
            <label>Edges</label>
            <select class="prop-select" value={sketch.fillet.edges}
              onchange={(e) => updateSketchFillet('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
              {#each edgeSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <button class="remove-btn" onclick={removeSketchFillet}>Remove Fillet</button>
        {:else}
          <button class="apply-btn full-width" onclick={addSketchFillet}>Add Fillet</button>
        {/if}
      </div>

      <!-- Chamfer (only when extruded) -->
      <div class="prop-section">
        <div class="prop-section-title">Chamfer</div>
        {#if sketch.chamfer}
          <div class="prop-row">
            <label>Distance</label>
            <input type="number" value={sketch.chamfer.distance} step="0.1" min="0.01"
              oninput={(e) => numInput(e, (v) => updateSketchChamfer('distance', v))} />
          </div>
          <div class="prop-row">
            <label>Edges</label>
            <select class="prop-select" value={sketch.chamfer.edges}
              onchange={(e) => updateSketchChamfer('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
              {#each edgeSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <button class="remove-btn" onclick={removeSketchChamfer}>Remove Chamfer</button>
        {:else}
          <button class="apply-btn full-width" onclick={addSketchChamfer}>Add Chamfer</button>
        {/if}
      </div>
    {/if}

    <!-- Constraints -->
    {#if (sketch.constraints ?? []).length > 0}
      <div class="prop-section">
        <div class="prop-section-title">Constraints ({(sketch.constraints ?? []).length})</div>
        {#each (sketch.constraints ?? []) as constraint}
          <div class="prop-row constraint-row">
            <span class="constraint-label">{constraintLabel(constraint)}</span>
            <button class="constraint-remove-btn" onclick={() => removeConstraint(constraint.id)}
              title="Remove constraint">&times;</button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Actions -->
    <div class="prop-actions">
      <button class="apply-btn full-width" onclick={editSketch}>
        Edit Sketch
      </button>
      <button class="delete-btn" onclick={deleteSketch}>
        Delete Sketch
      </button>
    </div>
  </div>

{:else}
  <div class="no-selection">
    <span class="no-selection-text">No object selected</span>
    <span class="no-selection-hint">Click an object in the viewport or add one from the toolbar</span>
  </div>
{/if}

<style>
  .properties-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: 12px;
    gap: 12px;
  }

  .prop-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-type-badge {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--accent);
    background: rgba(137, 180, 250, 0.12);
    padding: 2px 6px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .prop-type-badge.sketch-badge {
    color: #f9e2af;
    background: rgba(249, 226, 175, 0.12);
  }

  .prop-name-input {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .prop-name-input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .prop-name-text {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .prop-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .prop-section-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    margin-bottom: 2px;
  }

  .prop-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-row label {
    font-size: 11px;
    color: var(--text-secondary);
    width: 48px;
    flex-shrink: 0;
  }

  .prop-row input[type="number"] {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-primary);
    min-width: 0;
  }

  .prop-row input[type="number"]:focus {
    border-color: var(--accent);
    outline: none;
  }

  .prop-value {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-primary);
  }

  .prop-select {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    color: var(--text-primary);
    min-width: 0;
  }

  .prop-select:focus {
    border-color: var(--accent);
    outline: none;
  }

  .color-picker {
    width: 32px;
    height: 24px;
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 0;
    cursor: pointer;
    background: none;
  }

  .toggle-btn {
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 2px 10px;
    font-size: 11px;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .toggle-btn.active {
    color: var(--success);
    border-color: var(--success);
    background: rgba(166, 227, 161, 0.1);
  }

  .apply-btn {
    background: rgba(137, 180, 250, 0.1);
    border: 1px solid var(--accent);
    color: var(--accent);
    border-radius: 3px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
    transition: all 0.12s ease;
  }

  .apply-btn:hover {
    background: rgba(137, 180, 250, 0.2);
  }

  .apply-btn.full-width {
    width: 100%;
    padding: 4px 8px;
    margin-top: 4px;
  }

  .remove-btn {
    width: 100%;
    background: none;
    border: 1px solid var(--text-muted);
    color: var(--text-muted);
    border-radius: 3px;
    padding: 3px 8px;
    font-size: 11px;
    cursor: pointer;
    margin-top: 2px;
    transition: all 0.12s ease;
  }

  .remove-btn:hover {
    border-color: var(--error);
    color: var(--error);
    background: rgba(243, 139, 168, 0.1);
  }

  .prop-actions {
    margin-top: auto;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .delete-btn {
    width: 100%;
    background: none;
    border: 1px solid var(--error);
    color: var(--error);
    border-radius: 3px;
    padding: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .delete-btn:hover {
    background: rgba(243, 139, 168, 0.15);
  }

  .no-selection {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    padding: 24px;
    text-align: center;
  }

  .no-selection-text {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .no-selection-hint {
    font-size: 11px;
    color: var(--text-muted);
  }

  .constraint-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 0;
  }

  .constraint-label {
    font-size: 11px;
    color: #cba6f7;
  }

  .constraint-remove-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 14px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    transition: color 0.12s ease;
  }

  .constraint-remove-btn:hover {
    color: var(--error);
  }
</style>
