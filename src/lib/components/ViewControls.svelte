<script lang="ts">
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import type { DisplayMode } from '$lib/types/cad';

  const viewport = getViewportStore();

  function toggleGrid() {
    viewport.setGridVisible(!viewport.gridVisible);
  }

  function setMode(mode: DisplayMode) {
    viewport.setDisplayMode(mode);
  }

  function setSectionAxis(axis: 'X' | 'Y' | 'Z') {
    const normals: Record<string, [number, number, number]> = {
      X: [1, 0, 0],
      Y: [0, 1, 0],
      Z: [0, 0, 1],
    };
    viewport.setSectionPlane({
      ...viewport.sectionPlane,
      normal: normals[axis],
    });
  }

  function adjustSectionOffset(delta: number) {
    viewport.setSectionPlane({
      ...viewport.sectionPlane,
      offset: viewport.sectionPlane.offset + delta,
    });
  }
</script>

<div class="view-controls">
  <button
    class="view-btn"
    title="Top View (Numpad 7)"
    onclick={() => viewport.animateToView('top')}
  >T</button>
  <button
    class="view-btn"
    title="Front View (Numpad 1)"
    onclick={() => viewport.animateToView('front')}
  >F</button>
  <button
    class="view-btn"
    title="Right View (Numpad 3)"
    onclick={() => viewport.animateToView('right')}
  >R</button>
  <button
    class="view-btn"
    title="Isometric View (Numpad 0)"
    onclick={() => viewport.animateToView('iso')}
  >3D</button>

  <div class="separator"></div>

  <button
    class="view-btn"
    title="Fit All (Home)"
    onclick={() => viewport.fitAll()}
  >&#x2922;</button>
  <button
    class="view-btn"
    class:active={viewport.gridVisible}
    title="Toggle Grid"
    onclick={toggleGrid}
  >#</button>

  <div class="separator"></div>

  <button
    class="view-btn"
    class:active={viewport.displayMode === 'shaded'}
    title="Shaded"
    onclick={() => setMode('shaded')}
  >S</button>
  <button
    class="view-btn"
    class:active={viewport.displayMode === 'wireframe'}
    title="Wireframe"
    onclick={() => setMode('wireframe')}
  >W</button>
  <button
    class="view-btn"
    class:active={viewport.displayMode === 'shaded-edges'}
    title="Shaded + Edges"
    onclick={() => setMode('shaded-edges')}
  >SE</button>
  <button
    class="view-btn"
    class:active={viewport.displayMode === 'transparent'}
    title="X-ray / Transparent"
    onclick={() => setMode('transparent')}
  >X</button>
  <button
    class="view-btn"
    class:active={viewport.displayMode === 'section'}
    title="Section View"
    onclick={() => setMode('section')}
  >&#x2702;</button>

  {#if viewport.displayMode === 'section'}
    <div class="separator"></div>
    <button
      class="view-btn section-axis"
      class:active={viewport.sectionPlane.normal[0] === 1}
      title="Section X axis"
      onclick={() => setSectionAxis('X')}
    >X</button>
    <button
      class="view-btn section-axis"
      class:active={viewport.sectionPlane.normal[1] === 1}
      title="Section Y axis"
      onclick={() => setSectionAxis('Y')}
    >Y</button>
    <button
      class="view-btn section-axis"
      class:active={viewport.sectionPlane.normal[2] === 1}
      title="Section Z axis"
      onclick={() => setSectionAxis('Z')}
    >Z</button>
    <button
      class="view-btn"
      title="Move section plane +"
      onclick={() => adjustSectionOffset(1)}
    >+</button>
    <button
      class="view-btn"
      title="Move section plane -"
      onclick={() => adjustSectionOffset(-1)}
    >-</button>
  {/if}
</div>

<style>
  .view-controls {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    z-index: 3;
  }

  .view-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(24, 24, 37, 0.85);
    color: var(--text-secondary, #a6adc8);
    border: 1px solid rgba(69, 71, 90, 0.5);
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  .view-btn:hover {
    background: rgba(49, 50, 68, 0.95);
    color: var(--text-primary, #cdd6f4);
    border-color: rgba(88, 91, 112, 0.7);
  }

  .view-btn:active {
    background: rgba(69, 71, 90, 0.95);
  }

  .view-btn.active {
    border-color: var(--accent, #89b4fa);
    color: var(--accent, #89b4fa);
  }

  .section-axis {
    font-size: 9px;
  }

  .separator {
    height: 1px;
    background: rgba(69, 71, 90, 0.4);
    margin: 2px 4px;
  }
</style>
