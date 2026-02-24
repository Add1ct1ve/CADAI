<script lang="ts">
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { getDatumStore } from '$lib/stores/datum.svelte';
  import { getComponentStore } from '$lib/stores/component.svelte';
  import { getMateStore } from '$lib/stores/mate.svelte';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { getContextMenuStore } from '$lib/stores/context-menu.svelte';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import type { FeatureItem, ComponentId } from '$lib/types/cad';

  const featureTree = getFeatureTreeStore();
  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();
  const history = getHistoryStore();
  const contextMenu = getContextMenuStore();

  let dragIndex = $state<number | null>(null);
  let dropIndex = $state<number | null>(null);
  let focusedIndex = $state<number | null>(null);
  let treeRef = $state<HTMLElement | null>(null);
  let rollbackDebounce: ReturnType<typeof setTimeout> | null = null;

  // Expand/collapse state for components
  let collapsedComponents = $state<Set<ComponentId>>(new Set());

  function toggleComponentExpand(compId: ComponentId) {
    const next = new Set(collapsedComponents);
    if (next.has(compId)) {
      next.delete(compId);
    } else {
      next.add(compId);
    }
    collapsedComponents = next;
  }

  function isComponentCollapsed(compId: ComponentId): boolean {
    return collapsedComponents.has(compId);
  }

  // Capture snapshot for undo (delegates to page-level helper via store snapshots)
  function pushUndo() {
    // We access stores directly to build a snapshot matching SceneSnapshot shape
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const ftSnap = featureTree.snapshot();
    const datumSnap = datumStore.snapshot();
    const compSnap = componentStore.snapshot();
    const mateSnap = mateStore.snapshot();
    history.pushSnapshot({
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
      featureTree: ftSnap,
      datumPlanes: datumSnap.datumPlanes,
      datumAxes: datumSnap.datumAxes,
      selectedDatumId: datumSnap.selectedDatumId,
      components: compSnap.components,
      componentNameCounter: compSnap.nameCounter,
      selectedComponentId: compSnap.selectedComponentId,
      mates: mateSnap.mates,
      selectedMateId: mateSnap.selectedMateId,
    });
  }

  function handleClick(item: FeatureItem) {
    if (item.kind === 'component') {
      scene.clearSelection();
      sketchStore.selectSketch(null);
      datumStore.selectDatum(null);
      mateStore.selectMate(null);
      componentStore.selectComponent(item.id);
      return;
    }
    if (item.kind === 'mate') {
      scene.clearSelection();
      sketchStore.selectSketch(null);
      datumStore.selectDatum(null);
      componentStore.selectComponent(null);
      mateStore.selectMate(item.id);
      return;
    }
    // Clear component and mate selection when selecting other items
    componentStore.selectComponent(null);
    mateStore.selectMate(null);
    if (item.kind === 'primitive') {
      scene.select(item.id);
      sketchStore.selectSketch(null);
      datumStore.selectDatum(null);
    } else if (item.kind === 'sketch') {
      scene.clearSelection();
      sketchStore.selectSketch(item.id);
      datumStore.selectDatum(null);
    } else {
      // datum-plane or datum-axis
      scene.clearSelection();
      sketchStore.selectSketch(null);
      datumStore.selectDatum(item.id);
    }
  }

  function handleDoubleClick(item: FeatureItem) {
    handleClick(item);
    window.dispatchEvent(new CustomEvent('feature-tree:edit'));
  }

  function handleSuppress(e: MouseEvent, item: FeatureItem) {
    e.stopPropagation();
    pushUndo();
    featureTree.toggleSuppressed(item.id);
    triggerPipeline(100);
    runPythonExecution();
  }

  function handleDelete(e: MouseEvent, item: FeatureItem) {
    e.stopPropagation();
    pushUndo();
    if (item.kind === 'component') {
      // Dissolve: remove component grouping, features become root-level
      featureTree.unregisterComponent(item.id);
      componentStore.removeComponent(item.id);
    } else if (item.kind === 'primitive') {
      scene.removeObject(item.id);
    } else if (item.kind === 'sketch') {
      sketchStore.removeSketch(item.id);
    } else if (item.kind === 'datum-plane') {
      datumStore.removeDatumPlane(item.id);
    } else if (item.kind === 'datum-axis') {
      datumStore.removeDatumAxis(item.id);
    } else if (item.kind === 'mate') {
      mateStore.removeMate(item.id);
    }
    triggerPipeline(100);
    runPythonExecution();
  }

  function handleContextMenu(e: MouseEvent, item: FeatureItem) {
    e.preventDefault();
    e.stopPropagation();

    const menuItems = [];

    if (item.kind !== 'component') {
      menuItems.push({
        label: 'Edit Properties',
        action: () => { handleClick(item); window.dispatchEvent(new CustomEvent('feature-tree:edit')); },
      });
    }

    if (item.kind !== 'component' && item.kind !== 'mate') {
      menuItems.push({
        label: item.suppressed ? 'Unsuppress' : 'Suppress',
        action: () => { pushUndo(); featureTree.toggleSuppressed(item.id); triggerPipeline(100); runPythonExecution(); },
      });
    }

    menuItems.push({ separator: true, label: '', action: () => {} });
    menuItems.push({
      label: item.kind === 'component' ? 'Dissolve Component' : 'Delete',
      action: () => { handleDelete({ stopPropagation: () => {} } as MouseEvent, item); },
    });

    contextMenu.show(e.clientX, e.clientY, menuItems);
  }

  function handleToggleVisibility(e: MouseEvent, item: FeatureItem) {
    e.stopPropagation();
    if (item.kind === 'component') {
      pushUndo();
      const comp = componentStore.getComponentById(item.id);
      if (comp) componentStore.setVisible(item.id, !comp.visible);
      triggerPipeline(100);
    }
  }

  function handleToggleGrounded(e: MouseEvent, item: FeatureItem) {
    e.stopPropagation();
    if (item.kind === 'component') {
      pushUndo();
      const comp = componentStore.getComponentById(item.id);
      if (comp) componentStore.setGrounded(item.id, !comp.grounded);
    }
  }

  // ── Drag and drop ──
  function handleDragStart(e: DragEvent, index: number) {
    dragIndex = index;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', String(index));
    }
  }

  function handleDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'move';
    }
    dropIndex = index;
  }

  function handleDragLeave() {
    dropIndex = null;
  }

  function handleDrop(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex !== null && dragIndex !== index) {
      pushUndo();
      featureTree.reorder(dragIndex, index);
      triggerPipeline(100);
      runPythonExecution();
    }
    dragIndex = null;
    dropIndex = null;
  }

  function handleDragEnd() {
    dragIndex = null;
    dropIndex = null;
  }

  // ── Rollback slider ──
  function handleRollback(e: Event) {
    const target = e.target as HTMLInputElement;
    const val = parseInt(target.value, 10);
    const features = featureTree.features;
    const maxIdx = features.length - 1;

    featureTree.setRollbackIndex(val >= maxIdx ? null : val);

    if (rollbackDebounce) clearTimeout(rollbackDebounce);
    rollbackDebounce = setTimeout(() => {
      triggerPipeline(0);
      runPythonExecution();
    }, 300);
  }

  // ── Keyboard ──
  function handleKeydown(e: KeyboardEvent) {
    const features = featureTree.features;
    if (features.length === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusedIndex = focusedIndex === null ? 0 : Math.min(focusedIndex + 1, features.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIndex = focusedIndex === null ? 0 : Math.max(focusedIndex - 1, 0);
    } else if (e.key === 'Enter' && focusedIndex !== null) {
      handleClick(features[focusedIndex]);
    } else if ((e.key === 'Delete' || e.key === 'Backspace') && focusedIndex !== null) {
      e.preventDefault();
      handleDelete(e as unknown as MouseEvent, features[focusedIndex]);
    } else if (e.key === ' ' && focusedIndex !== null) {
      e.preventDefault();
      pushUndo();
      featureTree.toggleSuppressed(features[focusedIndex].id);
      triggerPipeline(100);
      runPythonExecution();
    }
  }

  function isSelected(item: FeatureItem): boolean {
    if (item.kind === 'component') return componentStore.selectedComponentId === item.id;
    if (item.kind === 'mate') return mateStore.selectedMateId === item.id;
    if (item.kind === 'primitive') return scene.selectedIds.includes(item.id);
    if (item.kind === 'sketch') return sketchStore.selectedSketchId === item.id;
    return datumStore.selectedDatumId === item.id;
  }

  /** Check if a child feature's parent component is collapsed */
  function isHiddenByCollapse(item: FeatureItem): boolean {
    if (!item.componentId) return false;
    return isComponentCollapsed(item.componentId);
  }

  /** Check if item belongs to a hidden component (for dimming) */
  function isInHiddenComponent(item: FeatureItem): boolean {
    if (!item.componentId) return false;
    const comp = componentStore.getComponentById(item.componentId);
    return comp !== null && !comp.visible;
  }

  function isPastRollback(index: number): boolean {
    if (featureTree.rollbackIndex === null) return false;
    return index > featureTree.rollbackIndex;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="feature-tree"
  bind:this={treeRef}
  tabindex="0"
  role="tree"
  onkeydown={handleKeydown}
>
  <div class="tree-header">
    <span class="tree-title">Features</span>
    {#if featureTree.features.length > 0}
      <span class="tree-count">{featureTree.features.length}</span>
    {/if}
  </div>

  <div class="tree-list">
    {#if featureTree.features.length === 0}
      <div class="tree-empty">
        No features yet. Add primitives or sketches using the toolbar.
      </div>
    {:else}
      {#each featureTree.features as item, index (item.id + (item.componentId ?? ''))}
        {#if !isHiddenByCollapse(item)}
          <div
            class="tree-item"
            role="treeitem"
            class:selected={isSelected(item)}
            class:suppressed={item.suppressed}
            class:past-rollback={isPastRollback(index)}
            class:dragging={dragIndex === index}
            class:drop-target={dropIndex === index && dragIndex !== index}
            class:focused={focusedIndex === index}
            class:component-item={item.kind === 'component'}
            class:component-child={item.depth > 0}
            class:hidden-component={isInHiddenComponent(item)}
            style:padding-left="{12 + item.depth * 16}px"
            style:border-left-color={item.kind === 'component' ? item.color : undefined}
            draggable={item.depth === 0 ? 'true' : 'false'}
            onclick={() => handleClick(item)}
            ondblclick={() => handleDoubleClick(item)}
            oncontextmenu={(e) => handleContextMenu(e, item)}
            ondragstart={(e) => item.depth === 0 ? handleDragStart(e, index) : null}
            ondragover={(e) => item.depth === 0 ? handleDragOver(e, index) : null}
            ondragleave={item.depth === 0 ? handleDragLeave : undefined}
            ondrop={(e) => item.depth === 0 ? handleDrop(e, index) : null}
            ondragend={item.depth === 0 ? handleDragEnd : undefined}
          >
            {#if item.kind === 'component'}
              <button
                class="expand-btn"
                onclick={(e) => { e.stopPropagation(); toggleComponentExpand(item.id); }}
                title={isComponentCollapsed(item.id) ? 'Expand' : 'Collapse'}
              >
                {isComponentCollapsed(item.id) ? '\u25B6' : '\u25BC'}
              </button>
            {/if}
            <span class="item-icon">{item.icon}</span>
            <div class="item-content">
              <span class="item-name">{item.name}</span>
              <span class="item-detail">{item.detail}</span>
            </div>
            <div class="item-actions">
              {#if item.kind === 'component'}
                <button
                  class="action-btn visibility-btn"
                  class:hidden-eye={!componentStore.getComponentById(item.id)?.visible}
                  title={componentStore.getComponentById(item.id)?.visible ? 'Hide' : 'Show'}
                  onclick={(e) => handleToggleVisibility(e, item)}
                >
                  {componentStore.getComponentById(item.id)?.visible ? '\u{1F441}' : '\u{1F441}\u{FE0E}'}
                </button>
                <button
                  class="action-btn ground-btn"
                  class:grounded={componentStore.getComponentById(item.id)?.grounded}
                  title={componentStore.getComponentById(item.id)?.grounded ? 'Unground' : 'Ground'}
                  onclick={(e) => handleToggleGrounded(e, item)}
                >
                  {componentStore.getComponentById(item.id)?.grounded ? '\u{1F4CC}' : '\u{1F4CC}'}
                </button>
              {/if}
              {#if item.kind !== 'component'}
                <button
                  class="action-btn"
                  class:active={item.suppressed}
                  title={item.suppressed ? 'Unsuppress' : 'Suppress'}
                  onclick={(e) => handleSuppress(e, item)}
                >
                  {item.suppressed ? '\u25C9' : '\u25CE'}
                </button>
              {/if}
              {#if item.kind === 'mate'}
                <!-- Mates show as "mate" kind with chain icon -->
              {/if}
              <button
                class="action-btn delete-btn"
                title={item.kind === 'component' ? 'Dissolve Component' : 'Delete'}
                onclick={(e) => handleDelete(e, item)}
              >
                \u2715
              </button>
            </div>
          </div>
        {/if}
      {/each}
    {/if}
  </div>

  {#if featureTree.features.length > 1}
    <div class="rollback-bar">
      <label class="rollback-label">
        <span>Rollback</span>
        <span class="rollback-value">
          {featureTree.rollbackIndex === null
            ? featureTree.features.length
            : featureTree.rollbackIndex + 1} / {featureTree.features.length}
        </span>
      </label>
      <input
        type="range"
        class="rollback-slider"
        min="0"
        max={featureTree.features.length - 1}
        value={featureTree.rollbackIndex ?? featureTree.features.length - 1}
        oninput={handleRollback}
      />
    </div>
  {/if}
</div>

<style>
  .feature-tree {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-surface);
    outline: none;
  }

  .tree-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-mantle);
    flex-shrink: 0;
  }

  .tree-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }

  .tree-count {
    font-size: 9px;
    font-weight: 700;
    background: var(--accent);
    color: var(--bg-base);
    border-radius: 8px;
    padding: 0 5px;
    min-width: 14px;
    text-align: center;
    line-height: 14px;
  }

  .tree-list {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 0;
  }

  .tree-empty {
    padding: 16px 12px;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
    line-height: 1.5;
  }

  .tree-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    cursor: pointer;
    border-left: 3px solid transparent;
    transition: all 0.1s ease;
    user-select: none;
    position: relative;
  }

  .tree-item:hover {
    background: var(--bg-overlay);
  }

  .tree-item:hover .item-actions {
    opacity: 1;
  }

  .tree-item.selected {
    border-left-color: var(--accent);
    background: rgba(137, 180, 250, 0.08);
  }

  .tree-item.suppressed {
    opacity: 0.45;
  }

  .tree-item.suppressed .item-name {
    text-decoration: line-through;
  }

  .tree-item.past-rollback {
    opacity: 0.25;
    border-left-style: dashed;
    border-left-color: var(--border);
  }

  .tree-item.dragging {
    opacity: 0.3;
  }

  .tree-item.drop-target {
    border-top: 2px solid var(--accent);
  }

  .tree-item.focused {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }

  .tree-item.component-item {
    border-left-width: 3px;
    border-left-style: solid;
  }

  .tree-item.component-child {
    font-size: 11px;
    padding-top: 4px;
    padding-bottom: 4px;
  }

  .tree-item.hidden-component {
    opacity: 0.35;
  }

  .expand-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 2px;
    font-size: 8px;
    line-height: 1;
    flex-shrink: 0;
    width: 14px;
    text-align: center;
    transition: color 0.1s ease;
  }

  .expand-btn:hover {
    color: var(--text-primary);
  }

  .visibility-btn {
    font-size: 10px;
  }

  .visibility-btn.hidden-eye {
    opacity: 0.4;
  }

  .ground-btn {
    font-size: 10px;
  }

  .ground-btn.grounded {
    color: var(--warning);
  }

  .item-icon {
    font-size: 14px;
    flex-shrink: 0;
    width: 18px;
    text-align: center;
  }

  .item-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .item-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-detail {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.1s ease;
    flex-shrink: 0;
  }

  .action-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px 4px;
    font-size: 11px;
    border-radius: 3px;
    line-height: 1;
    transition: all 0.1s ease;
  }

  .action-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }

  .action-btn.active {
    color: var(--warning);
  }

  .delete-btn:hover {
    color: var(--error);
  }

  /* Rollback bar */
  .rollback-bar {
    padding: 8px 12px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-mantle);
    flex-shrink: 0;
  }

  .rollback-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 10px;
    color: var(--text-muted);
    margin-bottom: 4px;
  }

  .rollback-value {
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .rollback-slider {
    width: 100%;
    height: 4px;
    appearance: none;
    -webkit-appearance: none;
    background: var(--border-subtle);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .rollback-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
  }

  .rollback-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }
</style>
