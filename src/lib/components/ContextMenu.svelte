<script lang="ts">
  import { getContextMenuStore } from '$lib/stores/context-menu.svelte';

  const menu = getContextMenuStore();

  let menuRef = $state<HTMLElement | null>(null);

  // Boundary clamping
  let clampedX = $derived(() => {
    if (!menuRef) return menu.x;
    const menuWidth = menuRef.offsetWidth || 180;
    const maxX = window.innerWidth - menuWidth - 8;
    return Math.min(menu.x, maxX);
  });

  let clampedY = $derived(() => {
    if (!menuRef) return menu.y;
    const menuHeight = menuRef.offsetHeight || 200;
    const maxY = window.innerHeight - menuHeight - 8;
    return Math.min(menu.y, maxY);
  });

  function handleItemClick(action: () => void) {
    action();
    menu.hide();
  }

  function handleBackdropClick() {
    menu.hide();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      menu.hide();
    }
  }
</script>

<svelte:window onkeydown={menu.visible ? handleKeydown : undefined} />

{#if menu.visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="context-backdrop" onclick={handleBackdropClick}></div>
  <div
    class="context-menu"
    bind:this={menuRef}
    style:left="{clampedX()}px"
    style:top="{clampedY()}px"
    role="menu"
  >
    {#each menu.items as item}
      {#if item.separator}
        <div class="context-separator"></div>
      {:else}
        <button
          class="context-item"
          class:disabled={item.disabled}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => handleItemClick(item.action)}
        >
          {#if item.icon}
            <span class="context-icon">{item.icon}</span>
          {/if}
          <span class="context-label">{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .context-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 9998;
  }

  .context-menu {
    position: fixed;
    z-index: 9999;
    min-width: 160px;
    max-width: 240px;
    background: var(--bg-mantle, #181825);
    border: 1px solid var(--border, #45475a);
    border-radius: 6px;
    padding: 4px 0;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  .context-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 28px;
    padding: 0 12px;
    background: none;
    border: none;
    color: var(--text-primary, #cdd6f4);
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s;
  }

  .context-item:hover:not(.disabled) {
    background: var(--bg-overlay, rgba(69, 71, 90, 0.4));
  }

  .context-item.disabled {
    color: var(--text-muted, #6c7086);
    cursor: default;
  }

  .context-icon {
    font-size: 12px;
    width: 16px;
    text-align: center;
    flex-shrink: 0;
  }

  .context-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .context-separator {
    height: 1px;
    background: var(--border, #45475a);
    margin: 4px 8px;
  }
</style>
