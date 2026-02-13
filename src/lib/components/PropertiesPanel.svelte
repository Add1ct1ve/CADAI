<script lang="ts">
  import { getSceneStore } from '$lib/stores/scene.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { getDatumStore } from '$lib/stores/datum.svelte';
  import { getComponentStore } from '$lib/stores/component.svelte';
  import { getMateStore } from '$lib/stores/mate.svelte';
  import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
  import { triggerPipeline, runPythonExecution } from '$lib/services/execution-pipeline';
  import { getHistoryStore } from '$lib/stores/history.svelte';
  import type { PrimitiveParams, EdgeSelector, FaceSelector, FilletParams, ChamferParams, SketchConstraint, SketchOperation, ShellParams, HoleParams, HoleType, BooleanOpType, SplitPlane, PatternOp, PatternType, DatumPlaneDefinition } from '$lib/types/cad';
  import { isDatumPlane, isDatumAxis } from '$lib/types/cad';
  import { MATERIALS, getMaterial, DEFAULT_METALNESS, DEFAULT_ROUGHNESS, DEFAULT_OPACITY } from '$lib/data/materials';

  const scene = getSceneStore();
  const sketchStore = getSketchStore();
  const datumStore = getDatumStore();
  const componentStore = getComponentStore();
  const mateStore = getMateStore();
  const featureTree = getFeatureTreeStore();
  const history = getHistoryStore();

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Move-by-delta state
  let deltaX = $state(0);
  let deltaY = $state(0);
  let deltaZ = $state(0);

  // Scale factor state
  let scaleFactor = $state(1.0);

  const edgeSelectorOptions: { value: EdgeSelector; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'top', label: 'Top' },
    { value: 'bottom', label: 'Bottom' },
    { value: 'vertical', label: 'Vertical' },
  ];

  const faceSelectorOptions: { value: FaceSelector; label: string }[] = [
    { value: '>Z', label: 'Top (+Z)' },
    { value: '<Z', label: 'Bottom (-Z)' },
    { value: '>X', label: 'Right (+X)' },
    { value: '<X', label: 'Left (-X)' },
    { value: '>Y', label: 'Front (+Y)' },
    { value: '<Y', label: 'Back (-Y)' },
  ];

  const holeTypeOptions: { value: HoleType; label: string }[] = [
    { value: 'through', label: 'Through' },
    { value: 'blind', label: 'Blind' },
    { value: 'counterbore', label: 'Counterbore' },
    { value: 'countersink', label: 'Countersink' },
  ];

  function debounced(fn: () => void, ms = 300) {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(fn, ms);
  }

  // Full snapshot helper for undo
  function captureSnapshot() {
    const sceneSnap = scene.snapshot();
    const sketchSnap = sketchStore.snapshot();
    const datumSnap = datumStore.snapshot();
    const compSnap = componentStore.snapshot();
    const mateSnap = mateStore.snapshot();
    const ftSnap = featureTree.snapshot();
    return {
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
    };
  }

  function normalizeDisplayName(value: string, fallback: string): string {
    const normalized = value.replace(/\s+/g, ' ').trim();
    return normalized.length > 0 ? normalized : fallback;
  }

  function updateName(e: Event) {
    const obj = scene.firstSelected;
    if (!obj) return;
    const value = normalizeDisplayName((e.target as HTMLInputElement).value, obj.name);
    scene.updateObject(obj.id, { name: value });
  }

  // Capture snapshot before property panel mutations (debounced to avoid excessive snapshots)
  let snapshotCaptured = false;
  function captureOnce() {
    if (!snapshotCaptured) {
      history.pushSnapshot(captureSnapshot());
      snapshotCaptured = true;
      setTimeout(() => { snapshotCaptured = false; }, 500);
    }
  }

  function triggerAndRun() {
    triggerPipeline(100);
    debounced(() => runPythonExecution());
  }

  function updateParam(key: string, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const newParams = { ...obj.params, [key]: value } as PrimitiveParams;
    scene.updateParams(obj.id, newParams);
    debounced(() => triggerPipeline());
  }

  function updatePosition(axis: 0 | 1 | 2, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const pos = [...obj.transform.position] as [number, number, number];
    pos[axis] = value;
    scene.updateTransform(obj.id, { ...obj.transform, position: pos });
    debounced(() => triggerPipeline());
  }

  function updateRotation(axis: 0 | 1 | 2, value: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    const rot = [...obj.transform.rotation] as [number, number, number];
    rot[axis] = value;
    scene.updateTransform(obj.id, { ...obj.transform, rotation: rot });
    debounced(() => triggerPipeline());
  }

  function updateColor(e: Event) {
    const obj = scene.firstSelected;
    if (!obj) return;
    const value = (e.target as HTMLInputElement).value;
    scene.updateObject(obj.id, { color: value });
  }

  function toggleVisible() {
    const obj = scene.firstSelected;
    if (!obj) return;
    scene.updateObject(obj.id, { visible: !obj.visible });
    debounced(() => triggerPipeline());
  }

  function deleteObject() {
    history.pushSnapshot(captureSnapshot());
    scene.deleteSelected();
    triggerPipeline(100);
  }

  function applyMoveDelta() {
    const obj = scene.firstSelected;
    if (!obj) return;
    history.pushSnapshot(captureSnapshot());
    const pos = obj.transform.position;
    const newPos: [number, number, number] = [pos[0] + deltaX, pos[1] + deltaY, pos[2] + deltaZ];
    scene.updateTransform(obj.id, { ...obj.transform, position: newPos });
    deltaX = 0;
    deltaY = 0;
    deltaZ = 0;
    triggerPipeline(100);
  }

  function applyScaleFactor() {
    const obj = scene.firstSelected;
    if (!obj || scaleFactor <= 0) return;
    history.pushSnapshot(captureSnapshot());
    const p = obj.params;
    let newParams: PrimitiveParams;
    switch (p.type) {
      case 'box':
        newParams = { ...p, width: p.width * scaleFactor, depth: p.depth * scaleFactor, height: p.height * scaleFactor };
        break;
      case 'cylinder':
        newParams = { ...p, radius: p.radius * scaleFactor, height: p.height * scaleFactor };
        break;
      case 'sphere':
        newParams = { ...p, radius: p.radius * scaleFactor };
        break;
      case 'cone':
        newParams = { ...p, bottomRadius: p.bottomRadius * scaleFactor, topRadius: p.topRadius * scaleFactor, height: p.height * scaleFactor };
        break;
    }
    scene.updateParams(obj.id, newParams);
    scaleFactor = 1.0;
    triggerPipeline(100);
  }

  function numInput(e: Event, callback: (v: number) => void) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(value)) callback(value);
  }

  // ── Object Fillet/Chamfer ──

  function addObjectFillet() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setFillet(obj.id, { radius: 1, edges: 'all' });
    triggerAndRun();
  }

  function updateObjectFillet(key: 'radius' | 'edges', value: number | EdgeSelector) {
    const obj = scene.firstSelected;
    if (!obj || !obj.fillet) return;
    captureOnce();
    scene.setFillet(obj.id, { ...obj.fillet, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectFillet() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setFillet(obj.id, undefined);
    triggerAndRun();
  }

  function addObjectChamfer() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setChamfer(obj.id, { distance: 0.5, edges: 'all' });
    triggerAndRun();
  }

  function updateObjectChamfer(key: 'distance' | 'edges', value: number | EdgeSelector) {
    const obj = scene.firstSelected;
    if (!obj || !obj.chamfer) return;
    captureOnce();
    scene.setChamfer(obj.id, { ...obj.chamfer, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectChamfer() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setChamfer(obj.id, undefined);
    triggerAndRun();
  }

  // ── Sketch 3D Operations (Extrude/Revolve/Sweep) ──

  function addSketchExtrude() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setOperation(sketch.id, { type: 'extrude', distance: 10, mode: 'add' });
    triggerAndRun();
  }

  function addSketchRevolve() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setOperation(sketch.id, { type: 'revolve', angle: 360, mode: 'add', axisDirection: 'Y', axisOffset: 0 });
    triggerAndRun();
  }

  function addSketchSweep() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    // Find first other sketch as default path
    const otherSketch = sketchStore.sketches.find(s => s.id !== sketch.id && s.entities.length > 0);
    sketchStore.setOperation(sketch.id, { type: 'sweep', mode: 'add', pathSketchId: otherSketch?.id ?? '' });
    triggerAndRun();
  }

  function updateSketchOperation(partial: Partial<SketchOperation>) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.operation) return;
    captureOnce();
    sketchStore.setOperation(sketch.id, { ...sketch.operation, ...partial } as SketchOperation);
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function updateSketchOperationImmediate(partial: Partial<SketchOperation>) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.operation) return;
    captureOnce();
    sketchStore.setOperation(sketch.id, { ...sketch.operation, ...partial } as SketchOperation);
    triggerAndRun();
  }

  function removeSketchOperation() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setOperation(sketch.id, undefined);
    sketchStore.setSketchFillet(sketch.id, undefined);
    sketchStore.setSketchChamfer(sketch.id, undefined);
    sketchStore.setSketchShell(sketch.id, undefined);
    sketchStore.setSketchHoles(sketch.id, undefined);
    triggerAndRun();
  }

  // ── Sketch Shell ──

  function addSketchShell() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchShell(sketch.id, { thickness: -2, face: '>Z' });
    triggerAndRun();
  }

  function updateSketchShell(key: string, value: number | FaceSelector) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.shell) return;
    captureOnce();
    sketchStore.setSketchShell(sketch.id, { ...sketch.shell, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchShell() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchShell(sketch.id, undefined);
    triggerAndRun();
  }

  // ── Sketch Holes ──

  function addSketchHole() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.addSketchHole(sketch.id, { holeType: 'through', diameter: 5, position: [0, 0], face: '>Z' });
    triggerAndRun();
  }

  function updateSketchHole(index: number, key: string, value: any) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.holes?.[index]) return;
    captureOnce();
    let updated = { ...sketch.holes[index], [key]: value };
    // Set sensible defaults when hole type changes
    if (key === 'holeType') {
      if (value === 'blind') updated = { ...updated, depth: updated.depth ?? 5 };
      if (value === 'counterbore') updated = { ...updated, cboreDiameter: updated.cboreDiameter ?? updated.diameter * 1.6, cboreDepth: updated.cboreDepth ?? 3 };
      if (value === 'countersink') updated = { ...updated, cskDiameter: updated.cskDiameter ?? updated.diameter * 2, cskAngle: updated.cskAngle ?? 82 };
    }
    sketchStore.updateSketchHole(sketch.id, index, updated);
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchHole(index: number) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.removeSketchHole(sketch.id, index);
    triggerAndRun();
  }

  // ── Object Shell ──

  function addObjectShell() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setShell(obj.id, { thickness: -2, face: '>Z' });
    triggerAndRun();
  }

  function updateObjectShell(key: string, value: number | FaceSelector) {
    const obj = scene.firstSelected;
    if (!obj || !obj.shell) return;
    captureOnce();
    scene.setShell(obj.id, { ...obj.shell, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectShell() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setShell(obj.id, undefined);
    triggerAndRun();
  }

  // ── Object Holes ──

  function addObjectHole() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.addHole(obj.id, { holeType: 'through', diameter: 5, position: [0, 0], face: '>Z' });
    triggerAndRun();
  }

  function updateObjectHole(index: number, key: string, value: any) {
    const obj = scene.firstSelected;
    if (!obj || !obj.holes?.[index]) return;
    captureOnce();
    let updated = { ...obj.holes[index], [key]: value };
    // Set sensible defaults when hole type changes
    if (key === 'holeType') {
      if (value === 'blind') updated = { ...updated, depth: updated.depth ?? 5 };
      if (value === 'counterbore') updated = { ...updated, cboreDiameter: updated.cboreDiameter ?? updated.diameter * 1.6, cboreDepth: updated.cboreDepth ?? 3 };
      if (value === 'countersink') updated = { ...updated, cskDiameter: updated.cskDiameter ?? updated.diameter * 2, cskAngle: updated.cskAngle ?? 82 };
    }
    scene.updateHole(obj.id, index, updated);
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeObjectHole(index: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.removeHole(obj.id, index);
    triggerAndRun();
  }

  // ── Object Boolean ──

  const booleanTypeOptions: { value: BooleanOpType; label: string }[] = [
    { value: 'union', label: 'Union' },
    { value: 'subtract', label: 'Subtract' },
    { value: 'intersect', label: 'Intersect' },
  ];

  const splitPlaneOptions: { value: SplitPlane; label: string }[] = [
    { value: 'XY', label: 'XY' },
    { value: 'XZ', label: 'XZ' },
    { value: 'YZ', label: 'YZ' },
  ];

  const keepSideOptions: { value: string; label: string }[] = [
    { value: 'both', label: 'Both' },
    { value: 'positive', label: 'Positive' },
    { value: 'negative', label: 'Negative' },
  ];

  function getBooleanTargets() {
    const obj = scene.firstSelected;
    const targets: { id: string; name: string }[] = [];
    for (const o of scene.objects) {
      if (o.id === obj?.id) continue;
      if (o.visible && !o.booleanOp) {
        targets.push({ id: o.id, name: o.name });
      }
    }
    // Also include operated add-mode sketches
    const sketchStore_ = sketchStore;
    for (const s of sketchStore_.sketches) {
      if (s.operation && s.operation.mode === 'add' && s.entities.length > 0) {
        targets.push({ id: s.id, name: s.name });
      }
    }
    return targets;
  }

  function addBooleanOp(type: BooleanOpType) {
    const obj = scene.firstSelected;
    if (!obj) return;
    const targets = getBooleanTargets();
    if (targets.length === 0) return;
    captureOnce();
    scene.setBooleanOp(obj.id, { type, targetId: targets[0].id });
    triggerAndRun();
  }

  function updateBooleanOpType(type: BooleanOpType) {
    const obj = scene.firstSelected;
    if (!obj || !obj.booleanOp) return;
    captureOnce();
    scene.setBooleanOp(obj.id, { ...obj.booleanOp, type });
    triggerAndRun();
  }

  function updateBooleanTarget(targetId: string) {
    const obj = scene.firstSelected;
    if (!obj || !obj.booleanOp) return;
    captureOnce();
    scene.setBooleanOp(obj.id, { ...obj.booleanOp, targetId });
    triggerAndRun();
  }

  function removeBooleanOp() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setBooleanOp(obj.id, undefined);
    triggerAndRun();
  }

  // ── Object Split ──

  function addSplitOp() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setSplitOp(obj.id, { plane: 'XY', offset: 0, keepSide: 'both' });
    triggerAndRun();
  }

  function updateSplitOp(key: string, value: any) {
    const obj = scene.firstSelected;
    if (!obj || !obj.splitOp) return;
    captureOnce();
    scene.setSplitOp(obj.id, { ...obj.splitOp, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function updateSplitOpImmediate(key: string, value: any) {
    const obj = scene.firstSelected;
    if (!obj || !obj.splitOp) return;
    captureOnce();
    scene.setSplitOp(obj.id, { ...obj.splitOp, [key]: value });
    triggerAndRun();
  }

  function removeSplitOp() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setSplitOp(obj.id, undefined);
    triggerAndRun();
  }

  // ── Object Pattern ──

  const patternTypeOptions: { value: PatternType; label: string }[] = [
    { value: 'mirror', label: 'Mirror' },
    { value: 'linear', label: 'Linear' },
    { value: 'circular', label: 'Circular' },
  ];

  const patternPlaneOptions: { value: string; label: string }[] = [
    { value: 'XY', label: 'XY' }, { value: 'XZ', label: 'XZ' }, { value: 'YZ', label: 'YZ' },
  ];

  const patternAxisOptions: { value: string; label: string }[] = [
    { value: 'X', label: 'X' }, { value: 'Y', label: 'Y' }, { value: 'Z', label: 'Z' },
  ];

  const patternDirOptions: { value: string; label: string }[] = [
    { value: 'X', label: 'X' }, { value: 'Y', label: 'Y' }, { value: 'Z', label: 'Z' },
  ];

  function addPatternOp(type: PatternType) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    let op: PatternOp;
    switch (type) {
      case 'mirror': op = { type: 'mirror', plane: 'XY', offset: 0, keepOriginal: true }; break;
      case 'linear': op = { type: 'linear', direction: 'X', spacing: 20, count: 3 }; break;
      case 'circular': op = { type: 'circular', axis: 'Z', count: 6, fullAngle: 360 }; break;
    }
    scene.setPatternOp(obj.id, op);
    triggerAndRun();
  }

  function switchPatternType(newType: PatternType) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    let op: PatternOp;
    switch (newType) {
      case 'mirror': op = { type: 'mirror', plane: 'XY', offset: 0, keepOriginal: true }; break;
      case 'linear': op = { type: 'linear', direction: 'X', spacing: 20, count: 3 }; break;
      case 'circular': op = { type: 'circular', axis: 'Z', count: 6, fullAngle: 360 }; break;
    }
    scene.setPatternOp(obj.id, op);
    triggerAndRun();
  }

  function updatePatternOp(key: string, value: any) {
    const obj = scene.firstSelected;
    if (!obj || !obj.patternOp) return;
    captureOnce();
    scene.setPatternOp(obj.id, { ...obj.patternOp, [key]: value } as PatternOp);
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function updatePatternOpImmediate(key: string, value: any) {
    const obj = scene.firstSelected;
    if (!obj || !obj.patternOp) return;
    captureOnce();
    scene.setPatternOp(obj.id, { ...obj.patternOp, [key]: value } as PatternOp);
    triggerAndRun();
  }

  function removePatternOp() {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.setPatternOp(obj.id, undefined);
    triggerAndRun();
  }

  function addSketchFillet() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, { radius: 1, edges: 'all' });
    triggerAndRun();
  }

  function updateSketchFillet(key: 'radius' | 'edges', value: number | EdgeSelector) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.fillet) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, { ...sketch.fillet, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchFillet() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchFillet(sketch.id, undefined);
    triggerAndRun();
  }

  function addSketchChamfer() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, { distance: 0.5, edges: 'all' });
    triggerAndRun();
  }

  function updateSketchChamfer(key: 'distance' | 'edges', value: number | EdgeSelector) {
    const sketch = sketchStore.selectedSketch;
    if (!sketch || !sketch.chamfer) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, { ...sketch.chamfer, [key]: value });
    debounced(() => { triggerPipeline(100); runPythonExecution(); });
  }

  function removeSketchChamfer() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    captureOnce();
    sketchStore.setSketchChamfer(sketch.id, undefined);
    triggerAndRun();
  }

  function removeConstraint(constraintId: string) {
    captureOnce();
    sketchStore.removeConstraint(constraintId);
    triggerPipeline(100);
  }

  function constraintLabel(c: SketchConstraint): string {
    switch (c.type) {
      case 'coincident': return 'Coincident';
      case 'horizontal': return 'Horizontal';
      case 'vertical': return 'Vertical';
      case 'parallel': return 'Parallel';
      case 'perpendicular': return 'Perpendicular';
      case 'equal': return 'Equal';
      case 'distance': return `Distance: ${c.value}`;
      case 'radius': return `Radius: ${c.value}`;
      case 'angle': return `Angle: ${c.value}\u00B0`;
    }
  }

  function editSketch() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    sketchStore.editSketch(sketch.id);
  }

  function deleteSketch() {
    const sketch = sketchStore.selectedSketch;
    if (!sketch) return;
    history.pushSnapshot(captureSnapshot());
    sketchStore.removeSketch(sketch.id);
    triggerAndRun();
  }

  // ── Datum properties ──

  function updateDatumPlaneDefinition(partial: Partial<DatumPlaneDefinition>) {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumPlane(datum)) return;
    captureOnce();
    datumStore.updateDatumPlane(datum.id, {
      definition: { ...datum.definition, ...partial } as DatumPlaneDefinition,
    });
    debounced(() => triggerPipeline(100));
  }

  function updateDatumPlaneColor(e: Event) {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumPlane(datum)) return;
    datumStore.updateDatumPlane(datum.id, { color: (e.target as HTMLInputElement).value });
  }

  function toggleDatumPlaneVisible() {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumPlane(datum)) return;
    datumStore.updateDatumPlane(datum.id, { visible: !datum.visible });
  }

  function deleteDatumPlane() {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumPlane(datum)) return;
    history.pushSnapshot(captureSnapshot());
    datumStore.removeDatumPlane(datum.id);
    triggerPipeline(100);
  }

  function updateDatumAxis(key: string, value: any) {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumAxis(datum)) return;
    captureOnce();
    datumStore.updateDatumAxis(datum.id, { [key]: value });
    debounced(() => triggerPipeline(100));
  }

  function updateDatumAxisColor(e: Event) {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumAxis(datum)) return;
    datumStore.updateDatumAxis(datum.id, { color: (e.target as HTMLInputElement).value });
  }

  function toggleDatumAxisVisible() {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumAxis(datum)) return;
    datumStore.updateDatumAxis(datum.id, { visible: !datum.visible });
  }

  function deleteDatumAxis() {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumAxis(datum)) return;
    history.pushSnapshot(captureSnapshot());
    datumStore.removeDatumAxis(datum.id);
    triggerPipeline(100);
  }

  // ── Material helpers ──

  function applyMaterial(materialId: string) {
    const obj = scene.firstSelected;
    if (!obj) return;
    if (!materialId) {
      // "Custom" selected — clear materialId but keep current values
      captureOnce();
      scene.updateObject(obj.id, { materialId: undefined });
      return;
    }
    const preset = getMaterial(materialId);
    if (!preset) return;
    captureOnce();
    scene.updateObject(obj.id, {
      materialId: preset.id,
      color: preset.color,
      metalness: preset.metalness,
      roughness: preset.roughness,
    });
  }

  function updateMetalness(val: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.updateObject(obj.id, { metalness: val, materialId: undefined });
  }

  function updateRoughness(val: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.updateObject(obj.id, { roughness: val, materialId: undefined });
  }

  function updateOpacity(val: number) {
    const obj = scene.firstSelected;
    if (!obj) return;
    captureOnce();
    scene.updateObject(obj.id, { opacity: val });
  }

  function sketchOnDatumPlane() {
    const datum = datumStore.selectedDatum;
    if (!datum || !isDatumPlane(datum)) return;
    sketchStore.enterSketchMode(datum.id);
    datumStore.selectDatum(null);
  }

  // ── Component properties ──

  function updateComponentName(e: Event) {
    const comp = componentStore.selectedComponent;
    if (!comp) return;
    const value = normalizeDisplayName((e.target as HTMLInputElement).value, comp.name);
    componentStore.updateComponent(comp.id, { name: value });
  }

  function updateComponentPosition(axis: 0 | 1 | 2, value: number) {
    const comp = componentStore.selectedComponent;
    if (!comp || comp.grounded) return;
    captureOnce();
    const pos = [...comp.transform.position] as [number, number, number];
    pos[axis] = value;
    componentStore.updateComponent(comp.id, { transform: { ...comp.transform, position: pos } });
    debounced(() => triggerPipeline());
  }

  function updateComponentRotation(axis: 0 | 1 | 2, value: number) {
    const comp = componentStore.selectedComponent;
    if (!comp || comp.grounded) return;
    captureOnce();
    const rot = [...comp.transform.rotation] as [number, number, number];
    rot[axis] = value;
    componentStore.updateComponent(comp.id, { transform: { ...comp.transform, rotation: rot } });
    debounced(() => triggerPipeline());
  }

  function toggleComponentGrounded() {
    const comp = componentStore.selectedComponent;
    if (!comp) return;
    captureOnce();
    componentStore.setGrounded(comp.id, !comp.grounded);
  }

  function toggleComponentVisible() {
    const comp = componentStore.selectedComponent;
    if (!comp) return;
    captureOnce();
    componentStore.setVisible(comp.id, !comp.visible);
    debounced(() => triggerPipeline());
  }

  function dissolveComponent() {
    const comp = componentStore.selectedComponent;
    if (!comp) return;
    history.pushSnapshot(captureSnapshot());
    componentStore.removeComponent(comp.id);
  }

  function updateComponentColor(e: Event) {
    const comp = componentStore.selectedComponent;
    if (!comp) return;
    componentStore.updateComponent(comp.id, { color: (e.target as HTMLInputElement).value });
  }

  // ── Mate properties ──

  function updateMateName(e: Event) {
    const mate = mateStore.selectedMate;
    if (!mate) return;
    const value = normalizeDisplayName((e.target as HTMLInputElement).value, mate.name);
    mateStore.updateMate(mate.id, { name: value });
  }

  function updateMateDistance(value: number) {
    const mate = mateStore.selectedMate;
    if (!mate || mate.type !== 'distance') return;
    captureOnce();
    mateStore.updateMate(mate.id, { distance: value } as any);
    triggerAndRun();
  }

  function updateMateAngle(value: number) {
    const mate = mateStore.selectedMate;
    if (!mate || mate.type !== 'angle') return;
    captureOnce();
    mateStore.updateMate(mate.id, { angle: value } as any);
    triggerAndRun();
  }

  function toggleMateFlipped() {
    const mate = mateStore.selectedMate;
    if (!mate || mate.type !== 'coincident') return;
    captureOnce();
    mateStore.updateMate(mate.id, { flipped: !mate.flipped } as any);
    triggerAndRun();
  }

  function deleteMate() {
    const mate = mateStore.selectedMate;
    if (!mate) return;
    history.pushSnapshot(captureSnapshot());
    mateStore.removeMate(mate.id);
    triggerPipeline(100);
    runPythonExecution();
  }

  // Build list of possible cut targets (other operated add-mode sketches + visible primitives)
  function getCutTargets() {
    const sketch = sketchStore.selectedSketch;
    const targets: { id: string; name: string }[] = [];

    // Add-mode sketches with 3D operations (excluding current)
    for (const s of sketchStore.sketches) {
      if (s.id === sketch?.id) continue;
      if (s.operation && s.operation.mode === 'add' && s.entities.length > 0) {
        targets.push({ id: s.id, name: s.name });
      }
    }

    // Visible scene objects
    for (const obj of scene.objects) {
      if (obj.visible) {
        targets.push({ id: obj.id, name: obj.name });
      }
    }

    return targets;
  }

  // Build list of possible path sketches (for sweep)
  function getPathSketches() {
    const sketch = sketchStore.selectedSketch;
    const paths: { id: string; name: string }[] = [];
    for (const s of sketchStore.sketches) {
      if (s.id === sketch?.id) continue;
      if (s.entities.length > 0 && !s.operation) {
        paths.push({ id: s.id, name: s.name });
      }
    }
    return paths;
  }
