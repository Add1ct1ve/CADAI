import type { SceneObject, PrimitiveParams, CadTransform, Sketch, SketchEntity, EdgeSelector, FilletParams, ChamferParams, ShellParams, HoleParams, RevolveParams, SketchPlane, BooleanOp, SplitOp, SplitPlane, PatternOp, DatumPlane, Component } from '$lib/types/cad';
import { getDatumStore } from '$lib/stores/datum.svelte';
import { getComponentStore } from '$lib/stores/component.svelte';
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
      return `${name} = Box(${params.width}, ${params.depth}, ${params.height})`;
    case 'cylinder':
      return `${name} = Cylinder(radius=${params.radius}, height=${params.height})`;
    case 'sphere':
      return `${name} = Sphere(radius=${params.radius})`;
    case 'cone':
      return `${name} = Cone(bottom_radius=${params.bottomRadius}, top_radius=${params.topRadius}, height=${params.height})`;
  }
}

function generateTransform(name: string, transform: CadTransform): string[] {
  const lines: string[] = [];
  const [x, y, z] = transform.position;
  const [rx, ry, rz] = transform.rotation;

  // Rotations — Build123d uses .rotate(Axis, angle)
  if (rx !== 0) lines.push(`${name} = ${name}.rotate(Axis.X, ${rx})`);
  if (ry !== 0) lines.push(`${name} = ${name}.rotate(Axis.Y, ${ry})`);
  if (rz !== 0) lines.push(`${name} = ${name}.rotate(Axis.Z, ${rz})`);

  // Translation — Build123d .move(Location(...))
  if (x !== 0 || y !== 0 || z !== 0) {
    lines.push(`${name} = ${name}.move(Location((${x}, ${y}, ${z})))`);
  }

  return lines;
}

function fmt(n: number): string {
  // Format number, removing trailing zeros
  return parseFloat(n.toFixed(4)).toString();
}

function edgeSelectorExpr(name: string, selector: EdgeSelector): string {
  if (selector.startsWith('edge:')) {
    const id = parseInt(selector.split(':')[1], 10);
    return `${name}.edges()[${id}]`;
  }
  switch (selector) {
    case 'all':
      return `${name}.edges()`;
    case 'top':
      return `${name}.edges().sort_by(Axis.Z)[-1:]`;
    case 'bottom':
      return `${name}.edges().sort_by(Axis.Z)[:1]`;
    case 'vertical':
      return `${name}.edges().filter_by(Axis.Z)`;
    default:
      return `${name}.edges()`;
  }
}

function generateFilletChamfer(name: string, fillet?: FilletParams, chamfer?: ChamferParams): string[] {
  const lines: string[] = [];
  if (fillet) {
    const edges = edgeSelectorExpr(name, fillet.edges);
    lines.push(`${name} = fillet(${edges}, radius=${fmt(fillet.radius)})`);
  }
  if (chamfer) {
    const edges = edgeSelectorExpr(name, chamfer.edges);
    lines.push(`${name} = chamfer(${edges}, length=${fmt(chamfer.distance)})`);
  }
  return lines;
}

function generateShell(name: string, shell?: ShellParams): string[] {
  if (!shell) return [];

  let faceExpr: string;

  if (shell.face.startsWith('face:')) {
    const id = parseInt(shell.face.split(':')[1], 10);
    faceExpr = `${name}.faces()[${id}]`;
  } else {
    // Build123d face selection
    const faceMap: Record<string, string> = {
      '>Z': `${name}.faces().sort_by(Axis.Z)[-1]`,
      '<Z': `${name}.faces().sort_by(Axis.Z)[0]`,
      '>Y': `${name}.faces().sort_by(Axis.Y)[-1]`,
      '<Y': `${name}.faces().sort_by(Axis.Y)[0]`,
      '>X': `${name}.faces().sort_by(Axis.X)[-1]`,
      '<X': `${name}.faces().sort_by(Axis.X)[0]`,
    };
    faceExpr = faceMap[shell.face] || `${name}.faces().sort_by(Axis.Z)[-1]`;
  }

  return [`${name} = offset_3d(${name}, openings=${faceExpr}, amount=${fmt(-Math.abs(shell.thickness))})`];
}

