<script lang="ts">
  import { getDrawingStore } from '$lib/stores/drawing.svelte';
  import { getSheetDimensions } from '$lib/types/drawing';
  import type { DrawingView, Dimension, DrawingNote } from '$lib/types/drawing';

  const store = getDrawingStore();

  let container: HTMLDivElement;
  let panX = $state(0);
  let panY = $state(0);
  let zoom = $state(1);
  let isPanning = $state(false);
  let panStart = $state({ x: 0, y: 0 });
  let dragView = $state<{ viewId: string; startX: number; startY: number; origX: number; origY: number } | null>(null);

  const drawing = $derived(store.activeDrawing);
  const sheetDims = $derived(drawing ? getSheetDimensions(drawing.paperSize, drawing.orientation) : [297, 210] as [number, number]);
  const sheetW = $derived(sheetDims[0]);
  const sheetH = $derived(sheetDims[1]);

  // Title block position
  const tbW = 170;
  const tbH = 40;
  const tbX = $derived(sheetW - 10 - tbW);
  const tbY = $derived(sheetH - 10 - tbH);
  const tbMid = $derived(tbX + tbW / 2);

  // Zoom with mouse wheel
  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    const newZoom = Math.max(0.1, Math.min(10, zoom * delta));

    // Zoom toward cursor position
    const rect = container.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;

    panX = cx - (cx - panX) * (newZoom / zoom);
    panY = cy - (cy - panY) * (newZoom / zoom);
    zoom = newZoom;
  }

  function handleMouseDown(e: MouseEvent) {
    // Middle button or space+left for panning
    if (e.button === 1 || (e.button === 0 && e.altKey)) {
      e.preventDefault();
      isPanning = true;
      panStart = { x: e.clientX - panX, y: e.clientY - panY };
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isPanning) {
      panX = e.clientX - panStart.x;
      panY = e.clientY - panStart.y;
    }
    if (dragView && drawing) {
      const rect = container.getBoundingClientRect();
      const dx = (e.clientX - dragView.startX) / zoom;
      const dy = (e.clientY - dragView.startY) / zoom;
      store.updateView(drawing.id, dragView.viewId, {
        x: dragView.origX + dx,
        y: dragView.origY + dy,
      });
    }
  }

  function handleMouseUp() {
    isPanning = false;
    dragView = null;
  }

  function handleViewMouseDown(e: MouseEvent, view: DrawingView) {
    if (e.button !== 0 || e.altKey) return;
    e.stopPropagation();
    store.selectView(view.id);

    if (store.drawingTool === 'select') {
      dragView = {
        viewId: view.id,
        startX: e.clientX,
        startY: e.clientY,
        origX: view.x,
        origY: view.y,
      };
    }
  }

  function handleDimensionClick(e: MouseEvent, dim: Dimension) {
    e.stopPropagation();
    store.selectDimension(dim.id);
  }

  function handleNoteClick(e: MouseEvent, note: DrawingNote) {
    e.stopPropagation();
    store.selectNote(note.id);
  }

  function handleSheetClick() {
    store.clearSelection();
  }

  function fitToContainer() {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const scaleX = (rect.width - 40) / sheetW;
    const scaleY = (rect.height - 40) / sheetH;
    zoom = Math.min(scaleX, scaleY);
    panX = (rect.width - sheetW * zoom) / 2;
    panY = (rect.height - sheetH * zoom) / 2;
  }

  function extractSvgInner(svgString: string): string {
    return svgString
      .replace(/<svg[^>]*>/, '')
      .replace(/<\/svg>\s*$/, '')
      .trim();
  }

  $effect(() => {
    if (container && drawing) {
      // Auto-fit on first render or when drawing changes
      fitToContainer();
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="drawing-viewer"
  bind:this={container}
  onwheel={handleWheel}
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
  onmouseleave={handleMouseUp}
>
  {#if drawing}
    <svg
      class="drawing-sheet"
      style="transform: translate({panX}px, {panY}px) scale({zoom}); transform-origin: 0 0;"
      viewBox="0 0 {sheetW} {sheetH}"
      width="{sheetW}"
      height="{sheetH}"
    >
      <!-- Paper background -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <rect
        class="paper"
        x="0" y="0"
        width={sheetW} height={sheetH}
        fill="white"
        stroke="#ccc"
        stroke-width="0.5"
        onclick={handleSheetClick}
      />

      <!-- Border -->
      <rect
        x="10" y="10"
        width={sheetW - 20} height={sheetH - 20}
        fill="none"
        stroke="#333"
        stroke-width="0.4"
      />

      <!-- Arrow markers for dimensions -->
      <defs>
        <marker id="arrow-start" markerWidth="6" markerHeight="4" refX="0" refY="2" orient="auto">
          <path d="M6,0 L0,2 L6,4" fill="none" stroke="black" stroke-width="0.3"/>
        </marker>
        <marker id="arrow-end" markerWidth="6" markerHeight="4" refX="6" refY="2" orient="auto">
          <path d="M0,0 L6,2 L0,4" fill="none" stroke="black" stroke-width="0.3"/>
        </marker>
      </defs>

      <!-- Views -->
      {#each drawing.views as view}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <g
          class="drawing-view"
          class:selected={view.id === store.selectedViewId}
          transform="translate({view.x}, {view.y})"
          onmousedown={(e) => handleViewMouseDown(e, view)}
        >
          {#if view.svgContent}
            <g transform="scale({view.scale})">
              {@html extractSvgInner(view.svgContent)}
            </g>
          {:else}
            <!-- Placeholder -->
            <rect
              x="-20" y="-15"
              width="40" height="30"
              fill="none" stroke="#ddd" stroke-width="0.3" stroke-dasharray="2,2"
            />
            <text x="0" y="2" font-size="3" fill="#999" text-anchor="middle">
              {store.isGenerating ? 'Generating...' : 'No content'}
            </text>
          {/if}
          <!-- View label -->
          <text
            x="0"
            y={(view.height || 30) / 2 + 5}
            font-size="3"
            fill="#333"
            text-anchor="middle"
            font-family="Arial, sans-serif"
          >
            {view.label}
            {#if view.scale !== 1}({view.scale < 1 ? `1:${Math.round(1/view.scale)}` : `${Math.round(view.scale)}:1`}){/if}
          </text>
          <!-- Selection border -->
          {#if view.id === store.selectedViewId}
            <rect
              x={-(view.width || 40) / 2 - 3}
              y={-(view.height || 30) / 2 - 3}
              width={(view.width || 40) + 6}
              height={(view.height || 30) + 6}
              fill="none"
              stroke="#89b4fa"
              stroke-width="0.4"
              stroke-dasharray="2,2"
            />
          {/if}
        </g>
      {/each}

      <!-- Dimensions -->
      {#each drawing.dimensions as dim}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <g
          class="drawing-dimension"
          class:selected={dim.id === store.selectedDimensionId}
          onclick={(e) => handleDimensionClick(e, dim)}
        >
          {#if dim.type === 'linear'}
            {@const isHoriz = Math.abs(dim.x2 - dim.x1) >= Math.abs(dim.y2 - dim.y1)}
            {#if isHoriz}
              {@const y = Math.min(dim.y1, dim.y2) - dim.offsetDistance}
              <!-- Extension lines -->
              <line x1={dim.x1} y1={dim.y1} x2={dim.x1} y2={y} stroke="black" stroke-width="0.15"/>
              <line x1={dim.x2} y1={dim.y2} x2={dim.x2} y2={y} stroke="black" stroke-width="0.15"/>
              <!-- Dimension line -->
              <line x1={dim.x1} y1={y} x2={dim.x2} y2={y} stroke="black" stroke-width="0.2" marker-start="url(#arrow-start)" marker-end="url(#arrow-end)"/>
              <!-- Text -->
              <text x={(dim.x1 + dim.x2) / 2} y={y - 1} font-size="2.5" text-anchor="middle" font-family="Arial, sans-serif">
                {dim.text ?? dim.value.toFixed(2)}
              </text>
            {:else}
              {@const x = Math.max(dim.x1, dim.x2) + dim.offsetDistance}
              <line x1={dim.x1} y1={dim.y1} x2={x} y2={dim.y1} stroke="black" stroke-width="0.15"/>
              <line x1={dim.x2} y1={dim.y2} x2={x} y2={dim.y2} stroke="black" stroke-width="0.15"/>
              <line x1={x} y1={dim.y1} x2={x} y2={dim.y2} stroke="black" stroke-width="0.2"/>
              <text x={x + 1} y={(dim.y1 + dim.y2) / 2} font-size="2.5" font-family="Arial, sans-serif" dominant-baseline="middle">
                {dim.text ?? dim.value.toFixed(2)}
              </text>
            {/if}
          {:else if dim.type === 'radial' || dim.type === 'diameter'}
            {@const prefix = dim.type === 'diameter' ? '\u00D8' : 'R'}
            <line x1={dim.x1} y1={dim.y1} x2={dim.x2} y2={dim.y2} stroke="black" stroke-width="0.2"/>
            <text x={(dim.x1 + dim.x2) / 2} y={(dim.y1 + dim.y2) / 2 - 1} font-size="2.5" text-anchor="middle" font-family="Arial, sans-serif">
              {dim.text ?? `${prefix}${dim.value.toFixed(2)}`}
            </text>
          {/if}
        </g>
      {/each}

      <!-- Notes -->
      {#each drawing.notes as note}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <text
          class="drawing-note"
          class:selected={note.id === store.selectedNoteId}
          x={note.x}
          y={note.y}
          font-size={note.fontSize * 0.35}
          font-weight={note.bold ? 'bold' : 'normal'}
          font-family="Arial, sans-serif"
          fill="#333"
          onclick={(e) => handleNoteClick(e, note)}
        >
          {note.text}
        </text>
      {/each}

      <!-- Title Block -->
      <g class="title-block">
        <rect x={tbX} y={tbY} width={tbW} height={tbH} fill="none" stroke="#333" stroke-width="0.4"/>
        <!-- Horizontal dividers -->
        <line x1={tbX} y1={tbY + 10} x2={tbX + tbW} y2={tbY + 10} stroke="#333" stroke-width="0.2"/>
        <line x1={tbX} y1={tbY + 20} x2={tbX + tbW} y2={tbY + 20} stroke="#333" stroke-width="0.2"/>
        <line x1={tbX} y1={tbY + 30} x2={tbX + tbW} y2={tbY + 30} stroke="#333" stroke-width="0.2"/>
        <!-- Vertical divider -->
        <line x1={tbMid} y1={tbY + 10} x2={tbMid} y2={tbY + tbH} stroke="#333" stroke-width="0.2"/>
        <!-- Title -->
        <text x={tbX + tbW / 2} y={tbY + 7} font-size="4" font-weight="bold" text-anchor="middle" font-family="Arial, sans-serif">{drawing.titleBlock.title}</text>
        <!-- Row 2 -->
        <text x={tbX + 2} y={tbY + 17} font-size="2.5" font-family="Arial, sans-serif">Author: {drawing.titleBlock.author}</text>
        <text x={tbMid + 2} y={tbY + 17} font-size="2.5" font-family="Arial, sans-serif">Date: {drawing.titleBlock.date}</text>
        <!-- Row 3 -->
        <text x={tbX + 2} y={tbY + 27} font-size="2.5" font-family="Arial, sans-serif">Scale: {drawing.titleBlock.scale}</text>
        <text x={tbMid + 2} y={tbY + 27} font-size="2.5" font-family="Arial, sans-serif">Sheet: {drawing.titleBlock.sheetNumber}</text>
        <!-- Row 4 -->
        <text x={tbX + 2} y={tbY + 37} font-size="2.5" font-family="Arial, sans-serif">Material: {drawing.titleBlock.material}</text>
        <text x={tbMid + 2} y={tbY + 37} font-size="2.5" font-family="Arial, sans-serif">Rev: {drawing.titleBlock.revision}</text>
      </g>
    </svg>

    <!-- Zoom info -->
    <div class="zoom-info">
      {Math.round(zoom * 100)}%
      <button class="zoom-btn" onclick={fitToContainer}>Fit</button>
    </div>
  {:else}
    <div class="empty-state">
      <p>No drawing loaded</p>
    </div>
  {/if}
</div>

<style>
  .drawing-viewer {
    width: 100%;
    height: 100%;
    background: var(--bg-crust, #11111b);
    overflow: hidden;
    position: relative;
    cursor: default;
    user-select: none;
  }

  .drawing-sheet {
    position: absolute;
    top: 0;
    left: 0;
    filter: drop-shadow(0 2px 8px rgba(0, 0, 0, 0.3));
  }

  .drawing-view {
    cursor: move;
  }

  .drawing-view.selected {
    filter: none;
  }

  .drawing-dimension.selected line,
  .drawing-dimension.selected text {
    stroke: #89b4fa;
    fill: #89b4fa;
  }

  .drawing-note.selected {
    fill: #89b4fa !important;
  }

  .zoom-info {
    position: absolute;
    bottom: 8px;
    right: 8px;
    background: var(--bg-mantle, #1e1e2e);
    border: 1px solid var(--border-subtle, #45475a);
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 11px;
    color: var(--text-muted, #6c7086);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .zoom-btn {
    background: none;
    border: 1px solid var(--border-subtle, #45475a);
    color: var(--text-secondary, #a6adc8);
    border-radius: 3px;
    padding: 1px 6px;
    font-size: 10px;
    cursor: pointer;
  }

  .zoom-btn:hover {
    background: var(--bg-overlay, rgba(255, 255, 255, 0.05));
    color: var(--text-primary, #cdd6f4);
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted, #6c7086);
    font-size: 14px;
  }
</style>
