import unittest

from python.manufacturing import count_degenerate_faces


class _MeshDegenerateAttr:
    def __init__(self):
        self.degenerate_faces = [0, 2]
        self.faces = [0, 1, 2, 3]


class _MeshNondegenerateCallable:
    def __init__(self):
        self.faces = [0, 1, 2, 3, 4]

    def nondegenerate_faces(self):
        return [0, 1, 2]


class _MeshAreaFallback:
    def __init__(self):
        self.faces = [0, 1, 2, 3]
        self.area_faces = [0.5, 0.0, 1e-16, 0.2]


class _MeshDegenerateBoolMask:
    def __init__(self):
        self.faces = [0, 1, 2, 3]
        self.degenerate_faces = [False, True, False, True]


class _MeshNondegenerateBoolMask:
    def __init__(self):
        self.faces = [0, 1, 2, 3]
        self.nondegenerate_faces = [True, False, True, False]


class _MeshUnknown:
    def __init__(self):
        self.faces = [0, 1]


class ManufacturingMeshCheckTests(unittest.TestCase):
    def test_count_degenerate_faces_uses_degenerate_attr(self):
        self.assertEqual(count_degenerate_faces(_MeshDegenerateAttr()), 2)

    def test_count_degenerate_faces_uses_nondegenerate_fallback(self):
        self.assertEqual(count_degenerate_faces(_MeshNondegenerateCallable()), 2)

    def test_count_degenerate_faces_uses_area_fallback(self):
        self.assertEqual(count_degenerate_faces(_MeshAreaFallback()), 2)

    def test_count_degenerate_faces_handles_degenerate_bool_mask(self):
        self.assertEqual(count_degenerate_faces(_MeshDegenerateBoolMask()), 2)

    def test_count_degenerate_faces_handles_nondegenerate_bool_mask(self):
        self.assertEqual(count_degenerate_faces(_MeshNondegenerateBoolMask()), 2)

    def test_count_degenerate_faces_unknown_defaults_zero(self):
        self.assertEqual(count_degenerate_faces(_MeshUnknown()), 0)


if __name__ == "__main__":
    unittest.main()