function generateHoles(name: string, holes?: HoleParams[]): string[] {
  if (!holes?.length) return [];
  const lines: string[] = [];
  for (const hole of holes) {
    const radius = hole.diameter / 2;

    // Build123d: subtract cylinders/cones in algebra mode
    switch (hole.holeType) {
      case 'through':
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cylinder(radius=${fmt(radius)}, height=${fmt(200)})`);
        break;
      case 'blind':
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cylinder(radius=${fmt(radius)}, height=${fmt(hole.depth ?? 5)})`);
        break;
      case 'counterbore': {
        const cboreR = (hole.cboreDiameter ?? hole.diameter * 1.6) / 2;
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cylinder(radius=${fmt(radius)}, height=${fmt(200)})`);
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cylinder(radius=${fmt(cboreR)}, height=${fmt(hole.cboreDepth ?? 3)})`);
        break;
      }
      case 'countersink': {
        const cskR = (hole.cskDiameter ?? hole.diameter * 2) / 2;
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cylinder(radius=${fmt(radius)}, height=${fmt(200)})`);
        lines.push(`${name} = ${name} - Pos(${fmt(hole.position[0])}, ${fmt(hole.position[1])}, 0) * Cone(bottom_radius=${fmt(cskR)}, top_radius=${fmt(radius)}, height=${fmt(cskR - radius)})`);
        break;
      }
    }
  }
  return lines;
}

/** Map sketch axis direction to Build123d Axis for revolve */
function revolveAxisObj(op: RevolveParams): string {
  // Build123d revolve uses Axis objects
  return op.axisDirection === 'X' ? 'Axis.X' : 'Axis.Y';
}

/** Generate a Build123d wire from a path sketch (for sweep) */
function generatePathWire(varName: string, pathSketch: Sketch): string[] {
  const lines: string[] = [];
  const plane = planeString(pathSketch.plane, pathSketch.origin);
  lines.push(`with BuildLine(${plane}) as _${varName}_line:`);
  for (const entity of pathSketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }
  lines.push(`${varName} = _${varName}_line.line`);
  return lines;
}

function generateSketchEntity(entity: SketchEntity): string[] {
  const lines: string[] = [];
  switch (entity.type) {
    case 'line':
      lines.push(`        Line((${fmt(entity.start[0])}, ${fmt(entity.start[1])}), (${fmt(entity.end[0])}, ${fmt(entity.end[1])}))`);
      break;
    case 'rectangle': {
      const w = Math.abs(entity.corner2[0] - entity.corner1[0]);
      const h = Math.abs(entity.corner2[1] - entity.corner1[1]);
      const cx = (entity.corner1[0] + entity.corner2[0]) / 2;
      const cy = (entity.corner1[1] + entity.corner2[1]) / 2;
      if (cx !== 0 || cy !== 0) {
        lines.push(`        with Locations((${fmt(cx)}, ${fmt(cy)})):`);
        lines.push(`            Rectangle(${fmt(w)}, ${fmt(h)})`);
      } else {
        lines.push(`        Rectangle(${fmt(w)}, ${fmt(h)})`);
      }
      break;
    }
    case 'circle':
      if (entity.center[0] !== 0 || entity.center[1] !== 0) {
        lines.push(`        with Locations((${fmt(entity.center[0])}, ${fmt(entity.center[1])})):`);
        lines.push(`            Circle(radius=${fmt(entity.radius)})`);
      } else {
        lines.push(`        Circle(radius=${fmt(entity.radius)})`);
      }
      break;
    case 'arc':
      lines.push(`        ThreePointArc((${fmt(entity.start[0])}, ${fmt(entity.start[1])}), (${fmt(entity.mid[0])}, ${fmt(entity.mid[1])}), (${fmt(entity.end[0])}, ${fmt(entity.end[1])}))`);
      break;
    case 'spline': {
      if (entity.points.length >= 2) {
        const ptsStr = entity.points.map(p => `(${fmt(p[0])}, ${fmt(p[1])})`).join(', ');
        lines.push(`        Spline(${ptsStr})`);
      }
      break;
    }
    case 'bezier': {
      if (entity.points.length >= 2) {
        const ptsStr = entity.points.map(p => `(${fmt(p[0])}, ${fmt(p[1])})`).join(', ');
        lines.push(`        Bezier(${ptsStr})`);
      }
      break;
    }
  }
  return lines;
}

/**
 * Generate Build123d Plane string for a sketch plane (standard or datum).
 */
function planeString(plane: SketchPlane, origin: [number, number, number]): string {
  if (plane === 'XY' || plane === 'XZ' || plane === 'YZ') {
    return `Plane.${plane}`;
  }

  // Handle face-derived planes: face:objectId:faceId:nx:ny:nz
  if (plane.startsWith('face:')) {
    const parts = plane.split(':');
    if (parts.length >= 6) {
      const nx = parseFloat(parts[3]);
      const ny = parseFloat(parts[4]);
      const nz = parseFloat(parts[5]);
      return `Plane(origin=(${fmt(origin[0])}, ${fmt(origin[1])}, ${fmt(origin[2])}), z_dir=(${fmt(nx)}, ${fmt(ny)}, ${fmt(nz)}))`;
    }
  }

  const datumPlane = getDatumStore().getDatumPlaneById(plane);
  if (!datumPlane) return 'Plane.XY';

  if (datumPlane.definition.type === 'offset') {
    return `Plane.${datumPlane.definition.basePlane}.offset(${fmt(datumPlane.definition.offset)})`;
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

  return `Plane(origin=(${fmt(p1[0])}, ${fmt(p1[1])}, ${fmt(p1[2])}), z_dir=(${fmt(normal[0])}, ${fmt(normal[1])}, ${fmt(normal[2])}))`;
}

function generateLoftBase(sketch: Sketch, symbolMap: SymbolMap, allSketches: Sketch[]): string[] {
  const lines: string[] = [];
  const op = sketch.operation;
  if (op?.type !== 'loft') return lines;

  const baseVarName = symbolForFeature(symbolMap, sketch.id, 'sketch');
  const varName = op.mode === 'cut' ? sketchCutterSymbol(symbolMap, sketch.id) : baseVarName;

  lines.push(`# --- ${sketch.name} (Loft) ---`);

  // Gather all profile sketches: the base sketch + additional profile sketch IDs
  const profileSketches = [sketch, ...op.profileSketchIds
    .map(id => allSketches.find(s => s.id === id))
    .filter(Boolean) as Sketch[]];

  lines.push(`with BuildPart() as _${varName}_builder:`);
  for (let i = 0; i < profileSketches.length; i++) {
    const ps = profileSketches[i];
    const plane = planeString(ps.plane, ps.origin);
    lines.push(`    with BuildSketch(${plane}) as _loft_sk${i}:`);
    for (const entity of ps.entities) {
      lines.push(...generateSketchEntity(entity));
    }
  }

  const ruledArg = op.ruled ? ', ruled=True' : '';
  const sections = profileSketches.map((_, i) => `_loft_sk${i}.sketch`).join(', ');
  lines.push(`    loft([${sections}]${ruledArg})`);
  lines.push(`${varName} = _${varName}_builder.part`);

  // Post-processing
  if (op) {
    lines.push(...generateFilletChamfer(varName, sketch.fillet, sketch.chamfer));
    lines.push(...generateShell(varName, sketch.shell));
    lines.push(...generateHoles(varName, sketch.holes));
  }

  lines.push('');
  return lines;
}

function generateSketchBase(sketch: Sketch, symbolMap: SymbolMap, allSketches?: Sketch[]): string[] {
  if (sketch.operation?.type === 'loft') {
    return generateLoftBase(sketch, symbolMap, allSketches ?? []);
  }

  const lines: string[] = [];
  const constraintCount = (sketch.constraints ?? []).length;
  const constraintInfo = constraintCount > 0 ? ` [${constraintCount} constraint${constraintCount !== 1 ? 's' : ''}]` : '';
  lines.push(`# --- ${sketch.name} (${sketch.plane} plane) ---${constraintInfo}`);

  const op = sketch.operation;
  const baseVarName = symbolForFeature(symbolMap, sketch.id, 'sketch');
  const varName = op?.mode === 'cut' ? sketchCutterSymbol(symbolMap, sketch.id) : baseVarName;

  // For sweep, generate path first
  if (op?.type === 'sweep' && allSketches) {
    const pathSketch = allSketches.find(s => s.id === op.pathSketchId);
    if (pathSketch) {
      lines.push(...generatePathWire(sketchPathSymbol(symbolMap, sketch.id), pathSketch));
    }
  }

  const plane = planeString(sketch.plane, sketch.origin);

  lines.push(`with BuildPart() as _${varName}_builder:`);
  lines.push(`    with BuildSketch(${plane}) as _sk:`);

  for (const entity of sketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }

  if (op?.type === 'extrude') {
    const taperArg = op.taper ? `, taper=${fmt(op.taper)}` : '';
    lines.push(`    extrude(amount=${fmt(op.distance)}${taperArg})`);
  } else if (op?.type === 'revolve') {
    lines.push(`    revolve(axis=${revolveAxisObj(op)}, revolution_arc=${fmt(op.angle)})`);
  } else if (op?.type === 'sweep') {
    lines.push(`    sweep(path=${sketchPathSymbol(symbolMap, sketch.id)})`);
  }

  lines.push(`${varName} = _${varName}_builder.part`);

  // Post-processing (only if 3D operation set)
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

  const cutterSize = 10000;
  // Build123d: create box at correct position and subtract
  if (split.keepSide === 'positive') {
    lines.push(`_split_cutter = Box(${cutterSize}, ${cutterSize}, ${cutterSize})`);
    if (split.plane === 'XY') {
      lines.push(`_split_cutter = _split_cutter.move(Location((0, 0, ${fmt(split.offset - cutterSize/2)})))`);
    } else if (split.plane === 'XZ') {
      lines.push(`_split_cutter = _split_cutter.move(Location((0, ${fmt(split.offset - cutterSize/2)}, 0)))`);
    } else {
      lines.push(`_split_cutter = _split_cutter.move(Location((${fmt(split.offset - cutterSize/2)}, 0, 0)))`);
    }
    lines.push(`${name} = ${name} - _split_cutter`);
  } else if (split.keepSide === 'negative') {
    lines.push(`_split_cutter = Box(${cutterSize}, ${cutterSize}, ${cutterSize})`);
    if (split.plane === 'XY') {
      lines.push(`_split_cutter = _split_cutter.move(Location((0, 0, ${fmt(split.offset + cutterSize/2)})))`);
    } else if (split.plane === 'XZ') {
      lines.push(`_split_cutter = _split_cutter.move(Location((0, ${fmt(split.offset + cutterSize/2)}, 0)))`);
    } else {
      lines.push(`_split_cutter = _split_cutter.move(Location((${fmt(split.offset + cutterSize/2)}, 0, 0)))`);
    }
    lines.push(`${name} = ${name} - _split_cutter`);
  } else {
    // Keep both halves
    lines.push(`_split_neg = Box(${cutterSize}, ${cutterSize}, ${cutterSize})`);
    lines.push(`_split_pos = Box(${cutterSize}, ${cutterSize}, ${cutterSize})`);
    if (split.plane === 'XY') {
      lines.push(`_split_neg = _split_neg.move(Location((0, 0, ${fmt(split.offset - cutterSize/2)})))`);
      lines.push(`_split_pos = _split_pos.move(Location((0, 0, ${fmt(split.offset + cutterSize/2)})))`);
    } else if (split.plane === 'XZ') {
      lines.push(`_split_neg = _split_neg.move(Location((0, ${fmt(split.offset - cutterSize/2)}, 0)))`);
      lines.push(`_split_pos = _split_pos.move(Location((0, ${fmt(split.offset + cutterSize/2)}, 0)))`);
    } else {
      lines.push(`_split_neg = _split_neg.move(Location((${fmt(split.offset - cutterSize/2)}, 0, 0)))`);
      lines.push(`_split_pos = _split_pos.move(Location((${fmt(split.offset + cutterSize/2)}, 0, 0)))`);
    }
    lines.push(`${name}_pos = ${name} - _split_neg`);
    lines.push(`${name}_neg = ${name} - _split_pos`);
  }
  lines.push('');
  return lines;
}

function generatePatternOp(name: string, pattern: PatternOp): string[] {
  const lines: string[] = [];
  lines.push(`# --- Pattern: ${pattern.type} ---`);

  switch (pattern.type) {
    case 'mirror': {
      const planeMap: Record<string, string> = { 'XY': 'Plane.XY', 'XZ': 'Plane.XZ', 'YZ': 'Plane.YZ' };
      const mirrorPlane = planeMap[pattern.plane] || 'Plane.XY';
      if (pattern.offset !== 0) {
        const offsetPlane = `${mirrorPlane}.offset(${fmt(pattern.offset)})`;
        if (pattern.keepOriginal) {
          lines.push(`${name} = ${name} + mirror(${name}, about=${offsetPlane})`);
        } else {
          lines.push(`${name} = mirror(${name}, about=${offsetPlane})`);
        }
      } else {
        if (pattern.keepOriginal) {
          lines.push(`${name} = ${name} + mirror(${name}, about=${mirrorPlane})`);
        } else {
          lines.push(`${name} = mirror(${name}, about=${mirrorPlane})`);
        }
      }
      break;
    }
    case 'linear': {
      const dirVec = pattern.direction === 'X' ? [1, 0, 0] : pattern.direction === 'Y' ? [0, 1, 0] : [0, 0, 1];
      lines.push(`_base_${name} = ${name}`);
      lines.push(`for _i in range(1, ${pattern.count}):`);
      lines.push(`    ${name} = ${name} + _base_${name}.move(Location((`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[0]},`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[1]},`);
      lines.push(`        ${fmt(pattern.spacing)} * _i * ${dirVec[2]})))`);
      break;
    }
    case 'circular': {
      const axisObj = pattern.axis === 'X' ? 'Axis.X' : pattern.axis === 'Y' ? 'Axis.Y' : 'Axis.Z';
      const angleStep = pattern.fullAngle / pattern.count;
      lines.push(`_base_${name} = ${name}`);
      lines.push(`for _i in range(1, ${pattern.count}):`);
      lines.push(`    ${name} = ${name} + _base_${name}.rotate(${axisObj}, ${fmt(angleStep)} * _i)`);
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
    return `from build123d import *\n\n# Empty scene — add objects using the toolbar\nresult = Box(1, 1, 1)\n`;
  }

  const lines: string[] = ['from build123d import *', ''];
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
        lines.push(`${targetVar} = ${targetVar} - ${sketchCutterSymbol(symbolMap, sketch.id)}`);
        lines.push('');
      }
    }
  }

  // Collect all generated results for assembly
  const assemblyEntries: Array<{
    varName: string;
    displayName: string;
  }> = [
    ...operatedAddSketches.map((s) => ({
      varName: symbolForFeature(symbolMap, s.id, 'sketch'),
      displayName: s.name,
    })),
    ...visibleObjects.map((o) => ({
      varName: symbolForFeature(symbolMap, o.id, 'obj'),
      displayName: o.name,
    })),
  ];

  if (assemblyEntries.length === 0) {
    lines.push('result = Box(1, 1, 1)');
  } else if (assemblyEntries.length === 1) {
    const entry = assemblyEntries[0];
    lines.push(`result = ${entry.varName}`);
  } else {
    lines.push('# Assemble all objects');
    lines.push('result = Compound(children=[');
    for (const entry of assemblyEntries) {
      lines.push(`    ${entry.varName},`);
    }
    lines.push('])');
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
  const assemblyEntries: Array<{ featureId?: string; varName: string; displayName: string }> = [];

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
    return `from build123d import *\n\n# Empty scene — add objects using the toolbar\nresult = Box(1, 1, 1)\n`;
  }

  const lines: string[] = ['from build123d import *', ''];

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
      const operator = op.opType === 'union' ? '+' : op.opType === 'subtract' ? '-' : '&';
      lines.push(`${targetVar} = ${targetVar} ${operator} ${toolVar}`);
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
        lines.push(`${targetVar} = ${targetVar} - ${sketchCutterSymbol(symbolMap, sketch.id)}`);
        lines.push('');
      }
    }
  }

  // ── Assembly (with component grouping) ──
  const allComponents = compStore.components;
  const activeIdSet = new Set(activeFeatureIds);

  // Build map: componentId → assembly entries in that component
  const compFeatureEntries = new Map<string, Array<{ featureId?: string; varName: string; displayName: string }>>();
  const entriesInComponents = new Set<{ featureId?: string; varName: string; displayName: string }>();
  for (const comp of allComponents) {
    if (!comp.visible) continue; // Skip hidden components entirely
    const entries: Array<{ featureId?: string; varName: string; displayName: string }> = [];
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
    lines.push('result = Box(1, 1, 1)');
  } else if (compFeatureEntries.size === 0 && assemblyEntries.length === 1) {
    // Simple case: single result, no components
    const entry = assemblyEntries[0];
    lines.push(`result = ${entry.varName}`);
  } else {
    lines.push('# Assemble all objects');

    // Collect all parts for final Compound
    const compoundParts: string[] = [];

    // Add component sub-assemblies
    for (const comp of allComponents) {
      if (!comp.visible) continue;
      const entries = compFeatureEntries.get(comp.id);
      if (!entries || entries.length === 0) continue;

      const compVarName = `${symbolForFeature(symbolMap, comp.id, 'comp')}_assy`;
      lines.push('');
      lines.push(`# Component: ${comp.name}`);
      lines.push(`${compVarName} = Compound(children=[`);
      for (const entry of entries) {
        lines.push(`    ${entry.varName},`);
      }
      lines.push(`])`);

      // Component transform
      const [tx, ty, tz] = comp.transform.position;
      if (!comp.grounded && (tx !== 0 || ty !== 0 || tz !== 0)) {
        lines.push(`${compVarName} = ${compVarName}.move(Location((${fmt(tx)}, ${fmt(ty)}, ${fmt(tz)})))`);
      }
      compoundParts.push(compVarName);
    }

    // Add root features (not in any component)
    for (const entry of rootEntries) {
      compoundParts.push(entry.varName);
    }

    lines.push('');
    lines.push('result = Compound(children=[');
    for (const part of compoundParts) {
      lines.push(`    ${part},`);
    }
    lines.push('])');
  }

  lines.push('');
  return lines.join('\n');
}
