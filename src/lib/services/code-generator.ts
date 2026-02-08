import type { SceneObject, PrimitiveParams, CadTransform, Sketch, SketchEntity } from '$lib/types/cad';

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

function generateSketch(sketch: Sketch): string[] {
  if (sketch.entities.length === 0) return [];

  const lines: string[] = [];
  lines.push(`# --- ${sketch.name} (${sketch.plane} plane) ---`);
  lines.push(`${sketch.name} = (`);
  lines.push(`    cq.Workplane("${sketch.plane}")`);

  for (const entity of sketch.entities) {
    lines.push(...generateSketchEntity(entity));
  }

  lines.push(`)`);
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

  // Generate sketches
  for (const sketch of nonEmptySketches) {
    lines.push(...generateSketch(sketch));
  }

  // Generate objects
  const visibleObjects = objects.filter((o) => o.visible);

  for (const obj of visibleObjects) {
    lines.push(`# --- ${obj.name} ---`);
    lines.push(generatePrimitive(obj.name, obj.params));

    const transformLines = generateTransform(obj.name, obj.transform);
    lines.push(...transformLines);

    lines.push('');
  }

  // Collect all named results
  const allNames: string[] = [
    ...nonEmptySketches.map((s) => s.name),
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
