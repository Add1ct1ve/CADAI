<script lang="ts">
  import { getToastStore } from '$lib/stores/toast.svelte';

  const toast = getToastStore();

  const typeColors: Record<string, string> = {
    success: '#a6e3a1',
    error: '#f38ba8',
    warning: '#f9e2af',
    info: '#89b4fa',
  };

  const typeIcons: Record<string, string> = {
    success: '\u2713',
    error: '\u2717',
    warning: '\u26A0',
    info: '\u2139',
  };
</script>

{#if toast.toasts.length > 0}
  <div class="toast-container">
    {#each toast.toasts as item (item.id)}
      <div
        class="toast-item"
        style:border-left-color={typeColors[item.type]}
      >
        <span class="toast-icon" style:color={typeColors[item.type]}>{typeIcons[item.type]}</span>
        <span class="toast-message">{item.message}</span>
        <button class="toast-close" onclick={() => toast.remove(item.id)}>&times;</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: 32px;
    right: 12px;
    display: flex;
    flex-direction: column-reverse;
    gap: 6px;
    z-index: 9999;
    pointer-events: none;
  }

  .toast-item {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 360px;
    padding: 8px 12px;
    background: var(--bg-mantle, #181825);
    border: 1px solid var(--border, #45475a);
    border-left: 3px solid;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    pointer-events: auto;
    animation: toast-slide-in 0.2s ease-out;
  }

  @keyframes toast-slide-in {
    from {
      opacity: 0;
      transform: translateX(20px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .toast-icon {
    font-size: 14px;
    flex-shrink: 0;
    width: 18px;
    text-align: center;
  }

  .toast-message {
    flex: 1;
    font-size: 12px;
    color: var(--text-primary, #cdd6f4);
    line-height: 1.4;
  }

  .toast-close {
    background: none;
    border: none;
    color: var(--text-muted, #6c7086);
    cursor: pointer;
    font-size: 16px;
    padding: 0 2px;
    line-height: 1;
    flex-shrink: 0;
    transition: color 0.1s;
  }

  .toast-close:hover {
    color: var(--text-primary, #cdd6f4);
  }
</style>
