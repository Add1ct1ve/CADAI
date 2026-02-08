<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import Chat from '$lib/components/Chat.svelte';
  import Viewport from '$lib/components/Viewport.svelte';
  import RightPanel from '$lib/components/RightPanel.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { projectNew, projectSave } from '$lib/services/project-actions';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import type { ToolId } from '$lib/types/cad';
  import { onMount } from 'svelte';

  const settings = getSettingsStore();
  const project = getProjectStore();
  const scene = getSceneStore();
  const tools = getToolStore();

  let settingsOpen = $state(false);

  onMount(() => {
    settings.load();
  });

  function handleKeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    const target = e.target as HTMLElement;
    const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

    // Global shortcuts (always active)
    if (ctrl && e.key === 'n') {
      e.preventDefault();
      projectNew();
      return;
    }
    if (ctrl && e.key === 's') {
      e.preventDefault();
      projectSave();
      return;
    }
    if (ctrl && e.key === 'r') {
      e.preventDefault();
      runCurrentCode();
      return;
    }

    // Tool shortcuts (only when not focused on an input)
    if (isInput) return;

    // Only allow tool shortcuts in parametric mode
    if (scene.codeMode === 'parametric') {
      const toolMap: Record<string, ToolId> = {
        v: 'select',
        g: 'translate',
        r: 'rotate',
        s: 'scale',
        '1': 'add-box',
        '2': 'add-cylinder',
        '3': 'add-sphere',
        '4': 'add-cone',
      };

      const tool = toolMap[e.key.toLowerCase()];
      if (tool) {
        e.preventDefault();
        tools.setTool(tool);
        return;
      }
    }

    // Delete selected objects
    if ((e.key === 'Delete' || e.key === 'Backspace') && scene.codeMode === 'parametric') {
      if (scene.selectedIds.length > 0) {
        e.preventDefault();
        scene.deleteSelected();
        triggerPipeline(100);
      }
      return;
    }

    // Escape: deselect or revert tool
    if (e.key === 'Escape') {
      if (tools.activeTool !== 'select') {
        tools.revertToSelect();
      } else {
        scene.clearSelection();
      }
    }
  }

  async function runCurrentCode() {
    try {
      await runPythonExecution();
    } catch (err) {
      console.error('Run code failed:', err);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell">
  <Toolbar onSettingsClick={() => { settingsOpen = true; }} />

  <div class="main-area">
    <SplitPane direction="horizontal" sizes={[25, 50, 25]}>
      {#snippet panes(index)}
        {#if index === 0}
          <Chat />
        {:else if index === 1}
          <Viewport />
        {:else}
          <RightPanel />
        {/if}
      {/snippet}
    </SplitPane>
  </div>

  <StatusBar />
  <Settings open={settingsOpen} onClose={() => { settingsOpen = false; }} />
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
