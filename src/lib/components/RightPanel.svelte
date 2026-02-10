<script lang="ts">
  import CodeEditor from '$lib/components/CodeEditor.svelte';
  import PropertiesPanel from '$lib/components/PropertiesPanel.svelte';
  import DrawingProperties from '$lib/components/DrawingProperties.svelte';
  import GenerationHistory from '$lib/components/GenerationHistory.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getGenerationHistoryStore } from '$lib/stores/generationHistory.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import type { GenerationEntry } from '$lib/types';
  import { onMount } from 'svelte';

  const scene = getSceneStore();
  const generationHistoryStore = getGenerationHistoryStore();
  const viewportStore = getViewportStore();

  let activeTab = $state<'code' | 'properties' | 'history'>('code');

  function handleRestoreEntry(entry: GenerationEntry) {
    window.dispatchEvent(new CustomEvent('generation-history:restore', { detail: entry }));
  }

  function handlePreviewEntry(entry: GenerationEntry) {
    if (entry.stl_base64) {
      viewportStore.setPendingStl(entry.stl_base64);
    }
  }

  onMount(() => {
    const handler = () => { activeTab = 'properties'; };
    window.addEventListener('feature-tree:edit', handler);
    return () => window.removeEventListener('feature-tree:edit', handler);
  });
</script>

<div class="right-panel">
  {#if scene.drawingMode}
    <div class="panel-tabs">
      <button class="tab-btn active">Drawing</button>
    </div>
    <div class="panel-body">
      <DrawingProperties />
    </div>
  {:else}
    <div class="panel-tabs">
      <button
        class="tab-btn"
        class:active={activeTab === 'code'}
        onclick={() => activeTab = 'code'}
      >
        Code
      </button>
      <button
        class="tab-btn"
        class:active={activeTab === 'properties'}
        onclick={() => activeTab = 'properties'}
      >
        Properties
        {#if scene.selectedIds.length > 0}
          <span class="tab-badge">{scene.selectedIds.length}</span>
        {/if}
      </button>
      <button
        class="tab-btn"
        class:active={activeTab === 'history'}
        onclick={() => activeTab = 'history'}
      >
        History
        {#if generationHistoryStore.entries.length > 0}
          <span class="tab-badge">{generationHistoryStore.entries.length}</span>
        {/if}
      </button>
    </div>

    <div class="panel-body">
      {#if activeTab === 'code'}
        <CodeEditor readonly={scene.codeMode === 'parametric'} />
      {:else if activeTab === 'properties'}
        <PropertiesPanel />
      {:else if activeTab === 'history'}
        <GenerationHistory
          onRestoreEntry={handleRestoreEntry}
          onPreviewEntry={handlePreviewEntry}
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  .right-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-surface);
  }

  .panel-tabs {
    display: flex;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-mantle);
    flex-shrink: 0;
  }

  .tab-btn {
    flex: 1;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 8px 12px;
    cursor: pointer;
    transition: all 0.12s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .tab-btn:hover {
    color: var(--text-secondary);
    background: var(--bg-overlay);
  }

  .tab-btn.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .tab-badge {
    font-size: 9px;
    font-weight: 700;
    background: var(--accent);
    color: var(--bg-base);
    border-radius: 8px;
    padding: 0 5px;
    min-width: 14px;
    text-align: center;
    line-height: 14px;
  }

  .panel-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
