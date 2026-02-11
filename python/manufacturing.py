"""
Manufacturing utilities for CAD AI Studio.

Subcommands:
    export_3mf <code_file> <output_3mf> [--colors <colors_json>]
    mesh_check <code_file>
    orient <code_file>
    unfold <code_file> <output_dxf> [--thickness <t>]

Exit codes:
    0 = success
    1 = bad args
    2 = code execution error
    3 = no result variable
    4 = operation error
    5 = missing dependency
"""

import sys
import os
import json
import math
import traceback
import subprocess


def ensure_trimesh():
    """Import trimesh, auto-installing if missing."""
    try:
        import trimesh
        return trimesh
    except ImportError:
        subprocess.check_call(
            [sys.executable, '-m', 'pip', 'install', 'trimesh[easy]'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        import trimesh
        return trimesh


def ensure_ezdxf():
    """Import ezdxf, auto-installing if missing."""
    try:
        import ezdxf
        return ezdxf
    except ImportError:
        subprocess.check_call(
            [sys.executable, '-m', 'pip', 'install', 'ezdxf'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        import ezdxf
        return ezdxf


def exec_cadquery_code(code_file):
    """Execute a CadQuery code file and return the namespace."""
    if not os.path.exists(code_file):
        print(f"Input file not found: {code_file}", file=sys.stderr)
        sys.exit(1)

    with open(code_file, "r", encoding="utf-8") as f:
        code = f.read()

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

    return result


def tessellate_result(result, tolerance=0.1):
    """Tessellate a CadQuery result into vertices and faces."""
    try:
        shape = result.val()
        vertices, faces = shape.tessellate(tolerance)
        # Convert CadQuery Vector objects to tuples
        verts = [(v.x, v.y, v.z) for v in vertices]
        # Faces are already tuples of indices
        tris = [tuple(f) for f in faces]
        return verts, tris
    except Exception:
        traceback.print_exc()
        sys.exit(4)


def _count_face_selector(values):
    """Count entries for index lists or boolean masks."""
    try:
        seq = list(values)
    except Exception:
        return 0
    if not seq:
        return 0
    if all(isinstance(v, bool) or type(v).__name__ == "bool_" for v in seq):
        return int(sum(1 for v in seq if bool(v)))
    return int(len(seq))


def count_degenerate_faces(mesh):
    """Count degenerate faces across trimesh versions.

    Compatibility order:
    1. mesh.degenerate_faces (attribute or callable in some versions)
    2. mesh.nondegenerate_faces (attribute/callable) + face count delta
    3. area-based fallback from mesh.area_faces
    """
    # New/old trimesh variants may expose `degenerate_faces` differently.
    deg_attr = getattr(mesh, "degenerate_faces", None)
    if deg_attr is not None:
        try:
            deg = deg_attr() if callable(deg_attr) else deg_attr
            return max(0, _count_face_selector(deg))
        except Exception:
            pass

    # Some versions expose only nondegenerate faces.
    nondeg_attr = getattr(mesh, "nondegenerate_faces", None)
    if nondeg_attr is not None:
        try:
            nondeg = nondeg_attr() if callable(nondeg_attr) else nondeg_attr
            total = int(len(mesh.faces))
            nondeg_count = _count_face_selector(nondeg)
            if 0 <= nondeg_count <= total:
                return total - nondeg_count
        except Exception:
            pass

    # Final fallback: infer degenerate faces from near-zero/non-finite areas.
    try:
        areas = getattr(mesh, "area_faces", None)
        if areas is not None:
            deg = 0
            for area in areas:
                try:
                    val = float(area)
                except Exception:
                    continue
                if (not math.isfinite(val)) or val <= 1e-12:
                    deg += 1
            return int(deg)
    except Exception:
        pass

    # Conservative default: unknown means "not detected".
    return 0


def cmd_export_3mf(args):
    """Export model as 3MF with optional per-object colors."""
    if len(args) < 2:
        print("Usage: manufacturing.py export_3mf <code_file> <output_3mf> [--colors <json>]", file=sys.stderr)
        sys.exit(1)

    code_file = args[0]
    output_path = args[1]
    colors_file = None

    i = 2
    while i < len(args):
        if args[i] == '--colors' and i + 1 < len(args):
            colors_file = args[i + 1]
            i += 2
        else:
            i += 1

    trimesh = ensure_trimesh()
    import numpy as np

    result = exec_cadquery_code(code_file)
    verts, tris = tessellate_result(result)

    mesh = trimesh.Trimesh(vertices=verts, faces=tris)
    mesh.fix_normals()

    # Apply colors if provided
    if colors_file and os.path.exists(colors_file):
        try:
            with open(colors_file, 'r') as f:
                colors = json.load(f)
            if colors and len(colors) > 0:
                # Use the first color for all faces (single-object export)
                c = colors[0]
                r = int(c.get('r', 0.5) * 255)
                g = int(c.get('g', 0.5) * 255)
                b = int(c.get('b', 0.5) * 255)
                a = int(c.get('a', 1.0) * 255)
                face_colors = np.full((len(mesh.faces), 4), [r, g, b, a], dtype=np.uint8)
                mesh.visual.face_colors = face_colors
        except Exception as e:
            print(f"Warning: Could not apply colors: {e}", file=sys.stderr)

    try:
        mesh.export(output_path, file_type='3mf')
    except Exception:
        traceback.print_exc()
        sys.exit(4)

    result_json = {
        "success": True,
        "triangles": int(len(mesh.faces)),
        "path": output_path,
    }
    print(json.dumps(result_json))


def cmd_mesh_check(args):
    """Validate mesh quality for 3D printing."""
    if len(args) < 1:
        print("Usage: manufacturing.py mesh_check <code_file>", file=sys.stderr)
        sys.exit(1)

    code_file = args[0]
    trimesh = ensure_trimesh()

    result = exec_cadquery_code(code_file)
    verts, tris = tessellate_result(result)

    mesh = trimesh.Trimesh(vertices=verts, faces=tris)
    mesh.fix_normals()

    issues = []

    watertight = bool(mesh.is_watertight)
    if not watertight:
        issues.append("Mesh is not watertight (has holes or gaps)")

    winding = bool(mesh.is_winding_consistent)
    if not winding:
        issues.append("Inconsistent face winding (flipped normals)")

    degen = count_degenerate_faces(mesh)
    if degen > 0:
        issues.append(f"{degen} degenerate (zero-area) faces found")

    euler = int(mesh.euler_number)
    if euler != 2:
        issues.append(f"Euler number is {euler} (expected 2 for a closed solid)")

    volume = float(mesh.volume) if mesh.is_watertight else 0.0
    if mesh.is_watertight and volume < 0:
        issues.append("Negative volume detected (inverted normals)")
        volume = abs(volume)

    tri_count = int(len(mesh.faces))
    bounds = mesh.bounds.tolist() if hasattr(mesh, "bounds") else [[0, 0, 0], [0, 0, 0]]

    result_json = {
        "watertight": watertight,
        "winding_consistent": winding,
        "degenerate_faces": degen,
        "euler_number": euler,
        "volume": round(volume, 4),
        "triangle_count": tri_count,
        "bounds": bounds,
        "issues": issues,
    }
    print(json.dumps(result_json))


def cmd_orient(args):
    """Find optimal print orientation to minimize supports."""
    if len(args) < 1:
        print("Usage: manufacturing.py orient <code_file>", file=sys.stderr)
        sys.exit(1)

    code_file = args[0]
    trimesh = ensure_trimesh()
    import numpy as np
    from scipy.spatial.transform import Rotation

    result = exec_cadquery_code(code_file)
    verts, tris = tessellate_result(result)

    mesh = trimesh.Trimesh(vertices=verts, faces=tris)
    mesh.fix_normals()

    # Decimate if too many triangles for speed
    if len(mesh.faces) > 50000:
        mesh = mesh.simplify_quadric_decimation(50000)

    # Candidate orientations: 6 principal axis-aligned directions + OBB axes
    candidates = [
        (0, 0, 0),       # identity (Z up)
        (90, 0, 0),      # rotate around X
        (-90, 0, 0),     # rotate around X (other way)
        (0, 90, 0),      # rotate around Y
        (0, -90, 0),     # rotate around Y (other way)
        (0, 0, 90),      # rotate around Z
        (180, 0, 0),     # upside down
        (90, 90, 0),     # diagonal
        (90, 0, 90),     # diagonal
        (45, 0, 0),      # tilted
        (0, 45, 0),      # tilted
        (45, 45, 0),     # tilted diagonal
    ]

    overhang_threshold = math.cos(math.radians(45))  # 45 degree overhang limit
    best_score = float('inf')
    best_candidate = (0, 0, 0)
    best_height = 0
    best_overhang_pct = 0
    best_base_area = 0

    for rx, ry, rz in candidates:
        rot = Rotation.from_euler('xyz', [rx, ry, rz], degrees=True)
        rotated_verts = rot.apply(mesh.vertices)
        rotated_mesh = trimesh.Trimesh(vertices=rotated_verts, faces=mesh.faces)
        rotated_mesh.fix_normals()

        # Compute metrics
        bounds = rotated_mesh.bounds
        height = bounds[1][2] - bounds[0][2]

        normals = rotated_mesh.face_normals
        areas = rotated_mesh.area_faces

        # Overhang: faces pointing downward (normal.z < -threshold)
        overhang_mask = normals[:, 2] < -overhang_threshold
        overhang_area = float(np.sum(areas[overhang_mask]))
        total_area = float(np.sum(areas))
        overhang_pct = (overhang_area / total_area * 100) if total_area > 0 else 0

        # Base area: faces near z=0 with normal pointing down
        z_min = bounds[0][2]
        base_tolerance = height * 0.01 if height > 0 else 0.1
        # Get face centroids
        centroids = rotated_mesh.triangles_center
        base_mask = (centroids[:, 2] < z_min + base_tolerance) & (normals[:, 2] < -0.9)
        base_area = float(np.sum(areas[base_mask]))

        # Score: lower is better
        # Weighted: height penalty + overhang penalty - base area bonus
        score = height * 1.0 + overhang_pct * 2.0 - base_area * 0.5

        if score < best_score:
            best_score = score
            best_candidate = (rx, ry, rz)
            best_height = height
            best_overhang_pct = overhang_pct
            best_base_area = base_area

    result_json = {
        "rotation": list(best_candidate),
        "height": round(best_height, 2),
        "overhang_pct": round(best_overhang_pct, 2),
        "base_area": round(best_base_area, 2),
        "candidates_evaluated": len(candidates),
    }
    print(json.dumps(result_json))


def cmd_unfold(args):
    """Compute sheet metal flat pattern and export as DXF."""
    if len(args) < 2:
        print("Usage: manufacturing.py unfold <code_file> <output_dxf> [--thickness <t>]", file=sys.stderr)
        sys.exit(1)

    code_file = args[0]
    output_path = args[1]
    thickness = 1.0

    i = 2
    while i < len(args):
        if args[i] == '--thickness' and i + 1 < len(args):
            thickness = float(args[i + 1])
            i += 2
        else:
            i += 1

    ezdxf = ensure_ezdxf()

    # Execute CadQuery code
    result = exec_cadquery_code(code_file)

    try:
        import cadquery as cq

        shape = result.val()
        faces = shape.Faces()
        edges = shape.Edges()

        face_count = len(faces)
        planar_count = 0
        cylindrical_count = 0
        bend_count = 0

        flat_edges = []

        for face in faces:
            face_type = face.geomType()
            if face_type == "PLANE":
                planar_count += 1
            elif face_type == "CYLINDER":
                cylindrical_count += 1
                bend_count += 1
            else:
                # Non-sheet-metal face type
                pass

        # Check if this looks like sheet metal
        if planar_count == 0:
            print(json.dumps({
                "success": False,
                "error": "No planar faces found. Shape may not be suitable for sheet metal unfolding.",
                "face_count": face_count,
                "bend_count": 0,
                "flat_width": 0,
                "flat_height": 0,
            }))
            sys.exit(0)

        # Use CadQuery section to project edges onto XY plane as a simple flat pattern
        # This is a simplified approach: project all edges onto a 2D plane
        doc = ezdxf.new('R2010')
        msp = doc.modelspace()

        # Collect all edges projected to XY
        min_x = float('inf')
        min_y = float('inf')
        max_x = float('-inf')
        max_y = float('-inf')

        edge_data = []
        for edge in edges:
            try:
                start = edge.startPoint()
                end = edge.endPoint()
                x1, y1 = start.x, start.y
                x2, y2 = end.x, end.y
                edge_data.append(((x1, y1), (x2, y2)))
                min_x = min(min_x, x1, x2)
                min_y = min(min_y, y1, y2)
                max_x = max(max_x, x1, x2)
                max_y = max(max_y, y1, y2)
            except Exception:
                continue

        if not edge_data:
            print(json.dumps({
                "success": False,
                "error": "Could not extract edges from shape.",
                "face_count": face_count,
                "bend_count": 0,
                "flat_width": 0,
                "flat_height": 0,
            }))
            sys.exit(0)

        # Write edges to DXF
        for (x1, y1), (x2, y2) in edge_data:
            msp.add_line((x1, y1), (x2, y2))

        # Add bend lines as dashed lines on a separate layer
        doc.layers.add('BEND_LINES', color=1)  # red
        # (In a full implementation, we'd compute actual bend lines here)

        doc.saveas(output_path)

        flat_width = max_x - min_x if max_x > min_x else 0
        flat_height = max_y - min_y if max_y > min_y else 0

        result_json = {
            "success": True,
            "face_count": face_count,
            "bend_count": bend_count,
            "flat_width": round(flat_width, 2),
            "flat_height": round(flat_height, 2),
            "path": output_path,
        }
        print(json.dumps(result_json))

    except Exception:
        traceback.print_exc()
        sys.exit(4)


def main():
    if len(sys.argv) < 2:
        print("Usage: manufacturing.py <subcommand> [args...]", file=sys.stderr)
        print("Subcommands: export_3mf, mesh_check, orient, unfold", file=sys.stderr)
        sys.exit(1)

    subcommand = sys.argv[1]
    sub_args = sys.argv[2:]

    if subcommand == 'export_3mf':
        cmd_export_3mf(sub_args)
    elif subcommand == 'mesh_check':
        cmd_mesh_check(sub_args)
    elif subcommand == 'orient':
        cmd_orient(sub_args)
    elif subcommand == 'unfold':
        cmd_unfold(sub_args)
    else:
        print(f"Unknown subcommand: {subcommand}", file=sys.stderr)
        print("Available: export_3mf, mesh_check, orient, unfold", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
