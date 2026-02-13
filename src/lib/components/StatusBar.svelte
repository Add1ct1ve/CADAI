<script lang="ts">
  import { onMount } from 'svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { checkPython } from '$lib/services/tauri';
  import { unitSuffix } from '$lib/services/units';
  import type { PythonStatus } from '$lib/types';

  const settings = getSettingsStore();
  const viewport = getViewportStore();
  const scene = getSceneStore();

  let pythonInfo = $state<PythonStatus | null>(null);
  let pythonCheckError = $state(false);

  let statusText = $derived(() => {
    if (viewport.isLoading) return 'Loading...';
    if (viewport.error) return 'Error';
    return 'Ready';
  });

  let pythonStatusText = $derived(() => {
    if (pythonCheckError) return 'Python: Check failed';
    if (!pythonInfo) return 'Python: Checking...';
    if (!pythonInfo.python_found) return 'Python: Not found';
    if (!pythonInfo.venv_ready) return `Python: ${pythonInfo.python_version ?? 'found'} (no venv)`;
    if (!pythonInfo.build123d_installed) return `Python: ${pythonInfo.python_version} (no Build123d)`;
    const b123dVer = pythonInfo.build123d_version ? ` ${pythonInfo.build123d_version}` : '';
    return `Python: ${pythonInfo.python_version} + Build123d${b123dVer}`;
  });

  let unitsText = $derived(() => {
    return `Units: ${unitSuffix(settings.config.display_units)}`;
  });

  let aiProvider = $derived(() => {
    return settings.config.ai_provider
      ? `AI: ${settings.config.ai_provider}`
      : 'AI: Not configured';
  });

  let selectionText = $derived(() => {
    const sel = scene.selectedObjects;
    if (sel.length === 0) return '';
    if (sel.length === 1) return `Selected: ${sel[0].name}`;
    return `${sel.length} objects selected`;
  });

  onMount(async () => {
    try {
      pythonInfo = await checkPython();
    } catch {
      pythonCheckError = true;
    }
  });
</script>

<div class="status-bar">
  <div class="status-left">
    <span class="status-item status-text">{statusText()}</span>
    {#if selectionText()}
      <span class="status-separator">|</span>
      <span class="status-item selection-text">{selectionText()}</span>
    {/if}
  </div>
  <div class="status-right">
    <span class="status-item">{unitsText()}</span>
    <span class="status-separator">|</span>
    <span class="status-item">{pythonStatusText()}</span>
    <span class="status-separator">|</span>
    <span class="status-item">{aiProvider()}</span>
  </div>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 24px;
    padding: 0 12px;
    background: var(--bg-mantle);
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }

  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status-item {
    font-size: 11px;
    color: var(--text-muted);
  }

  .status-text {
    color: var(--text-secondary);
  }

  .status-separator {
    color: var(--border);
    font-size: 11px;
  }

  .selection-text {
    color: var(--accent);
  }
</style>
