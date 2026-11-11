"""Array ranges expose contiguous elements; other fields retain inline storage."""
from array import array
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "target/field-helpers"))
from bindings import (FieldRecordView, FieldRowsView, RawChoiceView, RawChoicesView,
                      ReadHelperView, WriteHelperView, CreateHelperView, LanguageHelperView,
                      ArrayRangesView, ArrayChoicesView, FoldedHelpersView)
from intrlay import Storage

sys.path.insert(0, str(ROOT / "target/naming"))
from enum_properties import PayloadEventView, PayloadEventKind


def encoded(name):
    return bytearray((ROOT / f"target/field-helpers/{name}.bin").read_bytes())


fields = [
    ("matrix", 0, 64), ("padded", 64, 16), ("label", 80, 8),
    ("maybe", 96, 12), ("boxed", 108, 4), ("empty", 112, 0), ("child", 112, 12),
    ("flat_first", 124, 4), ("flat_rest_0", 128, 4), ("flat_rest_1", 132, 4),
]
record = FieldRecordView(encoded("record"))
assert FieldRecordView.BYTE_SIZE == 144
for name, offset, length in fields:
    assert getattr(record, f"{name}_byte_offset") == offset, name
    assert getattr(record, f"{name}_byte_length") == length, name
for name in ("matrix", "padded", "label", "values", "maybe", "boxed", "empty", "child", "flat", "flat_byte_offset", "set_matrix", "_cached_matrix"):
    assert not hasattr(record, name), name
assert struct.unpack_from("<f", record.storage.bytes, record.matrix_byte_offset + 60)[0] == 16
assert record.values_byte_length == 16 and record.values_byte_offset >= FieldRecordView.BYTE_SIZE
assert struct.unpack_from("<4f", record.storage.bytes, record.values_byte_offset) == (21, 22, 23, 24)
assert struct.unpack_from("<I", record.storage.bytes, record.label_byte_offset + 4)[0] == 11
assert record.storage.bytes[record.maybe_byte_offset] == 1
assert bytes(record.storage.bytes[76:80]) == bytes(4)
assert struct.unpack_from("<I", record.storage.bytes, record.child_byte_offset)[0] == 29
assert struct.unpack_from("<f", record.storage.bytes, record.flat_rest_1_byte_offset)[0] == 32
assert not hasattr(record, "get_padded_component")
assert [record.get_matrix_component(15), record.get_values_component(2), record.get_maybe_component(1), record.get_boxed_component(0)] == [16, 23, 26, 27]
record.set_matrix_component(15, 99)
struct.pack_into("<f", record.storage.bytes, record.padded_byte_offset + 4, 61)
record.set_values_component(2, 51)
record.set_maybe_component(1, 41)
record.set_boxed_component(0, 71)
assert record.storage.bytes == encoded("changed")

original = encoded("record")
backing = bytearray(len(original) + 64)
backing[32:32 + len(original)] = original
shifted = FieldRecordView(Storage(memoryview(backing)[16:32 + len(original)]), 16)
for name, offset, length in fields:
    assert getattr(shifted, f"{name}_byte_offset") == 16 + offset
    assert getattr(shifted, f"{name}_byte_length") == length
assert struct.unpack_from("<f", shifted.storage.bytes, shifted.matrix_byte_offset + 60)[0] == 16
assert shifted.values_byte_offset == record.values_byte_offset + 16
assert shifted.values_byte_length == 16
assert struct.unpack_from("<f", shifted.storage.bytes, shifted.values_byte_offset)[0] == 21
shifted.set_boxed_component(0, 72)
assert shifted.get_boxed_component(0) == 72
assert backing[:32] == bytes(32)
absent = FieldRecordView(encoded("absent"))
before = bytes(absent.storage.bytes)
assert absent.maybe_byte_offset == 96 and absent.maybe_byte_length == 12
assert absent.storage.bytes[absent.maybe_byte_offset] == 0
assert absent.get_maybe_component(0) is None
absent.set_maybe_component(0, 99)
assert bytes(absent.storage.bytes) == before

