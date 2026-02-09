"""
Drawing export script for CAD AI Studio.

Converts composed drawing SVGs to PDF or DXF format.

Usage:
    python drawing_export.py pdf <input_svg> <output_pdf>
    python drawing_export.py dxf <input_svg> <output_dxf>

Dependencies:
    PDF: cairosvg (pip install cairosvg)
    DXF: ezdxf (pip install ezdxf)

Exit codes:
    0 = success
    1 = bad arguments
    2 = missing dependency
    3 = conversion error
"""

import sys
import os
import traceback


def export_pdf(input_svg, output_pdf):
    """Convert SVG to PDF using cairosvg."""
    try:
        import cairosvg
    except ImportError:
        print(
            "cairosvg not installed. Install with: pip install cairosvg",
            file=sys.stderr,
        )
        sys.exit(2)

    try:
        cairosvg.svg2pdf(url=input_svg, write_to=output_pdf)
        print(f"PDF exported to {output_pdf}")
    except Exception:
        traceback.print_exc()
        sys.exit(3)


def export_dxf(input_svg, output_dxf):
    """Convert SVG to DXF using ezdxf with basic SVG path parsing."""
    try:
        import ezdxf
    except ImportError:
        print(
            "ezdxf not installed. Install with: pip install ezdxf",
            file=sys.stderr,
        )
        sys.exit(2)

    try:
        import re
        import xml.etree.ElementTree as ET

        # Parse the SVG
        tree = ET.parse(input_svg)
        root = tree.getroot()
        ns = {"svg": "http://www.w3.org/2000/svg"}

        # Create DXF document
        doc = ezdxf.new("R2010")
        msp = doc.modelspace()

        # Extract line elements
        for line_el in root.iter("{http://www.w3.org/2000/svg}line"):
            x1 = float(line_el.get("x1", 0))
            y1 = float(line_el.get("y1", 0))
            x2 = float(line_el.get("x2", 0))
            y2 = float(line_el.get("y2", 0))
            # SVG Y is inverted relative to DXF
            msp.add_line((x1, -y1), (x2, -y2))

        # Extract circle elements
        for circle_el in root.iter("{http://www.w3.org/2000/svg}circle"):
            cx = float(circle_el.get("cx", 0))
            cy = float(circle_el.get("cy", 0))
            r = float(circle_el.get("r", 0))
            if r > 0:
                msp.add_circle((cx, -cy), r)

        # Extract path elements (basic M/L commands)
        for path_el in root.iter("{http://www.w3.org/2000/svg}path"):
            d = path_el.get("d", "")
            if not d:
                continue
            # Simple SVG path parser for M, L, Z commands
            commands = re.findall(r"([MLHVCZmlhvcz])\s*([\d.,\s\-e+]*)", d)
            current = (0.0, 0.0)
            start = (0.0, 0.0)
            for cmd, args_str in commands:
                nums = [float(x) for x in re.findall(r"[+-]?[\d.]+(?:[eE][+-]?\d+)?", args_str)]
                if cmd == "M" and len(nums) >= 2:
                    current = (nums[0], nums[1])
                    start = current
                    # If more coordinate pairs follow, treat as line-to
                    i = 2
                    while i + 1 < len(nums):
                        next_pt = (nums[i], nums[i + 1])
                        msp.add_line((current[0], -current[1]), (next_pt[0], -next_pt[1]))
                        current = next_pt
                        i += 2
                elif cmd == "L" and len(nums) >= 2:
                    i = 0
                    while i + 1 < len(nums):
                        next_pt = (nums[i], nums[i + 1])
                        msp.add_line((current[0], -current[1]), (next_pt[0], -next_pt[1]))
                        current = next_pt
                        i += 2
                elif cmd == "H" and len(nums) >= 1:
                    for x in nums:
                        next_pt = (x, current[1])
                        msp.add_line((current[0], -current[1]), (next_pt[0], -next_pt[1]))
                        current = next_pt
                elif cmd == "V" and len(nums) >= 1:
                    for y in nums:
                        next_pt = (current[0], y)
                        msp.add_line((current[0], -current[1]), (next_pt[0], -next_pt[1]))
                        current = next_pt
                elif cmd == "Z" or cmd == "z":
                    if current != start:
                        msp.add_line((current[0], -current[1]), (start[0], -start[1]))
                    current = start

        # Extract text elements
        for text_el in root.iter("{http://www.w3.org/2000/svg}text"):
            x = float(text_el.get("x", 0))
            y = float(text_el.get("y", 0))
            content = text_el.text or ""
            # Get text from tspan children too
            for tspan in text_el.iter("{http://www.w3.org/2000/svg}tspan"):
                if tspan.text:
                    content += tspan.text
            if content.strip():
                msp.add_text(content.strip(), dxfattribs={"insert": (x, -y), "height": 3.0})

        doc.saveas(output_dxf)
        print(f"DXF exported to {output_dxf}")
    except SystemExit:
        raise
    except Exception:
        traceback.print_exc()
        sys.exit(3)


def main():
    if len(sys.argv) != 4:
        print("Usage: drawing_export.py <pdf|dxf> <input_svg> <output_file>", file=sys.stderr)
        sys.exit(1)

    fmt = sys.argv[1].lower()
    input_svg = sys.argv[2]
    output_file = sys.argv[3]

    if not os.path.exists(input_svg):
        print(f"Input SVG not found: {input_svg}", file=sys.stderr)
        sys.exit(1)

    if fmt == "pdf":
        export_pdf(input_svg, output_file)
    elif fmt == "dxf":
        export_dxf(input_svg, output_file)
    else:
        print(f"Unknown format: {fmt}. Use 'pdf' or 'dxf'.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
