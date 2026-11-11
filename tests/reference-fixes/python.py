from pathlib import Path
from array import array
import sys
from collections.abc import Callable

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'target/reference-fixes'))
from bindings import FixtureInput, FixtureView, DeepRootView, CachedRootView, ChoiceKind, ChoiceView
from intrlay import Storage, Some, IntView, FloatView, zeroed

def padded(value: float) -> FloatView:
    return memoryview(array("f", [value, 0, 0, 0]))

value: FixtureInput = {
    'custom': {'value': 1}, 'deep': 7, 'nested': Some(8), 'absent': None,
    'missing_inner': Some(None), 'values': [9],
    'items': [{'scalar': 10, 'child': Some({'value': 11}), 'projected': Some(padded(12.0))}],
    'cached': Some({'value': 13}), 'projected': Some(padded(14.0)), 'lanes': memoryview(array('f', [15, 0, 0, 0, 16, 0, 0, 0])),
    'choice': {'kind': ChoiceKind.Selected, 'scalar': Some(17), 'child': Some({'value': 18})},
    'inactive': {'kind': ChoiceKind.Empty}, 'before': 19, 'empty': memoryview(array('f')), 'after': 20,
    'empty_items': zeroed(2), 'read_only': Some(21),
}
owned = FixtureView.create(value)
assert bytes(owned.storage.bytes) == (ROOT / 'target/reference-fixes/fixture.bin').read_bytes()

class CountingStorage(Storage):
    reads: int = 0

    @property
    def u32(self) -> IntView:
        self.reads += 1
        return super().u32

storage = CountingStorage(owned.storage.bytes)
view = FixtureView(storage)
def reads[T](expected: int, action: Callable[[], T]) -> T:
    storage.reads = 0
    result = action()
    assert storage.reads == expected, (storage.reads, expected)
    return result

assert reads(11, lambda: view.deep) == 7
nested = reads(13, lambda: view.nested)
assert nested is not None and nested.value == 8
assert view.absent is None
missing = view.missing_inner
assert missing is not None and missing.value is None
values = view.values
# Ordinary indexing still checks length; the sequence header is cached.
assert reads(11, lambda: values[0]) == 9
items = view.items
assert reads(11, lambda: items.get_scalar(0)) == 10
child = items.get_child(0)
assert child is not None and child.value is not None and child.value.value == 11
projected = items.get_projected(0)
assert projected is not None and projected.value is not None and projected.value[0] == 12.0
cached = reads(2, lambda: view.cached)
assert cached is not None and cached.value is not None
assert reads(0, lambda: view.cached) is cached
cached.value.value = 23
fresh = FixtureView(owned.storage.bytes).cached
assert fresh is not None and fresh.value is not None and fresh.value.value == 23
projected = reads(2, lambda: view.projected)
assert projected is not None and projected.value is not None and projected.value[0] == 14.0
lanes = reads(2, lambda: view.lanes)
assert list(lanes) == [15.0, 0.0, 0.0, 0.0, 16.0, 0.0, 0.0, 0.0]
assert reads(0, lambda: view.lanes) is lanes
selected = view.choice.selected_scalar
assert selected is not None and selected.value is not None and selected.value.value == 17
selected_child = view.choice.selected_child
assert selected_child is not None and selected_child.value is not None
assert selected_child.value.value is not None and selected_child.value.value.value == 18
assert view.inactive.selected_child is None
no_payload = ChoiceView.create({'kind': ChoiceKind.Selected, 'scalar': None, 'child': None}).selected_child
assert no_payload is not None and no_payload.value is None
no_inner = ChoiceView.create({'kind': ChoiceKind.Selected, 'scalar': Some(None), 'child': Some(None)})
no_inner_payload = no_inner.selected_child
assert no_inner_payload is not None and no_inner_payload.value is not None
assert no_inner_payload.value.value is None and no_inner.selected_child is no_inner_payload
assert len(view.empty) == len(view.empty_items) == 0
assert view.before == 19 and view.after == 20
read_only = view.read_only
assert read_only is not None and read_only.value == 21
assert not hasattr(view, 'set_read_only')
view.set_deep(31)
view.set_nested(32)
values[0] = 33
items.set_scalar(0, 34)
assert view.deep == 31
nested = view.nested
assert nested is not None and nested.value == 32
assert values[0] == 33 and items[0].scalar == 34
assert DeepRootView.create(35).value == 35
root = CachedRootView.create(Some({'value': 36}))
assert root.value is root.value
root_value = root.value
assert root_value is not None and root_value.value is not None and root_value.value.value == 36
assert CachedRootView.create(None).value is None
root_none = CachedRootView.create(Some(None)).value
assert root_none is not None and root_none.value is None
print('Python reference loads: values, mutation, cache identity, bytes, and exact read counts passed')
