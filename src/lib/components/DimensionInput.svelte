<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    prompt: string;
    defaultValue: number;
    position: { x: number; y: number };
    onSubmit: (value: number) => void;
    onCancel: () => void;
  }

  let { prompt, defaultValue, position, onSubmit, onCancel }: Props = $props();

  let inputRef = $state<HTMLInputElement | null>(null);
  let value = $state(String(defaultValue));

  onMount(() => {
    if (inputRef) {
      inputRef.focus();
      inputRef.select();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      const num = parseFloat(value);
      if (!isNaN(num) && num > 0) {
        onSubmit(num);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="dimension-backdrop"
  onpointerdown={(e: PointerEvent) => { if (e.target === e.currentTarget) onCancel(); }}
>
  <div
    class="dimension-input"
    style="left: {position.x}px; top: {position.y}px;"
    onkeydown={handleKeydown}
  >
    <label class="dimension-label">{prompt}</label>
    <input
      bind:this={inputRef}
      type="number"
      bind:value
      step="0.1"
      min="0.01"
      class="dimension-field"
    />
    <div class="dimension-actions">
      <button class="dim-btn dim-ok" onclick={() => {
        const num = parseFloat(value);
        if (!isNaN(num) && num > 0) onSubmit(num);
      }}>OK</button>
      <button class="dim-btn dim-cancel" onclick={onCancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .dimension-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 100;
  }

  .dimension-input {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--bg-mantle, #181825);
    border: 1px solid var(--accent, #89b4fa);
    border-radius: 6px;
    padding: 8px 12px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    min-width: 140px;
    transform: translate(-50%, -100%) translateY(-8px);
  }

  .dimension-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted, #6c7086);
  }

  .dimension-field {
    background: var(--bg-base, #1e1e2e);
    border: 1px solid var(--border-subtle, #313244);
    border-radius: 3px;
    padding: 4px 8px;
    font-size: 13px;
    font-family: var(--font-mono, monospace);
    color: var(--text-primary, #cdd6f4);
    width: 100%;
    box-sizing: border-box;
  }

  .dimension-field:focus {
    border-color: var(--accent, #89b4fa);
    outline: none;
  }

  .dimension-actions {
    display: flex;
    gap: 4px;
  }

  .dim-btn {
    flex: 1;
    padding: 3px 8px;
    border-radius: 3px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid;
    transition: all 0.12s ease;
  }

  .dim-ok {
    background: rgba(137, 180, 250, 0.15);
    border-color: var(--accent, #89b4fa);
    color: var(--accent, #89b4fa);
  }

  .dim-ok:hover {
    background: rgba(137, 180, 250, 0.25);
  }

  .dim-cancel {
    background: none;
    border-color: var(--text-muted, #6c7086);
    color: var(--text-muted, #6c7086);
  }

  .dim-cancel:hover {
    border-color: var(--error, #f38ba8);
    color: var(--error, #f38ba8);
  }
</style>
