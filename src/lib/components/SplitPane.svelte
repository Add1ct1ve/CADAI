<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    direction?: 'horizontal' | 'vertical';
    sizes?: number[];
    minSize?: number;
    panes: Snippet<[number]>;
  }

  let { direction = 'horizontal', sizes = [33, 34, 33], minSize = 150, panes }: Props = $props();

  let containerRef = $state<HTMLElement | null>(null);
  let paneSizes = $state<number[]>([]);

  // Sync paneSizes with the sizes prop
  $effect(() => {
    paneSizes = [...sizes];
  });
  let dragging = $state<number | null>(null);
  let startPos = $state(0);
  let startSizes = $state<number[]>([]);

  function onDividerMouseDown(index: number, e: MouseEvent) {
    e.preventDefault();
    dragging = index;
    startPos = direction === 'horizontal' ? e.clientX : e.clientY;
    startSizes = [...paneSizes];

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  function onMouseMove(e: MouseEvent) {
    if (dragging === null || !containerRef) return;

    const containerRect = containerRef.getBoundingClientRect();
    const containerSize = direction === 'horizontal' ? containerRect.width : containerRect.height;
    const currentPos = direction === 'horizontal' ? e.clientX : e.clientY;
    const delta = currentPos - startPos;
    const deltaPercent = (delta / containerSize) * 100;

    const newSizes = [...startSizes];
    const leftIdx = dragging;
    const rightIdx = dragging + 1;

    // Calculate minimum percentage based on pixel min
    const minPercent = (minSize / containerSize) * 100;

    let newLeft = startSizes[leftIdx] + deltaPercent;
    let newRight = startSizes[rightIdx] - deltaPercent;

    // Enforce minimums
    if (newLeft < minPercent) {
      newLeft = minPercent;
      newRight = startSizes[leftIdx] + startSizes[rightIdx] - minPercent;
    }
    if (newRight < minPercent) {
      newRight = minPercent;
      newLeft = startSizes[leftIdx] + startSizes[rightIdx] - minPercent;
    }

    newSizes[leftIdx] = newLeft;
    newSizes[rightIdx] = newRight;
    paneSizes = newSizes;
  }

  function onMouseUp() {
    dragging = null;
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  }
</script>

<div
  class="split-pane split-{direction}"
  bind:this={containerRef}
  class:dragging={dragging !== null}
>
  {#each paneSizes as size, i}
    {#if i > 0}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="divider divider-{direction}"
        role="separator"
        tabindex="-1"
        onmousedown={(e) => onDividerMouseDown(i - 1, e)}
      ></div>
    {/if}
    <div
      class="pane"
      style="{direction === 'horizontal' ? 'width' : 'height'}: {size}%;"
    >
      {@render panes(i)}
    </div>
  {/each}
</div>

<style>
  .split-pane {
    display: flex;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .split-horizontal {
    flex-direction: row;
  }

  .split-vertical {
    flex-direction: column;
  }

  .pane {
    overflow: hidden;
    position: relative;
    min-width: 0;
    min-height: 0;
  }

  .divider {
    flex-shrink: 0;
    background: var(--border-subtle);
    transition: background-color 0.15s ease;
    z-index: 10;
  }

  .divider:hover,
  .dragging .divider {
    background: var(--accent);
  }

  .divider-horizontal {
    width: 4px;
    cursor: col-resize;
  }

  .divider-vertical {
    height: 4px;
    cursor: row-resize;
  }

  .dragging {
    cursor: col-resize;
    user-select: none;
  }

  .split-vertical .dragging {
    cursor: row-resize;
  }
</style>
