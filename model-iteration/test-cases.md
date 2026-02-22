# Test Cases (Regression Suite)

These are specific prompts tested against Aiden Build123d 32B. Use to validate whether the next iteration fixes capability gaps — do NOT train to pass these exact prompts.

| # | Prompt | Result | Gaps Exposed |
|---|--------|--------|--------------|
| 1 | Rounded box 80x50x30mm with cylindrical hole | Box + hole OK, fillets missing | API semantics (fillet radii), code mitigation (safe fillet wrapper) |
| 2 | Phone stand, 45° back, 80mm wide, 100mm tall, 10mm lip | Only base created, 3/4 steps skipped | API semantics (BuildPart misuse, hallucinated calls), solid body reasoning |
| 3 | Gear with 20 teeth, 50mm OD, 10mm thick, 10mm bore | Flat cylinder with hole, no teeth | Domain knowledge (involute profiles, parametric mechanisms) |
| 4 | Cable organizer, 5 U-shaped slots on rectangular base | Flat plate, no slots | Solid body reasoning (wrong boolean strategy), domain knowledge |
| 5 | Hex nut M10, 8.4mm bore, 18mm AF, 8mm tall, chamfered | Hex + chamfers OK, bore is hex not round, no threads | Domain knowledge (fastener anatomy, thread generation) |
| 6 | Cherry MX keycap 18x18mm, 2mm fillets, 8mm tall | Outer shape correct, completely solid (no cavity) | API semantics (offset in empty sketch), domain knowledge (keycap is hollow) |
| 7 | 50x40x3mm door latch plate with slot opening | IndentationError on all 4 attempts | Code generation quality (Python syntax) |
| 8 | 200x200x5mm mounting plate, rounded corners, 6 holes | Plate + fillets OK, holes in 1x6 row, last hole clips edge | Spatial reasoning (hole layout), domain knowledge (mounting patterns) |
| 9 | Pottery trimming spinner, 35mm base, 15mm tall, filleted, central hole | Base+shaft+ring built OK, 12mm hole severed 10mm shaft → disconnected solids, fillets skipped | Spatial reasoning (cut vs feature sizing), solid body reasoning (additive stacking) |
