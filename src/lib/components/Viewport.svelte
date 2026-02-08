<script lang="ts">
  import { ViewportEngine } from '$lib/services/viewport-engine';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { triggerPipeline } from '$lib/services/execution-pipeline';
  import { threeToCadPos, threeToCadRot } from '$lib/services/coord-utils';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { handleSketchClick } from '$lib/services/sketch-interaction';
  import { snapToSketchGrid } from '$lib/services/sketch-plane-utils';
  import ViewControls from '$lib/components/ViewControls.svelte';
  import type { PrimitiveType, ObjectId, CadTransform, PrimitiveParams } from '$lib/types/cad';
  import type * as THREE from 'three';
  import { onMount } from 'svelte';

  let containerRef = $state<HTMLElement | null>(null);
  let engine: ViewportEngine | null = null;
  const viewportStore = getViewportStore();
  const scene = getSceneStore();
  const tools = getToolStore();
  const sketchStore = getSketchStore();
  const history = getHistoryStore();

  // Tracking for diff-based preview mesh sync
  type ObjectFingerprint = { params: string; transform: string; color: string; visible: boolean };
  let prevObjectMap = new Map<ObjectId, ObjectFingerprint>();

  // Track which object is being dragged by gizmo to prevent feedback loop
  let transformDraggingId: ObjectId | null = null;
  // Separate flag to suppress click selection right after a gizmo drag ends
  let recentlyDragged = false;

  // Pre-drag snapshot for undo
  let preDragSnapshot: ReturnType<typeof captureFullSnapshot> | null = null;

  // Track previous code mode for mode-switch detection
  let prevCodeMode: string | null = null;

  // ── Full snapshot helpers (scene + sketch) ──
  function captureFullSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    return {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
    };
  }

  function restoreFullSnapshot(snapshot: ReturnType<typeof captureFullSnapshot>) {
    scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
    if (snapshot.sketches !== undefined) {
      sketchStore.restoreSnapshot({
        sketches: snapshot.sketches,
        activeSketchId: snapshot.activeSketchId ?? null,
      });
    }
  }

  onMount(() => {
    if (!containerRef) return;

    try {
      viewportStore.setLoading(true);
      engine = new ViewportEngine(containerRef);
      viewportStore.setEngine(engine);

      // Only show demo box in manual mode; parametric starts empty
      if (scene.codeMode !== 'parametric') {
        engine.loadDemoBox();
        viewportStore.setHasModel(true);
      }

      viewportStore.setLoading(false);
    } catch (err) {
      viewportStore.setError(String(err));
      viewportStore.setLoading(false);
      console.error('Failed to initialize viewport:', err);
      return;
    }

    // ── Transform callbacks ──

    engine.onTransformChange((id, group) => {
      // Capture snapshot before the first change in this drag
      if (transformDraggingId === null) {
        preDragSnapshot = captureFullSnapshot();
      }
      transformDraggingId = id;
      // Convert Three.js position/rotation to CadQuery coords and update scene store
      const cadPos = threeToCadPos(group.position);
      const cadRot = threeToCadRot(group.rotation);
      const transform: CadTransform = { position: cadPos, rotation: cadRot };
      scene.updateTransform(id, transform);
    });

    engine.onTransformEnd((id, group) => {
      // Push pre-drag snapshot for undo
      if (preDragSnapshot) {
        history.pushSnapshot(preDragSnapshot);
        preDragSnapshot = null;
      }

      // Clear drag flag synchronously so the diff $effect rebuilds the mesh
      transformDraggingId = null;
      recentlyDragged = true;
      setTimeout(() => { recentlyDragged = false; }, 50);

      const cadPos = threeToCadPos(group.position);
      const cadRot = threeToCadRot(group.rotation);
      const transform: CadTransform = { position: cadPos, rotation: cadRot };
      scene.updateTransform(id, transform);
      triggerPipeline(100);
    });

    engine.onScaleEnd((id, scale) => {
      // Push pre-drag snapshot for undo
      if (preDragSnapshot) {
        history.pushSnapshot(preDragSnapshot);
        preDragSnapshot = null;
      }

      // Clear drag flag synchronously so the diff $effect rebuilds the mesh
      transformDraggingId = null;
      recentlyDragged = true;
      setTimeout(() => { recentlyDragged = false; }, 50);

      // Apply uniform scale if toggled on
      if (tools.uniformScale) {
        const s = Math.max(scale.x, scale.y, scale.z);
        scale.set(s, s, s);
      }

      const obj = scene.getObjectById(id);
      if (!obj) return;

      const newParams = applyScaleToParams(obj.params, scale);
      scene.updateParams(id, newParams);
      triggerPipeline(100);
    });

    return () => {
      viewportStore.setEngine(null);
      if (engine) {
        engine.dispose();
        engine = null;
      }
    };
  });

  // ── Scale-to-params mapping ──

  function applyScaleToParams(params: PrimitiveParams, scale: THREE.Vector3): PrimitiveParams {
    switch (params.type) {
      case 'box':
        return {
          ...params,
          width: Math.max(0.1, params.width * scale.x),
          height: Math.max(0.1, params.height * scale.y),
          depth: Math.max(0.1, params.depth * scale.z),
        };
      case 'cylinder': {
        const radialScale = (scale.x + scale.z) / 2;
        return {
          ...params,
          radius: Math.max(0.1, params.radius * radialScale),
          height: Math.max(0.1, params.height * scale.y),
        };
      }
      case 'sphere': {
        const uniformScale = (scale.x + scale.y + scale.z) / 3;
        return {
          ...params,
          radius: Math.max(0.1, params.radius * uniformScale),
        };
      }
      case 'cone': {
        const radialScale = (scale.x + scale.z) / 2;
        return {
          ...params,
          bottomRadius: Math.max(0.1, params.bottomRadius * radialScale),
          topRadius: Math.max(0, params.topRadius * radialScale),
          height: Math.max(0.1, params.height * scale.y),
        };
      }
    }
  }

  // ── Watch for pending STL data — only in manual mode ──
  $effect(() => {
    const stl = viewportStore.pendingStl;
    if (stl && engine) {
      if (scene.codeMode === 'manual') {
        engine.loadSTLFromBase64(stl);
        viewportStore.setHasModel(true);
      }
      viewportStore.setPendingStl(null);
    }
  });

  // ── Watch for clear signal (e.g. "New" project) ──
  $effect(() => {
    if (viewportStore.pendingClear && engine) {
      engine.clearModel();
      engine.removeAllObjects();
      engine.exitSketchMode();
      prevObjectMap = new Map();
      viewportStore.setPendingClear(false);
      viewportStore.setHasModel(false);
    }
  });

  // ── Sync selection visuals ──
  $effect(() => {
    if (engine) {
      engine.setSelection(scene.selectedIds);
    }
  });

  // ── Sync hover visuals ──
  $effect(() => {
    if (engine) {
      engine.setHover(scene.hoveredId);
    }
  });

  // ── Diff-based preview mesh sync ──
  $effect(() => {
    if (!engine || scene.codeMode !== 'parametric') return;

    const currentObjects = scene.objects;
    const currentIds = new Set(currentObjects.map((o) => o.id));

    // Remove meshes for deleted objects
    for (const [id] of prevObjectMap) {
      if (!currentIds.has(id)) {
        engine.removeObject(id);
        prevObjectMap.delete(id);
      }
    }

    // Add/update meshes
    for (const obj of currentObjects) {
      // Skip invisible objects
      if (!obj.visible) {
        if (prevObjectMap.has(obj.id)) {
          engine.removeObject(obj.id);
          prevObjectMap.delete(obj.id);
        }
        continue;
      }

      // Skip the actively-dragged object to avoid feedback loop with TransformControls
      if (obj.id === transformDraggingId) continue;

      const paramsStr = JSON.stringify(obj.params);
      const transformStr = JSON.stringify(obj.transform);
      const prev = prevObjectMap.get(obj.id);

      if (!prev) {
        // New object — full add
        engine.addPreviewMesh(obj.id, obj.params, obj.transform, obj.color);
        prevObjectMap.set(obj.id, {
          params: paramsStr,
          transform: transformStr,
          color: obj.color,
          visible: obj.visible,
        });
      } else if (prev.params !== paramsStr || prev.color !== obj.color) {
        // Params or color changed — full rebuild
        engine.addPreviewMesh(obj.id, obj.params, obj.transform, obj.color);
        prevObjectMap.set(obj.id, {
          params: paramsStr,
          transform: transformStr,
          color: obj.color,
          visible: obj.visible,
        });
      } else if (prev.transform !== transformStr) {
        // Only transform changed — lightweight update
        engine.updateObjectTransform(obj.id, obj.transform);
        prevObjectMap.set(obj.id, {
          ...prev,
          transform: transformStr,
        });
      }
    }
  });

  // ── Sync active tool → transform gizmo mode ──
  $effect(() => {
    if (!engine) return;

    const tool = tools.activeTool;
    if (tool === 'translate' || tool === 'rotate' || tool === 'scale') {
      engine.setTransformMode(tool);
    } else {
      engine.setTransformMode(null);
    }
  });

  // ── Sync snap settings to engine ──
  $effect(() => {
    if (!engine) return;
    engine.setTranslationSnap(tools.translateSnap);
  });

  $effect(() => {
    if (!engine) return;
    engine.setRotationSnap(tools.rotationSnap);
  });

  // ── Sync selection → gizmo attachment ──
  $effect(() => {
    if (!engine) return;

    const tool = tools.activeTool;
    const isTransformTool = tool === 'translate' || tool === 'rotate' || tool === 'scale';

    if (isTransformTool && scene.selectedIds.length === 1) {
      engine.attachTransformToObject(scene.selectedIds[0]);
    } else {
      engine.attachTransformToObject(null);
    }
  });

  // ── Placement ghost preview ──
  $effect(() => {
    if (!engine) return;

    const tool = tools.activeTool;
    if (tool.startsWith('add-')) {
      const primitiveType = tool.replace('add-', '') as PrimitiveType;
      engine.showPlacementGhost(primitiveType);
    } else {
      engine.clearGhost();
    }
  });

  // ── Mode switch ──
  $effect(() => {
    if (!engine) return;

    const mode = scene.codeMode;
    if (prevCodeMode !== null && prevCodeMode !== mode) {
      if (mode === 'manual') {
        // Parametric → Manual: clear preview meshes
        engine.removeAllObjects();
        engine.setTransformMode(null);
        prevObjectMap = new Map();
      } else {
        // Manual → Parametric: clear STL model
        engine.clearModel();
        // Preview meshes will be auto-created by the diff $effect
      }
    }
    prevCodeMode = mode;
  });

  // ── Sketch mode: enter/exit on engine ──
  $effect(() => {
    if (!engine) return;

    const sketch = sketchStore.activeSketch;
    if (sketch) {
      engine.enterSketchMode(sketch);
    } else {
      engine.exitSketchMode();
    }
  });

  // ── Sketch mode: sync entities ──
  $effect(() => {
    if (!engine) return;

    const sketch = sketchStore.activeSketch;
    if (sketch) {
      engine.syncSketchEntities(sketch, sketchStore.selectedEntityIds, sketchStore.hoveredEntityId);
    }
  });

  // ── Sketch mode: update preview ──
  $effect(() => {
    if (!engine) return;

    const sketch = sketchStore.activeSketch;
    if (sketch && sketchStore.isInSketchMode) {
      engine.updateSketchPreview(
        sketchStore.activeSketchTool,
        sketchStore.drawingPoints,
        sketchStore.previewPoint,
        sketch,
      );
    } else {
      engine.clearSketchPreview();
    }
  });

  // ── Render inactive (finished) sketches ──
  $effect(() => {
    if (!engine) return;
    engine.syncInactiveSketches(sketchStore.sketches, sketchStore.activeSketchId);
  });

  function handlePointerMove(e: PointerEvent) {
    if (!engine) return;

    // ── Sketch mode pointer move ──
    if (sketchStore.isInSketchMode) {
      const sketch = sketchStore.activeSketch;
      if (!sketch) return;

      const rawPt = engine.getSketchPlaneIntersection(e, sketch);
      if (!rawPt) return;

      // Snap if enabled
      const pt = sketchStore.sketchSnap ? snapToSketchGrid(rawPt, sketchStore.sketchSnap) : rawPt;
      sketchStore.setPreviewPoint(pt);

      // Hover detection for select tool
      if (sketchStore.activeSketchTool === 'sketch-select') {
        const hitId = engine.raycastSketchEntities(e, sketch);
        sketchStore.setHoveredEntity(hitId);
      }

      // Set cursor
      if (containerRef) {
        if (sketchStore.activeSketchTool === 'sketch-select') {
          const hitId = engine.raycastSketchEntities(e, sketch);
          containerRef.style.cursor = hitId ? 'pointer' : '';
        } else {
          containerRef.style.cursor = 'crosshair';
        }
      }
      return;
    }

    // ── Normal 3D mode pointer move ──
    if (scene.codeMode !== 'parametric') return;

    // Update ghost position when placing a primitive
    if (tools.isAddTool) {
      const gridPos = engine.getGridIntersection(e);
      if (gridPos) {
        engine.updateGhostPosition(gridPos);
      }
    }

    const hitId = engine.raycastObjects(e);
    scene.setHovered(hitId);

    // Set cursor based on context
    if (containerRef) {
      if (tools.isAddTool) {
        containerRef.style.cursor = 'crosshair';
      } else if (hitId) {
        containerRef.style.cursor = 'pointer';
      } else {
        containerRef.style.cursor = '';
      }
    }
  }

  function handlePointerDown(e: PointerEvent) {
    if (!engine) return;
    // Only handle left-click
    if (e.button !== 0) return;

    // ── Sketch mode pointer down ──
    if (sketchStore.isInSketchMode) {
      const sketch = sketchStore.activeSketch;
      if (!sketch) return;

      const tool = sketchStore.activeSketchTool;

      // Select tool: select sketch entities
      if (tool === 'sketch-select') {
        const hitId = engine.raycastSketchEntities(e, sketch);
        sketchStore.selectEntity(hitId, e.shiftKey);
        return;
      }

      // Drawing tools
      const rawPt = engine.getSketchPlaneIntersection(e, sketch);
      if (!rawPt) return;

      const pt = sketchStore.sketchSnap ? snapToSketchGrid(rawPt, sketchStore.sketchSnap) : rawPt;

      const action = handleSketchClick(
        tool,
        pt,
        sketchStore.drawingPoints,
        () => sketchStore.newEntityId(),
      );

      switch (action.type) {
        case 'advance':
          // Clear and set drawing points
          sketchStore.clearDrawingState();
          for (const p of action.points) {
            sketchStore.addDrawingPoint(p);
          }
          break;
        case 'create':
          // Push snapshot for undo before adding entity
          history.pushSnapshot(captureFullSnapshot());
          sketchStore.addEntity(action.entity);
          // Set chain points or clear
          sketchStore.clearDrawingState();
          for (const p of action.chainPoints) {
            sketchStore.addDrawingPoint(p);
          }
          triggerPipeline(100);
          break;
      }
      return;
    }

    // ── Normal 3D mode ──
    if (scene.codeMode !== 'parametric') return;

    // Don't select during or right after gizmo drag
    if (engine.isTransformDragging() || recentlyDragged) return;

    const activeTool = tools.activeTool;

    // Adding a primitive
    if (activeTool.startsWith('add-')) {
      const primitiveType = activeTool.replace('add-', '') as PrimitiveType;
      const gridPos = engine.getGridIntersection(e);
      if (gridPos) {
        history.pushSnapshot(captureFullSnapshot());
        const obj = scene.addObject(primitiveType, gridPos);
        scene.select(obj.id);
        tools.revertToSelect();
        triggerPipeline();
      }
      return;
    }

    // Selection
    if (activeTool === 'select' || activeTool === 'translate' || activeTool === 'rotate' || activeTool === 'scale') {
      const hitId = engine.raycastObjects(e);
      if (hitId) {
        scene.select(hitId, e.shiftKey);
      } else {
        scene.clearSelection();
      }
    }
  }

  function handleFinishSketch() {
    sketchStore.exitSketchMode();
    triggerPipeline(100);
  }

  // Sketch tool hint text
  const sketchToolHints: Record<string, string> = {
    'sketch-select': 'Click to select entities, Shift+click for multi-select',
    'sketch-line': 'Click to set start point, click again to draw line. Escape to stop chaining.',
    'sketch-rect': 'Click first corner, then opposite corner',
    'sketch-circle': 'Click center, then drag to set radius',
    'sketch-arc': 'Click start point, end point, then mid point of arc',
  };
