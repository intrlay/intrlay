"""Exercise composed annotations through actual generated Python APIs."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "target/compositions"))
from bindings import RenamedListView, ProjectionSetView, WrappedScalarsView, CachedChildrenView, OptionalRootView, CachedRootView
from intrlay import Some
from bindings import NumericItemsView

numeric = NumericItemsView.create({'items': [{'id': 7, 'position': [1., 2., 3.], 'matrix': [[4., 5.], [6., 7.]], 'padded': 8., 'weights': [9., 10.], 'wide': [11, 12], 'read_only': [13., 14.], 'write_only': [15, 16]}]})
owner = bytearray(16) + numeric.buffer
items = NumericItemsView(memoryview(owner)[16:]).items
assert items.get_position_component(0, 2) == 3
assert items.get_matrix_component(0, 3) == 7
assert items.get_padded_component(0, 3) == 0
assert items.get_weights_component(0, 1) == 10
items.set_position_component(0, 1, 22.)
items.set_weights_component(0, 0, 29.)
items.set_wide_component(0, 1, 0xffffffffffffffff)
items.set_write_only_component(0, 0, -123)
assert items[0].position[1] == 22 and items[0].weights[0] == 29
assert items[0].wide[1] == 0xffffffffffffffff and items[0].write_only[0] == -123
items[0].position[2] = 33
assert items.get_position_component(0, 2) == 33
assert not hasattr(items, 'set_read_only_component') and not hasattr(items, 'get_write_only_component')

renamed = RenamedListView.create({"items": [{"first": 11, "second": 22}]})
assert renamed.items.get_first(0) == 11
renamed.items.set_second(0, 23)
assert renamed.items[0].second == 23

projection = ProjectionSetView.create({"direct": 1.0, "optional": 2.0, "boxed": 3.0, "elements": [4.0, 5.0], "read_only": 6.0})
for value in [projection.direct, projection.optional, projection.boxed, projection.elements[0], projection.read_only]:
    assert isinstance(value, memoryview) and len(value) == 4
assert projection.direct is projection.direct
assert projection.optional is projection.optional
assert projection.boxed is projection.boxed
assert projection.optional is not None
projection.optional[0] = 7
assert projection.optional[0] == 7
assert projection.read_only is not None and not projection.read_only.readonly

wrapped = WrappedScalarsView.create({"present": 1, "absent": None, "boxed": 2, "nested": Some(3), "missing_inner": Some(None), "items": [4, None], "fixed": [5, None], "read_only": 6, "write_only": 7})
length = len(wrapped.buffer)
wrapped.present = 11
wrapped.absent = 12
wrapped.set_boxed(13)
wrapped.set_nested(14)
wrapped.set_missing_inner(15)
wrapped.items[0] = 16
wrapped.items[1] = 17
wrapped.fixed[0] = 18
wrapped.fixed[1] = 19
wrapped.set_write_only(20)
assert (wrapped.present, wrapped.absent, wrapped.boxed) == (11, 12, 13)
assert wrapped.nested is not None and wrapped.nested.value == 14
assert wrapped.missing_inner is not None and wrapped.missing_inner.value == 15
assert list(wrapped.items) == [16, None] and list(wrapped.fixed) == [18, 19]
assert len(wrapped.buffer) == length
assert not hasattr(wrapped, "set_read_only") and not hasattr(wrapped, "write_only")

root_items: list[Some[int | None] | None] = [None, Some(None), Some(2)]
for item in root_items:
    root = OptionalRootView.create(item)
    root.set(9)
    assert root.value is None if item is None else root.value is not None and root.value.value == 9

cached = CachedChildrenView.create({"child": {"value": 1}, "absent": None, "boxed": {"value": 2}, "nested": Some({"value": 3}), "choice": {"kind": "Value", "child": {"value": 4}}})
assert cached.child is cached.child and cached.child is not None
assert cached.absent is None and cached.boxed is cached.boxed
assert cached.nested is cached.nested and cached.nested is not None and cached.nested.value is not None
assert cached.choice.value_child is cached.choice.value_child and cached.choice.value_child is not None
assert cached.choice.value_child.value is not None
cached.child.value = 21
cached.nested.value.value = 22
cached.choice.value_child.value.value = 23
assert cached.child.value == 21 and cached.nested.value.value == 22
assert cached.choice.value_child.value.value == 23
before = bytes(cached.buffer)
try:
    setattr(cached.nested, "value", None)
except AttributeError:
    pass
else:
    raise AssertionError("cached Some payload reference must be read-only")
fresh = CachedChildrenView(cached.buffer)
assert fresh.nested is not None and fresh.nested.value is not None
assert cached.nested.value.value == fresh.nested.value.value == 22
assert bytes(cached.buffer) == before
print("Python composition regressions passed")
cached_root = CachedRootView.create({"value": 1})
assert cached_root.value is cached_root.value and cached_root.value is not None
cached_root.value.value = 2
assert cached_root.value.value == 2
