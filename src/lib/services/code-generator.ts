import type { SceneObject, PrimitiveParams, CadTransform, Sketch, SketchEntity, EdgeSelector, FilletParams, ChamferParams } from '$lib/types/cad';

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

function generateSketchBase(sketch: Sketch): string[] {
  const lines: string[] = [];
  const constraintCount = (sketch.constraints ?? []).length;
  const constraintInfo = constraintCount > 0 ? ` [${constraintCount} constraint${constraintCount !== 1 ? 's' : ''}]` : '';
  lines.push(`# --- ${sketch.name} (${sketch.plane} plane) ---${constraintInfo}`);

  const varName = sketch.extrude?.mode === 'cut' ? `${sketch.name}_cutter` : sketch.name;

  lines.push(`${varName} = (`);
  lines.push(`    cq.Workplane("${sketch.plane}")`);

  for (const entity of sketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }

  if (sketch.extrude) {
    lines.push(`    .extrude(${fmt(sketch.extrude.distance)})`);
  }

  lines.push(`)`);

  // Fillet/chamfer (only if extruded — 2D sketches can't have these)
  if (sketch.extrude) {
    lines.push(...generateFilletChamfer(varName, sketch.fillet, sketch.chamfer));
  }

  lines.push('');
  return lines;
}

export function generateCode(objects: SceneObject[], sketches: Sketch[] = []): string {
  const hasObjects = objects.length > 0;
  const nonEmptySketches = sketches.filter((s) => s.entities.length > 0);
  const hasSketches = nonEmptySketches.length > 0;

  if (!hasObjects && !hasSketches) {
    return `import cadquery as cq\n\n# Empty scene — add objects using the toolbar\nresult = cq.Workplane("XY").box(1, 1, 1)\n`;
  }

  const lines: string[] = ['import cadquery as cq', ''];

  // Separate sketches into add-mode and cut-mode
  const addSketches = nonEmptySketches.filter((s) => !s.extrude || s.extrude.mode === 'add');
  const cutSketches = nonEmptySketches.filter((s) => s.extrude?.mode === 'cut');
  // Non-extruded sketches are 2D-only, excluded from assembly
  const extrudedAddSketches = addSketches.filter((s) => s.extrude);

  // Generate add-mode sketches (including non-extruded for code display)
  for (const sketch of addSketches) {
    lines.push(...generateSketchBase(sketch));
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

    lines.push('');
  }

  // Generate cut-mode sketches
  for (const sketch of cutSketches) {
    lines.push(...generateSketchBase(sketch));

    // Apply cut to target
    const targetId = sketch.extrude?.cutTargetId;
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
  // Only include extruded add-sketches and visible primitives
  const allNames: string[] = [
    ...extrudedAddSketches.map((s) => s.name),
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
