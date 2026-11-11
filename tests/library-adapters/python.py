from pathlib import Path
from array import array
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "target/library-adapters"))
from bindings import SampleView

data = Path("target/library-adapters/sample.bin").read_bytes()
view = SampleView(data)
assert view.bounds_min_y == -2 and view.bounds_max_z == 6
assert list(view.points) == [7, 8, 9, 0, 10, 11, 12, 0]
assert view.matrix.y_axis.z == 6
assert (view.mask.x, view.mask.y, view.mask.z) == (True, False, True)
assert view.labels[1] == "two 🦀"
rebuilt = SampleView.create({
    "bounds": {"min": {"x": -1, "y": -2, "z": -3}, "max": {"x": 4, "y": 5, "z": 6}},
    "points": memoryview(array("f", [7, 8, 9, 0, 10, 11, 12, 0])),
    "matrix": {"x_axis": {"x": 1, "y": 2, "z": 3}, "y_axis": {"x": 4, "y": 5, "z": 6}, "z_axis": {"x": 7, "y": 8, "z": 9}},
    "mask": {"x": True, "y": False, "z": True},
    "labels": ["one", "two 🦀"],
})
assert bytes(rebuilt.storage.bytes) == data
rebuilt.bounds_min_y = -20
rebuilt.points[4] = 100
assert rebuilt.bounds_min_y == -20 and rebuilt.points[4] == 100
