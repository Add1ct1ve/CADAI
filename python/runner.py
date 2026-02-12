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
from collections.abc import Mapping


def _is_string_like(value):
    return isinstance(value, (str, bytes, bytearray))


def _extract_exportables(result):
    """
    Flatten a user-provided `result` into CadQuery-exportable objects.

    Supports:
    - a single Shape / Solid / Compound
    - Workplane (via .vals()/.val())
    - list/tuple/set of the above
    - dict of named parts (values are inspected)
    """
    found = []
    invalid = []
    seen_ids = set()

    def add_candidate(candidate):
        if candidate is None:
            return
        if _is_string_like(candidate):
            invalid.append(f"string:{str(candidate)[:40]}")
            return

        if isinstance(candidate, Mapping):
            for value in candidate.values():
                add_candidate(value)
            return

        if isinstance(candidate, (list, tuple, set)):
            for value in candidate:
                add_candidate(value)
            return

        # Workplane with multiple objects.
        if hasattr(candidate, "vals") and callable(candidate.vals):
            try:
                values = candidate.vals()
                if values:
                    for value in values:
                        add_candidate(value)
                    return
            except Exception:
                pass

        # Workplane with a single object.
        if hasattr(candidate, "val") and callable(candidate.val):
            try:
                value = candidate.val()
                if value is not None and value is not candidate:
                    add_candidate(value)
                    return
            except Exception:
                pass

        # CadQuery shape-like object.
        if hasattr(candidate, "wrapped"):
            obj_id = id(candidate)
            if obj_id not in seen_ids:
                seen_ids.add(obj_id)
                found.append(candidate)
            return

        # Numbers / bools / other scalars are not exportable geometry.
        if isinstance(candidate, (int, float, bool)):
            invalid.append(f"scalar:{candidate}")
            return

        # Unknown object type that did not resolve to a CadQuery shape.
        invalid.append(f"type:{type(candidate).__name__}")

    add_candidate(result)
    return found, invalid


def _count_solids(shape):
    """Count solid sub-shapes using OCP topology explorer."""
    from OCP.TopAbs import TopAbs_SOLID
    from OCP.TopExp import TopExp_Explorer

    wrapped = shape.wrapped if hasattr(shape, "wrapped") else shape
    explorer = TopExp_Explorer(wrapped, TopAbs_SOLID)
    count = 0
    while explorer.More():
        count += 1
        explorer.Next()
    return count


def _ensure_single_solid(normalized, cq):
    """
    Verify result is a single solid body.
    If multiple solids found, attempt OCP-level fuse.
    Exit code 5 if unfixable.
    """
    try:
        count = _count_solids(normalized)
        if count <= 1:
            return normalized

        # Try fusing touching/overlapping solids
        from OCP.TopAbs import TopAbs_SOLID
        from OCP.TopExp import TopExp_Explorer
        from OCP.BRepAlgoAPI import BRepAlgoAPI_Fuse

        wrapped = normalized.wrapped if hasattr(normalized, "wrapped") else normalized
        explorer = TopExp_Explorer(wrapped, TopAbs_SOLID)
        solids = []
        while explorer.More():
            solids.append(explorer.Current())
            explorer.Next()

        fused = solids[0]
        for s in solids[1:]:
            op = BRepAlgoAPI_Fuse(fused, s)
            if op.IsDone():
                fused = op.Shape()
            else:
                print(
                    f"SPLIT_BODY: result has {count} disconnected solids (fuse failed)",
                    file=sys.stderr,
                )
                sys.exit(5)

        fused_shape = cq.Shape(fused)
        if _count_solids(fused_shape) == 1:
            print(f"FUSED: {count} solids merged into 1", file=sys.stderr)
            return fused_shape

        print(
            f"SPLIT_BODY: result has {count} disconnected solids after fuse attempt",
            file=sys.stderr,
        )
        sys.exit(5)
    except SystemExit:
        raise  # re-raise sys.exit
    except Exception as e:
        # Don't block export if the check itself errors
        print(f"Warning: solid count check skipped: {e}", file=sys.stderr)
        return normalized


def _normalize_result_for_export(result, cq):
    exportables, invalid = _extract_exportables(result)
    if not exportables:
        raise ValueError(
            "result did not contain exportable CadQuery geometry. "
            "Assign a CadQuery Workplane/Shape or a collection of those to `result`."
        )

    if invalid:
        examples = ", ".join(invalid[:3])
        raise ValueError(
            "result mixed geometry with non-geometry values "
            f"({examples}). Assign only CadQuery geometry objects to `result`."
        )

    if len(exportables) == 1:
        return exportables[0]

    # Multiple parts: export as one compound (preserves all generated solids).
    return cq.Compound.makeCompound(exportables)


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

    # Export based on file extension
    try:
        import cadquery as cq
        normalized = _normalize_result_for_export(result, cq)
        normalized = _ensure_single_solid(normalized, cq)
        ext = os.path.splitext(output_file)[1].lower()
        if ext in ('.step', '.stp'):
            cq.exporters.export(normalized, output_file, cq.exporters.ExportTypes.STEP)
        else:
            cq.exporters.export(normalized, output_file, cq.exporters.ExportTypes.STL)
    except Exception:
        traceback.print_exc()
        sys.exit(4)

    print(f"Exported to {output_file}")


if __name__ == "__main__":
    main()
