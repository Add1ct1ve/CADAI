import type { PrimitiveParams, MassProperties } from '$lib/types/cad';

export function computeMassProperties(params: PrimitiveParams, density?: number): MassProperties {
  const PI = Math.PI;

  let volume: number;
  let surfaceArea: number;
  let centerOfMass: [number, number, number];

  switch (params.type) {
    case 'box': {
      const { width: w, depth: d, height: h } = params;
      volume = w * d * h;
      surfaceArea = 2 * (w * d + w * h + d * h);
      centerOfMass = [0, 0, 0];
      break;
    }
    case 'cylinder': {
      const { radius: r, height: h } = params;
      volume = PI * r * r * h;
      surfaceArea = 2 * PI * r * (r + h);
      centerOfMass = [0, 0, 0];
      break;
    }
    case 'sphere': {
      const { radius: r } = params;
      volume = (4 / 3) * PI * r * r * r;
      surfaceArea = 4 * PI * r * r;
      centerOfMass = [0, 0, 0];
      break;
    }
    case 'cone': {
      const { bottomRadius: r1, topRadius: r2, height: h } = params;
      volume = (PI * h / 3) * (r1 * r1 + r1 * r2 + r2 * r2);
      const slant = Math.sqrt((r1 - r2) * (r1 - r2) + h * h);
      surfaceArea = PI * (r1 + r2) * slant + PI * r1 * r1 + PI * r2 * r2;
      // Center of mass for a truncated cone along its axis
      const comZ = r1 === r2
        ? 0 // cylinder-like, symmetric
        : h * (r1 * r1 + 2 * r1 * r2 + 3 * r2 * r2) / (4 * (r1 * r1 + r1 * r2 + r2 * r2)) - h / 2;
      centerOfMass = [0, 0, comZ];
      break;
    }
  }

  const base: MassProperties = { volume, surfaceArea, centerOfMass };

  if (density != null && density > 0) {
    return { ...base, density, mass: density * volume };
  }

  return base;
}
