import type { SceneObject, PrimitiveParams, CadTransform, Sketch, SketchEntity, EdgeSelector, FilletParams, ChamferParams, ShellParams, HoleParams, RevolveParams, SketchPlane, BooleanOp, SplitOp, SplitPlane, PatternOp, DatumPlane, Component } from '$lib/types/cad';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';
import { getMateStore } from '$lib/stores/mate.svelte';
import { getFeatureTreeStore } from '$lib/stores/feature-tree.svelte';
import type { SymbolMap } from '$lib/types/symbol-map';

const PYTHON_RESERVED_WORDS = new Set([
  'false', 'none', 'true', 'and', 'as', 'assert', 'async', 'await', 'break',
  'class', 'continue', 'def', 'del', 'elif', 'else', 'except', 'finally',
  'for', 'from', 'global', 'if', 'import', 'in', 'is', 'lambda', 'nonlocal',
  'not', 'or', 'pass', 'raise', 'return', 'try', 'while', 'with', 'yield',
  'match', 'case',
]);

function sanitizeIdentifier(raw: string, fallback: string): string {
  const trimmed = raw.trim();
  const base = (trimmed || fallback).replace(/[^a-zA-Z0-9_]/g, '_');
  const normalized = base.replace(/_+/g, '_').replace(/^_+/, '');
  const withPrefix = normalized.length > 0 ? normalized : fallback;
  const startsValid = /^[a-zA-Z_]/.test(withPrefix);
  const candidate = startsValid ? withPrefix : `_${withPrefix}`;
  return PYTHON_RESERVED_WORDS.has(candidate.toLowerCase())
    ? `${candidate}_var`
    : candidate;
}

function symbolForId(prefix: string, id: string): string {
  return sanitizeIdentifier(`${prefix}_${id}`, `${prefix}_feature`);
}

function buildSymbolMap(
  objects: SceneObject[],
  sketches: Sketch[],
  components: Component[],
): SymbolMap {
  const map: SymbolMap = {};
  for (const obj of objects) map[obj.id] = symbolForId('obj', obj.id);
  for (const sketch of sketches) map[sketch.id] = symbolForId('sketch', sketch.id);
  for (const component of components) map[component.id] = symbolForId('comp', component.id);
  return map;
}

function symbolForFeature(symbolMap: SymbolMap, id: string, fallbackPrefix: string): string {
  return symbolMap[id] ?? symbolForId(fallbackPrefix, id);
}

function sketchCutterSymbol(symbolMap: SymbolMap, sketchId: string): string {
  return `${symbolForFeature(symbolMap, sketchId, 'sketch')}_cutter`;
}

function sketchPathSymbol(symbolMap: SymbolMap, sketchId: string): string {
  return `${symbolForFeature(symbolMap, sketchId, 'sketch')}_path`;
}

function pyString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

function generatePrimitive(name: string, params: PrimitiveParams): string {
  switch (params.type) {
    case 'box':
      return `${name} = cq.Workplane("XY").box(${params.width}, ${params.depth}, ${params.height})`;
    case 'cylinder':
      return `${name} = cq.Workplane("XY").circle(${params.radius}).extrude(${params.height})`;
    case 'sphere':
      return `${name} = cq.Workplane("XY").sphere(${params.radius})`;
    case 'cone':
      // CadQuery cone via revolution of a line profile
      return `${name} = cq.Solid.makeCone(${params.bottomRadius}, ${params.topRadius}, ${params.height})`;
  }
}

function generateTransform(name: string, transform: CadTransform): string[] {
  const lines: string[] = [];
  const [x, y, z] = transform.position;
  const [rx, ry, rz] = transform.rotation;

  if (rx !== 0 || ry !== 0 || rz !== 0) {
    if (rx !== 0) lines.push(`${name} = ${name}.rotate((0,0,0), (1,0,0), ${rx})`);
    if (ry !== 0) lines.push(`${name} = ${name}.rotate((0,0,0), (0,1,0), ${ry})`);
    if (rz !== 0) lines.push(`${name} = ${name}.rotate((0,0,0), (0,0,1), ${rz})`);
  }

  if (x !== 0 || y !== 0 || z !== 0) {
    lines.push(`${name} = ${name}.translate((${x}, ${y}, ${z}))`);
  }

  return lines;
}

function fmt(n: number): string {
  // Format number, removing trailing zeros
  return parseFloat(n.toFixed(4)).toString();
}

