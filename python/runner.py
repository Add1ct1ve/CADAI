"""
Build123d execution wrapper for CAD AI Studio.

This script is invoked by the Rust backend as a subprocess.
It executes Build123d code and exports the result as STL.

Usage:
    python runner.py <input_file> <output_file>

The input file should contain valid Build123d Python code.
The code MUST assign the final result to a variable named 'result'.
The output file will be written as binary STL.
"""

import sys
import os
import ast
import json
import builtins
import traceback
import faulthandler
from collections.abc import Mapping

# Enable faulthandler so OCP/OpenCascade segfaults produce a traceback
# instead of a silent crash.
faulthandler.enable()


def _is_string_like(value):
    return isinstance(value, (str, bytes, bytearray))


def _extract_exportables(result):
    """
    Flatten a user-provided `result` into Build123d-exportable objects.

    Supports:
    - a single Shape / Solid / Compound
    - Workplane (via .vals()/.val()) for CadQuery backward compat
    - BuildPart context manager results (via .part)
    - BuildSketch context manager results (via .sketch)
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

        # Workplane with multiple objects (CadQuery backward compat).
        if hasattr(candidate, "vals") and callable(candidate.vals):
            try:
                values = candidate.vals()
                if values:
                    for value in values:
                        add_candidate(value)
                    return
            except Exception:
                pass

        # Workplane with a single object (CadQuery backward compat).
        if hasattr(candidate, "val") and callable(candidate.val):
            try:
                value = candidate.val()
                if value is not None and value is not candidate:
                    add_candidate(value)
                    return
            except Exception:
                pass

        # Build123d BuildPart context manager result.
        if hasattr(candidate, "part"):
            part = candidate.part
            if part is not None:
                add_candidate(part)
                return

        # Build123d BuildSketch context manager result.
        if hasattr(candidate, "sketch"):
            sketch = candidate.sketch
            if sketch is not None:
                add_candidate(sketch)
                return

        # Build123d / CadQuery shape-like object (both use .wrapped for OCCT).
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

        # Unknown object type that did not resolve to a Build123d shape.
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


def _ensure_single_solid(normalized):
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

        from build123d import Solid
        fused_shape = Solid(fused)
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


def _normalize_result_for_export(result):
    exportables, invalid = _extract_exportables(result)
    if not exportables:
        raise ValueError(
            "result did not contain exportable Build123d geometry. "
            "Assign a Build123d Part/Shape or a collection of those to `result`."
        )

    if invalid:
        examples = ", ".join(invalid[:3])
        raise ValueError(
            "result mixed geometry with non-geometry values "
            f"({examples}). Assign only Build123d geometry objects to `result`."
        )

    if len(exportables) == 1:
        return exportables[0]

    # Multiple parts: export as one compound (preserves all generated solids).
    from build123d import Compound
    return Compound(children=exportables)


def _extract_topology(shape):
    """Extract face/edge topology from a Build123d shape."""
    try:
        from OCP.TopExp import TopExp_Explorer
        from OCP.TopAbs import TopAbs_FACE, TopAbs_EDGE
        from OCP.BRep import BRep_Tool
        from OCP.BRepGProp import BRepGProp
        from OCP.GProp import GProp_GProps

        wrapped = shape.wrapped if hasattr(shape, "wrapped") else shape

        faces = []
        face_explorer = TopExp_Explorer(wrapped, TopAbs_FACE)
        face_id = 0
        while face_explorer.More():
            face = face_explorer.Current()

            # Compute face area
            props = GProp_GProps()
            BRepGProp.SurfaceProperties_s(face, props)
            area = props.Mass()

            # Get face normal (approximate from surface at midpoint)
            normal = [0, 0, 1]
            try:
                from OCP.BRepAdaptor import BRepAdaptor_Surface
                from OCP.gp import gp_Pnt, gp_Vec
                surf = BRepAdaptor_Surface(face)
                u_mid = (surf.FirstUParameter() + surf.LastUParameter()) / 2
                v_mid = (surf.FirstVParameter() + surf.LastVParameter()) / 2
                pnt = gp_Pnt()
                d1u = gp_Vec()
                d1v = gp_Vec()
                surf.D1(u_mid, v_mid, pnt, d1u, d1v)
                normal_vec = d1u.Crossed(d1v)
                if normal_vec.Magnitude() > 1e-10:
                    normal_vec.Normalize()
                    normal = [normal_vec.X(), normal_vec.Y(), normal_vec.Z()]
            except Exception:
                pass

            faces.append({
                "id": face_id,
                "normal": normal,
                "area": area,
                "triangleIndices": [],  # Would need mesh correlation
            })
            face_id += 1
            face_explorer.Next()

        edges = []
        edge_explorer = TopExp_Explorer(wrapped, TopAbs_EDGE)
        edge_id = 0
        while edge_explorer.More():
            edge = edge_explorer.Current()
            edges.append({
                "id": edge_id,
                "vertexPairs": [],  # Simplified for now
            })
            edge_id += 1
            edge_explorer.Next()

        return {"faces": faces, "edges": edges}
    except Exception as e:
        return {"faces": [], "edges": [], "error": str(e)}


def _indent_width(line):
    width = 0
    for ch in line:
        if ch == " ":
            width += 1
        elif ch == "\t":
            width += 4
        else:
            break
    return width


def _is_try_header(stripped):
    header = stripped.split("#", 1)[0].rstrip()
    if not header.endswith(":"):
        return False
    head = header[:-1].strip()
    return head == "try" or head.startswith("except") or head == "else" or head == "finally"


_VALID_NAMES_CACHE = None


def _get_valid_names():
    """Build set of valid callable names from build123d + builtins + math."""
    global _VALID_NAMES_CACHE
    if _VALID_NAMES_CACHE is not None:
        return _VALID_NAMES_CACHE

    names = set()

    # Python builtins (print, range, len, dict, list, type, ...)
    names.update(dir(builtins))

    # math module
    import math
    names.update(dir(math))

    # Common stdlib names used in generated code
    names.update([
        "deepcopy", "copy", "partial", "reduce",
        "pi", "tau", "e", "inf",
    ])

    # build123d — the main namespace
    try:
        import build123d
        names.update(dir(build123d))
        # Also grab names from star-import (what `from build123d import *` gives)
        if hasattr(build123d, "__all__"):
            names.update(build123d.__all__)
    except ImportError:
        pass

    # OCP names that appear in valid build123d code
    names.update([
        "TopAbs_SOLID", "TopAbs_FACE", "TopAbs_EDGE", "TopAbs_WIRE",
        "TopExp_Explorer", "BRepAlgoAPI_Fuse",
        "gp_Pnt", "gp_Vec", "gp_Dir", "gp_Ax1", "gp_Ax2",
    ])

    # Names injected by the Rust postprocess wrappers (executor.rs)
    names.update([
        "_orig_fillet", "_orig_chamfer", "max_fillet",
    ])

    _VALID_NAMES_CACHE = names
    return names


def _collect_code_defined_names(tree):
    """Collect all names defined within the code's own AST.

    Covers: def, class, assignments, imports, for-loop targets,
    with-as targets, and comprehension variables.
    """
    names = set()
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            names.add(node.name)
        elif isinstance(node, ast.ClassDef):
            names.add(node.name)
        elif isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    names.add(target.id)
                elif isinstance(target, ast.Tuple):
                    for elt in target.elts:
                        if isinstance(elt, ast.Name):
                            names.add(elt.id)
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.add(node.target.id)
        elif isinstance(node, (ast.Import, ast.ImportFrom)):
            for alias in node.names:
                names.add(alias.asname if alias.asname else alias.name.split(".")[0])
        elif isinstance(node, ast.For):
            if isinstance(node.target, ast.Name):
                names.add(node.target.id)
            elif isinstance(node.target, ast.Tuple):
                for elt in node.target.elts:
                    if isinstance(elt, ast.Name):
                        names.add(elt.id)
        elif isinstance(node, ast.withitem) and isinstance(node.optional_vars, ast.Name):
            names.add(node.optional_vars.id)
    return names


def strip_unknown_calls(code):
    """
    Parse code AST, find bare function calls to names that are not in the
    build123d/builtins namespace AND not defined within the code itself,
    and replace those lines with `pass`.
    Returns (cleaned_code, removed_names).
    """
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return code, []

    valid = _get_valid_names()
    code_defined = _collect_code_defined_names(tree)

    # Collect line numbers and the unknown name for lines that call unknown functions.
    # We only flag bare Name(...) calls, not attribute calls like obj.method().
    bad_lines = {}  # line_number -> name
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        # Bare call: SomeFunction(...)
        if isinstance(func, ast.Name):
            if func.id not in valid and func.id not in code_defined:
                bad_lines[node.lineno] = func.id

    if not bad_lines:
        return code, []

    lines = code.splitlines()
    removed = []
    for lineno, name in sorted(bad_lines.items()):
        idx = lineno - 1  # ast is 1-indexed
        if 0 <= idx < len(lines):
            original = lines[idx]
            indent = original[: len(original) - len(original.lstrip())]
            lines[idx] = f"{indent}pass  # stripped unknown: {name}"
            removed.append(name)
            print(f"[validator] Stripped unknown call on line {lineno}: {name}", file=sys.stderr)

    return "\n".join(lines), removed


def guard_fillet_chamfer(code):
    """
    Wrap unguarded .fillet()/.chamfer() lines in try/except blocks.
    Line-based and indentation-aware; does not parse multi-line statements.
    """
    lines = code.splitlines()
    protected = []
    out = []

    for line in lines:
        stripped = line.lstrip()
        indent = _indent_width(line)

        if stripped:
            while protected and indent <= protected[-1]:
                protected.pop()

        in_try = bool(protected) and indent > protected[-1]
        has_fillet = "fillet(" in line or "chamfer(" in line
        is_comment = stripped.startswith("#")
        already_guarded = "auto-fillet-guard" in line

        if has_fillet and not in_try and not is_comment and stripped and not already_guarded:
            indent_str = line[: len(line) - len(stripped)]
            out.append(f"{indent_str}# auto-fillet-guard")
            out.append(f"{indent_str}try:")
            out.append(f"{indent_str}    {stripped}")
            out.append(f"{indent_str}except Exception:")
            out.append(f"{indent_str}    pass")
        else:
            out.append(line)

        if stripped and _is_try_header(stripped):
            protected.append(indent)

    return "\n".join(out)


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

    code, _stripped = strip_unknown_calls(code)
    code = guard_fillet_chamfer(code)

    # Execute the Build123d code
    # Inject noop shims for CadQuery/OCP viewer functions that AI models
    # sometimes emit.  Without these the code would crash with NameError.
    def _noop(*args, **kwargs):
        pass

    namespace = {
        "show_object": _noop,
        "show": _noop,
        "cq_show": _noop,
    }
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
        normalized = _normalize_result_for_export(result)
        normalized = _ensure_single_solid(normalized)
        from build123d import export_stl, export_step
        ext = os.path.splitext(output_file)[1].lower()
        if ext in ('.step', '.stp'):
            export_step(normalized, output_file)
        else:
            export_stl(normalized, output_file)
    except Exception:
        traceback.print_exc()
        sys.exit(4)

    # Extract topology data and write as sidecar JSON
    try:
        topology = _extract_topology(normalized)
        topology_file = output_file + ".topology.json"
        with open(topology_file, "w", encoding="utf-8") as tf:
            json.dump(topology, tf)
        print(f"TOPOLOGY:{topology_file}", file=sys.stderr)
    except Exception as e:
        print(f"Warning: topology extraction failed: {e}", file=sys.stderr)

    print(f"Exported to {output_file}")


if __name__ == "__main__":
    main()
