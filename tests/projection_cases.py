"""Packed inputs and indexed enum payloads share the ordinary wire representation."""
from array import array
from pathlib import Path

import intrlay
from contracts import IndexedChoiceKind, IndexedEnumsView, NumericInputsView, NumericTag, ZNumericTag, NumericEnumFieldsView
from contracts import ProjectionEdgesView, FlattenedEmptyChoiceKind, CachedEmptyChoiceKind
from contracts import EmptyNumericTailView, ZeroSizeSequencesView

tail_owner = bytearray([0xa5] * 11)
tail_input = {"prefix": [7] * 9, "trailer": memoryview(array("d"))}
tail = EmptyNumericTailView.create_in_place(memoryview(tail_owner)[1:10], tail_input)
tail.replace(tail_input)
assert tail_owner == bytearray([0xa5, *([7] * 9), 0xa5])
try:
    EmptyNumericTailView.create({**tail_input, "trailer": memoryview(array("d", [1.]))})
except ValueError:
    pass
else:
    raise AssertionError("nonempty projection accepted")
sequence_owner = bytearray([0xa5] * 24)
for numeric in [memoryview(array("d")), intrlay.zeroed(3)]:
    sequence = ZeroSizeSequencesView.create_in_place(memoryview(sequence_owner)[4:20], {"numeric": numeric, "ordinary": intrlay.zeroed(2)})
    assert sequence.storage.u32[1] == (3 if isinstance(numeric, int) else 0)
    assert sequence.storage.u32[3] == 2
assert sequence_owner[:4] == sequence_owner[20:] == bytearray([0xa5] * 4)

edges = ProjectionEdgesView.create({
    "flattened": {"kind": FlattenedEmptyChoiceKind.Empty, "payload": {}},
    "items": [{"kind": FlattenedEmptyChoiceKind.Empty, "payload": {}}],
    "cached": {"kind": CachedEmptyChoiceKind.Empty, "payload": memoryview(array("f"))},
    "floats": memoryview(array("f")), "integers": memoryview(array("I")),
})
assert edges.flattened.tag == FlattenedEmptyChoiceKind.Empty
assert edges.items.get_tag(0) == FlattenedEmptyChoiceKind.Empty
assert len(edges.floats) == len(edges.integers) == 0
assert len(edges.cached.empty_payload) == 0
edges.flattened.replace({"kind": FlattenedEmptyChoiceKind.Absent})
edges.cached.replace({"kind": CachedEmptyChoiceKind.Absent})
assert edges.flattened.tag == FlattenedEmptyChoiceKind.Absent
assert edges.cached.empty_payload is None
edges.cached.replace({"kind": CachedEmptyChoiceKind.Empty, "payload": memoryview(array("f"))})
assert len(edges.cached.empty_payload) == 0

choices = IndexedEnumsView.create({
    "items": [
        {"kind": IndexedChoiceKind.Data, "label": "payload", "vector": [1., 2., 3.], "child": {"value": 7}},
        {"kind": IndexedChoiceKind.Empty}, {"kind": IndexedChoiceKind.Optional, "value": None},
        {"kind": IndexedChoiceKind.Unit, "value": None},
    ],
    "fixed": [{"kind": IndexedChoiceKind.Number, "value": 9}, {"kind": IndexedChoiceKind.Optional, "value": 10}],
})
assert bytes(choices.storage.bytes) == Path("target/regressions/indexed-enums.bin").read_bytes()
items = choices.items
assert not hasattr(items, "__getitem__")
assert items.get_tag(0) == IndexedChoiceKind.Data
assert items.get_data_label(0) == "payload"
assert items.get_data_vector_component(0, 1) == 2
assert items.get_data_child(0).value == 7
assert items.get_optional_value(2).value is None
assert items.get_optional_value(1) is None
assert items.get_unit_value(3).value is None
assert items.get_unit_value(1) is None
before = bytes(choices.storage.bytes)
items.set_number_value(0, 99)
items.set_data_vector_component(1, 0, 99.)
items.set_data_child(1, {"value": 99})
assert bytes(choices.storage.bytes) == before
items.set_data_vector_component(0, 1, 20.)
items.get_data_child(0).value = 30
choices.fixed.set_number_value(0, 40)
assert items.get_data_vector(0)[1] == 20
assert items.get_data_child(0).value == 30
assert choices.fixed[0].number_value == 40

values = {
    "matrix": memoryview(array("f", [1, 2, 3, 4])),
    "points": memoryview(array("f", [5, 6, 7, 8, 9, 10])),
    "tags": memoryview(array("I", [NumericTag.Second, NumericTag.First])),
    "empty": memoryview(array("f")),
}
expected = Path("target/regressions/numeric-inputs.bin").read_bytes()
assert bytes(NumericInputsView.create(values).storage.bytes) == expected
assert bytes(NumericInputsView.create_in_place(bytearray(len(expected)), values).storage.bytes) == expected
for field, count in [("points", 4), ("matrix", 3)]:
    try:
        NumericInputsView.create({**values, field: memoryview(array("f", [0.] * count))})
    except (ValueError, OverflowError):
        pass
    else:
        raise AssertionError("numeric projection accepted an invalid lane count")
empty = NumericInputsView.create({"matrix": intrlay.zeroed(), "points": intrlay.zeroed(2), "tags": memoryview(array("I")), "empty": intrlay.zeroed()})
assert list(empty.matrix) == [0, 0, 0, 0]
assert list(empty.points) == [0, 0, 0, 0, 0, 0]
for count in [-1, 0x100000000]:
    try:
        NumericInputsView.create({**values, "points": intrlay.zeroed(count)})
    except (ValueError, OverflowError):
        pass
    else:
        raise AssertionError("numeric projection accepted an invalid element count")
enum_fields = NumericEnumFieldsView.create({"plain": ZNumericTag.First, "numeric": memoryview(array("I", [NumericTag.Second]))})
assert enum_fields.plain is NumericTag.First and enum_fields.numeric[0] == ZNumericTag.Second
enum_fields.plain = NumericTag.Second
assert enum_fields.plain is ZNumericTag.Second
