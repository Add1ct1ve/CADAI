<script lang="ts">
  import { projectNew, projectOpen, projectSave, projectExportStl } from '$lib/services/project-actions';

  interface Props {
    onSettingsClick: () => void;
  }

  let { onSettingsClick }: Props = $props();

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
</script>

<div class="toolbar">
  <div class="toolbar-left">
    <span class="app-title">CAD AI Studio</span>
  </div>

  <div class="toolbar-center">
    <button class="toolbar-btn" onclick={handleNew} title="New Project (Ctrl+N)" disabled={isBusy}>
      New
    </button>
    <button class="toolbar-btn" onclick={handleOpen} title="Open Project (Ctrl+O)" disabled={isBusy}>
      Open
    </button>
    <button class="toolbar-btn" onclick={handleSave} title="Save Project (Ctrl+S)" disabled={isBusy}>
      Save
    </button>
    <div class="toolbar-separator"></div>
    <button class="toolbar-btn" onclick={handleExport} title="Export STL" disabled={isBusy}>
      Export STL
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

  .settings-btn {
    font-size: 16px;
    padding: 2px 8px;
  }

  .toolbar-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
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