</script>

<div
  class="viewport-container"
  bind:this={containerRef}
  onpointermove={handlePointerMove}
  onpointerdown={handlePointerDown}
>
  {#if viewportStore.isLoading}
    <div class="viewport-overlay">
      <span class="loading-text">Initializing 3D viewport...</span>
    </div>
  {/if}
  {#if viewportStore.error}
    <div class="viewport-overlay error">
      <span class="error-text">Viewport error: {viewportStore.error}</span>
    </div>
  {/if}

  <ViewControls />

  <!-- Scene info overlay -->
  {#if scene.codeMode === 'parametric' && scene.objects.length > 0 && !sketchStore.isInSketchMode}
    <div class="scene-info">
      {scene.objects.length} object{scene.objects.length !== 1 ? 's' : ''}
    </div>
  {/if}

  <!-- Tool hint overlay (3D mode) -->
  {#if tools.isAddTool && !sketchStore.isInSketchMode}
    <div class="tool-hint">
      Click on the grid to place {tools.activeTool.replace('add-', '')}
    </div>
  {/if}

  <!-- Sketch mode overlay -->
  {#if sketchStore.isInSketchMode}
    <div class="sketch-overlay">
      <div class="sketch-hint">
        {sketchToolHints[sketchStore.activeSketchTool] ?? ''}
      </div>
      <button class="finish-sketch-btn" onclick={handleFinishSketch}>
        Finish Sketch
      </button>
    </div>
    <div class="scene-info sketch-info">
      Sketch: {sketchStore.activeSketch?.name ?? ''} ({sketchStore.activeSketch?.plane})
      {#if sketchStore.activeSketch}
        &middot; {sketchStore.activeSketch.entities.length} entit{sketchStore.activeSketch.entities.length !== 1 ? 'ies' : 'y'}
      {/if}
    </div>
  {/if}
</div>

<style>
  .viewport-container {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    background: #1a1a2e;
  }

  .viewport-overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(26, 26, 46, 0.8);
    z-index: 5;
  }

  .loading-text {
    color: var(--text-secondary);
    font-size: 14px;
  }

  .viewport-overlay.error {
    background: rgba(26, 26, 46, 0.9);
  }

  .error-text {
    color: var(--error);
    font-size: 13px;
  }

  .scene-info {
    position: absolute;
    bottom: 8px;
    left: 8px;
    font-size: 10px;
    color: var(--text-muted);
    background: rgba(24, 24, 37, 0.8);
    padding: 2px 8px;
    border-radius: 3px;
    pointer-events: none;
    z-index: 2;
  }

  .sketch-info {
    color: #f9e2af;
    border: 1px solid rgba(249, 226, 175, 0.3);
  }

  .tool-hint {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    background: rgba(24, 24, 37, 0.9);
    padding: 4px 12px;
    border-radius: 4px;
    border: 1px solid var(--accent);
    pointer-events: none;
    z-index: 2;
  }

  .sketch-overlay {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 12px;
    z-index: 3;
  }

  .sketch-hint {
    font-size: 11px;
    color: #f9e2af;
    background: rgba(24, 24, 37, 0.9);
    padding: 4px 12px;
    border-radius: 4px;
    border: 1px solid rgba(249, 226, 175, 0.4);
    pointer-events: none;
    white-space: nowrap;
  }

  .finish-sketch-btn {
    background: rgba(249, 226, 175, 0.15);
    border: 1px solid #f9e2af;
    color: #f9e2af;
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
    white-space: nowrap;
  }

  .finish-sketch-btn:hover {
    background: rgba(249, 226, 175, 0.25);
  }
</style>
