<script lang="ts">
  import CodeEditor from '$lib/components/CodeEditor.svelte';
  import PropertiesPanel from '$lib/components/PropertiesPanel.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { onMount } from 'svelte';

  const scene = getSceneStore();

  let activeTab = $state<'code' | 'properties'>('code');

  onMount(() => {
    const handler = () => { activeTab = 'properties'; };
    window.addEventListener('feature-tree:edit', handler);
    return () => window.removeEventListener('feature-tree:edit', handler);
  });
</script>

<div class="right-panel">
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
  </div>

  <div class="panel-body">
    {#if activeTab === 'code'}
      <CodeEditor readonly={scene.codeMode === 'parametric'} />
    {:else}
      <PropertiesPanel />
    {/if}
  </div>
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
