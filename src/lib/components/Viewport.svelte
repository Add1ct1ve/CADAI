<script lang="ts">
  import { ViewportEngine } from '$lib/services/viewport-engine';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { getDatumStore } from '$lib/stores/datum.svelte';
  import { getComponentStore } from '$lib/stores/component.svelte';
  import { getMateStore } from '$lib/stores/mate.svelte';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import { threeToCadPos, threeToCadRot } from '$lib/services/coord-utils';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import { handleSketchClick } from '$lib/services/sketch-interaction';
  import { handleConstraintSelection } from '$lib/services/constraint-interaction';
  import { handleSketchOp, handleTrim } from '$lib/services/sketch-operations';
  import type { PendingSketchOp, SketchOpAction } from '$lib/services/sketch-operations';
  import { snapToSketchGrid, computeDatumPlaneInfo } from '$lib/services/sketch-plane-utils';
  import { isDatumPlane } from '$lib/types/cad';
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { normalToFaceSelector } from '$lib/services/mate-utils';
  import ViewControls from '$lib/components/ViewControls.svelte';
  import DimensionInput from '$lib/components/DimensionInput.svelte';
  import MeasurePanel from '$lib/components/MeasurePanel.svelte';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getMeasureStore } from '$lib/stores/measure.svelte';
  import { computeMassProperties } from '$lib/services/mass-properties';
  import { nanoid } from 'nanoid';
  import type { PrimitiveType, ObjectId, CadTransform, PrimitiveParams, SketchConstraint, SketchToolId, MeasurePoint } from '$lib/types/cad';
  import * as THREE from 'three';
  import { onMount } from 'svelte';

  let containerRef = $state<HTMLElement | null>(null);
  let engine: ViewportEngine | null = null;
  const viewportStore = getViewportStore();
  const scene = getSceneStore();
  const tools = getToolStore();
  const sketchStore = getSketchStore();
  const datumStore = getDatumStore();
  const featureTree = getFeatureTreeStore();
  const history = getHistoryStore();
  const measureStore = getMeasureStore();
  const settingsStore = getSettingsStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();

  // Tracking for diff-based preview mesh sync
  type ObjectFingerprint = { params: string; transform: string; color: string; visible: boolean; metalness: number; roughness: number; opacity: number };
  let prevObjectMap = new Map<ObjectId, ObjectFingerprint>();

  // Track which object is being dragged by gizmo to prevent feedback loop
  let transformDraggingId: ObjectId | null = null;
  // Separate flag to suppress click selection right after a gizmo drag ends
  let recentlyDragged = false;

  // Pre-drag snapshot for undo
  let preDragSnapshot: ReturnType<typeof captureFullSnapshot> | null = null;

  // Track previous code mode for mode-switch detection
  let prevCodeMode: string | null = null;

  // Dimension input state for constraint tools
  let dimensionInputVisible = $state(false);
  let dimensionInputPrompt = $state('');
  let dimensionInputDefaultValue = $state(0);
  let dimensionInputPosition = $state({ x: 0, y: 0 });
  let pendingConstraint = $state<Partial<SketchConstraint> | null>(null);
  let pendingSketchOp = $state<PendingSketchOp | null>(null);
  let constraintStatusMessage = $state('');

  // ── Full snapshot helpers (scene + sketch + datum + mate) ──
  function captureFullSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const datumSnap = datumStore.snapshot();
    const compSnap = componentStore.snapshot();
    const mateSnap = mateStore.snapshot();
    return {
      ...sceneSnap,
      sketches: sketchSnap.sketches,
      activeSketchId: sketchSnap.activeSketchId,
      selectedSketchId: sketchSnap.selectedSketchId,
      datumPlanes: datumSnap.datumPlanes,
      datumAxes: datumSnap.datumAxes,
      selectedDatumId: datumSnap.selectedDatumId,
      components: compSnap.components,
      componentNameCounter: compSnap.nameCounter,
      selectedComponentId: compSnap.selectedComponentId,
      mates: mateSnap.mates,
      selectedMateId: mateSnap.selectedMateId,
    };
  }

  function restoreFullSnapshot(snapshot: ReturnType<typeof captureFullSnapshot>) {
    scene.restoreSnapshot({ objects: snapshot.objects, selectedIds: snapshot.selectedIds });
    if (snapshot.sketches !== undefined) {
      sketchStore.restoreSnapshot({
        sketches: snapshot.sketches,
        activeSketchId: snapshot.activeSketchId ?? null,
        selectedSketchId: snapshot.selectedSketchId ?? null,
      });
    }
    if (snapshot.datumPlanes !== undefined || snapshot.datumAxes !== undefined) {
      datumStore.restoreSnapshot({
        datumPlanes: snapshot.datumPlanes ?? [],
        datumAxes: snapshot.datumAxes ?? [],
        selectedDatumId: snapshot.selectedDatumId ?? null,
      });
    }
    if (snapshot.mates !== undefined) {
      mateStore.restoreSnapshot({
        mates: snapshot.mates,
        selectedMateId: snapshot.selectedMateId ?? null,
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

      // Apply persisted grid and theme settings
      const cfg = settingsStore.config;
      if (cfg.grid_size !== 100 || cfg.grid_spacing !== 1) {
        engine.rebuildGrid(cfg.grid_size, cfg.grid_spacing);
      }
      if (cfg.theme === 'light') {
        engine.setThemeColors('light');
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

  // ── Watch for pending STL data — works in both manual and parametric modes ──
  $effect(() => {
    const stl = viewportStore.pendingStl;
    if (stl && engine) {
      engine.loadSTLFromBase64(stl);
      viewportStore.setHasModel(true);
      viewportStore.setPendingStl(null);
    }
  });

  // ── Watch for clear signal (e.g. "New" project) ──
  $effect(() => {
    if (viewportStore.pendingClear && engine) {
      engine.clearModel();
      engine.removeAllObjects();
      engine.removeAllDatums();
      engine.clearAllMeasurements();
      engine.exitSketchMode();
      measureStore.clearAll();
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

    // Build component feature map once per sync cycle
    const compFeatureMap = componentStore.getFeatureComponentMap();

    // Add/update meshes
    for (const obj of currentObjects) {
      // Skip invisible objects (including those in hidden components)
      const compId = compFeatureMap.get(obj.id);
      const comp = compId ? componentStore.getComponentById(compId) : null;
      const effectivelyVisible = obj.visible && (!comp || comp.visible);
      if (!effectivelyVisible) {
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
      const curMetalness = obj.metalness ?? 0.3;
      const curRoughness = obj.roughness ?? 0.7;
      const curOpacity = obj.opacity ?? 1.0;
      const prev = prevObjectMap.get(obj.id);

      const fingerprint: ObjectFingerprint = {
        params: paramsStr,
        transform: transformStr,
        color: obj.color,
        visible: obj.visible,
        metalness: curMetalness,
        roughness: curRoughness,
        opacity: curOpacity,
      };

      if (!prev) {
        // New object — full add
        engine.addPreviewMesh(obj.id, obj.params, obj.transform, obj.color, obj.metalness, obj.roughness, obj.opacity);
        prevObjectMap.set(obj.id, fingerprint);
      } else if (prev.params !== paramsStr) {
        // Params changed — full rebuild
        engine.addPreviewMesh(obj.id, obj.params, obj.transform, obj.color, obj.metalness, obj.roughness, obj.opacity);
        prevObjectMap.set(obj.id, fingerprint);
      } else if (prev.color !== obj.color || prev.metalness !== curMetalness || prev.roughness !== curRoughness || prev.opacity !== curOpacity) {
        // Material/color changed — lightweight material update (no geometry rebuild)
        engine.updateObjectMaterial(obj.id, obj.color, obj.metalness, obj.roughness, obj.opacity);
        if (prev.transform !== transformStr) {
          engine.updateObjectTransform(obj.id, obj.transform);
        }
        prevObjectMap.set(obj.id, fingerprint);
      } else if (prev.transform !== transformStr) {
        // Only transform changed — lightweight update
        engine.updateObjectTransform(obj.id, obj.transform);
        prevObjectMap.set(obj.id, { ...prev, transform: transformStr });
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
      // Don't attach gizmo if the feature is in a grounded component
      const selId = scene.selectedIds[0];
      const selComp = componentStore.getComponentForFeature(selId);
      if (selComp?.grounded) {
        engine.attachTransformToObject(null);
      } else {
        engine.attachTransformToObject(selId);
      }
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
      engine.syncSketchEntities(sketch, sketchStore.selectedEntityIds, sketchStore.hoveredEntityId, sketchStore.constraintState);
    }
  });

  // ── Sketch mode: sync constraint visuals ──
  $effect(() => {
    if (!engine) return;

    const sketch = sketchStore.activeSketch;
    if (sketch) {
      engine.syncSketchConstraints(sketch, sketchStore.constraintState);
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

  // ── Highlight selected inactive sketch ──
  $effect(() => {
    if (!engine) return;
    engine.highlightInactiveSketch(sketchStore.selectedSketchId);
  });

  // ── Sync display mode ──
  $effect(() => {
    if (engine) engine.setDisplayMode(viewportStore.displayMode);
  });

  // ── Sync section plane ──
  $effect(() => {
    if (engine) engine.setSectionPlane(viewportStore.sectionPlane);
  });

  // ── Sync datum plane/axis visualizations ──
  $effect(() => {
    if (!engine) return;

    // Access reactive properties to trigger re-runs
    const planes = datumStore.datumPlanes;
    const axes = datumStore.datumAxes;
    const suppressedIds = featureTree.suppressedIds;

    engine.removeAllDatums();

    const datumCompMap = componentStore.getFeatureComponentMap();

    for (const dp of planes) {
      if (!dp.visible || suppressedIds.has(dp.id)) continue;
      const dpCompId = datumCompMap.get(dp.id);
      const dpComp = dpCompId ? componentStore.getComponentById(dpCompId) : null;
      if (dpComp && !dpComp.visible) continue;
      const info = computeDatumPlaneInfo(dp);
      engine.addDatumPlane(dp.id, info.origin, info.normal, info.u, info.v, dp.color);
    }

    for (const da of axes) {
      if (!da.visible || suppressedIds.has(da.id)) continue;
      const daCompId = datumCompMap.get(da.id);
      const daComp = daCompId ? componentStore.getComponentById(daCompId) : null;
      if (daComp && !daComp.visible) continue;
      // Convert CQ coords to Three.js: (x, y, z) -> (x, z, -y)
      const origin = new THREE.Vector3(da.origin[0], da.origin[2], -da.origin[1]);
      const direction = new THREE.Vector3(da.direction[0], da.direction[2], -da.direction[1]);
      engine.addDatumAxis(da.id, origin, direction, da.color);
    }
  });

  // ── Sync measurements to engine ──
  $effect(() => {
    if (!engine) return;

    const currentMeasurements = measureStore.measurements;
    const engineIds = engine.getMeasurementIds();
    const storeIds = new Set(currentMeasurements.map((m) => m.id));

    // Remove measurements no longer in store
    for (const id of engineIds) {
      if (!storeIds.has(id)) {
        engine.removeMeasurement(id);
      }
    }

    // Add new measurements
    for (const m of currentMeasurements) {
      if (!engineIds.has(m.id)) {
        switch (m.type) {
          case 'distance':
            engine.addDistanceMeasurement(
              m.id,
              new THREE.Vector3(...m.point1.worldPos),
              new THREE.Vector3(...m.point2.worldPos),
              m.distance,
            );
            break;
          case 'angle':
            engine.addAngleMeasurement(
              m.id,
              new THREE.Vector3(...m.vertex.worldPos),
              new THREE.Vector3(...m.arm1.worldPos),
              new THREE.Vector3(...m.arm2.worldPos),
              m.angleDegrees,
            );
            break;
          case 'radius':
            engine.addRadiusMeasurement(
              m.id,
              new THREE.Vector3(...m.center),
              m.radius,
            );
            break;
          case 'bbox':
            engine.addBBoxMeasurement(m.id, m.objectId);
            break;
        }
      }
    }
  });

  // ── Sync pending marker ──
  $effect(() => {
    if (!engine) return;
    const pending = measureStore.pendingPoints;
    if (pending.length > 0) {
      engine.showPendingMarker(new THREE.Vector3(...pending[pending.length - 1].worldPos));
    } else {
      engine.clearPendingMarker();
    }
  });

  // ── Clear measure tool state when switching away ──
  $effect(() => {
    if (tools.activeTool !== 'measure') {
      measureStore.setMeasureTool(null);
      measureStore.clearPending();
    }
  });

  // ── Exploded view effect ──
  $effect(() => {
    if (!engine) return;

    const factor = viewportStore.explodeFactor;
    const enabled = viewportStore.explodeEnabled;

    if (!enabled || factor === 0) {
      engine.clearExplosion();
      return;
    }

    // Compute assembly centroid from all component objects
    const compStore = componentStore;
    const allComps = compStore.components.filter((c) => c.visible);
    if (allComps.length < 2) {
      engine.clearExplosion();
      return;
    }

    // Compute centroid of all component bounding boxes
    const centroid = new THREE.Vector3();
    let count = 0;
    for (const comp of allComps) {
      for (const fid of comp.featureIds) {
        const bbox = engine.getObjectBBox(fid);
        if (bbox) {
          const center = new THREE.Vector3();
          bbox.getCenter(center);
          centroid.add(center);
          count++;
        }
      }
    }
    if (count === 0) return;
    centroid.divideScalar(count);

    // Compute explosion offsets per component
    const explosionScale = 50;
    const offsets = new Map<string, THREE.Vector3>();
    for (const comp of allComps) {
      const compCenter = new THREE.Vector3();
      let compCount = 0;
      for (const fid of comp.featureIds) {
        const bbox = engine.getObjectBBox(fid);
        if (bbox) {
          const center = new THREE.Vector3();
          bbox.getCenter(center);
          compCenter.add(center);
          compCount++;
        }
      }
      if (compCount === 0) continue;
      compCenter.divideScalar(compCount);

      const dir = compCenter.clone().sub(centroid);
      if (dir.length() < 0.01) dir.set(1, 0, 0); // fallback direction
      dir.normalize();
      const offset = dir.multiplyScalar(factor * explosionScale);

      for (const fid of comp.featureIds) {
        offsets.set(fid, offset);
      }
    }

    engine.applyExplosion(offsets);
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

      // Hover detection for select tool, constraint tools, or operation tools
      const currentTool = sketchStore.activeSketchTool;
      const isConstraintTool = currentTool.startsWith('sketch-constraint-');
      const isOpTool = currentTool.startsWith('sketch-op-');
      if (currentTool === 'sketch-select' || isConstraintTool || isOpTool) {
        const hitId = engine.raycastSketchEntities(e, sketch);
        sketchStore.setHoveredEntity(hitId);
      }

      // Set cursor
      if (containerRef) {
        if (currentTool === 'sketch-select' || isConstraintTool || isOpTool) {
          const hitId = engine.raycastSketchEntities(e, sketch);
          containerRef.style.cursor = hitId ? 'pointer' : '';
        } else {
          containerRef.style.cursor = 'crosshair';
        }
      }
      return;
    }

    // ── Measure mode pointer move ──
    if (tools.activeTool === 'measure' && !sketchStore.isInSketchMode) {
      if (containerRef) containerRef.style.cursor = 'crosshair';
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

  // Mate creation dimension input state
  let mateDimInputVisible = $state(false);
  let mateDimInputPrompt = $state('');
  let mateDimInputDefault = $state(10);
  let mateDimInputPosition = $state({ x: 0, y: 0 });
  let pendingMateRef2 = $state<{ componentId: string; featureId: string; faceSelector: import('$lib/types/cad').FaceSelector } | null>(null);

  function handleMateDimSubmit(value: number) {
    if (!pendingMateRef2) return;
    const mateType = mateStore.mateCreationMode;
    history.pushSnapshot(captureFullSnapshot());
    if (mateType === 'distance') {
      mateStore.completeMateCreation(pendingMateRef2, { distance: value });
    } else if (mateType === 'angle') {
      mateStore.completeMateCreation(pendingMateRef2, { angle: value });
    }
    mateDimInputVisible = false;
    pendingMateRef2 = null;
    triggerPipeline(100);
    runPythonExecution();
  }

  function handleMateDimCancel() {
    mateDimInputVisible = false;
    pendingMateRef2 = null;
    mateStore.cancelMateCreation();
  }

  function handlePointerDown(e: PointerEvent) {
    if (!engine) return;
    // Only handle left-click
    if (e.button !== 0) return;

    // ── Mate creation mode ──
    if (mateStore.mateCreationMode) {
      const hit = engine.raycastSurface(e);
      if (hit?.objectId && hit.normal) {
        const comp = componentStore.getComponentForFeature(hit.objectId);
        if (!comp) return;
        const faceSelector = normalToFaceSelector(hit.normal);
        const ref = { componentId: comp.id, featureId: hit.objectId, faceSelector };

        if (!mateStore.pendingRef1) {
          mateStore.setPendingRef1(ref);
          constraintStatusMessage = `Face on ${comp.name} selected. Now select second face on a different component.`;
        } else {
          if (ref.componentId === mateStore.pendingRef1.componentId) return; // same component
          const mateType = mateStore.mateCreationMode;
          if (mateType === 'distance' || mateType === 'angle') {
            // Show dimension input for distance/angle mates
            pendingMateRef2 = ref;
            mateDimInputPrompt = mateType === 'distance' ? 'Distance:' : 'Angle (degrees):';
            mateDimInputDefault = mateType === 'distance' ? 10 : 90;
            mateDimInputPosition = { x: e.clientX, y: e.clientY };
            mateDimInputVisible = true;
          } else {
            // Coincident / concentric: complete immediately
            history.pushSnapshot(captureFullSnapshot());
            mateStore.completeMateCreation(ref);
            constraintStatusMessage = '';
            triggerPipeline(100);
            runPythonExecution();
          }
        }
      }
      return; // consume click
    }

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

      // Constraint tools: use additive entity selection, then check for constraint creation
      if (tool.startsWith('sketch-constraint-')) {
        const hitId = engine.raycastSketchEntities(e, sketch);
        if (hitId) {
          sketchStore.selectEntity(hitId, true);
        }

        // Check if we have enough selections for this constraint
        const action = handleConstraintSelection(
          tool,
          sketchStore.selectedEntityIds,
          sketch.entities,
          () => sketchStore.newEntityId(),
        );

        constraintStatusMessage = '';
        switch (action.type) {
          case 'create':
            history.pushSnapshot(captureFullSnapshot());
            sketchStore.addConstraint(action.constraint);
            sketchStore.selectEntity(null);
            triggerPipeline(100);
            break;
          case 'need-value':
            pendingConstraint = action.partial;
            dimensionInputPrompt = action.prompt;
            dimensionInputDefaultValue = action.defaultValue;
            dimensionInputPosition = { x: e.clientX, y: e.clientY };
            dimensionInputVisible = true;
            break;
          case 'need-more':
            constraintStatusMessage = action.message;
            break;
          case 'invalid':
            constraintStatusMessage = action.message;
            sketchStore.selectEntity(null);
            break;
        }
        return;
      }

      // Sketch operation tools
      if (tool.startsWith('sketch-op-')) {
        constraintStatusMessage = '';

        // Trim uses click point on entity directly
        if (tool === 'sketch-op-trim') {
          const hitId = engine.raycastSketchEntities(e, sketch);
          if (!hitId) return;
          const rawPt = engine.getSketchPlaneIntersection(e, sketch);
          if (!rawPt) return;
          const pt = sketchStore.sketchSnap ? snapToSketchGrid(rawPt, sketchStore.sketchSnap) : rawPt;
          const opAction = handleTrim(pt, hitId, sketch.entities, () => sketchStore.newEntityId());
          applySketchOpAction(opAction, e);
          return;
        }

        // Other ops: select entities, then dispatch
        const hitId = engine.raycastSketchEntities(e, sketch);
        if (hitId) {
          sketchStore.selectEntity(hitId, true);
        }

        const opAction = handleSketchOp(
          tool,
          sketchStore.selectedEntityIds,
          sketch.entities,
          () => sketchStore.newEntityId(),
        );
        applySketchOpAction(opAction, e);
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

    // ── Measure mode ──
    if (tools.activeTool === 'measure' && !sketchStore.isInSketchMode) {
      const measureTool = measureStore.activeMeasureTool;
      if (!measureTool) return;

      // Try raycast surface first, fall back to grid
      const surfaceHit = engine.raycastSurface(e);
      let worldPos: [number, number, number];
      let objectId: ObjectId | undefined;

      if (surfaceHit) {
        worldPos = [surfaceHit.point.x, surfaceHit.point.y, surfaceHit.point.z];
        objectId = surfaceHit.objectId ?? undefined;
      } else {
        const gridPos = engine.getGridIntersection(e);
        if (!gridPos) return;
        // Grid returns CadQuery coords; convert to Three.js Y-up for worldPos
        worldPos = [gridPos[0], 0, -gridPos[1]];
      }

      const point: MeasurePoint = { worldPos, objectId };

      switch (measureTool) {
        case 'measure-distance':
        case 'measure-angle':
          measureStore.addPendingPoint(point);
          break;

        case 'measure-radius': {
          // Single click on object: read radius from params
          if (!objectId) return;
          const obj = scene.getObjectById(objectId);
          if (!obj) return;
          if (obj.params.type === 'cylinder') {
            measureStore.addMeasurement({
              type: 'radius',
              id: nanoid(10),
              center: worldPos,
              objectId,
              radius: obj.params.radius,
            });
          } else if (obj.params.type === 'sphere') {
            measureStore.addMeasurement({
              type: 'radius',
              id: nanoid(10),
              center: worldPos,
              objectId,
              radius: obj.params.radius,
            });
          }
          break;
        }

        case 'measure-bbox': {
          if (!objectId) return;
          const bbox = engine.getObjectBBox(objectId);
          if (!bbox) return;
          const size = new THREE.Vector3();
          bbox.getSize(size);
          measureStore.addMeasurement({
            type: 'bbox',
            id: nanoid(10),
            objectId,
            min: [bbox.min.x, bbox.min.y, bbox.min.z],
            max: [bbox.max.x, bbox.max.y, bbox.max.z],
            sizeX: size.x,
            sizeY: size.y,
            sizeZ: size.z,
          });
          break;
        }
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
        sketchStore.selectSketch(null); // Clear sketch selection when object selected
      } else {
        // Try to select an inactive sketch
        const hitSketchId = engine.raycastInactiveSketches(e);
        if (hitSketchId) {
          sketchStore.selectSketch(hitSketchId);
          scene.clearSelection();
        } else {
          scene.clearSelection();
          sketchStore.selectSketch(null);
        }
      }
    }
  }

  function handleFinishSketch() {
    sketchStore.exitSketchMode();
    triggerPipeline(100);
  }

  function handleDimensionSubmit(value: number) {
    if (pendingConstraint) {
      const constraint = { ...pendingConstraint, value } as SketchConstraint;
      history.pushSnapshot(captureFullSnapshot());
      sketchStore.addConstraint(constraint);
      sketchStore.selectEntity(null);
      triggerPipeline(100);
    } else if (pendingSketchOp) {
      const sketch = sketchStore.activeSketch;
      if (sketch) {
        const opAction = handleSketchOp(
          pendingSketchOp.opType as SketchToolId,
          pendingSketchOp.entityIds,
          sketch.entities,
          () => sketchStore.newEntityId(),
          value,
          pendingSketchOp.clickPoint,
        );
        if (opAction.type === 'replace') {
          history.pushSnapshot(captureFullSnapshot());
          sketchStore.applySketchOp(opAction.removeIds, opAction.addEntities);
          sketchStore.selectEntity(null);
          triggerPipeline(100);
        } else if (opAction.type === 'invalid') {
          constraintStatusMessage = opAction.message;
        }
      }
    }
    dimensionInputVisible = false;
    pendingConstraint = null;
    pendingSketchOp = null;
  }

  function handleDimensionCancel() {
    dimensionInputVisible = false;
    pendingConstraint = null;
    pendingSketchOp = null;
    sketchStore.selectEntity(null);
  }

  function applySketchOpAction(action: SketchOpAction, e: PointerEvent) {
    switch (action.type) {
      case 'replace':
        history.pushSnapshot(captureFullSnapshot());
        sketchStore.applySketchOp(action.removeIds, action.addEntities);
        sketchStore.selectEntity(null);
        triggerPipeline(100);
        break;
      case 'need-value':
        pendingSketchOp = action.pendingOp;
        dimensionInputPrompt = action.prompt;
        dimensionInputDefaultValue = action.defaultValue;
        dimensionInputPosition = { x: e.clientX, y: e.clientY };
        dimensionInputVisible = true;
        break;
      case 'need-more':
        constraintStatusMessage = action.message;
        break;
      case 'invalid':
        constraintStatusMessage = action.message;
        sketchStore.selectEntity(null);
        break;
    }
  }

  // Sketch tool hint text
  const sketchToolHints: Record<string, string> = {
    'sketch-select': 'Click to select entities, Shift+click for multi-select',
    'sketch-line': 'Click to set start point, click again to draw line. Escape to stop chaining.',
    'sketch-rect': 'Click first corner, then opposite corner',
    'sketch-circle': 'Click center, then drag to set radius',
    'sketch-arc': 'Click start point, end point, then mid point of arc',
    'sketch-constraint-coincident': 'Click 2 entities to make endpoints coincident',
    'sketch-constraint-horizontal': 'Click a line to make it horizontal',
    'sketch-constraint-vertical': 'Click a line to make it vertical',
    'sketch-constraint-parallel': 'Click 2 lines to make them parallel',
    'sketch-constraint-perpendicular': 'Click 2 lines to make them perpendicular',
    'sketch-constraint-equal': 'Click 2 entities to make them equal length/radius',
    'sketch-constraint-distance': 'Click 2 entities to set distance between endpoints',
    'sketch-constraint-radius': 'Click a circle or arc to set its radius',
    'sketch-constraint-angle': 'Click 2 lines to set the angle between them',
    'sketch-op-trim': 'Click on a segment to trim it at intersections',
    'sketch-op-extend': 'Click a line to extend it to the nearest intersection',
    'sketch-op-offset': 'Select entity, then enter offset distance',
    'sketch-op-mirror': 'Select entities to mirror, then click the mirror line',
    'sketch-op-fillet': 'Select 2 connected lines, then enter fillet radius',
    'sketch-op-chamfer': 'Select 2 connected lines, then enter chamfer distance',
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

  {#if tools.activeTool === 'measure' && !sketchStore.isInSketchMode}
    <MeasurePanel />
  {/if}

  <!-- Scene info overlay -->
  {#if scene.codeMode === 'parametric' && scene.objects.length > 0 && !sketchStore.isInSketchMode}
    <div class="scene-info">
      {scene.objects.length} object{scene.objects.length !== 1 ? 's' : ''}
    </div>
  {/if}

  <!-- Tool hint overlay (3D mode) -->
  {#if mateStore.mateCreationMode && !sketchStore.isInSketchMode}
    <div class="tool-hint mate-hint">
      {#if !mateStore.pendingRef1}
        Select first face on a component ({mateStore.mateCreationMode} mate)
      {:else}
        Select second face on a different component
      {/if}
      <button class="cancel-mate-btn" onclick={() => { mateStore.cancelMateCreation(); constraintStatusMessage = ''; }}>Cancel</button>
    </div>
  {:else if tools.isAddTool && !sketchStore.isInSketchMode}
    <div class="tool-hint">
      Click on the grid to place {tools.activeTool.replace('add-', '')}
    </div>
  {/if}

  <!-- Sketch mode overlay -->
  {#if sketchStore.isInSketchMode}
    <div class="sketch-overlay">
      <div class="sketch-hint">
        {constraintStatusMessage || (sketchToolHints[sketchStore.activeSketchTool] ?? '')}
      </div>
      <button class="finish-sketch-btn" onclick={handleFinishSketch}>
        Finish Sketch
      </button>
    </div>
    <div class="scene-info sketch-info">
      Sketch: {sketchStore.activeSketch?.name ?? ''} ({sketchStore.activeSketch?.plane})
      {#if sketchStore.activeSketch}
        &middot; {sketchStore.activeSketch.entities.length} entit{sketchStore.activeSketch.entities.length !== 1 ? 'ies' : 'y'}
        &middot; {(sketchStore.activeSketch.constraints ?? []).length} constraint{(sketchStore.activeSketch.constraints ?? []).length !== 1 ? 's' : ''}
      {/if}
    </div>
    <!-- DOF / constraint state badge -->
    {#if sketchStore.constraintState === 'well-constrained'}
      <div class="constraint-badge well-constrained">FULLY CONSTRAINED</div>
    {:else if sketchStore.constraintState === 'over-constrained'}
      <div class="constraint-badge over-constrained">OVER-CONSTRAINED</div>
    {:else if sketchStore.degreesOfFreedom >= 0}
      <div class="constraint-badge under-constrained">DOF: {sketchStore.degreesOfFreedom}</div>
    {/if}
  {/if}

  <!-- Dimension input overlay -->
  {#if dimensionInputVisible}
    <DimensionInput
      prompt={dimensionInputPrompt}
      defaultValue={dimensionInputDefaultValue}
      position={dimensionInputPosition}
      onSubmit={handleDimensionSubmit}
      onCancel={handleDimensionCancel}
    />
  {/if}

  <!-- Mate dimension input overlay -->
  {#if mateDimInputVisible}
    <DimensionInput
      prompt={mateDimInputPrompt}
      defaultValue={mateDimInputDefault}
      position={mateDimInputPosition}
      onSubmit={handleMateDimSubmit}
      onCancel={handleMateDimCancel}
    />
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

  .constraint-badge {
    position: absolute;
    bottom: 28px;
    left: 8px;
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 2px 8px;
    border-radius: 3px;
    pointer-events: none;
    z-index: 2;
  }

  .constraint-badge.well-constrained {
    color: #a6e3a1;
    background: rgba(166, 227, 161, 0.15);
    border: 1px solid rgba(166, 227, 161, 0.4);
  }

  .constraint-badge.over-constrained {
    color: #f38ba8;
    background: rgba(243, 139, 168, 0.15);
    border: 1px solid rgba(243, 139, 168, 0.4);
  }

  .constraint-badge.under-constrained {
    color: #f9e2af;
    background: rgba(249, 226, 175, 0.1);
    border: 1px solid rgba(249, 226, 175, 0.3);
  }

  .mate-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    border-color: #f2cdcd;
    color: #f2cdcd;
  }

  .cancel-mate-btn {
    background: rgba(243, 139, 168, 0.15);
    border: 1px solid #f38ba8;
    color: #f38ba8;
    padding: 2px 8px;
    border-radius: 3px;
    font-size: 10px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .cancel-mate-btn:hover {
    background: rgba(243, 139, 168, 0.25);
  }
</style>
