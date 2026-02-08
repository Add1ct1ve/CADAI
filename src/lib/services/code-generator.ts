import type { SceneObject, PrimitiveParams, CadTransform } from '$lib/types/cad';

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

export function generateCode(objects: SceneObject[]): string {
  if (objects.length === 0) {
    return `import cadquery as cq\n\n# Empty scene — add objects using the toolbar\nresult = cq.Workplane("XY").box(1, 1, 1)\n`;
  }

  const lines: string[] = ['import cadquery as cq', ''];

  const visibleObjects = objects.filter((o) => o.visible);

  for (const obj of visibleObjects) {
    lines.push(`# --- ${obj.name} ---`);
    lines.push(generatePrimitive(obj.name, obj.params));

    const transformLines = generateTransform(obj.name, obj.transform);
    lines.push(...transformLines);

    lines.push('');
  }

  if (visibleObjects.length === 0) {
    lines.push('result = cq.Workplane("XY").box(1, 1, 1)');
  } else if (visibleObjects.length === 1) {
    const obj = visibleObjects[0];
    // For Solid types (cone), wrap in Workplane
    if (obj.params.type === 'cone') {
      lines.push(`result = cq.Workplane("XY").add(${obj.name})`);
    } else {
      lines.push(`result = ${obj.name}`);
    }
  } else {
    lines.push('# Assemble all objects');
    lines.push('assy = cq.Assembly()');
    for (const obj of visibleObjects) {
      if (obj.params.type === 'cone') {
        lines.push(`assy.add(cq.Workplane("XY").add(${obj.name}), name="${obj.name}")`);
      } else {
        lines.push(`assy.add(${obj.name}, name="${obj.name}")`);
      }
    }
    lines.push('result = assy.toCompound()');
  }

  lines.push('');
  return lines.join('\n');
}
