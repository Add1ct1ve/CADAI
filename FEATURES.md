# CadAI Feature Roadmap

> This is the master feature tracking file. Claude Code should read relevant sections, implement features, and update status when complete.

**Status Legend:**
- ⬜ Not started
- 🟡 In progress
- ✅ Complete
- ⏸️ Blocked/Waiting
- 🔴 Has bugs/needs fix

**Priority Legend:**
- P0 = Critical (must have for usable CAD)
- P1 = Important (expected in CAD software)
- P2 = Nice to have
- P3 = Advanced/Future

---

## Current Sprint: Phase 1 - Core Foundation

**Goal:** Basic sketching and 3D operations working

---

## Phase 1: Core CAD Foundation (P0)

### 1.1 Fix Existing Tools
| Feature | Status | Notes |
|---------|--------|-------|
| Select tool | ⬜ | Currently does nothing. Need: click to select body/face/edge, highlight selection, selection info panel |
| Scale tool polish | ⬜ | Has glitches. Need: uniform scale, scale about center, precise input |
| Move tool polish | ⬜ | Works but need: axis constraints, precise input field |
| Rotate tool polish | ⬜ | Works but need: angle input, rotate about point |

### 1.2 Project/File Basics
| Feature | Status | Notes |
|---------|--------|-------|
| New project | ⬜ | Clear scene, reset state |
| Save project (.cadai) | ⬜ | Save: CadQuery code + viewport state + history |
| Open project | ⬜ | Load .cadai file |
| Export STL | ✅ | Already working |
| Export STEP | ⬜ | CadQuery supports this |
| Undo/Redo | ⬜ | Code history stack |
| Autosave | ⬜ | Save draft every 60s |

### 1.3 View Controls
| Feature | Status | Notes |
|---------|--------|-------|
| Orbit/Pan/Zoom | ✅ | Three.js OrbitControls |
| Standard views (Top/Front/Right/Iso) | ⬜ | Buttons that set camera position |
| Fit all | ⬜ | Zoom to fit entire model |
| Grid toggle | ⬜ | Show/hide grid |
| Axis indicator | ⬜ | XYZ gizmo in corner |

### 1.4 Basic Sketching
| Feature | Status | Notes |
|---------|--------|-------|
| Enter sketch mode | ⬜ | Click plane/face to start sketch |
| Exit sketch mode | ⬜ | Button to finish and exit |
| Draw line | ⬜ | Click-click to place points |
| Draw rectangle | ⬜ | Corner to corner |
| Draw circle | ⬜ | Center + radius |
| Draw arc | ⬜ | 3-point arc |
| Delete sketch entity | ⬜ | Select + delete key |

### 1.5 Basic 3D Operations
| Feature | Status | Notes |
|---------|--------|-------|
| Extrude (add) | ⬜ | Select sketch → extrude up |
| Extrude (cut) | ⬜ | Select sketch → cut down |
| Extrude distance input | ⬜ | Type exact height |
| Fillet edges | ⬜ | Select edge(s), enter radius |
| Chamfer edges | ⬜ | Select edge(s), enter distance |

---

## Phase 2: Parametric Power (P0-P1)

### 2.1 Sketch Constraints
| Feature | Status | Notes |
|---------|--------|-------|
| Coincident | ⬜ | Point on point |
| Horizontal | ⬜ | Line is horizontal |
| Vertical | ⬜ | Line is vertical |
| Parallel | ⬜ | Two lines parallel |
| Perpendicular | ⬜ | Two lines at 90° |
| Equal length | ⬜ | Two lines same length |
| Distance dimension | ⬜ | Set distance between points |
| Radius dimension | ⬜ | Set circle/arc radius |
| Angle dimension | ⬜ | Set angle between lines |
| Constraint icons | ⬜ | Visual indicators on sketch |
| Fully constrained indicator | ⬜ | Sketch turns green when done |

### 2.2 Feature Tree
| Feature | Status | Notes |
|---------|--------|-------|
| Feature list sidebar | ⬜ | Shows all operations in order |
| Click to select feature | ⬜ | Highlights in viewport |
| Double-click to edit | ⬜ | Opens feature parameters |
| Drag to reorder | ⬜ | Changes operation order |
| Suppress feature | ⬜ | Temporarily disable |
| Delete feature | ⬜ | Remove from history |
| Rollback slider | ⬜ | View model at any point in history |

### 2.3 Sketch Operations
| Feature | Status | Notes |
|---------|--------|-------|
| Trim | ⬜ | Click to remove segment |
| Extend | ⬜ | Extend line to intersection |
| Offset | ⬜ | Parallel copy at distance |
| Mirror sketch | ⬜ | Mirror about line |
| Fillet (sketch) | ⬜ | Round sketch corners |
| Chamfer (sketch) | ⬜ | Angled sketch corners |

---

## Phase 3: Advanced Modeling (P1)

### 3.1 More 3D Features
| Feature | Status | Notes |
|---------|--------|-------|
| Revolve (add) | ⬜ | Rotate sketch about axis |
| Revolve (cut) | ⬜ | Cut by revolving |
| Sweep | ⬜ | Extrude along path |
| Loft | ⬜ | Blend between profiles |
| Shell | ⬜ | Hollow out solid |
| Draft | ⬜ | Add taper angle to faces |
| Hole wizard | ⬜ | Counterbore, countersink, tap |

