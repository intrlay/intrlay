"""Execute the generated buffer API against the Rust reference bytes."""
import importlib.util
from pathlib import Path
import sys

directory = Path(sys.argv[1]).resolve()
spec = importlib.util.spec_from_file_location("bindings", directory / "bindings.py")
bindings = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bindings)
nullable_bytes = (directory / "nullable.bin").read_bytes()
nullable = bindings.NullableCasesView(nullable_bytes)
assert nullable.absent is None and nullable.zero == 0
assert nullable.text == "" and nullable.missing_text is None
assert nullable.absent_unit is None and nullable.present_unit.value is None
assert nullable.nested_absent is None
assert nullable.nested_none is not None and nullable.nested_none.value is None
assert nullable.nested_value.value == 7
assert nullable.boxed_none is not None and nullable.boxed_none.value is None
assert nullable.triple.value.value is None
assert nullable.variants[0].value_value is None
assert nullable.variants[1].value_value is not None and nullable.variants[1].value_value.value is None
assert nullable.variants[2].value_value.value == 9
some = bindings.Some
nullable_value = dict(
    absent=None, zero=0, text="", missing_text=None,
    absent_unit=None, present_unit=some(None),
    nested_absent=None, nested_none=some(None), nested_value=some(7),
    boxed_none=some(None), triple=some(some(None)),
    variants=[dict(kind=bindings.NullableVariantKind.Empty), dict(kind=bindings.NullableVariantKind.Value, value=None), dict(kind=bindings.NullableVariantKind.Value, value=9)],
)
assert bytes(bindings.NullableCasesView.create(nullable_value).buffer) == nullable_bytes
rust_bytes = (directory / "rust.bin").read_bytes()
view = bindings.MessageView(rust_bytes)
assert view.sequence == 0x123456789abcdef0
assert view.title == "fixture 🦀 한글"
assert isinstance(view.points, memoryview)
assert list(view.points) == [1, 2, 3, 0, 4, 5, 6, 0]
assert view.commands[1].draw_point_size == 0.75
assert view.tree.next.id == 8
assert view.tree.next.next is None
assert view.optional == 0

value = dict(
    sequence=0x123456789abcdef0,
    title="fixture 🦀 한글",
    points=[dict(x=1, y=2, z=3), dict(x=4, y=5, z=6)],
    commands=[dict(kind=bindings.DrawingCommandKind.Clear), dict(kind=bindings.DrawingCommandKind.DrawPoint, position=dict(x=7, y=8, z=9), size=0.75, label="example")],
    tree=dict(id=7, next=dict(id=8, next=None)),
    optional=0,
    fixed=[10, 20, 30],
)
created = bindings.MessageView.create(value)
assert bytes(created.buffer) == rust_bytes
created.points[6] = 99
assert bindings.MessageView(created.buffer).points[6] == 99
created.sequence = 42
assert created.sequence == 42
(directory / "python.bin").write_bytes(bindings.MessageView.create(value).buffer)
print("Python: exact bytes, recursive views, optional zero, numeric projection mutation passed")
