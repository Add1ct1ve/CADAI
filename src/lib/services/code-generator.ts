import type { SceneObject, PrimitiveParams, CadTransform, Sketch, SketchEntity, EdgeSelector, FilletParams, ChamferParams, ShellParams, HoleParams, RevolveParams, SketchPlane, BooleanOp, SplitOp, SplitPlane } from '$lib/types/cad';

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
  lines.push(`    cq.Workplane("${pathSketch.plane}")`);
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

function generateSketchBase(sketch: Sketch, allSketches?: Sketch[]): string[] {
  const lines: string[] = [];
  const constraintCount = (sketch.constraints ?? []).length;
  const constraintInfo = constraintCount > 0 ? ` [${constraintCount} constraint${constraintCount !== 1 ? 's' : ''}]` : '';
  lines.push(`# --- ${sketch.name} (${sketch.plane} plane) ---${constraintInfo}`);

  const op = sketch.operation;
  const varName = op?.mode === 'cut' ? `${sketch.name}_cutter` : sketch.name;

  // For sweep, generate path sketch inline first
  if (op?.type === 'sweep' && allSketches) {
    const pathSketch = allSketches.find(s => s.id === op.pathSketchId);
    if (pathSketch) {
      lines.push(...generatePathWire(`${sketch.name}_path`, pathSketch));
    }
  }

  lines.push(`${varName} = (`);
  lines.push(`    cq.Workplane("${sketch.plane}")`);

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
    lines.push(`    .sweep(${sketch.name}_path)`);
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

export function generateCode(objects: SceneObject[], sketches: Sketch[] = [], activeFeatureIds?: string[]): string {
  // When activeFeatureIds is provided, use feature-tree ordering and filtering
  if (activeFeatureIds) {
    return generateCodeOrdered(objects, sketches, activeFeatureIds);
  }

  // Legacy fallback: original behavior (insertion order)
  const hasObjects = objects.length > 0;
  const nonEmptySketches = sketches.filter((s) => s.entities.length > 0);
  const hasSketches = nonEmptySketches.length > 0;

  if (!hasObjects && !hasSketches) {
    return `import cadquery as cq\n\n# Empty scene — add objects using the toolbar\nresult = cq.Workplane("XY").box(1, 1, 1)\n`;
  }

  const lines: string[] = ['import cadquery as cq', ''];

  // Separate sketches into add-mode and cut-mode
  const addSketches = nonEmptySketches.filter((s) => !s.operation || s.operation.mode === 'add');
  const cutSketches = nonEmptySketches.filter((s) => s.operation?.mode === 'cut');
  // Non-operated sketches are 2D-only, excluded from assembly
  const operatedAddSketches = addSketches.filter((s) => s.operation);

  // Generate add-mode sketches (including non-operated for code display)
  for (const sketch of addSketches) {
    lines.push(...generateSketchBase(sketch, nonEmptySketches));
  }

  // Generate objects (primitives)
  const visibleObjects = objects.filter((o) => o.visible);

  for (const obj of visibleObjects) {
    lines.push(`# --- ${obj.name} ---`);
    lines.push(generatePrimitive(obj.name, obj.params));

    const transformLines = generateTransform(obj.name, obj.transform);
    lines.push(...transformLines);

    // Fillet/chamfer on primitives
    lines.push(...generateFilletChamfer(obj.name, obj.fillet, obj.chamfer));
    lines.push(...generateShell(obj.name, obj.shell));
    lines.push(...generateHoles(obj.name, obj.holes));

    lines.push('');
  }

  // Generate cut-mode sketches
  for (const sketch of cutSketches) {
    lines.push(...generateSketchBase(sketch, nonEmptySketches));

    // Apply cut to target
    const targetId = sketch.operation?.mode === 'cut' ? (sketch.operation as any).cutTargetId : undefined;
    if (targetId) {
      // Find target name (could be another sketch or a primitive)
      const targetSketch = nonEmptySketches.find((s) => s.id === targetId);
      const targetObj = visibleObjects.find((o) => o.id === targetId);
      const targetName = targetSketch?.name ?? targetObj?.name;
      if (targetName) {
        lines.push(`${targetName} = ${targetName}.cut(${sketch.name}_cutter)`);
        lines.push('');
      }
    }
  }

  // Collect all named results for assembly
  // Only include operated add-sketches and visible primitives
  const allNames: string[] = [
    ...operatedAddSketches.map((s) => s.name),
    ...visibleObjects.map((o) => o.name),
  ];

  if (allNames.length === 0) {
    lines.push('result = cq.Workplane("XY").box(1, 1, 1)');
  } else if (allNames.length === 1) {
    const name = allNames[0];
    // Check if it's a cone (Solid type) that needs wrapping
    const obj = visibleObjects.find((o) => o.name === name);
    if (obj && obj.params.type === 'cone') {
      lines.push(`result = cq.Workplane("XY").add(${name})`);
    } else {
      lines.push(`result = ${name}`);
    }
  } else {
    lines.push('# Assemble all objects');
    lines.push('assy = cq.Assembly()');
    for (const name of allNames) {
      const obj = visibleObjects.find((o) => o.name === name);
      if (obj && obj.params.type === 'cone') {
        lines.push(`assy.add(cq.Workplane("XY").add(${name}), name="${name}")`);
      } else {
        lines.push(`assy.add(${name}, name="${name}")`);
      }
    }
    lines.push('result = assy.toCompound()');
  }

  lines.push('');
  return lines.join('\n');
}

/** Feature-tree-ordered code generation */
function generateCodeOrdered(objects: SceneObject[], sketches: Sketch[], activeFeatureIds: string[]): string {
  // Build lookup maps
  const objMap = new Map(objects.map((o) => [o.id, o]));
  const allSketches = sketches.filter((s) => s.entities.length > 0);
  const sketchMap = new Map(allSketches.map((s) => [s.id, s]));

  // Categorize features in order
  const addFeatures: Array<{ type: 'object'; obj: SceneObject } | { type: 'sketch'; sketch: Sketch }> = [];
  const cutSketches: Sketch[] = [];
  const booleanOps: Array<{ toolName: string; opType: string; targetName: string }> = [];
  const splitOps: Array<{ name: string; split: SplitOp; obj: SceneObject }> = [];
  const booleanToolIds = new Set<string>();

  for (const id of activeFeatureIds) {
    const obj = objMap.get(id);
    if (obj && obj.visible) {
      if (obj.booleanOp) {
        booleanToolIds.add(obj.id);
      }
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
  const assemblyNames: string[] = [];
  const allVisibleObjects: SceneObject[] = [];

  for (const feature of addFeatures) {
    if (feature.type === 'sketch') {
      lines.push(...generateSketchBase(feature.sketch, allSketches));
      if (feature.sketch.operation) {
        assemblyNames.push(feature.sketch.name);
      }
    } else {
      const obj = feature.obj;
      allVisibleObjects.push(obj);
      lines.push(`# --- ${obj.name} ---`);
      lines.push(generatePrimitive(obj.name, obj.params));
      lines.push(...generateTransform(obj.name, obj.transform));
      lines.push(...generateFilletChamfer(obj.name, obj.fillet, obj.chamfer));
      lines.push(...generateShell(obj.name, obj.shell));
      lines.push(...generateHoles(obj.name, obj.holes));
      lines.push('');

      // Collect boolean ops for pass 2
      if (obj.booleanOp) {
        const targetObj = objMap.get(obj.booleanOp.targetId);
        const targetSketch = sketchMap.get(obj.booleanOp.targetId);
        const targetName = targetObj?.name ?? targetSketch?.name;
        if (targetName) {
          booleanOps.push({ toolName: obj.name, opType: obj.booleanOp.type, targetName });
        }
      }

      // Collect split ops for pass 3
      if (obj.splitOp) {
        splitOps.push({ name: obj.name, split: obj.splitOp, obj });
      }

      // Only add to assembly if NOT a boolean tool
      if (!obj.booleanOp) {
        assemblyNames.push(obj.name);
      }
    }
  }

  // ── Pass 2: Emit boolean operations ──
  if (booleanOps.length > 0) {
    lines.push('# --- Boolean operations ---');
    for (const op of booleanOps) {
      const method = op.opType === 'union' ? 'union' : op.opType === 'subtract' ? 'cut' : 'intersect';
      lines.push(`${op.targetName} = ${op.targetName}.${method}(${op.toolName})`);
    }
    lines.push('');
  }

  // ── Pass 3: Emit split operations ──
  for (const { name, split, obj } of splitOps) {
    lines.push(...generateSplitOp(name, split));
    // Replace assembly name if split produces two halves
    if (split.keepSide === 'both') {
      const idx = assemblyNames.indexOf(name);
      if (idx !== -1) {
        assemblyNames.splice(idx, 1, `${name}_pos`, `${name}_neg`);
      }
    }
  }

  // ── Pass 4: Cut-mode sketches ──
  for (const sketch of cutSketches) {
    lines.push(...generateSketchBase(sketch, allSketches));

    const cutTargetId = sketch.operation?.mode === 'cut' ? (sketch.operation as any).cutTargetId : undefined;
    if (cutTargetId) {
      const targetSketch = sketchMap.get(cutTargetId);
      const targetObj = objMap.get(cutTargetId);
      const targetName = targetSketch?.name ?? targetObj?.name;
      if (targetName) {
        lines.push(`${targetName} = ${targetName}.cut(${sketch.name}_cutter)`);
        lines.push('');
      }
    }
  }

  // ── Assembly ──
  if (assemblyNames.length === 0) {
    lines.push('result = cq.Workplane("XY").box(1, 1, 1)');
  } else if (assemblyNames.length === 1) {
    const name = assemblyNames[0];
    const obj = allVisibleObjects.find((o) => o.name === name);
    if (obj && obj.params.type === 'cone') {
      lines.push(`result = cq.Workplane("XY").add(${name})`);
    } else {
      lines.push(`result = ${name}`);
    }
  } else {
    lines.push('# Assemble all objects');
    lines.push('assy = cq.Assembly()');
    for (const name of assemblyNames) {
      const obj = allVisibleObjects.find((o) => o.name === name);
      if (obj && obj.params.type === 'cone') {
        lines.push(`assy.add(cq.Workplane("XY").add(${name}), name="${name}")`);
      } else {
        lines.push(`assy.add(${name}, name="${name}")`);
      }
    }
    lines.push('result = assy.toCompound()');
  }

  lines.push('');
  return lines.join('\n');
}
