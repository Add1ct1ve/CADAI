"""
STEP/IGES file importer.
Takes a file path, imports the CAD geometry using build123d/OCCT,
exports STL for visualization, and writes metadata.
"""
import sys
import json
import base64
import tempfile
import os


def import_cad_file(file_path: str) -> dict:
    """Import a STEP or IGES file and return STL + metadata."""
    ext = os.path.splitext(file_path)[1].lower()

    try:
        from build123d import import_step, import_stl
        from OCP.STEPControl import STEPControl_Reader
        from OCP.IGESControl import IGESControl_Reader
        from OCP.BRep import BRep_Builder
        from OCP.TopoDS import TopoDS_Shape

        if ext in ('.step', '.stp'):
            shape = import_step(file_path)
        elif ext in ('.iges', '.igs'):
            # build123d may not have import_iges, use OCP directly
            reader = IGESControl_Reader()
            status = reader.ReadFile(file_path)
            if status != 1:
                return {"error": f"Failed to read IGES file: status {status}"}
            reader.TransferRoots()
            shape = reader.OneShape()
        else:
            return {"error": f"Unsupported file format: {ext}"}

        # Export to STL for visualization
        from build123d import export_stl, Compound, Shape

        # Handle different return types
        if hasattr(shape, 'wrapped'):
            # It's a build123d Shape object
            b123d_shape = shape
        else:
            # It's an OCP shape, wrap it
            b123d_shape = Shape(shape)

        stl_path = tempfile.mktemp(suffix='.stl')
        export_stl(b123d_shape, stl_path)

        with open(stl_path, 'rb') as f:
            stl_data = base64.b64encode(f.read()).decode('utf-8')

        os.unlink(stl_path)

        # Get bounding box for metadata
        bbox = b123d_shape.bounding_box()
        metadata = {
            "file_path": file_path,
            "format": ext.lstrip('.'),
            "bbox_min": [bbox.min.X, bbox.min.Y, bbox.min.Z],
            "bbox_max": [bbox.max.X, bbox.max.Y, bbox.max.Z],
        }

        return {
            "stl_base64": stl_data,
            "metadata": metadata,
        }

    except ImportError as e:
        return {"error": f"build123d not available: {e}"}
    except Exception as e:
        return {"error": str(e)}


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Usage: importer.py <file_path>"}))
        sys.exit(1)

    result = import_cad_file(sys.argv[1])
    print(json.dumps(result))
