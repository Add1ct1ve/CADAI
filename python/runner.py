"""
CadQuery execution wrapper for CAD AI Studio.

This script is invoked by the Rust backend as a subprocess.
It executes CadQuery code and exports the result as STL.

Usage:
    python runner.py <input_file> <output_file>

The input file should contain valid CadQuery Python code.
The code MUST assign the final result to a variable named 'result'.
The output file will be written as binary STL.
"""

import sys
import os
import traceback


def main():
    if len(sys.argv) != 3:
        print("Usage: runner.py <input_file> <output_stl_file>", file=sys.stderr)
        sys.exit(1)

    input_file = sys.argv[1]
    output_file = sys.argv[2]

    if not os.path.exists(input_file):
        print(f"Input file not found: {input_file}", file=sys.stderr)
        sys.exit(1)

    with open(input_file, "r", encoding="utf-8") as f:
        code = f.read()

    # Execute the CadQuery code
    namespace = {}
    try:
        exec(code, namespace)
    except Exception:
        traceback.print_exc()
        sys.exit(2)

    # Get the result variable
    result = namespace.get("result")
    if result is None:
        print("Error: Code must assign final geometry to 'result' variable.", file=sys.stderr)
        sys.exit(3)

    # Export to STL
    try:
        import cadquery as cq
        cq.exporters.export(result, output_file, cq.exporters.ExportTypes.STL)
    except Exception:
        traceback.print_exc()
        sys.exit(4)

    print(f"STL exported to {output_file}")


if __name__ == "__main__":
    main()
