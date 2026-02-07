<script lang="ts">
  import { ViewportEngine } from '$lib/services/viewport-engine';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { onMount } from 'svelte';

  let containerRef = $state<HTMLElement | null>(null);
  let engine: ViewportEngine | null = null;
  const viewportStore = getViewportStore();

  onMount(() => {
    if (!containerRef) return;

    try {
      viewportStore.setLoading(true);
      engine = new ViewportEngine(containerRef);
      engine.loadDemoBox();
      viewportStore.setHasModel(true);
      viewportStore.setLoading(false);
    } catch (err) {
      viewportStore.setError(String(err));
      viewportStore.setLoading(false);
      console.error('Failed to initialize viewport:', err);
    }

    return () => {
      if (engine) {
        engine.dispose();
        engine = null;
      }
    };
  });

  // Watch for pending STL data from code execution
  $effect(() => {
    const stl = viewportStore.pendingStl;
    if (stl && engine) {
      engine.loadSTLFromBase64(stl);
      viewportStore.setPendingStl(null);
      viewportStore.setHasModel(true);
    }
  });
</script>

<div class="viewport-container" bind:this={containerRef}>
  {#if viewportStore.isLoading}
    <div class="viewport-overlay">
      <span class="loading-text">Initializing 3D viewport...</span>
    </div>
  {/if}
  {#if viewportStore.error}
    <div class="viewport-overlay error">
      <span class="error-text">Viewport error: {viewportStore.error}</span>
    </div>
  {/if}
</div>

<style>
  .viewport-container {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    background: #1a1a2e;
  }

  .viewport-overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(26, 26, 46, 0.8);
    z-index: 5;
  }

  .loading-text {
    color: var(--text-secondary);
    font-size: 14px;
  }

  .viewport-overlay.error {
    background: rgba(26, 26, 46, 0.9);
  }

  .error-text {
    color: var(--error);
    font-size: 13px;
  }
</style>
