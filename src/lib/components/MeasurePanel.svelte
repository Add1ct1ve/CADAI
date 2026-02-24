<script lang="ts">
  import { getMeasureStore } from '$lib/stores/measure.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { computeMassProperties } from '$lib/services/mass-properties';
  import { getMaterial } from '$lib/data/materials';
  import { formatUnit, toDisplayArea, toDisplayVolume, areaUnitSuffix, volumeUnitSuffix } from '$lib/services/units';
  import type { MeasureToolId } from '$lib/types/cad';

  const measureStore = getMeasureStore();
  const scene = getSceneStore();
  const settingsStore = getSettingsStore();

  let units = $derived(settingsStore.config.display_units);

  let showMass = $state(false);

  const subTools: { id: MeasureToolId; label: string; hint: string }[] = [
    { id: 'measure-distance', label: 'Dist', hint: 'Click two points to measure distance' },
    { id: 'measure-angle', label: 'Angle', hint: 'Click vertex, then two arm points to measure angle' },
    { id: 'measure-radius', label: 'Radius', hint: 'Click on a cylinder or sphere to read radius' },
    { id: 'measure-bbox', label: 'BBox', hint: 'Click on an object to show bounding box' },
  ];

  function toggleTool(id: MeasureToolId) {
    if (measureStore.activeMeasureTool === id) {
      measureStore.setMeasureTool(null);
    } else {
      measureStore.setMeasureTool(id);
      showMass = false;
    }
  }

  function toggleMass() {
    showMass = !showMass;
    if (showMass) {
      measureStore.setMeasureTool(null);
      // Compute for selected object
      const obj = scene.firstSelected;
      if (obj) {
        const preset = obj.materialId ? getMaterial(obj.materialId) : undefined;
        const density = preset?.density;
        const props = computeMassProperties(obj.params, density);
        measureStore.setMassProperties(obj.id, props);
      } else {
        measureStore.setMassProperties(null, null);
      }
    } else {
      measureStore.setMassProperties(null, null);
    }
  }

  let activeHint = $derived(
    subTools.find((t) => t.id === measureStore.activeMeasureTool)?.hint ??
    (showMass ? 'Click an object to see mass properties' : 'Select a measurement sub-tool')
  );

  let pendingCount = $derived(measureStore.pendingPoints.length);
</script>