function edgeSelectorToCadQuery(selector: EdgeSelector): string {
  switch (selector) {
    case 'all':
      return '.edges()';
    case 'top':
      return '.edges(">Z")';
    case 'bottom':
      return '.edges("<Z")';
    case 'vertical':
      return '.edges("|Z")';
  }
}

function generateFilletChamfer(name: string, fillet?: FilletParams, chamfer?: ChamferParams): string[] {
  const lines: string[] = [];
  if (fillet) {
    lines.push(`${name} = ${name}${edgeSelectorToCadQuery(fillet.edges)}.fillet(${fmt(fillet.radius)})`);
  }
  if (chamfer) {
    lines.push(`${name} = ${name}${edgeSelectorToCadQuery(chamfer.edges)}.chamfer(${fmt(chamfer.distance)})`);
  }
  return lines;
}

function generateShell(name: string, shell?: ShellParams): string[] {
  if (!shell) return [];
  return [`${name} = ${name}.faces("${shell.face}").shell(${fmt(shell.thickness)})`];
}

function generateHoles(name: string, holes?: HoleParams[]): string[] {
  if (!holes?.length) return [];
  const lines: string[] = [];
  for (const hole of holes) {
    const face = `.faces("${hole.face}").workplane()`;
    const pos = (hole.position[0] !== 0 || hole.position[1] !== 0)
      ? `.center(${fmt(hole.position[0])}, ${fmt(hole.position[1])})` : '';
    switch (hole.holeType) {
      case 'through':
        lines.push(`${name} = ${name}${face}${pos}.hole(${fmt(hole.diameter)})`);
        break;
      case 'blind':
        lines.push(`${name} = ${name}${face}${pos}.hole(${fmt(hole.diameter)}, ${fmt(hole.depth ?? 5)})`);
        break;
      case 'counterbore':
        lines.push(`${name} = ${name}${face}${pos}.cboreHole(${fmt(hole.diameter)}, ${fmt(hole.cboreDiameter ?? hole.diameter * 1.6)}, ${fmt(hole.cboreDepth ?? 3)})`);
        break;
      case 'countersink':
        lines.push(`${name} = ${name}${face}${pos}.cskHole(${fmt(hole.diameter)}, ${fmt(hole.cskDiameter ?? hole.diameter * 2)}, ${fmt(hole.cskAngle ?? 82)})`);
        break;
    }
  }
  return lines;
}

/** Map sketch plane + axis direction + offset to CadQuery revolve args */
function revolveAxis(plane: SketchPlane, op: RevolveParams): string {
  // CadQuery .revolve(angle, axisStart, axisEnd) uses 2D sketch-plane coordinates
  // axisDirection='X' means axis along sketch X, 'Y' means along sketch Y
  // axisOffset is the perpendicular offset from origin
  const offset = op.axisOffset;
  if (op.axisDirection === 'X') {
    // Axis along X at Y=offset
    return `(0, ${fmt(offset)}, 0), (1, ${fmt(offset)}, 0)`;
  } else {
    // Axis along Y at X=offset
    return `(${fmt(offset)}, 0, 0), (${fmt(offset)}, 1, 0)`;
  }
}

/** Generate a CadQuery wire from a path sketch (for sweep) */
function generatePathWire(varName: string, pathSketch: Sketch): string[] {
  const lines: string[] = [];
  lines.push(`${varName} = (`);
  lines.push(`    ${workplaneString(pathSketch.plane, pathSketch.origin)}`);
  for (const entity of pathSketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }
  lines.push(`    .wire()`);
  lines.push(`)`);
  return lines;
}

function generateSketchEntity(entity: SketchEntity): string[] {
  const lines: string[] = [];
  switch (entity.type) {
    case 'line':
      lines.push(`    .moveTo(${fmt(entity.start[0])}, ${fmt(entity.start[1])})`);
      lines.push(`    .lineTo(${fmt(entity.end[0])}, ${fmt(entity.end[1])})`);
      break;
    case 'rectangle': {
      const w = Math.abs(entity.corner2[0] - entity.corner1[0]);
      const h = Math.abs(entity.corner2[1] - entity.corner1[1]);
      const cx = (entity.corner1[0] + entity.corner2[0]) / 2;
      const cy = (entity.corner1[1] + entity.corner2[1]) / 2;
      lines.push(`    .center(${fmt(cx)}, ${fmt(cy)})`);
      lines.push(`    .rect(${fmt(w)}, ${fmt(h)})`);
      break;
    }
    case 'circle':
      lines.push(`    .center(${fmt(entity.center[0])}, ${fmt(entity.center[1])})`);
      lines.push(`    .circle(${fmt(entity.radius)})`);
      break;
    case 'arc':
      lines.push(`    .moveTo(${fmt(entity.start[0])}, ${fmt(entity.start[1])})`);
      lines.push(`    .threePointArc((${fmt(entity.mid[0])}, ${fmt(entity.mid[1])}), (${fmt(entity.end[0])}, ${fmt(entity.end[1])}))`);
      break;
  }
  return lines;
}

