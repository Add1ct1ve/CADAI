<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import Chat from '$lib/components/Chat.svelte';
  import Viewport from '$lib/components/Viewport.svelte';
  import CodeEditor from '$lib/components/CodeEditor.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { projectNew, projectSave } from '$lib/services/project-actions';
  import { executeCode } from '$lib/services/tauri';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { onMount } from 'svelte';

  const settings = getSettingsStore();
  const project = getProjectStore();
  const viewportStore = getViewportStore();

  let settingsOpen = $state(false);

  onMount(() => {
    settings.load();
  });

  function handleKeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;

    if (ctrl && e.key === 'n') {
      e.preventDefault();
      projectNew();
    } else if (ctrl && e.key === 's') {
      e.preventDefault();
      projectSave();
    } else if (ctrl && e.key === 'r') {
      e.preventDefault();
      // Run current code
      runCurrentCode();
    }
  }

  async function runCurrentCode() {
    try {
      const result = await executeCode(project.code);
      if (result.success && result.stl_base64) {
        viewportStore.setPendingStl(result.stl_base64);
      }
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
          <CodeEditor />
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
