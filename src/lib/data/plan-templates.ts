import type { PlanTemplate } from '$lib/types';

export const PLAN_TEMPLATES: PlanTemplate[] = [
  {
    id: 'box',
    name: 'Simple Box',
    description: 'A basic rectangular box with customizable dimensions',
    plan_text: `### Object Analysis
- Type: Rectangular prism (box)
- Dimensions: 50mm x 30mm x 20mm (L x W x H)
- Features: Sharp edges, flat faces

### CadQuery Approach
- Single Workplane("XY").box() call
- Centered at origin by default
- Optional: fillets on edges

### Build Plan
1. Create base box with .box(50, 30, 20)
2. (Optional) Add fillets with .edges().fillet(2)

### Approximation Notes
- Exact geometry, no approximation needed`,
  },
  {
    id: 'cylinder',
    name: 'Cylinder',
    description: 'A cylinder with configurable radius and height',
    plan_text: `### Object Analysis
- Type: Cylinder
- Dimensions: radius 15mm, height 40mm
- Features: Smooth circular profile

### CadQuery Approach
- Workplane("XY").circle(radius).extrude(height)
- Centered at origin on XY plane

### Build Plan
1. Create circle sketch with .circle(15)
2. Extrude to height with .extrude(40)

### Approximation Notes
- Exact geometry, no approximation needed`,
  },
  {
    id: 'enclosure',
    name: 'Electronics Enclosure',
    description: 'A hollow box with lid, screw posts, and ventilation slots',
    plan_text: `### Object Analysis
- Type: Hollow rectangular enclosure with lid
- Outer dimensions: 80mm x 60mm x 35mm
- Wall thickness: 2mm
- Features: Screw posts (4x), ventilation slots, lid lip

### CadQuery Approach
- Create outer shell with .box() then .shell()
- Add screw posts with .pushPoints() and .circle().extrude()
- Cut ventilation slots with .rect().cutThruAll()
- Lid: separate .box() with lip offset

### Build Plan
1. Create outer box 80x60x35mm
2. Shell with 2mm walls (open top)
3. Add 4 corner screw posts (ID 3mm, OD 6mm, height 30mm)
4. Cut ventilation slots on side face (6 slots, 2mm x 15mm)
5. Create lid 80x60x4mm with 1mm lip inset

### Approximation Notes
- Screw posts simplified as cylinders (no threads)
- Snap-fit details omitted for printability`,
  },
  {
    id: 'bracket',
    name: 'L-Bracket',
    description: 'An L-shaped mounting bracket with holes',
    plan_text: `### Object Analysis
- Type: L-shaped bracket
- Base: 50mm x 30mm x 3mm
- Upright: 30mm x 30mm x 3mm
- Mounting holes: 2 on base (5mm dia), 2 on upright (5mm dia)
- Fillet at junction: 5mm radius

### CadQuery Approach
- Create L-profile as 2D sketch, extrude 30mm
- Add fillet at inner corner
- Cut mounting holes with .faces().workplane().hole()

### Build Plan
1. Sketch L-profile on XZ plane (50mm base, 30mm upright, 3mm thick)
2. Extrude 30mm in Y direction
3. Fillet inner corner with 5mm radius
4. Add 2 base mounting holes (5mm dia, centered at 12.5mm and 37.5mm)
5. Add 2 upright mounting holes (5mm dia, centered at 10mm and 20mm)

### Approximation Notes
- Exact geometry, no approximation needed`,
  },
  {
    id: 'gear',
    name: 'Spur Gear',
    description: 'A simple spur gear with involute-approximation teeth',
    plan_text: `### Object Analysis
- Type: Spur gear
- Module: 2mm
- Teeth: 20
- Pressure angle: 20 degrees
- Face width: 10mm
- Bore: 8mm diameter

### CadQuery Approach
- Calculate gear dimensions from module and tooth count
- Pitch diameter = module * teeth = 40mm
- Outer diameter = pitch + 2*module = 44mm
- Root diameter = pitch - 2.5*module = 35mm
- Approximate teeth as trapezoids on polar array
- Use 2D sketch with polar pattern, then extrude

### Build Plan
1. Create base cylinder (OD 44mm, height 10mm)
2. Cut center bore (8mm diameter)
3. Create tooth profile as 2D trapezoid approximation
4. Cut between teeth using polar pattern (20 cuts)
5. Add hub (16mm OD, 2mm protrusion each side)

### Approximation Notes
- Tooth profile is trapezoidal approximation (not true involute)
- Adequate for visualization; not for manufacturing`,
  },
];