/**
 * Generate CadQuery workplane string for a sketch plane (standard or datum).
 */
function workplaneString(plane: SketchPlane, origin: [number, number, number]): string {
  // Standard planes at origin
  if (plane === 'XY' || plane === 'XZ' || plane === 'YZ') {
    return `cq.Workplane("${plane}")`;
  }

  // Datum plane reference
  const datumPlane = getDatumStore().getDatumPlaneById(plane);
  if (!datumPlane) return `cq.Workplane("XY")`;

  if (datumPlane.definition.type === 'offset') {
    return `cq.Workplane("${datumPlane.definition.basePlane}").workplane(offset=${fmt(datumPlane.definition.offset)})`;
  }

  // 3-point plane: compute normal from cross product
  const { p1, p2, p3 } = datumPlane.definition;
  const ab = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
  const ac = [p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];
  const nx = ab[1] * ac[2] - ab[2] * ac[1];
  const ny = ab[2] * ac[0] - ab[0] * ac[2];
  const nz = ab[0] * ac[1] - ab[1] * ac[0];
  const len = Math.sqrt(nx * nx + ny * ny + nz * nz);
  const normal = len > 1e-10 ? [nx / len, ny / len, nz / len] : [0, 0, 1];

  return `cq.Workplane(cq.Plane(origin=(${fmt(p1[0])}, ${fmt(p1[1])}, ${fmt(p1[2])}), normal=(${fmt(normal[0])}, ${fmt(normal[1])}, ${fmt(normal[2])})))`;
}

function generateSketchBase(sketch: Sketch, symbolMap: SymbolMap, allSketches?: Sketch[]): string[] {
  const lines: string[] = [];
  const constraintCount = (sketch.constraints ?? []).length;
  const constraintInfo = constraintCount > 0 ? ` [${constraintCount} constraint${constraintCount !== 1 ? 's' : ''}]` : '';
  lines.push(`# --- ${sketch.name} (${sketch.plane} plane) ---${constraintInfo}`);

  const op = sketch.operation;
  const baseVarName = symbolForFeature(symbolMap, sketch.id, 'sketch');
  const varName = op?.mode === 'cut' ? sketchCutterSymbol(symbolMap, sketch.id) : baseVarName;

  // For sweep, generate path sketch inline first
  if (op?.type === 'sweep' && allSketches) {
    const pathSketch = allSketches.find(s => s.id === op.pathSketchId);
    if (pathSketch) {
      lines.push(...generatePathWire(sketchPathSymbol(symbolMap, sketch.id), pathSketch));
    }
  }

  lines.push(`${varName} = (`);
  lines.push(`    ${workplaneString(sketch.plane, sketch.origin)}`);

  for (const entity of sketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }

  // Apply 3D operation
  if (op?.type === 'extrude') {
    const taperArg = op.taper ? `, taper=${fmt(op.taper)}` : '';
    lines.push(`    .extrude(${fmt(op.distance)}${taperArg})`);
  } else if (op?.type === 'revolve') {
    lines.push(`    .revolve(${fmt(op.angle)}, ${revolveAxis(sketch.plane, op)})`);
  } else if (op?.type === 'sweep') {
    lines.push(`    .sweep(${sketchPathSymbol(symbolMap, sketch.id)})`);
  }

  lines.push(`)`);

  // Post-processing chain (only if 3D operation set)
  if (op) {
    lines.push(...generateFilletChamfer(varName, sketch.fillet, sketch.chamfer));
    lines.push(...generateShell(varName, sketch.shell));
    lines.push(...generateHoles(varName, sketch.holes));
  }

  lines.push('');
  return lines;
}

