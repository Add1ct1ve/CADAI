<script lang="ts">
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { triggerPipeline } from '$lib/services/execution-pipeline';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import type { PrimitiveParams, CadTransform, BoxParams, CylinderParams, SphereParams, ConeParams } from '$lib/types/cad';

  const scene = getSceneStore();
  const history = getHistoryStore();

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Move-by-delta state
  let deltaX = $state(0);
  let deltaY = $state(0);
  let deltaZ = $state(0);

  // Scale factor state
  let scaleFactor = $state(1.0);

  function debounced(fn: () => void, ms = 300) {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(fn, ms);
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
      history.pushSnapshot(scene.snapshot());
      snapshotCaptured = true;
      // Reset after debounce window so the next edit group captures a new snapshot
      setTimeout(() => { snapshotCaptured = false; }, 500);
    }
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
    history.pushSnapshot(scene.snapshot());
    scene.deleteSelected();
    triggerPipeline(100);
  }

  function applyMoveDelta() {
    const obj = scene.firstSelected;
    if (!obj) return;
    history.pushSnapshot(scene.snapshot());
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
    history.pushSnapshot(scene.snapshot());
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

  .prop-actions {
    margin-top: auto;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle);
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
</style>