rows = FieldRowsView(encoded("rows"))
assert len(rows.rows) == 2
assert rows.rows_byte_offset == 16 and rows.rows_byte_length == 288
for name in ("__getitem__", "get_matrix", "set_matrix", "get_flat_byte_offset", "_cached_matrix"):
    assert not hasattr(rows.rows, name), name
for name, offset, length in fields:
    assert getattr(rows.rows, f"get_{name}_byte_offset")(0) == 16 + offset
    assert getattr(rows.rows, f"get_{name}_byte_offset")(1) == 160 + offset
    assert getattr(rows.rows, f"get_{name}_byte_length")(1) == length
for index in range(2):
    assert rows.rows.get_values_byte_length(index) == 16
    assert struct.unpack_from("<f", rows.storage.bytes, rows.rows.get_values_byte_offset(index))[0] == 21
rows.rows.set_matrix_component(1, 15, 99)
assert not hasattr(rows.rows, "get_padded_component")
struct.pack_into("<f", rows.storage.bytes, rows.rows.get_padded_byte_offset(1) + 4, 61)
rows.rows.set_values_component(1, 2, 51)
rows.rows.set_maybe_component(1, 1, 41)
rows.rows.set_boxed_component(1, 0, 71)
assert rows.rows.get_matrix_component(0, 15) == 16
assert rows.rows.get_matrix_component(1, 15) == 99
visible = rows.visible[0]
assert visible.lanes is visible.lanes
assert not hasattr(visible, "get_lanes_component")
assert not hasattr(rows.visible, "get_lanes_component")
assert rows.visible.get_lanes(0) is not rows.visible.get_lanes(0)
visible.lanes[0] = 70
assert rows.storage.bytes == encoded("rowsChanged")

inactive = RawChoiceView(encoded("inactive"))
for name, offset, length in (("value", 4, 8), ("label", 12, 8), ("maybe", 20, 12), ("boxed", 32, 4)):
    assert getattr(inactive, f"data_{name}_byte_offset") == offset
    assert getattr(inactive, f"data_{name}_byte_length") == length
assert inactive.get_data_value_component(0) is None
assert inactive.get_data_boxed_component(0) is None
before = bytes(inactive.storage.bytes)
inactive.set_data_boxed_component(0, 99)
assert bytes(inactive.storage.bytes) == before
active = RawChoiceView(encoded("active"))
assert active.data_maybe_byte_length == 12
assert active.get_data_maybe_component(0) is None
choices = RawChoicesView(encoded("choices"))
assert choices.items.get_data_value_byte_offset(0) == 12
assert choices.items.get_data_boxed_byte_offset(0) == 40
assert choices.items.get_data_value_byte_offset(1) == 48
assert choices.items.get_data_value_byte_length(0) == 8
assert choices.items.get_data_value_component(0, 0) is None
before = bytes(choices.storage.bytes)
choices.items.set_data_value_component(0, 0, 99)
assert bytes(choices.storage.bytes) == before
choices.items.set_data_value_component(1, 1, 90)
choices.items.set_data_boxed_component(1, 0, 80)
assert choices.storage.bytes == encoded("choicesChanged")

lanes = memoryview(array("f", [1, 2]))
read = ReadHelperView.create({"lanes": lanes})
assert read.get_lanes_component(1) == 2 and read.lanes_byte_length == 8
assert not hasattr(read, "set_lanes_component")
write = WriteHelperView.create({"lanes": lanes})
write.set_lanes_component(1, 9)
assert struct.unpack_from("<f", write.storage.bytes, 4)[0] == 9
assert not hasattr(write, "get_lanes_component") and not hasattr(write, "lanes_byte_offset")
created = CreateHelperView.create({"lanes": lanes})
assert struct.unpack_from("<f", created.storage.bytes, 4)[0] == 2
for name in ("get_lanes_component", "set_lanes_component", "lanes_byte_offset"):
    assert not hasattr(created, name), name