function generateSplitOp(name: string, split: SplitOp): string[] {
  const lines: string[] = [];
  lines.push(`# --- Split ${name} on ${split.plane} at offset ${fmt(split.offset)} ---`);

  // Build a large half-space cutter on the split plane
  // The half-space is a 1000x1000x1000 box on one side of the plane
  const planeMap: Record<SplitPlane, { axis: string; offset: (o: number) => string; negTrans: (o: number) => string }> = {
    'XY': { axis: 'Z', offset: (o) => `offset=cq.Vector(0, 0, ${fmt(o)})`, negTrans: (o) => `(0, 0, ${fmt(o - 1000)})` },
    'XZ': { axis: 'Y', offset: (o) => `offset=cq.Vector(0, ${fmt(o)}, 0)`, negTrans: (o) => `(0, ${fmt(o - 1000)}, 0)` },
    'YZ': { axis: 'X', offset: (o) => `offset=cq.Vector(${fmt(o)}, 0, 0)`, negTrans: (o) => `(${fmt(o - 1000)}, 0, 0)` },
  };
  const p = planeMap[split.plane];

  if (split.keepSide === 'positive') {
    // Cut away the negative half (below the plane)
    lines.push(`_split_cutter = cq.Workplane("${split.plane}").transformed(${p.offset(split.offset)}).box(1000, 1000, 1000, centered=(True, True, False)).translate(${p.negTrans(split.offset)})`);
    lines.push(`${name} = ${name}.cut(_split_cutter)`);
  } else if (split.keepSide === 'negative') {
    // Cut away the positive half (above the plane)
    lines.push(`_split_cutter = cq.Workplane("${split.plane}").transformed(${p.offset(split.offset)}).box(1000, 1000, 1000, centered=(True, True, False))`);
    lines.push(`${name} = ${name}.cut(_split_cutter)`);
  } else {
    // Keep both: create two halves
    lines.push(`_split_pos_cutter = cq.Workplane("${split.plane}").transformed(${p.offset(split.offset)}).box(1000, 1000, 1000, centered=(True, True, False)).translate(${p.negTrans(split.offset)})`);
    lines.push(`_split_neg_cutter = cq.Workplane("${split.plane}").transformed(${p.offset(split.offset)}).box(1000, 1000, 1000, centered=(True, True, False))`);
    lines.push(`${name}_pos = ${name}.cut(_split_pos_cutter)`);
    lines.push(`${name}_neg = ${name}.cut(_split_neg_cutter)`);
  }
  lines.push('');
  return lines;
}