<div class="measure-panel" onpointerdown={(e) => e.stopPropagation()}>
  <div class="measure-header">MEASURE</div>

  <div class="measure-tools">
    {#each subTools as tool}
      <button
        class="measure-tool-btn"
        class:active={measureStore.activeMeasureTool === tool.id}
        onclick={() => toggleTool(tool.id)}
        title={tool.hint}
      >
        {tool.label}
      </button>
    {/each}
    <button
      class="measure-tool-btn"
      class:active={showMass}
      onclick={toggleMass}
      title="Show mass properties for selected object"
    >
      Mass
    </button>
  </div>

  <div class="measure-hint">{activeHint}</div>

  {#if measureStore.feedbackMessage}
    <div class="measure-feedback">{measureStore.feedbackMessage}</div>
  {/if}

  {#if pendingCount > 0}
    <div class="pending-info">
      {pendingCount} point{pendingCount !== 1 ? 's' : ''} placed
      <button class="clear-pending-btn" onclick={() => measureStore.clearPending()}>Reset</button>
    </div>
  {/if}

  {#if measureStore.measurements.length > 0}
    <div class="measure-list">
      {#each measureStore.measurements as m}
        <div class="measure-item">
          <span class="measure-value">
            {#if m.type === 'distance'}
              Dist: {formatUnit(m.distance, units)}
            {:else if m.type === 'angle'}
              Angle: {m.angleDegrees.toFixed(1)}&deg;
            {:else if m.type === 'radius'}
              R = {formatUnit(m.radius, units)}
            {:else if m.type === 'bbox'}
              BBox: {formatUnit(m.sizeX, units, 2)} &times; {formatUnit(m.sizeY, units, 2)} &times; {formatUnit(m.sizeZ, units, 2)}
            {/if}
          </span>
          <button class="remove-btn" onclick={() => measureStore.removeMeasurement(m.id)} title="Remove">&times;</button>
        </div>
      {/each}
      <button class="clear-all-btn" onclick={() => measureStore.clearAll()}>Clear All</button>
    </div>
  {/if}

  {#if showMass && measureStore.massProperties}
    <div class="mass-props">
      <div class="mass-row">
        <span class="mass-label">Volume</span>
        <span class="mass-value">{toDisplayVolume(measureStore.massProperties.volume, units).toFixed(3)} {volumeUnitSuffix(units)}</span>
      </div>
      <div class="mass-row">
        <span class="mass-label">Surface</span>
        <span class="mass-value">{toDisplayArea(measureStore.massProperties.surfaceArea, units).toFixed(3)} {areaUnitSuffix(units)}</span>
      </div>
      <div class="mass-row">
        <span class="mass-label">CoM</span>
        <span class="mass-value">
          ({measureStore.massProperties.centerOfMass[0].toFixed(1)},
           {measureStore.massProperties.centerOfMass[1].toFixed(1)},
           {measureStore.massProperties.centerOfMass[2].toFixed(1)})
        </span>
      </div>
      {#if measureStore.massProperties.density != null}
        <div class="mass-row">
          <span class="mass-label">Density</span>
          <span class="mass-value">{measureStore.massProperties.density.toFixed(3)} g/cm&sup3;</span>
        </div>
      {/if}
      {#if measureStore.massProperties.mass != null}
        <div class="mass-row">
          <span class="mass-label">Mass</span>
          <span class="mass-value">{measureStore.massProperties.mass.toFixed(3)} g</span>
        </div>
      {/if}
      <div class="mass-note">
        {#if measureStore.massProperties.density != null}
          Mass = volume &times; density. Approximate — modifiers not included.
        {:else}
          Assign a material for mass calculation.
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .measure-panel {
    position: absolute;
    top: 8px;
    left: 8px;
    background: rgba(24, 24, 37, 0.92);
    border: 1px solid rgba(148, 226, 213, 0.3);
    border-radius: 6px;
    padding: 8px 10px;
    z-index: 4;
    min-width: 180px;
    max-width: 240px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    backdrop-filter: blur(8px);
  }

  .measure-header {
    font-size: 9px;
    font-weight: 700;
    color: #94e2d5;
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .measure-tools {
    display: flex;
    gap: 3px;
    flex-wrap: wrap;
  }

  .measure-tool-btn {
    background: rgba(148, 226, 213, 0.08);
    border: 1px solid rgba(148, 226, 213, 0.25);
    color: #94e2d5;
    padding: 3px 8px;
    border-radius: 3px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .measure-tool-btn:hover {
    background: rgba(148, 226, 213, 0.18);
    border-color: #94e2d5;
  }

  .measure-tool-btn.active {
    background: rgba(148, 226, 213, 0.25);
    border-color: #94e2d5;
    color: #fff;
    font-weight: 600;
  }

  .measure-hint {
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.3;
  }

  .measure-feedback {
    font-size: 10px;
    color: #f38ba8;
    background: rgba(243, 139, 168, 0.1);
    border: 1px solid rgba(243, 139, 168, 0.25);
    border-radius: 3px;
    padding: 3px 6px;
    line-height: 1.3;
  }

  .pending-info {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #f9e2af;
  }

  .clear-pending-btn {
    background: none;
    border: 1px solid rgba(249, 226, 175, 0.3);
    color: #f9e2af;
    font-size: 9px;
    padding: 1px 6px;
    border-radius: 3px;
    cursor: pointer;
  }

  .clear-pending-btn:hover {
    background: rgba(249, 226, 175, 0.1);
  }

  .measure-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    border-top: 1px solid rgba(148, 226, 213, 0.15);
    padding-top: 4px;
  }

  .measure-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 2px 0;
  }

  .measure-value {
    font-size: 11px;
    font-family: var(--font-mono);
    color: #94e2d5;
  }

  .remove-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 14px;
    cursor: pointer;
    padding: 0 3px;
    line-height: 1;
  }

  .remove-btn:hover {
    color: #f38ba8;
  }

  .clear-all-btn {
    background: none;
    border: 1px solid rgba(148, 226, 213, 0.2);
    color: var(--text-muted);
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 3px;
    cursor: pointer;
    margin-top: 2px;
    align-self: flex-start;
  }

  .clear-all-btn:hover {
    color: #94e2d5;
    border-color: rgba(148, 226, 213, 0.4);
  }

  .mass-props {
    border-top: 1px solid rgba(148, 226, 213, 0.15);
    padding-top: 4px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .mass-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }

  .mass-label {
    font-size: 10px;
    color: var(--text-muted);
  }

  .mass-value {
    font-size: 11px;
    font-family: var(--font-mono);
    color: #94e2d5;
  }

  .mass-note {
    font-size: 9px;
    color: var(--text-muted);
    font-style: italic;
    opacity: 0.7;
  }
</style>