filtered = LanguageHelperView.create({"filtered": memoryview(array("f", [3, 4]))})
assert filtered.filtered_byte_offset == 0 and filtered.filtered_byte_length == 8
assert not hasattr(filtered, "filtered") and not hasattr(filtered, "get_filtered_component")
arrays = ArrayRangesView(encoded("arrays"))
def u32(view, offset):
    return struct.unpack_from("<I", view.storage.bytes, offset)[0]
assert arrays.records_byte_length == 24
assert u32(arrays, arrays.records_byte_offset) == 101
assert u32(arrays, arrays.records_byte_offset + 12) == 202
assert arrays.fixed_byte_length == 24
assert u32(arrays, arrays.fixed_byte_offset) == 303
assert u32(arrays, arrays.fixed_byte_offset + 12) == 404
assert arrays.padded_byte_length == 32
assert struct.unpack_from("<f", arrays.storage.bytes, arrays.padded_byte_offset + 16)[0] == 4
assert u32(arrays, arrays.padded_byte_offset + 12) == 0
assert arrays.nested_byte_length == 16
inner = arrays.nested_byte_offset + u32(arrays, arrays.nested_byte_offset)
assert u32(arrays, inner) == 11
assert arrays.empty_byte_length == 0 and arrays.empty_byte_offset <= len(arrays.storage.bytes)
assert arrays.units_byte_length == 0 and arrays.units_byte_offset <= len(arrays.storage.bytes)
assert arrays.folded_byte_length == 8 and u32(arrays, arrays.folded_byte_offset) == 51
array_choices = ArrayChoicesView(encoded("arrayChoices"))
struct.pack_into("<I", array_choices.storage.bytes, 12, 0xffffffff)
assert array_choices.items.get_data_records_byte_offset(0) is None
assert array_choices.items.get_data_records_byte_length(0) is None
assert array_choices.items.get_data_units_byte_length(0) is None
assert array_choices.items.get_data_records_byte_length(1) == 12
assert array_choices.items.get_data_units_byte_length(1) == 0
assert u32(array_choices, array_choices.items.get_data_records_byte_offset(1)) == 505
folded_helpers = FoldedHelpersView.create({"samples": [{"lanes": [3, 7]}]})
assert folded_helpers.storage.bytes == encoded("foldedHelpers")
assert folded_helpers.samples.get_lanes_component(0, 1) == 7
assert folded_helpers.samples is folded_helpers.samples
assert not hasattr(folded_helpers.samples, "__getitem__")
assert not hasattr(folded_helpers.samples, "get_lanes")
folded_helpers.samples.set_lanes_component(0, 1, 11)
assert folded_helpers.storage.bytes == encoded("foldedHelpersChanged")
payload = PayloadEventView.create({"kind": PayloadEventKind.VariantTaskDone, "input_request_id": 7, "input_saved_id": 11})
assert payload.write_task_done_request_id == 7
payload.write_task_done_request_id = 19
assert payload.write_task_done_request_id == 19 and payload.read_task_done_saved_id == 11
assert PayloadEventView.read_task_done_saved_id.fset is None
payload.replace({"kind": PayloadEventKind.Delivered, "Ticket": 23})
assert payload.write_delivered_ticket == 23
payload.write_delivered_ticket = 29
assert payload.write_delivered_ticket == 29 and payload.write_task_done_request_id is None
payload.replace({"kind": PayloadEventKind.VariantFlattened, "input_payload": {"input_request_id": 31}})
assert payload.write_flattened_payload_request_id == 31
payload.write_flattened_payload_request_id = 37
assert payload.write_flattened_payload_request_id == 37
payload.replace({"kind": PayloadEventKind.VariantSetOnly, "input_request_id": 41})
payload.set_read_set_only_request_id(43)
assert payload.input_storage.u32[1] == 43
assert not hasattr(payload, "read_set_only_request_id")
print("Python: field ranges, component-only access, policies, and byte parity passed")
