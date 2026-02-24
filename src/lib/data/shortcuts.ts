export interface ShortcutEntry {
  key: string;
  action: string;
  category: 'global' | 'view' | 'tools' | 'sketch' | 'boolean';
  context?: string;
}

export const SHORTCUTS: ShortcutEntry[] = [
  // Global
  { key: 'Ctrl+N', action: 'New project', category: 'global' },
  { key: 'Ctrl+S', action: 'Save project', category: 'global' },
  { key: 'Ctrl+R', action: 'Run code', category: 'global' },
  { key: 'Ctrl+Z', action: 'Undo', category: 'global' },
  { key: 'Ctrl+Y', action: 'Redo', category: 'global' },
  { key: 'Escape', action: 'Cancel / Deselect', category: 'global' },
  { key: 'Delete', action: 'Delete selected', category: 'global' },
  { key: '?', action: 'Keyboard shortcuts', category: 'global' },

  // View
  { key: 'Home', action: 'Fit all', category: 'view' },
  { key: 'Numpad 7', action: 'Top view', category: 'view' },
  { key: 'Numpad 1', action: 'Front view', category: 'view' },
  { key: 'Numpad 3', action: 'Right view', category: 'view' },
  { key: 'Numpad 0', action: 'Isometric view', category: 'view' },
  { key: 'Ctrl+Numpad 7', action: 'Bottom view', category: 'view' },
  { key: 'Ctrl+Numpad 1', action: 'Back view', category: 'view' },
  { key: 'Ctrl+Numpad 3', action: 'Left view', category: 'view' },
  { key: 'F', action: 'Zoom to selection', category: 'view' },
  { key: 'Numpad 5', action: 'Toggle perspective/orthographic', category: 'view' },

  // Tools (parametric mode)
  { key: 'V', action: 'Select tool', category: 'tools' },
  { key: 'G', action: 'Translate', category: 'tools' },
  { key: 'R', action: 'Rotate', category: 'tools' },
  { key: 'S', action: 'Scale', category: 'tools' },
  { key: '1', action: 'Add Box', category: 'tools' },
  { key: '2', action: 'Add Cylinder', category: 'tools' },
  { key: '3', action: 'Add Sphere', category: 'tools' },
  { key: '4', action: 'Add Cone', category: 'tools' },
  { key: 'E', action: 'Extrude sketch', category: 'tools' },

  // Boolean / Pattern
  { key: 'Ctrl+Shift+U', action: 'Union', category: 'boolean' },
  { key: 'Ctrl+Shift+D', action: 'Subtract', category: 'boolean' },
  { key: 'Ctrl+Shift+I', action: 'Intersect', category: 'boolean' },
  { key: 'Ctrl+Shift+P', action: 'Split body', category: 'boolean' },
  { key: 'Ctrl+Shift+M', action: 'Mirror pattern', category: 'boolean' },
  { key: 'Ctrl+Shift+L', action: 'Linear pattern', category: 'boolean' },
  { key: 'Ctrl+Shift+O', action: 'Circular pattern', category: 'boolean' },

  // Sketch mode
  { key: 'V', action: 'Select', category: 'sketch', context: 'sketch' },
  { key: 'L', action: 'Line', category: 'sketch', context: 'sketch' },
  { key: 'R', action: 'Rectangle', category: 'sketch', context: 'sketch' },
  { key: 'C', action: 'Circle', category: 'sketch', context: 'sketch' },
  { key: 'A', action: 'Arc', category: 'sketch', context: 'sketch' },
  { key: 'O', action: 'Coincident', category: 'sketch', context: 'sketch' },
  { key: 'H', action: 'Horizontal', category: 'sketch', context: 'sketch' },
  { key: 'I', action: 'Vertical', category: 'sketch', context: 'sketch' },
  { key: 'P', action: 'Parallel', category: 'sketch', context: 'sketch' },
  { key: 'T', action: 'Perpendicular', category: 'sketch', context: 'sketch' },
  { key: 'E', action: 'Equal', category: 'sketch', context: 'sketch' },
  { key: 'D', action: 'Distance', category: 'sketch', context: 'sketch' },
  { key: 'Q', action: 'Radius', category: 'sketch', context: 'sketch' },
  { key: 'N', action: 'Angle', category: 'sketch', context: 'sketch' },
  { key: 'X', action: 'Trim', category: 'sketch', context: 'sketch' },
  { key: 'W', action: 'Extend', category: 'sketch', context: 'sketch' },
  { key: 'F', action: 'Offset', category: 'sketch', context: 'sketch' },
  { key: 'M', action: 'Mirror', category: 'sketch', context: 'sketch' },
  { key: 'G', action: 'Fillet', category: 'sketch', context: 'sketch' },
  { key: 'J', action: 'Chamfer', category: 'sketch', context: 'sketch' },
];

export const CATEGORY_LABELS: Record<string, string> = {
  global: 'Global',
  view: 'View',
  tools: 'Tools (3D Mode)',
  boolean: 'Boolean & Patterns',
  sketch: 'Sketch Mode',
};
