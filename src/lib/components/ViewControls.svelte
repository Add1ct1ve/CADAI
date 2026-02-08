<script lang="ts">
  import { getViewportStore } from '$lib/stores/viewport.svelte';

  const viewport = getViewportStore();

  function toggleGrid() {
    viewport.setGridVisible(!viewport.gridVisible);
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

  .separator {
    height: 1px;
    background: rgba(69, 71, 90, 0.4);
    margin: 2px 4px;
  }
</style>