</script>

{#if scene.firstSelected}
  {@const obj = scene.firstSelected}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge">{obj.params.type}</span>
      <input
        class="prop-name-input"
        type="text"
        value={obj.name}
        oninput={updateName}
      />
    </div>

    <!-- Dimensions -->
    <div class="prop-section">
      <div class="prop-section-title">Dimensions</div>

      {#if obj.params.type === 'box'}
        <div class="prop-row">
          <span class="prop-label">Width</span>
          <input type="number" value={obj.params.width} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('width', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Depth</span>
          <input type="number" value={obj.params.depth} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('depth', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Height</span>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {:else if obj.params.type === 'cylinder'}
        <div class="prop-row">
          <span class="prop-label">Radius</span>
          <input type="number" value={obj.params.radius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('radius', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Height</span>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {:else if obj.params.type === 'sphere'}
        <div class="prop-row">
          <span class="prop-label">Radius</span>
          <input type="number" value={obj.params.radius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('radius', v))} />
        </div>
      {:else if obj.params.type === 'cone'}
        <div class="prop-row">
          <span class="prop-label">Bottom R</span>
          <input type="number" value={obj.params.bottomRadius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('bottomRadius', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Top R</span>
          <input type="number" value={obj.params.topRadius} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('topRadius', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Height</span>
          <input type="number" value={obj.params.height} step="0.5"
            oninput={(e) => numInput(e, (v) => updateParam('height', v))} />
        </div>
      {/if}
    </div>

    <!-- Scale Factor -->
    <div class="prop-section">
      <div class="prop-section-title">Scale</div>
      <div class="prop-row">
        <span class="prop-label">Factor</span>
        <input type="number" bind:value={scaleFactor} step="0.1" min="0.01" />
        <button class="apply-btn" onclick={applyScaleFactor}>Apply</button>
      </div>
    </div>

    <!-- Position -->
    <div class="prop-section">
      <div class="prop-section-title">Position</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={obj.transform.position[0]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(0, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={obj.transform.position[1]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(1, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={obj.transform.position[2]} step="1"
          oninput={(e) => numInput(e, (v) => updatePosition(2, v))} />
      </div>
    </div>

    <!-- Move By -->
    <div class="prop-section">
      <div class="prop-section-title">Move By</div>
      <div class="prop-row">
        <span class="prop-label">dX</span>
        <input type="number" bind:value={deltaX} step="1" />
      </div>
      <div class="prop-row">
        <span class="prop-label">dY</span>
        <input type="number" bind:value={deltaY} step="1" />
      </div>
      <div class="prop-row">
        <span class="prop-label">dZ</span>
        <input type="number" bind:value={deltaZ} step="1" />
      </div>
      <button class="apply-btn full-width" onclick={applyMoveDelta}>Apply Move</button>
    </div>

    <!-- Rotation -->
    <div class="prop-section">
      <div class="prop-section-title">Rotation</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={obj.transform.rotation[0]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(0, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={obj.transform.rotation[1]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(1, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={obj.transform.rotation[2]} step="5"
          oninput={(e) => numInput(e, (v) => updateRotation(2, v))} />
      </div>
    </div>

    <!-- Fillet (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Fillet</div>
      {#if obj.fillet}
        <div class="prop-row">
          <span class="prop-label">Radius</span>
          <input type="number" value={obj.fillet.radius} step="0.1" min="0.01"
            oninput={(e) => numInput(e, (v) => updateObjectFillet('radius', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Edges</span>
          <select class="prop-select" value={obj.fillet.edges}
            onchange={(e) => updateObjectFillet('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
            {#each edgeSelectorOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeObjectFillet}>Remove Fillet</button>
      {:else}
        <button class="apply-btn full-width" onclick={addObjectFillet}>Add Fillet</button>
      {/if}
    </div>

    <!-- Chamfer (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Chamfer</div>
      {#if obj.chamfer}
        <div class="prop-row">
          <span class="prop-label">Distance</span>
          <input type="number" value={obj.chamfer.distance} step="0.1" min="0.01"
            oninput={(e) => numInput(e, (v) => updateObjectChamfer('distance', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Edges</span>
          <select class="prop-select" value={obj.chamfer.edges}
            onchange={(e) => updateObjectChamfer('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
            {#each edgeSelectorOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeObjectChamfer}>Remove Chamfer</button>
      {:else}
        <button class="apply-btn full-width" onclick={addObjectChamfer}>Add Chamfer</button>
      {/if}
    </div>

    <!-- Shell (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Shell</div>
      {#if obj.shell}
        <div class="prop-row">
          <span class="prop-label">Thickness</span>
          <input type="number" value={obj.shell.thickness} step="0.5"
            oninput={(e) => numInput(e, (v) => updateObjectShell('thickness', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Face</span>
          <select class="prop-select" value={obj.shell.face}
            onchange={(e) => updateObjectShell('face', (e.target as HTMLSelectElement).value as FaceSelector)}>
            {#each faceSelectorOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeObjectShell}>Remove Shell</button>
      {:else}
        <button class="apply-btn full-width" onclick={addObjectShell}>Add Shell</button>
      {/if}
    </div>

    <!-- Holes (Object) -->
    <div class="prop-section">
      <div class="prop-section-title">Holes ({obj.holes?.length ?? 0})</div>
      {#each (obj.holes ?? []) as hole, index}
        <div class="hole-item">
          <div class="prop-row">
            <span class="prop-label">Type</span>
            <select class="prop-select" value={hole.holeType}
              onchange={(e) => updateObjectHole(index, 'holeType', (e.target as HTMLSelectElement).value)}>
              {#each holeTypeOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <span class="prop-label">Dia</span>
            <input type="number" value={hole.diameter} step="0.5" min="0.1"
              oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'diameter', v))} />
          </div>
          {#if hole.holeType === 'blind'}
            <div class="prop-row">
              <span class="prop-label">Depth</span>
              <input type="number" value={hole.depth ?? 5} step="0.5" min="0.1"
                oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'depth', v))} />
            </div>
          {/if}
          {#if hole.holeType === 'counterbore'}
            <div class="prop-row">
              <span class="prop-label">CB Dia</span>
              <input type="number" value={hole.cboreDiameter ?? 8} step="0.5" min="0.1"
                oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'cboreDiameter', v))} />
            </div>
            <div class="prop-row">
              <span class="prop-label">CB Dep</span>
              <input type="number" value={hole.cboreDepth ?? 3} step="0.5" min="0.1"
                oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'cboreDepth', v))} />
            </div>
          {/if}
          {#if hole.holeType === 'countersink'}
            <div class="prop-row">
              <span class="prop-label">CS Dia</span>
              <input type="number" value={hole.cskDiameter ?? 10} step="0.5" min="0.1"
                oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'cskDiameter', v))} />
            </div>
            <div class="prop-row">
              <span class="prop-label">CS Angle</span>
              <input type="number" value={hole.cskAngle ?? 82} step="1" min="1" max="180"
                oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'cskAngle', v))} />
            </div>
          {/if}
          <div class="prop-row">
            <span class="prop-label">Face</span>
            <select class="prop-select" value={hole.face}
              onchange={(e) => updateObjectHole(index, 'face', (e.target as HTMLSelectElement).value)}>
              {#each faceSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <span class="prop-label">Pos X</span>
            <input type="number" value={hole.position[0]} step="1"
              oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'position', [v, hole.position[1]]))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Pos Y</span>
            <input type="number" value={hole.position[1]} step="1"
              oninput={(e) => numInput(e, (v) => updateObjectHole(index, 'position', [hole.position[0], v]))} />
          </div>
          <button class="remove-btn" onclick={() => removeObjectHole(index)}>Remove Hole</button>
        </div>
      {/each}
      <button class="apply-btn full-width" onclick={addObjectHole}>Add Hole</button>
    </div>

    <!-- Boolean Operation -->
    <div class="prop-section">
      <div class="prop-section-title">Boolean</div>
      {#if obj.booleanOp}
        <div class="prop-row">
          <span class="prop-label">Type</span>
          <select class="prop-select" value={obj.booleanOp.type}
            onchange={(e) => updateBooleanOpType((e.target as HTMLSelectElement).value as BooleanOpType)}>
            {#each booleanTypeOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <div class="prop-row">
          <span class="prop-label">Target</span>
          <select class="prop-select" value={obj.booleanOp.targetId}
            onchange={(e) => updateBooleanTarget((e.target as HTMLSelectElement).value)}>
            {#each getBooleanTargets() as target}
              <option value={target.id}>{target.name}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeBooleanOp}>Remove Boolean</button>
      {:else if !obj.splitOp && !obj.patternOp}
        <span class="prop-hint">Select 2 objects for boolean, or:</span>
        <div class="op-buttons">
          <button class="apply-btn full-width" onclick={() => addBooleanOp('union')}>Set as Union Tool</button>
          <button class="apply-btn full-width" onclick={() => addBooleanOp('subtract')}>Set as Subtract Tool</button>
          <button class="apply-btn full-width" onclick={() => addBooleanOp('intersect')}>Set as Intersect Tool</button>
        </div>
      {/if}
    </div>

    <!-- Split -->
    <div class="prop-section">
      <div class="prop-section-title">Split</div>
      {#if obj.splitOp}
        <div class="prop-row">
          <span class="prop-label">Plane</span>
          <select class="prop-select" value={obj.splitOp.plane}
            onchange={(e) => updateSplitOpImmediate('plane', (e.target as HTMLSelectElement).value)}>
            {#each splitPlaneOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <div class="prop-row">
          <span class="prop-label">Offset</span>
          <input type="number" value={obj.splitOp.offset} step="1"
            oninput={(e) => numInput(e, (v) => updateSplitOp('offset', v))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Keep</span>
          <select class="prop-select" value={obj.splitOp.keepSide}
            onchange={(e) => updateSplitOpImmediate('keepSide', (e.target as HTMLSelectElement).value)}>
            {#each keepSideOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <button class="remove-btn" onclick={removeSplitOp}>Remove Split</button>
      {:else if !obj.booleanOp && !obj.patternOp}
        <button class="apply-btn full-width" onclick={addSplitOp}>Split Body</button>
      {/if}
    </div>

    <!-- Pattern -->
    <div class="prop-section">
      <div class="prop-section-title">Pattern</div>
      {#if obj.patternOp}
        <div class="prop-row">
          <span class="prop-label">Type</span>
          <select class="prop-select" value={obj.patternOp.type}
            onchange={(e) => switchPatternType((e.target as HTMLSelectElement).value as PatternType)}>
            {#each patternTypeOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>

        {#if obj.patternOp.type === 'mirror'}
          <div class="prop-row">
            <span class="prop-label">Plane</span>
            <select class="prop-select" value={obj.patternOp.plane}
              onchange={(e) => updatePatternOpImmediate('plane', (e.target as HTMLSelectElement).value)}>
              {#each patternPlaneOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <span class="prop-label">Offset</span>
            <input type="number" value={obj.patternOp.offset} step="1"
              oninput={(e) => numInput(e, (v) => updatePatternOp('offset', v))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Keep Orig</span>
            <button class="toggle-btn" class:active={obj.patternOp.keepOriginal}
              onclick={() => { if (obj.patternOp?.type === 'mirror') updatePatternOpImmediate('keepOriginal', !obj.patternOp.keepOriginal); }}>
              {obj.patternOp.keepOriginal ? 'Yes' : 'No'}
            </button>
          </div>
        {:else if obj.patternOp.type === 'linear'}
          <div class="prop-row">
            <span class="prop-label">Dir</span>
            <select class="prop-select" value={obj.patternOp.direction}
              onchange={(e) => updatePatternOpImmediate('direction', (e.target as HTMLSelectElement).value)}>
              {#each patternDirOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <span class="prop-label">Spacing</span>
            <input type="number" value={obj.patternOp.spacing} step="1" min="0.1"
              oninput={(e) => numInput(e, (v) => updatePatternOp('spacing', v))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Count</span>
            <input type="number" value={obj.patternOp.count} step="1" min="2" max="50"
              oninput={(e) => numInput(e, (v) => updatePatternOp('count', Math.max(2, Math.round(v))))} />
          </div>
        {:else if obj.patternOp.type === 'circular'}
          <div class="prop-row">
            <span class="prop-label">Axis</span>
            <select class="prop-select" value={obj.patternOp.axis}
              onchange={(e) => updatePatternOpImmediate('axis', (e.target as HTMLSelectElement).value)}>
              {#each patternAxisOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <span class="prop-label">Count</span>
            <input type="number" value={obj.patternOp.count} step="1" min="2" max="50"
              oninput={(e) => numInput(e, (v) => updatePatternOp('count', Math.max(2, Math.round(v))))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Angle</span>
            <input type="number" value={obj.patternOp.fullAngle} step="15" min="1" max="360"
              oninput={(e) => numInput(e, (v) => updatePatternOp('fullAngle', v))} />
          </div>
        {/if}

        <button class="remove-btn" onclick={removePatternOp}>Remove Pattern</button>
      {:else if !obj.booleanOp && !obj.splitOp}
        <div class="op-buttons">
          <button class="apply-btn full-width" onclick={() => addPatternOp('mirror')}>Mirror Body</button>
          <button class="apply-btn full-width" onclick={() => addPatternOp('linear')}>Linear Pattern</button>
          <button class="apply-btn full-width" onclick={() => addPatternOp('circular')}>Circular Pattern</button>
        </div>
      {/if}
    </div>

    <!-- Appearance -->
    <div class="prop-section">
      <div class="prop-section-title">Appearance</div>
      <div class="prop-row">
        <span class="prop-label">Material</span>
        <select class="prop-select" value={obj.materialId ?? ''}
          onchange={(e) => applyMaterial((e.target as HTMLSelectElement).value)}>
          <option value="">Custom</option>
          {#each MATERIALS as mat}
            <option value={mat.id}>{mat.name}</option>
          {/each}
        </select>
      </div>
      <div class="prop-row">
        <span class="prop-label">Color</span>
        <input type="color" value={obj.color} oninput={updateColor} class="color-picker" />
      </div>
      <div class="prop-row">
        <span class="prop-label">Metal</span>
        <input type="range" class="slider" min="0" max="1" step="0.05"
          value={obj.metalness ?? DEFAULT_METALNESS}
          oninput={(e) => updateMetalness(parseFloat((e.target as HTMLInputElement).value))} />
        <span class="slider-val">{(obj.metalness ?? DEFAULT_METALNESS).toFixed(2)}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Rough</span>
        <input type="range" class="slider" min="0" max="1" step="0.05"
          value={obj.roughness ?? DEFAULT_ROUGHNESS}
          oninput={(e) => updateRoughness(parseFloat((e.target as HTMLInputElement).value))} />
        <span class="slider-val">{(obj.roughness ?? DEFAULT_ROUGHNESS).toFixed(2)}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Opacity</span>
        <input type="range" class="slider" min="0" max="1" step="0.05"
          value={obj.opacity ?? DEFAULT_OPACITY}
          oninput={(e) => updateOpacity(parseFloat((e.target as HTMLInputElement).value))} />
        <span class="slider-val">{Math.round((obj.opacity ?? DEFAULT_OPACITY) * 100)}%</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Visible</span>
        <button class="toggle-btn" class:active={obj.visible} onclick={toggleVisible}>
          {obj.visible ? 'Yes' : 'No'}
        </button>
      </div>
    </div>

    <!-- Actions -->
    <div class="prop-actions">
      <button class="delete-btn" onclick={deleteObject}>
        Delete
      </button>
    </div>
  </div>

{:else if sketchStore.selectedSketch}
  {@const sketch = sketchStore.selectedSketch}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge sketch-badge">sketch</span>
      <span class="prop-name-text">{sketch.name}</span>
    </div>

    <!-- Sketch Info -->
    <div class="prop-section">
      <div class="prop-section-title">Info</div>
      <div class="prop-row">
        <span class="prop-label">Plane</span>
        <span class="prop-value">{sketch.plane}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Entities</span>
        <span class="prop-value">{sketch.entities.length}</span>
      </div>
    </div>

    <!-- 3D Operation -->
    <div class="prop-section">
      <div class="prop-section-title">3D Operation</div>
      {#if !sketch.operation}
        <div class="op-buttons">
          <button class="apply-btn full-width" onclick={addSketchExtrude}>Extrude</button>
          <button class="apply-btn full-width" onclick={addSketchRevolve}>Revolve</button>
          <button class="apply-btn full-width" onclick={addSketchSweep}>Sweep</button>
        </div>
      {:else if sketch.operation.type === 'extrude'}
        <div class="prop-row">
          <span class="prop-label">Distance</span>
          <input type="number" value={sketch.operation.distance} step="1" min="0.1"
            oninput={(e) => numInput(e, (v) => updateSketchOperation({ distance: v }))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Taper</span>
          <input type="number" value={sketch.operation.taper ?? 0} step="1" min="0" max="89"
            oninput={(e) => numInput(e, (v) => updateSketchOperation({ taper: v || undefined }))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Mode</span>
          <select class="prop-select" value={sketch.operation.mode}
            onchange={(e) => updateSketchOperationImmediate({ mode: (e.target as HTMLSelectElement).value as 'add' | 'cut' })}>
            <option value="add">Add</option>
            <option value="cut">Cut</option>
          </select>
        </div>
        {#if sketch.operation.mode === 'cut'}
          <div class="prop-row">
            <span class="prop-label">Target</span>
            <select class="prop-select" value={sketch.operation.cutTargetId ?? ''}
              onchange={(e) => updateSketchOperationImmediate({ cutTargetId: (e.target as HTMLSelectElement).value || undefined })}>
              <option value="">None</option>
              {#each getCutTargets() as target}
                <option value={target.id}>{target.name}</option>
              {/each}
            </select>
          </div>
        {/if}
        <button class="remove-btn" onclick={removeSketchOperation}>Remove Extrude</button>
      {:else if sketch.operation.type === 'revolve'}
        <div class="prop-row">
          <span class="prop-label">Angle</span>
          <input type="number" value={sketch.operation.angle} step="15" min="1" max="360"
            oninput={(e) => numInput(e, (v) => updateSketchOperation({ angle: v }))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Axis</span>
          <select class="prop-select" value={sketch.operation.axisDirection}
            onchange={(e) => updateSketchOperationImmediate({ axisDirection: (e.target as HTMLSelectElement).value as 'X' | 'Y' })}>
            <option value="X">X</option>
            <option value="Y">Y</option>
          </select>
        </div>
        <div class="prop-row">
          <span class="prop-label">Offset</span>
          <input type="number" value={sketch.operation.axisOffset} step="1"
            oninput={(e) => numInput(e, (v) => updateSketchOperation({ axisOffset: v }))} />
        </div>
        <div class="prop-row">
          <span class="prop-label">Mode</span>
          <select class="prop-select" value={sketch.operation.mode}
            onchange={(e) => updateSketchOperationImmediate({ mode: (e.target as HTMLSelectElement).value as 'add' | 'cut' })}>
            <option value="add">Add</option>
            <option value="cut">Cut</option>
          </select>
        </div>
        {#if sketch.operation.mode === 'cut'}
          <div class="prop-row">
            <span class="prop-label">Target</span>
            <select class="prop-select" value={sketch.operation.cutTargetId ?? ''}
              onchange={(e) => updateSketchOperationImmediate({ cutTargetId: (e.target as HTMLSelectElement).value || undefined })}>
              <option value="">None</option>
              {#each getCutTargets() as target}
                <option value={target.id}>{target.name}</option>
              {/each}
            </select>
          </div>
        {/if}
        <button class="remove-btn" onclick={removeSketchOperation}>Remove Revolve</button>
      {:else if sketch.operation.type === 'sweep'}
        <div class="prop-row">
          <span class="prop-label">Path</span>
          <select class="prop-select" value={sketch.operation.pathSketchId}
            onchange={(e) => updateSketchOperationImmediate({ pathSketchId: (e.target as HTMLSelectElement).value })}>
            <option value="">Select path...</option>
            {#each getPathSketches() as ps}
              <option value={ps.id}>{ps.name}</option>
            {/each}
          </select>
        </div>
        <div class="prop-row">
          <span class="prop-label">Mode</span>
          <select class="prop-select" value={sketch.operation.mode}
            onchange={(e) => updateSketchOperationImmediate({ mode: (e.target as HTMLSelectElement).value as 'add' | 'cut' })}>
            <option value="add">Add</option>
            <option value="cut">Cut</option>
          </select>
        </div>
        {#if sketch.operation.mode === 'cut'}
          <div class="prop-row">
            <span class="prop-label">Target</span>
            <select class="prop-select" value={sketch.operation.cutTargetId ?? ''}
              onchange={(e) => updateSketchOperationImmediate({ cutTargetId: (e.target as HTMLSelectElement).value || undefined })}>
              <option value="">None</option>
              {#each getCutTargets() as target}
                <option value={target.id}>{target.name}</option>
              {/each}
            </select>
          </div>
        {/if}
        <button class="remove-btn" onclick={removeSketchOperation}>Remove Sweep</button>
      {/if}
    </div>

    <!-- Post-processing (only when 3D operation is set) -->
    {#if sketch.operation}
      <!-- Fillet -->
      <div class="prop-section">
        <div class="prop-section-title">Fillet</div>
        {#if sketch.fillet}
          <div class="prop-row">
            <span class="prop-label">Radius</span>
            <input type="number" value={sketch.fillet.radius} step="0.1" min="0.01"
              oninput={(e) => numInput(e, (v) => updateSketchFillet('radius', v))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Edges</span>
            <select class="prop-select" value={sketch.fillet.edges}
              onchange={(e) => updateSketchFillet('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
              {#each edgeSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <button class="remove-btn" onclick={removeSketchFillet}>Remove Fillet</button>
        {:else}
          <button class="apply-btn full-width" onclick={addSketchFillet}>Add Fillet</button>
        {/if}
      </div>

      <!-- Chamfer -->
      <div class="prop-section">
        <div class="prop-section-title">Chamfer</div>
        {#if sketch.chamfer}
          <div class="prop-row">
            <span class="prop-label">Distance</span>
            <input type="number" value={sketch.chamfer.distance} step="0.1" min="0.01"
              oninput={(e) => numInput(e, (v) => updateSketchChamfer('distance', v))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Edges</span>
            <select class="prop-select" value={sketch.chamfer.edges}
              onchange={(e) => updateSketchChamfer('edges', (e.target as HTMLSelectElement).value as EdgeSelector)}>
              {#each edgeSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <button class="remove-btn" onclick={removeSketchChamfer}>Remove Chamfer</button>
        {:else}
          <button class="apply-btn full-width" onclick={addSketchChamfer}>Add Chamfer</button>
        {/if}
      </div>

      <!-- Shell -->
      <div class="prop-section">
        <div class="prop-section-title">Shell</div>
        {#if sketch.shell}
          <div class="prop-row">
            <span class="prop-label">Thickness</span>
            <input type="number" value={sketch.shell.thickness} step="0.5"
              oninput={(e) => numInput(e, (v) => updateSketchShell('thickness', v))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">Face</span>
            <select class="prop-select" value={sketch.shell.face}
              onchange={(e) => updateSketchShell('face', (e.target as HTMLSelectElement).value as FaceSelector)}>
              {#each faceSelectorOptions as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
          <button class="remove-btn" onclick={removeSketchShell}>Remove Shell</button>
        {:else}
          <button class="apply-btn full-width" onclick={addSketchShell}>Add Shell</button>
        {/if}
      </div>

      <!-- Holes -->
      <div class="prop-section">
        <div class="prop-section-title">Holes ({sketch.holes?.length ?? 0})</div>
        {#each (sketch.holes ?? []) as hole, index}
          <div class="hole-item">
            <div class="prop-row">
              <span class="prop-label">Type</span>
              <select class="prop-select" value={hole.holeType}
                onchange={(e) => updateSketchHole(index, 'holeType', (e.target as HTMLSelectElement).value)}>
                {#each holeTypeOptions as opt}
                  <option value={opt.value}>{opt.label}</option>
                {/each}
              </select>
            </div>
            <div class="prop-row">
              <span class="prop-label">Dia</span>
              <input type="number" value={hole.diameter} step="0.5" min="0.1"
                oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'diameter', v))} />
            </div>
            {#if hole.holeType === 'blind'}
              <div class="prop-row">
                <span class="prop-label">Depth</span>
                <input type="number" value={hole.depth ?? 5} step="0.5" min="0.1"
                  oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'depth', v))} />
              </div>
            {/if}
            {#if hole.holeType === 'counterbore'}
              <div class="prop-row">
                <span class="prop-label">CB Dia</span>
                <input type="number" value={hole.cboreDiameter ?? 8} step="0.5" min="0.1"
                  oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'cboreDiameter', v))} />
              </div>
              <div class="prop-row">
                <span class="prop-label">CB Dep</span>
                <input type="number" value={hole.cboreDepth ?? 3} step="0.5" min="0.1"
                  oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'cboreDepth', v))} />
              </div>
            {/if}
            {#if hole.holeType === 'countersink'}
              <div class="prop-row">
                <span class="prop-label">CS Dia</span>
                <input type="number" value={hole.cskDiameter ?? 10} step="0.5" min="0.1"
                  oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'cskDiameter', v))} />
              </div>
              <div class="prop-row">
                <span class="prop-label">CS Angle</span>
                <input type="number" value={hole.cskAngle ?? 82} step="1" min="1" max="180"
                  oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'cskAngle', v))} />
              </div>
            {/if}
            <div class="prop-row">
              <span class="prop-label">Face</span>
              <select class="prop-select" value={hole.face}
                onchange={(e) => updateSketchHole(index, 'face', (e.target as HTMLSelectElement).value)}>
                {#each faceSelectorOptions as opt}
                  <option value={opt.value}>{opt.label}</option>
                {/each}
              </select>
            </div>
            <div class="prop-row">
              <span class="prop-label">Pos X</span>
              <input type="number" value={hole.position[0]} step="1"
                oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'position', [v, hole.position[1]]))} />
            </div>
            <div class="prop-row">
              <span class="prop-label">Pos Y</span>
              <input type="number" value={hole.position[1]} step="1"
                oninput={(e) => numInput(e, (v) => updateSketchHole(index, 'position', [hole.position[0], v]))} />
            </div>
            <button class="remove-btn" onclick={() => removeSketchHole(index)}>Remove Hole</button>
          </div>
        {/each}
        <button class="apply-btn full-width" onclick={addSketchHole}>Add Hole</button>
      </div>
    {/if}

    <!-- Constraints -->
    {#if (sketch.constraints ?? []).length > 0}
      <div class="prop-section">
        <div class="prop-section-title">Constraints ({(sketch.constraints ?? []).length})</div>
        {#each (sketch.constraints ?? []) as constraint}
          <div class="prop-row constraint-row">
            <span class="constraint-label">{constraintLabel(constraint)}</span>
            <button class="constraint-remove-btn" onclick={() => removeConstraint(constraint.id)}
              title="Remove constraint">&times;</button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Actions -->
    <div class="prop-actions">
      <button class="apply-btn full-width" onclick={editSketch}>
        Edit Sketch
      </button>
      <button class="delete-btn" onclick={deleteSketch}>
        Delete Sketch
      </button>
    </div>
  </div>

{:else if datumStore.selectedDatum && isDatumPlane(datumStore.selectedDatum)}
  {@const datum = datumStore.selectedDatum}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge datum-badge">datum plane</span>
      <span class="prop-name-text">{datum.name}</span>
    </div>

    <div class="prop-section">
      <div class="prop-section-title">Definition</div>
      {#if datum.definition.type === 'offset'}
        <div class="prop-row">
          <span class="prop-label">Base</span>
          <select class="prop-select" value={datum.definition.basePlane}
            onchange={(e) => updateDatumPlaneDefinition({ basePlane: (e.target as HTMLSelectElement).value as 'XY' | 'XZ' | 'YZ' })}>
            <option value="XY">XY</option>
            <option value="XZ">XZ</option>
            <option value="YZ">YZ</option>
          </select>
        </div>
        <div class="prop-row">
          <span class="prop-label">Offset</span>
          <input type="number" value={datum.definition.offset} step="1"
            oninput={(e) => numInput(e, (v) => updateDatumPlaneDefinition({ offset: v }))} />
        </div>
      {:else}
        {#each [{ label: 'P1', val: datum.definition.p1, key: 'p1' },
                 { label: 'P2', val: datum.definition.p2, key: 'p2' },
                 { label: 'P3', val: datum.definition.p3, key: 'p3' }] as pt}
          <div class="prop-row">
            <span class="prop-label">{pt.label} X</span>
            <input type="number" value={pt.val[0]} step="1"
              oninput={(e) => numInput(e, (v) => updateDatumPlaneDefinition({ [pt.key]: [v, pt.val[1], pt.val[2]] }))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">{pt.label} Y</span>
            <input type="number" value={pt.val[1]} step="1"
              oninput={(e) => numInput(e, (v) => updateDatumPlaneDefinition({ [pt.key]: [pt.val[0], v, pt.val[2]] }))} />
          </div>
          <div class="prop-row">
            <span class="prop-label">{pt.label} Z</span>
            <input type="number" value={pt.val[2]} step="1"
              oninput={(e) => numInput(e, (v) => updateDatumPlaneDefinition({ [pt.key]: [pt.val[0], pt.val[1], v] }))} />
          </div>
        {/each}
      {/if}
    </div>

    <div class="prop-section">
      <div class="prop-section-title">Appearance</div>
      <div class="prop-row">
        <span class="prop-label">Color</span>
        <input type="color" value={datum.color} oninput={updateDatumPlaneColor} class="color-picker" />
      </div>
      <div class="prop-row">
        <span class="prop-label">Visible</span>
        <button class="toggle-btn" class:active={datum.visible} onclick={toggleDatumPlaneVisible}>
          {datum.visible ? 'Yes' : 'No'}
        </button>
      </div>
    </div>

    <div class="prop-actions">
      <button class="apply-btn full-width" onclick={sketchOnDatumPlane}>
        Sketch on this Plane
      </button>
      <button class="delete-btn" onclick={deleteDatumPlane}>
        Delete
      </button>
    </div>
  </div>

{:else if datumStore.selectedDatum && isDatumAxis(datumStore.selectedDatum)}
  {@const datum = datumStore.selectedDatum}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge datum-badge">datum axis</span>
      <span class="prop-name-text">{datum.name}</span>
    </div>

    <div class="prop-section">
      <div class="prop-section-title">Origin</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={datum.origin[0]} step="1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('origin', [v, datum.origin[1], datum.origin[2]]))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={datum.origin[1]} step="1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('origin', [datum.origin[0], v, datum.origin[2]]))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={datum.origin[2]} step="1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('origin', [datum.origin[0], datum.origin[1], v]))} />
      </div>
    </div>

    <div class="prop-section">
      <div class="prop-section-title">Direction</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={datum.direction[0]} step="0.1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('direction', [v, datum.direction[1], datum.direction[2]]))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={datum.direction[1]} step="0.1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('direction', [datum.direction[0], v, datum.direction[2]]))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={datum.direction[2]} step="0.1"
          oninput={(e) => numInput(e, (v) => updateDatumAxis('direction', [datum.direction[0], datum.direction[1], v]))} />
      </div>
    </div>

    <div class="prop-section">
      <div class="prop-section-title">Appearance</div>
      <div class="prop-row">
        <span class="prop-label">Color</span>
        <input type="color" value={datum.color} oninput={updateDatumAxisColor} class="color-picker" />
      </div>
      <div class="prop-row">
        <span class="prop-label">Visible</span>
        <button class="toggle-btn" class:active={datum.visible} onclick={toggleDatumAxisVisible}>
          {datum.visible ? 'Yes' : 'No'}
        </button>
      </div>
    </div>

    <div class="prop-actions">
      <button class="delete-btn" onclick={deleteDatumAxis}>
        Delete
      </button>
    </div>
  </div>

{:else if componentStore.selectedComponent}
  {@const comp = componentStore.selectedComponent}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge component-badge">component</span>
      <input
        class="prop-name-input"
        type="text"
        value={comp.name}
        oninput={updateComponentName}
      />
    </div>

    <!-- Info -->
    <div class="prop-section">
      <div class="prop-section-title">Info</div>
      <div class="prop-row">
        <span class="prop-label">Features</span>
        <span class="prop-value">{comp.featureIds.length}</span>
      </div>
      {#if comp.sourceFile}
        <div class="prop-row">
          <span class="prop-label">Source</span>
          <span class="prop-value source-path" title={comp.sourceFile}>{comp.sourceFile.split(/[\\/]/).pop()}</span>
        </div>
      {/if}
    </div>

    <!-- Position -->
    <div class="prop-section">
      <div class="prop-section-title">Position {comp.grounded ? '(locked)' : ''}</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={comp.transform.position[0]} step="1" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentPosition(0, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={comp.transform.position[1]} step="1" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentPosition(1, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={comp.transform.position[2]} step="1" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentPosition(2, v))} />
      </div>
    </div>

    <!-- Rotation -->
    <div class="prop-section">
      <div class="prop-section-title">Rotation {comp.grounded ? '(locked)' : ''}</div>
      <div class="prop-row">
        <span class="prop-label">X</span>
        <input type="number" value={comp.transform.rotation[0]} step="5" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentRotation(0, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Y</span>
        <input type="number" value={comp.transform.rotation[1]} step="5" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentRotation(1, v))} />
      </div>
      <div class="prop-row">
        <span class="prop-label">Z</span>
        <input type="number" value={comp.transform.rotation[2]} step="5" disabled={comp.grounded}
          oninput={(e) => numInput(e, (v) => updateComponentRotation(2, v))} />
      </div>
    </div>

    <!-- Options -->
    <div class="prop-section">
      <div class="prop-section-title">Options</div>
      <div class="prop-row">
        <span class="prop-label">Grounded</span>
        <button class="toggle-btn" class:active={comp.grounded} onclick={toggleComponentGrounded}>
          {comp.grounded ? 'Yes' : 'No'}
        </button>
      </div>
      <div class="prop-row">
        <span class="prop-label">Visible</span>
        <button class="toggle-btn" class:active={comp.visible} onclick={toggleComponentVisible}>
          {comp.visible ? 'Yes' : 'No'}
        </button>
      </div>
      <div class="prop-row">
        <span class="prop-label">Color</span>
        <input type="color" value={comp.color} oninput={updateComponentColor} class="color-picker" />
      </div>
    </div>

    <!-- Actions -->
    <div class="prop-actions">
      <button class="apply-btn full-width" onclick={dissolveComponent}>
        Dissolve Component
      </button>
    </div>
  </div>

{:else if mateStore.selectedMate}
  {@const mate = mateStore.selectedMate}
  <div class="properties-panel">
    <div class="prop-header">
      <span class="prop-type-badge mate-badge">{mate.type}</span>
      <input
        class="prop-name-input"
        type="text"
        value={mate.name}
        oninput={updateMateName}
      />
    </div>

    <!-- Reference A -->
    <div class="prop-section">
      <div class="prop-section-title">Reference A</div>
      <div class="prop-row">
        <span class="prop-label">Component</span>
        <span class="prop-value">{componentStore.getComponentById(mate.ref1.componentId)?.name ?? '?'}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Face</span>
        <span class="prop-value">{mate.ref1.faceSelector}</span>
      </div>
    </div>

    <!-- Reference B -->
    <div class="prop-section">
      <div class="prop-section-title">Reference B</div>
      <div class="prop-row">
        <span class="prop-label">Component</span>
        <span class="prop-value">{componentStore.getComponentById(mate.ref2.componentId)?.name ?? '?'}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Face</span>
        <span class="prop-value">{mate.ref2.faceSelector}</span>
      </div>
    </div>

    <!-- Parameters -->
    {#if mate.type === 'distance'}
      <div class="prop-section">
        <div class="prop-section-title">Parameters</div>
        <div class="prop-row">
          <span class="prop-label">Distance</span>
          <input type="number" value={mate.distance} step="1"
            oninput={(e) => numInput(e, (v) => updateMateDistance(v))} />
        </div>
      </div>
    {/if}

    {#if mate.type === 'angle'}
      <div class="prop-section">
        <div class="prop-section-title">Parameters</div>
        <div class="prop-row">
          <span class="prop-label">Angle</span>
          <input type="number" value={mate.angle} step="5"
            oninput={(e) => numInput(e, (v) => updateMateAngle(v))} />
        </div>
      </div>
    {/if}

    {#if mate.type === 'coincident'}
      <div class="prop-section">
        <div class="prop-section-title">Options</div>
        <div class="prop-row">
          <span class="prop-label">Flipped</span>
          <button class="toggle-btn" class:active={mate.flipped} onclick={toggleMateFlipped}>
            {mate.flipped ? 'Yes' : 'No'}
          </button>
        </div>
      </div>
    {/if}

    <!-- Actions -->
    <div class="prop-actions">
      <button class="delete-btn" onclick={deleteMate}>
        Delete Mate
      </button>
    </div>
  </div>

{:else}
  <div class="no-selection">
    <span class="no-selection-text">No object selected</span>
    <span class="no-selection-hint">Click an object in the viewport or add one from the toolbar</span>
  </div>
{/if}

<style>
  .properties-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: 12px;
    gap: 12px;
  }

  .prop-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-type-badge {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--accent);
    background: rgba(137, 180, 250, 0.12);
    padding: 2px 6px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .prop-type-badge.sketch-badge {
    color: #f9e2af;
    background: rgba(249, 226, 175, 0.12);
  }

  .prop-type-badge.datum-badge {
    color: #f5c2e7;
    background: rgba(245, 194, 231, 0.12);
  }

  .prop-type-badge.component-badge {
    color: #94e2d5;
    background: rgba(148, 226, 213, 0.12);
  }

  .prop-type-badge.mate-badge {
    color: #f2cdcd;
    background: rgba(242, 205, 205, 0.12);
  }

  .source-path {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }

  .prop-name-input {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .prop-name-input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .prop-name-text {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .prop-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .prop-section-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    margin-bottom: 2px;
  }

  .prop-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-row .prop-label {
    font-size: 11px;
    color: var(--text-secondary);
    width: 48px;
    flex-shrink: 0;
  }

  .prop-row input[type="number"] {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-primary);
    min-width: 0;
  }

  .prop-row input[type="number"]:focus {
    border-color: var(--accent);
    outline: none;
  }

  .prop-value {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-primary);
  }

  .prop-select {
    flex: 1;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 6px;
    font-size: 12px;
    color: var(--text-primary);
    min-width: 0;
  }

  .prop-select:focus {
    border-color: var(--accent);
    outline: none;
  }

  .color-picker {
    width: 32px;
    height: 24px;
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 0;
    cursor: pointer;
    background: none;
  }

  .toggle-btn {
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 2px 10px;
    font-size: 11px;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .toggle-btn.active {
    color: var(--success);
    border-color: var(--success);
    background: rgba(166, 227, 161, 0.1);
  }

  .apply-btn {
    background: rgba(137, 180, 250, 0.1);
    border: 1px solid var(--accent);
    color: var(--accent);
    border-radius: 3px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
    transition: all 0.12s ease;
  }

  .apply-btn:hover {
    background: rgba(137, 180, 250, 0.2);
  }

  .apply-btn.full-width {
    width: 100%;
    padding: 4px 8px;
    margin-top: 4px;
  }

  .remove-btn {
    width: 100%;
    background: none;
    border: 1px solid var(--text-muted);
    color: var(--text-muted);
    border-radius: 3px;
    padding: 3px 8px;
    font-size: 11px;
    cursor: pointer;
    margin-top: 2px;
    transition: all 0.12s ease;
  }

  .remove-btn:hover {
    border-color: var(--error);
    color: var(--error);
    background: rgba(243, 139, 168, 0.1);
  }

  .prop-actions {
    margin-top: auto;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .delete-btn {
    width: 100%;
    background: none;
    border: 1px solid var(--error);
    color: var(--error);
    border-radius: 3px;
    padding: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .delete-btn:hover {
    background: rgba(243, 139, 168, 0.15);
  }

  .no-selection {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    padding: 24px;
    text-align: center;
  }

  .no-selection-text {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .no-selection-hint {
    font-size: 11px;
    color: var(--text-muted);
  }

  .constraint-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 0;
  }

  .constraint-label {
    font-size: 11px;
    color: #cba6f7;
  }

  .constraint-remove-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 14px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    transition: color 0.12s ease;
  }

  .constraint-remove-btn:hover {
    color: var(--error);
  }

  .op-buttons {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .hole-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    margin-bottom: 4px;
  }

  .prop-hint {
    font-size: 10px;
    color: var(--text-muted);
    margin-bottom: 2px;
  }

  .slider {
    flex: 1;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: var(--border-subtle);
    border-radius: 2px;
    outline: none;
    min-width: 0;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider-val {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    width: 36px;
    text-align: right;
    flex-shrink: 0;
  }
</style>
