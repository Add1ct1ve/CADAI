<script lang="ts">
  import { projectNew, projectOpen, projectSave, projectExportStl } from '$lib/services/project-actions';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import type { ToolId } from '$lib/types/cad';

  interface Props {
    onSettingsClick: () => void;
  }

  let { onSettingsClick }: Props = $props();

  const toolStore = getToolStore();
  const scene = getSceneStore();

  let isBusy = $state(false);
  let statusMessage = $state('');

  function showStatus(msg: string, duration = 3000) {
    statusMessage = msg;
    setTimeout(() => {
      statusMessage = '';
    }, duration);
  }

  async function handleNew() {
    try {
      const result = await projectNew();
      if (result) showStatus(result);
    } catch (err) {
      showStatus(`Failed: ${err}`);
    }
  }

  async function handleOpen() {
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

  async function handleExport() {
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

  function setTool(tool: ToolId) {
    toolStore.setTool(tool);
  }

  function toggleCodeMode() {
    scene.setCodeMode(scene.codeMode === 'parametric' ? 'manual' : 'parametric');
  }

  const toolButtons: { id: ToolId; label: string; shortcut: string; group: 'select' | 'primitive' }[] = [
    { id: 'select', label: 'Select', shortcut: 'V', group: 'select' },
    { id: 'translate', label: 'Move', shortcut: 'G', group: 'select' },
    { id: 'rotate', label: 'Rotate', shortcut: 'R', group: 'select' },
    { id: 'scale', label: 'Scale', shortcut: 'S', group: 'select' },
  ];

  const primitiveButtons: { id: ToolId; label: string; shortcut: string }[] = [
    { id: 'add-box', label: 'Box', shortcut: '1' },
    { id: 'add-cylinder', label: 'Cyl', shortcut: '2' },
    { id: 'add-sphere', label: 'Sphere', shortcut: '3' },
    { id: 'add-cone', label: 'Cone', shortcut: '4' },
  ];
</script>

<div class="toolbar">
  <div class="toolbar-left">
    <span class="app-title">CAD AI Studio</span>
  </div>

  <div class="toolbar-center">
    <!-- File actions -->
    <button class="toolbar-btn" onclick={handleNew} title="New Project (Ctrl+N)" disabled={isBusy}>
      New
    </button>
    <button class="toolbar-btn" onclick={handleOpen} title="Open Project (Ctrl+O)" disabled={isBusy}>
      Open
    </button>
    <button class="toolbar-btn" onclick={handleSave} title="Save Project (Ctrl+S)" disabled={isBusy}>
      Save
    </button>
    <button class="toolbar-btn" onclick={handleExport} title="Export STL" disabled={isBusy}>
      Export
    </button>

    <div class="toolbar-separator"></div>

    <!-- Selection / Transform tools -->
    {#each toolButtons as btn}
      <button
        class="toolbar-btn tool-btn"
        class:tool-active={toolStore.activeTool === btn.id}
        onclick={() => setTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
        disabled={scene.codeMode !== 'parametric'}
      >
        {btn.label}
      </button>
    {/each}

    <!-- Snap controls (shown when a transform tool is active) -->
    {#if toolStore.activeTool === 'translate'}
      <button
        class="toolbar-btn snap-btn"
        class:snap-active={toolStore.translateSnap !== null}
        onclick={() => toolStore.setTranslateSnap(toolStore.translateSnap ? null : 1)}
        title="Toggle translation snap (1 unit)"
      >
        Snap: 1u
      </button>
    {/if}
    {#if toolStore.activeTool === 'rotate'}
      <button
        class="toolbar-btn snap-btn"
        class:snap-active={toolStore.rotationSnap !== null}
        onclick={() => toolStore.setRotationSnap(toolStore.rotationSnap ? null : 15)}
        title="Toggle rotation snap (15 degrees)"
      >
        Snap: 15°
      </button>
    {/if}
    {#if toolStore.activeTool === 'scale'}
      <button
        class="toolbar-btn snap-btn"
        class:snap-active={toolStore.uniformScale}
        onclick={() => toolStore.setUniformScale(!toolStore.uniformScale)}
        title="Toggle uniform scaling (all axes equal)"
      >
        Uniform
      </button>
    {/if}

    <div class="toolbar-separator"></div>

    <!-- Primitive tools -->
    {#each primitiveButtons as btn}
      <button
        class="toolbar-btn tool-btn"
        class:tool-active={toolStore.activeTool === btn.id}
        onclick={() => setTool(btn.id)}
        title="{btn.label} ({btn.shortcut})"
        disabled={scene.codeMode !== 'parametric'}
      >
        {btn.label}
      </button>
    {/each}

    <div class="toolbar-separator"></div>

    <!-- Code mode toggle -->
    <button
      class="toolbar-btn mode-btn"
      class:mode-parametric={scene.codeMode === 'parametric'}
      class:mode-manual={scene.codeMode === 'manual'}
      onclick={toggleCodeMode}
      title="Toggle between parametric and manual code mode"
    >
      {scene.codeMode === 'parametric' ? 'Parametric' : 'Manual'}
    </button>

    {#if statusMessage}
      <span class="toolbar-status">{statusMessage}</span>
    {/if}
  </div>

  <div class="toolbar-right">
    <button class="toolbar-btn settings-btn" onclick={onSettingsClick} title="Settings">
      &#9881;
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 12px;
    background: var(--bg-mantle);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 12px;
    -webkit-app-region: drag;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .app-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.3px;
    -webkit-app-region: drag;
  }

  .toolbar-center {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 1;
    -webkit-app-region: no-drag;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
  }

  .toolbar-btn {
    background: none;
    border: 1px solid transparent;
    color: var(--text-secondary);
    padding: 4px 10px;
    border-radius: 3px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.12s ease;
    white-space: nowrap;
  }

  .toolbar-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }

  .toolbar-btn:active {
    background: var(--bg-surface);
  }

  .toolbar-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .tool-btn.tool-active {
    background: rgba(137, 180, 250, 0.15);
    border-color: var(--accent);
    color: var(--accent);
  }

  .snap-btn {
    font-size: 10px;
    padding: 2px 6px;
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .snap-btn.snap-active {
    color: var(--success);
    border-color: var(--success);
    background: rgba(166, 227, 161, 0.1);
  }

  .mode-btn {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    padding: 3px 8px;
  }

  .mode-btn.mode-parametric {
    color: var(--accent);
    border-color: var(--accent);
    background: rgba(137, 180, 250, 0.1);
  }

  .mode-btn.mode-manual {
    color: var(--warning);
    border-color: var(--warning);
    background: rgba(249, 226, 175, 0.1);
  }

  .settings-btn {
    font-size: 16px;
    padding: 2px 8px;
  }

  .toolbar-separator {
    width: 1px;
    height: 20px;
    background: var(--border-subtle);
    margin: 0 4px;
  }

  .toolbar-status {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: 8px;
    padding: 2px 8px;
    background: var(--bg-overlay);
    border-radius: 3px;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