### 3.2 Booleans
| Feature | Status | Notes |
|---------|--------|-------|
| Union/Combine | ⬜ | Fuse bodies together |
| Subtract/Cut | ⬜ | Remove intersection |
| Intersect | ⬜ | Keep only intersection |
| Split body | ⬜ | Divide by plane |

### 3.3 Patterns
| Feature | Status | Notes |
|---------|--------|-------|
| Mirror body | ⬜ | Mirror about plane |
| Linear pattern | ⬜ | Repeat in line |
| Circular pattern | ⬜ | Repeat around axis |

### 3.4 Reference Geometry
| Feature | Status | Notes |
|---------|--------|-------|
| Offset plane | ⬜ | New plane at distance |
| Plane through 3 points | ⬜ | Define custom plane |
| Datum axis | ⬜ | Construction axis |

---

## Phase 4: Polish & UX (P1-P2)

### 4.1 Display Modes
| Feature | Status | Notes |
|---------|--------|-------|
| Shaded mode | ⬜ | Default solid view |
| Wireframe mode | ⬜ | Edges only |
| Shaded + edges | ⬜ | Solid with edge lines |
| Transparent/X-ray | ⬜ | See through bodies |
| Section view | ⬜ | Clipping plane |

### 4.2 Measurements
| Feature | Status | Notes |
|---------|--------|-------|
| Measure distance | ⬜ | Point to point |
| Measure angle | ⬜ | Between faces/lines |
| Measure radius | ⬜ | Click arc/circle |
| Mass properties | ⬜ | Volume, center of mass |
| Bounding box | ⬜ | Overall dimensions |

### 4.3 Materials & Appearance
| Feature | Status | Notes |
|---------|--------|-------|
| Color picker | ⬜ | Set body color |
| Transparency | ⬜ | Opacity slider |
| Material library | ⬜ | Steel, aluminum, plastic, etc. |
| Assign material | ⬜ | For mass properties |

### 4.4 Settings
| Feature | Status | Notes |
|---------|--------|-------|
| Units (mm/inch) | ⬜ | Default and display |
| Grid size | ⬜ | Adjustable grid |
| Snap settings | ⬜ | Grid, vertex, edge snap |
| Theme (dark/light) | ⬜ | UI theme toggle |
| Keyboard shortcuts | ⬜ | Customizable |

---

## Phase 5: Assemblies (P2)

### 5.1 Component Management
| Feature | Status | Notes |
|---------|--------|-------|
| Insert component | ⬜ | Load part from file |
| Component tree | ⬜ | Hierarchy view |
| Hide/show component | ⬜ | Visibility toggle |
| Ground component | ⬜ | Fix in place |

### 5.2 Assembly Mates
| Feature | Status | Notes |
|---------|--------|-------|
| Coincident mate | ⬜ | Face to face |
| Concentric mate | ⬜ | Axis to axis |
| Distance mate | ⬜ | Offset between |
| Angle mate | ⬜ | Fixed angle |
| Interference check | ⬜ | Collision detection |
| Exploded view | ⬜ | Spread parts apart |

---

## Phase 6: Documentation (P2-P3)

### 6.1 2D Drawings
| Feature | Status | Notes |
|---------|--------|-------|
| Create drawing | ⬜ | 2D sheet from 3D model |
| Standard views | ⬜ | Front/top/right projections |
| Section view | ⬜ | Cross-section |
| Dimensions | ⬜ | Auto or manual |
| Notes/text | ⬜ | Annotations |
| Title block | ⬜ | Template |
| Export PDF | ⬜ | Print-ready output |
| Export DXF | ⬜ | For laser/CNC |

### 6.2 Manufacturing
| Feature | Status | Notes |
|---------|--------|-------|
| Export 3MF | ⬜ | 3D printing with colors |
| Mesh check | ⬜ | Watertight validation |
| Orientation tool | ⬜ | Best print orientation |
| Sheet metal unfold | ⬜ | Flat pattern export |

---

## Backlog / Ideas

- AI prompt templates for common parts
- Version control (git-like branching)
- Real-time collaboration
- Cloud sync
- Plugin system
- CAM toolpath generation
- FEA simulation integration
- AR/VR preview
- Generative design

---

## Implementation Notes

### CadQuery Mapping
Most features map to CadQuery operations:
- Extrude → `cq.Workplane().extrude()`
- Fillet → `.fillet()`
- Chamfer → `.chamfer()`
- Shell → `.shell()`
- Revolve → `.revolve()`
- Sweep → `.sweep()`
- Loft → `.loft()`
- Union → `.union()`
- Cut → `.cut()`
- Intersect → `.intersect()`

### Three.js Considerations
- Use `THREE.Raycaster` for selection
- `TransformControls` for move/rotate/scale gizmos
- `EdgesGeometry` for wireframe overlay
- Clipping planes for section view

### File Format (.cadai)
JSON structure:
```json
{
  "version": "1.0",
  "units": "mm",
  "code": "import cadquery as cq\n...",
  "history": [...],
  "viewport": {
    "camera": {...},
    "target": {...}
  },
  "materials": {...}
}
```

---

## How to Update This File

When completing a feature:
1. Change status from ⬜ to ✅
2. Add any relevant notes
3. If bugs found, mark 🔴 and describe issue
4. Update "Current Sprint" section if moving to new phase