function generatePatternOp(name: string, pattern: PatternOp): string[] {
  const lines: string[] = [];
  lines.push(`# --- Pattern: ${pattern.type} ---`);

  switch (pattern.type) {
    case 'mirror': {
      const plane = pattern.plane;
      if (pattern.keepOriginal) {
        if (pattern.offset !== 0) {
          const axisVec = plane === 'XY' ? `(0, 0, ${fmt(-pattern.offset)})`
            : plane === 'XZ' ? `(0, ${fmt(-pattern.offset)}, 0)`
            : `(${fmt(-pattern.offset)}, 0, 0)`;
          const backVec = plane === 'XY' ? `(0, 0, ${fmt(pattern.offset)})`
            : plane === 'XZ' ? `(0, ${fmt(pattern.offset)}, 0)`
            : `(${fmt(pattern.offset)}, 0, 0)`;
          lines.push(`${name} = ${name}.union(${name}.translate(${axisVec}).mirror("${plane}").translate(${backVec}))`);
        } else {
          lines.push(`${name} = ${name}.union(${name}.mirror("${plane}"))`);
        }
      } else {
        if (pattern.offset !== 0) {
          const axisVec = plane === 'XY' ? `(0, 0, ${fmt(-pattern.offset)})`
            : plane === 'XZ' ? `(0, ${fmt(-pattern.offset)}, 0)`
            : `(${fmt(-pattern.offset)}, 0, 0)`;
          const backVec = plane === 'XY' ? `(0, 0, ${fmt(pattern.offset)})`
            : plane === 'XZ' ? `(0, ${fmt(pattern.offset)}, 0)`
            : `(${fmt(pattern.offset)}, 0, 0)`;
          lines.push(`${name} = ${name}.translate(${axisVec}).mirror("${plane}").translate(${backVec})`);
        } else {
          lines.push(`${name} = ${name}.mirror("${plane}")`);
        }
      }
      break;
    }
    case 'linear': {
      const dirVec = pattern.direction === 'X' ? [1, 0, 0] : pattern.direction === 'Y' ? [0, 1, 0] : [0, 0, 1];
      lines.push(`_base_${name} = ${name}`);
      lines.push(`for _i in range(1, ${pattern.count}):`);
      lines.push(`    ${name} = ${name}.union(_base_${name}.translate((`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[0]},`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[1]},`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[2]})))`);
      break;
    }
    case 'circular': {
      const axisVec = pattern.axis === 'X' ? '(1, 0, 0)' : pattern.axis === 'Y' ? '(0, 1, 0)' : '(0, 0, 1)';
      const angleStep = pattern.fullAngle / pattern.count;
      lines.push(`_base_${name} = ${name}`);
      lines.push(`for _i in range(1, ${pattern.count}):`);
      lines.push(`    ${name} = ${name}.union(_base_${name}.rotate((0,0,0), ${axisVec}, ${fmt(angleStep)} * _i))`);
      break;
    }
  }
  lines.push('');
  return lines;
}

export function generateCode(objects: SceneObject[], sketches: Sketch[] = [], activeFeatureIds?: string[]): string {
  // When activeFeatureIds is provided, use feature-tree ordering and filtering
  if (activeFeatureIds) {
    return generateCodeOrdered(objects, sketches, activeFeatureIds);
  }

  // Legacy fallback: original behavior (insertion order)
  const codegenObjects = objects.filter((o) => !o.importedMeshBase64);
  const hasObjects = codegenObjects.length > 0;
  const nonEmptySketches = sketches.filter((s) => s.entities.length > 0);
  const hasSketches = nonEmptySketches.length > 0;

  if (!hasObjects && !hasSketches) {
    return `import cadquery as cq\n\n# Empty scene — add objects using the toolbar\nresult = cq.Workplane("XY").box(1, 1, 1)\n`;
  }

  const lines: string[] = ['import cadquery as cq', ''];
  const componentStore = getComponentStore();
  const symbolMap = buildSymbolMap(codegenObjects, nonEmptySketches, componentStore.components);

  // Separate sketches into add-mode and cut-mode
  const addSketches = nonEmptySketches.filter((s) => !s.operation || s.operation.mode === 'add');
  const cutSketches = nonEmptySketches.filter((s) => s.operation?.mode === 'cut');
  // Non-operated sketches are 2D-only, excluded from assembly
  const operatedAddSketches = addSketches.filter((s) => s.operation);

  // Generate add-mode sketches (including non-operated for code display)
  for (const sketch of addSketches) {
    lines.push(...generateSketchBase(sketch, symbolMap, nonEmptySketches));
  }

  // Generate objects (primitives)
  const visibleObjects = codegenObjects.filter((o) => o.visible);

  for (const obj of visibleObjects) {
    const objVar = symbolForFeature(symbolMap, obj.id, 'obj');
    lines.push(`# --- ${obj.name} ---`);
    lines.push(generatePrimitive(objVar, obj.params));

    const transformLines = generateTransform(objVar, obj.transform);
    lines.push(...transformLines);

    // Fillet/chamfer on primitives
    lines.push(...generateFilletChamfer(objVar, obj.fillet, obj.chamfer));
    lines.push(...generateShell(objVar, obj.shell));
    lines.push(...generateHoles(objVar, obj.holes));

    lines.push('');
  }

  // Generate cut-mode sketches
  for (const sketch of cutSketches) {
    lines.push(...generateSketchBase(sketch, symbolMap, nonEmptySketches));

    // Apply cut to target
    const targetId = sketch.operation?.mode === 'cut'
      ? (sketch.operation as { cutTargetId?: string }).cutTargetId
      : undefined;
    if (targetId) {
      // Find target symbol (could be another sketch or a primitive)
      const targetSketch = nonEmptySketches.find((s) => s.id === targetId);
      const targetObj = visibleObjects.find((o) => o.id === targetId);
      const targetVar = targetSketch || targetObj
        ? symbolForFeature(symbolMap, targetId, targetSketch ? 'sketch' : 'obj')
        : undefined;
      if (targetVar) {
        lines.push(`${targetVar} = ${targetVar}.cut(${sketchCutterSymbol(symbolMap, sketch.id)})`);
        lines.push('');
      }
    }
  }

  // Collect all generated results for assembly
  const assemblyEntries: Array<{
    varName: string;
    displayName: string;
    isCone: boolean;
  }> = [
    ...operatedAddSketches.map((s) => ({
      varName: symbolForFeature(symbolMap, s.id, 'sketch'),
      displayName: s.name,
      isCone: false,
    })),
    ...visibleObjects.map((o) => ({
      varName: symbolForFeature(symbolMap, o.id, 'obj'),
      displayName: o.name,
      isCone: o.params.type === 'cone',
    })),
  ];

  if (assemblyEntries.length === 0) {
    lines.push('result = cq.Workplane("XY").box(1, 1, 1)');
  } else if (assemblyEntries.length === 1) {
    const entry = assemblyEntries[0];
    if (entry.isCone) {
      lines.push(`result = cq.Workplane("XY").add(${entry.varName})`);
    } else {
      lines.push(`result = ${entry.varName}`);
    }
  } else {
    lines.push('# Assemble all objects');
    lines.push('assy = cq.Assembly()');
    for (const entry of assemblyEntries) {
      if (entry.isCone) {
        lines.push(`assy.add(cq.Workplane("XY").add(${entry.varName}), name="${pyString(entry.displayName)}")`);
      } else {
        lines.push(`assy.add(${entry.varName}, name="${pyString(entry.displayName)}")`);
      }
    }
    lines.push('result = assy.toCompound()');
  }

  lines.push('');
  return lines.join('\n');
}

