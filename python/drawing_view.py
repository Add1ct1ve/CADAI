"""
Build123d SVG projection generator for CAD AI Studio.

Generates orthographic projection SVGs from Build123d models.

Usage:
    python drawing_view.py <code_file> <output_svg> <proj_x> <proj_y> <proj_z> [--hidden] [--section PLANE OFFSET]

Exit codes:
    0 = success
    2 = code execution error
    3 = no result variable
    4 = SVG export error
    5 = section error
"""

import sys
import os
import json
import traceback
import re


def parse_svg_dimensions(svg_path):
    """Parse the generated SVG to extract viewBox dimensions."""
    try:
        with open(svg_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Try to find viewBox attribute
        vb_match = re.search(r'viewBox="([^"]+)"', content)
        if vb_match:
            parts = vb_match.group(1).split()
            if len(parts) == 4:
                return {
                    "min_x": float(parts[0]),
                    "min_y": float(parts[1]),
                    "width": float(parts[2]),
                    "height": float(parts[3]),
                }

        # Fallback: try width/height attributes
        w_match = re.search(r'width="([\d.]+)', content)
        h_match = re.search(r'height="([\d.]+)', content)
        if w_match and h_match:
            return {
                "min_x": 0,
                "min_y": 0,
                "width": float(w_match.group(1)),
                "height": float(h_match.group(1)),
            }

        return {"min_x": 0, "min_y": 0, "width": 100, "height": 100}
    except Exception:
        return {"min_x": 0, "min_y": 0, "width": 100, "height": 100}


def main():
    if len(sys.argv) < 6:
        print(
            "Usage: drawing_view.py <code_file> <output_svg> <proj_x> <proj_y> <proj_z> [--hidden] [--section PLANE OFFSET]",
            file=sys.stderr,
        )
        sys.exit(1)

    code_file = sys.argv[1]
    output_svg = sys.argv[2]
    proj_x = float(sys.argv[3])
    proj_y = float(sys.argv[4])
    proj_z = float(sys.argv[5])

    show_hidden = "--hidden" in sys.argv

    section_plane = None
    section_offset = 0.0
    if "--section" in sys.argv:
        idx = sys.argv.index("--section")
        if idx + 2 < len(sys.argv):
            section_plane = sys.argv[idx + 1]
            section_offset = float(sys.argv[idx + 2])
        else:
            print("--section requires PLANE and OFFSET arguments", file=sys.stderr)
            sys.exit(1)

    if not os.path.exists(code_file):
        print(f"Code file not found: {code_file}", file=sys.stderr)
        sys.exit(1)

    with open(code_file, "r", encoding="utf-8") as f:
        code = f.read()

    # Execute the Build123d code
    namespace = {}
    try:
        exec(code, namespace)
    except Exception:
        traceback.print_exc()
        sys.exit(2)

    result = namespace.get("result")
    if result is None:
        print("Error: Code must assign final geometry to 'result' variable.", file=sys.stderr)
        sys.exit(3)

    try:
        from build123d import ExportSVG, Solid, Vector, Axis

        # Unwrap BuildPart context results if needed
        if hasattr(result, "part") and result.part is not None:
            result = result.part
        elif hasattr(result, "val") and callable(result.val):
            result = result.val()

        # Handle section view
        if section_plane:
            try:
                from build123d import Box, Location

                # Create a large cutting box to remove half the model
                cut_size = 10000
                plane_offsets = {
                    "XY": (0, 0, cut_size / 2 + section_offset),
                    "XZ": (0, cut_size / 2 + section_offset, 0),
                    "YZ": (cut_size / 2 + section_offset, 0, 0),
                }
                offset = plane_offsets.get(section_plane, (0, 0, cut_size / 2 + section_offset))

                cut_box = Solid.make_box(cut_size, cut_size, cut_size)
                cut_box = cut_box.locate(
                    Location(
                        (offset[0] - cut_size / 2, offset[1] - cut_size / 2, offset[2] - cut_size / 2)
                    )
                )
                result = result - cut_box
            except Exception:
                traceback.print_exc()
                sys.exit(5)

        # Export SVG projection using Build123d's ExportSVG
        svg = ExportSVG(
            line_weight=0.25,
        )
        svg.add_shape(result, line_type=ExportSVG.LineType.VISIBLE)
        if show_hidden:
            svg.add_shape(result, line_type=ExportSVG.LineType.HIDDEN)
        svg.write(output_svg)

    except SystemExit:
        raise
    except Exception:
        traceback.print_exc()
        sys.exit(4)

    # Parse dimensions and output as JSON
    dims = parse_svg_dimensions(output_svg)
    print(json.dumps(dims))


if __name__ == "__main__":
    main()
