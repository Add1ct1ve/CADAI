<script lang="ts">
  import { SHORTCUTS, CATEGORY_LABELS } from '$lib/data/shortcuts';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  const categories = Object.keys(CATEGORY_LABELS);

  function shortcutsForCategory(cat: string) {
    return SHORTCUTS.filter((s) => s.category === cat);
  }

  function handleOverlayClick() {
    onClose();
  }

  function handlePanelClick(e: MouseEvent) {
    e.stopPropagation();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

{#if open}
<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="shortcuts-overlay" onclick={handleOverlayClick} onkeydown={handleKeydown} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="shortcuts-panel" onclick={handlePanelClick} role="dialog" aria-modal="true" aria-label="Keyboard Shortcuts" tabindex="-1">
    <div class="shortcuts-header">
      <h2 class="shortcuts-title">Keyboard Shortcuts</h2>
      <button class="close-btn" onclick={onClose} title="Close">&times;</button>
    </div>

    <div class="shortcuts-body">
      {#each categories as cat}
        <div class="shortcut-section">
          <h3 class="section-title">{CATEGORY_LABELS[cat]}</h3>
          <div class="shortcut-grid">
            {#each shortcutsForCategory(cat) as entry}
              <div class="shortcut-row">
                <kbd class="shortcut-key">{entry.key}</kbd>
                <span class="shortcut-action">{entry.action}</span>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
</div>
{/if}

<style>
  .shortcuts-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1001;
    backdrop-filter: blur(2px);
  }

  .shortcuts-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .shortcuts-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .shortcuts-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    border-radius: 3px;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .shortcuts-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .shortcut-section {
    /* no margin needed, gap handles it */
  }

  .section-title {
    margin: 0 0 8px 0;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--accent);
  }

  .shortcut-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 16px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
  }

  kbd.shortcut-key {
    display: inline-block;
    min-width: 60px;
    padding: 2px 6px;
    background: var(--bg-base);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-primary);
    text-align: center;
    white-space: nowrap;
  }

  .shortcut-action {
    font-size: 12px;
    color: var(--text-secondary);
  }
</style>