/** Feature-tree-ordered code generation */
function generateCodeOrdered(objects: SceneObject[], sketches: Sketch[], activeFeatureIds: string[]): string {
  const codegenObjects = objects.filter((o) => !o.importedMeshBase64);
  // Build lookup maps
  const objMap = new Map(codegenObjects.map((o) => [o.id, o]));
  const allSketches = sketches.filter((s) => s.entities.length > 0);
  const sketchMap = new Map(allSketches.map((s) => [s.id, s]));
  const compStore = getComponentStore();
  const symbolMap = buildSymbolMap(codegenObjects, allSketches, compStore.components);

  // Categorize features in order
  const addFeatures: Array<{ type: 'object'; obj: SceneObject } | { type: 'sketch'; sketch: Sketch }> = [];
  const cutSketches: Sketch[] = [];
  const booleanOps: Array<{ toolId: string; opType: string; targetId: string }> = [];
  const splitOps: Array<{ featureId: string; split: SplitOp }> = [];
  const patternOps: Array<{ featureId: string; pattern: PatternOp }> = [];
  const generatedFeatureIds = new Set<string>();
  const assemblyEntries: Array<{ featureId?: string; varName: string; displayName: string; isCone: boolean }> = [];

  // Build feature→component map for visibility checks
  const featureCompMap = compStore.getFeatureComponentMap();

  for (const id of activeFeatureIds) {
    // Skip features in hidden components
    const fCompId = featureCompMap.get(id);
    if (fCompId) {
      const fComp = compStore.getComponentById(fCompId);
      if (fComp && !fComp.visible) continue;
    }

    const obj = objMap.get(id);
    if (obj && obj.visible) {
      addFeatures.push({ type: 'object', obj });
      continue;
    }
    const sketch = sketchMap.get(id);
    if (sketch) {
      if (sketch.operation?.mode === 'cut') {
        cutSketches.push(sketch);
      } else {
        addFeatures.push({ type: 'sketch', sketch });
      }
    }
  }

  if (addFeatures.length === 0 && cutSketches.length === 0) {
    return `import cadquery as cq\n\n# Empty scene — add objects using the toolbar\nresult = cq.Workplane("XY").box(1, 1, 1)\n`;
  }

  const lines: string[] = ['import cadquery as cq', ''];

  // ── Pass 1: Generate all geometry ──
  for (const feature of addFeatures) {
    if (feature.type === 'sketch') {
      lines.push(...generateSketchBase(feature.sketch, symbolMap, allSketches));
      generatedFeatureIds.add(feature.sketch.id);
      if (feature.sketch.operation) {
        assemblyEntries.push({
          featureId: feature.sketch.id,
          varName: symbolForFeature(symbolMap, feature.sketch.id, 'sketch'),
          displayName: feature.sketch.name,
          isCone: false,
        });
      }
    } else {
      const obj = feature.obj;
      const objVar = symbolForFeature(symbolMap, obj.id, 'obj');
      generatedFeatureIds.add(obj.id);
      lines.push(`# --- ${obj.name} ---`);
      lines.push(generatePrimitive(objVar, obj.params));
      lines.push(...generateTransform(objVar, obj.transform));
      lines.push(...generateFilletChamfer(objVar, obj.fillet, obj.chamfer));
      lines.push(...generateShell(objVar, obj.shell));
      lines.push(...generateHoles(objVar, obj.holes));
      lines.push('');

      // Collect boolean ops for pass 2
      if (obj.booleanOp) {
        if (objMap.has(obj.booleanOp.targetId) || sketchMap.has(obj.booleanOp.targetId)) {
          booleanOps.push({
            toolId: obj.id,
            opType: obj.booleanOp.type,
            targetId: obj.booleanOp.targetId,
          });
        }
      }

      // Collect split ops for pass 3
      if (obj.splitOp) {
        splitOps.push({ featureId: obj.id, split: obj.splitOp });
      }

      // Collect pattern ops for pass 3.5
      if (obj.patternOp) {
        patternOps.push({ featureId: obj.id, pattern: obj.patternOp });
      }

      // Only add to assembly if NOT a boolean tool
      if (!obj.booleanOp) {
        assemblyEntries.push({
          featureId: obj.id,
          varName: objVar,
          displayName: obj.name,
          isCone: obj.params.type === 'cone',
        });
      }
    }
  }

  // ── Pass 2: Emit boolean operations ──
  if (booleanOps.length > 0) {
    lines.push('# --- Boolean operations ---');
    for (const op of booleanOps) {
      if (!generatedFeatureIds.has(op.toolId) || !generatedFeatureIds.has(op.targetId)) continue;
      const toolVar = symbolForFeature(symbolMap, op.toolId, 'obj');
      const targetVar = symbolForFeature(
        symbolMap,
        op.targetId,
        sketchMap.has(op.targetId) ? 'sketch' : 'obj',
      );
      const method = op.opType === 'union' ? 'union' : op.opType === 'subtract' ? 'cut' : 'intersect';
      lines.push(`${targetVar} = ${targetVar}.${method}(${toolVar})`);
    }
    lines.push('');
  }

  // ── Pass 3: Emit split operations ──
  for (const { featureId, split } of splitOps) {
    if (!generatedFeatureIds.has(featureId)) continue;
    const featureVar = symbolForFeature(symbolMap, featureId, 'obj');
    lines.push(...generateSplitOp(featureVar, split));
    // Replace assembly name if split produces two halves
    if (split.keepSide === 'both') {
      const idx = assemblyEntries.findIndex((entry) => entry.featureId === featureId);
      if (idx !== -1) {
        const original = assemblyEntries[idx];
        assemblyEntries.splice(
          idx,
          1,
          { ...original, varName: `${featureVar}_pos` },
          { ...original, varName: `${featureVar}_neg` },
        );
      }
    }
  }

  // ── Pass 3.5: Pattern operations ──
  for (const { featureId, pattern } of patternOps) {
    if (!generatedFeatureIds.has(featureId)) continue;
    lines.push(...generatePatternOp(symbolForFeature(symbolMap, featureId, 'obj'), pattern));
  }

  // ── Pass 4: Cut-mode sketches ──
  for (const sketch of cutSketches) {
    lines.push(...generateSketchBase(sketch, symbolMap, allSketches));
    generatedFeatureIds.add(sketch.id);

    const cutTargetId = sketch.operation?.mode === 'cut'
      ? (sketch.operation as { cutTargetId?: string }).cutTargetId
      : undefined;
    if (cutTargetId) {
      if (!generatedFeatureIds.has(cutTargetId)) continue;
      const targetVar = symbolForFeature(
        symbolMap,
        cutTargetId,
        sketchMap.has(cutTargetId) ? 'sketch' : 'obj',
      );
      if (targetVar) {
        lines.push(`${targetVar} = ${targetVar}.cut(${sketchCutterSymbol(symbolMap, sketch.id)})`);
        lines.push('');
      }
    }
  }

  // ── Assembly (with component grouping) ──
  const allComponents = compStore.components;
  const activeIdSet = new Set(activeFeatureIds);

  // Build map: componentId → assembly entries in that component
  const compFeatureEntries = new Map<string, Array<{ featureId?: string; varName: string; displayName: string; isCone: boolean }>>();
  const entriesInComponents = new Set<{ featureId?: string; varName: string; displayName: string; isCone: boolean }>();
  for (const comp of allComponents) {
    if (!comp.visible) continue; // Skip hidden components entirely
    const entries: Array<{ featureId?: string; varName: string; displayName: string; isCone: boolean }> = [];
    for (const fid of comp.featureIds) {
      if (!activeIdSet.has(fid)) continue;
      const matching = assemblyEntries.filter((entry) => entry.featureId === fid);
      for (const entry of matching) {
        entries.push(entry);
        entriesInComponents.add(entry);
      }
    }
    if (entries.length > 0) {
      compFeatureEntries.set(comp.id, entries);
    }
  }

  // Root features = assembly entries not in any component
  const rootEntries = assemblyEntries.filter((entry) => !entriesInComponents.has(entry));

  if (assemblyEntries.length === 0) {
    lines.push('result = cq.Workplane("XY").box(1, 1, 1)');
  } else if (compFeatureEntries.size === 0 && assemblyEntries.length === 1) {
    // Simple case: single result, no components
    const entry = assemblyEntries[0];
    if (entry.isCone) {
      lines.push(`result = cq.Workplane("XY").add(${entry.varName})`);
    } else {
      lines.push(`result = ${entry.varName}`);
    }
  } else {
    lines.push('# Assemble all objects');
    lines.push('assy = cq.Assembly()');

    // Build compId→varName map for mate references
    const compVarNameMap = new Map<string, string>();

    // Add component sub-assemblies
    for (const comp of allComponents) {
      if (!comp.visible) continue;
      const entries = compFeatureEntries.get(comp.id);
      if (!entries || entries.length === 0) continue;

      const compVarName = `${symbolForFeature(symbolMap, comp.id, 'comp')}_assy`;
      compVarNameMap.set(comp.id, comp.name);
      lines.push('');
      lines.push(`# Component: ${comp.name}`);
      lines.push(`${compVarName} = cq.Assembly()`);
      for (const entry of entries) {
        if (entry.isCone) {
          lines.push(`${compVarName}.add(cq.Workplane("XY").add(${entry.varName}), name="${pyString(entry.displayName)}")`);
        } else {
          lines.push(`${compVarName}.add(${entry.varName}, name="${pyString(entry.displayName)}")`);
        }
      }
      // Component transform
      const [tx, ty, tz] = comp.transform.position;
      if (comp.grounded || (tx === 0 && ty === 0 && tz === 0)) {
        lines.push(`assy.add(${compVarName}, name="${pyString(comp.name)}")`);
      } else {
        lines.push(`assy.add(${compVarName}, name="${pyString(comp.name)}", loc=cq.Location(cq.Vector(${fmt(tx)}, ${fmt(ty)}, ${fmt(tz)})))`);
      }
    }

    // Add root features (not in any component)
    for (const entry of rootEntries) {
      if (entry.isCone) {
        lines.push(`assy.add(cq.Workplane("XY").add(${entry.varName}), name="${pyString(entry.displayName)}")`);
      } else {
        lines.push(`assy.add(${entry.varName}, name="${pyString(entry.displayName)}")`);
      }
    }

    // ── Assembly Mates ──
    const ftStore = getFeatureTreeStore();
    const suppressedIdSet = ftStore.suppressedIds;
    const activeMates = getMateStore().mates.filter((m) => !suppressedIdSet.has(m.id));
    if (activeMates.length > 0 && compVarNameMap.size > 0) {
      lines.push('');
      lines.push('# --- Assembly Mates ---');
      for (const mate of activeMates) {
        const c1 = compVarNameMap.get(mate.ref1.componentId);
        const c2 = compVarNameMap.get(mate.ref2.componentId);
        if (!c1 || !c2) continue;
        const r1 = `"${pyString(c1)}@faces@${pyString(mate.ref1.faceSelector)}"`;
        const r2 = `"${pyString(c2)}@faces@${pyString(mate.ref2.faceSelector)}"`;
        switch (mate.type) {
          case 'coincident':
            lines.push(`assy.constrain(${r1}, ${r2}, "Plane")`);
            break;
          case 'concentric':
            lines.push(`assy.constrain("${pyString(c1)}", "${pyString(c2)}", "Axis", param=0)`);
            break;
          case 'distance':
            lines.push(`assy.constrain(${r1}, ${r2}, "Plane", param=${mate.distance})`);
            break;
          case 'angle':
            lines.push(`assy.constrain(${r1}, ${r2}, "Plane", param=${mate.angle})`);
            break;
        }
      }
      lines.push('try:');
      lines.push('    assy.solve()');
      lines.push('except Exception:');
      lines.push('    pass  # Fallback to manual positioning if solve fails');
    }

    lines.push('result = assy.toCompound()');
  }

  lines.push('');
  return lines.join('\n');
}
