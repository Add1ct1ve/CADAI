import type { MaterialId } from '$lib/types/cad';

export interface MaterialPreset {
  id: MaterialId;
  name: string;
  color: string;
  metalness: number;
  roughness: number;
  density: number; // g/cm³
}

export const MATERIALS: MaterialPreset[] = [
  { id: 'steel',    name: 'Steel',    color: '#a0a0b0', metalness: 0.8,  roughness: 0.4,  density: 7.85  },
  { id: 'aluminum', name: 'Aluminum', color: '#c0c8d0', metalness: 0.7,  roughness: 0.35, density: 2.70  },
  { id: 'copper',   name: 'Copper',   color: '#c87533', metalness: 0.9,  roughness: 0.3,  density: 8.96  },
  { id: 'brass',    name: 'Brass',    color: '#d4a843', metalness: 0.85, roughness: 0.35, density: 8.50  },
  { id: 'titanium', name: 'Titanium', color: '#8a8a9a', metalness: 0.75, roughness: 0.45, density: 4.51  },
  { id: 'plastic',  name: 'Plastic',  color: '#e0e0e0', metalness: 0.0,  roughness: 0.8,  density: 1.20  },
  { id: 'abs',      name: 'ABS',      color: '#f5f5dc', metalness: 0.0,  roughness: 0.75, density: 1.05  },
  { id: 'nylon',    name: 'Nylon',    color: '#f0ead6', metalness: 0.0,  roughness: 0.7,  density: 1.14  },
  { id: 'wood',     name: 'Wood',     color: '#a0784c', metalness: 0.0,  roughness: 0.9,  density: 0.60  },
  { id: 'glass',    name: 'Glass',    color: '#d4eaf7', metalness: 0.1,  roughness: 0.05, density: 2.50  },
  { id: 'rubber',   name: 'Rubber',   color: '#2a2a2a', metalness: 0.0,  roughness: 0.95, density: 1.10  },
  { id: 'gold',     name: 'Gold',     color: '#ffd700', metalness: 1.0,  roughness: 0.2,  density: 19.32 },
];

export const MATERIAL_MAP = new Map(MATERIALS.map(m => [m.id, m]));

export function getMaterial(id: string): MaterialPreset | undefined {
  return MATERIAL_MAP.get(id);
}

export const DEFAULT_METALNESS = 0.3;
export const DEFAULT_ROUGHNESS = 0.7;
export const DEFAULT_OPACITY = 1.0;
