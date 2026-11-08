"""Exercise the annotated entry points using real caller-owned storage."""

from array import array
import ctypes
from pathlib import Path
from typing import get_type_hints

from edge_bindings import (
    AccessModesView, CreateOnlyView, NumericValuesView, OptionalPointsView,
    ScalarView, UnitFieldsView,
)

scalar = ScalarView.create(1.5)
assert scalar.value == 1.5
scalar.value = 2.5
assert scalar.value == 2.5

# Public keyword names remain stable even when a layout does not use the input.
assert ScalarView.compute_byte_length(value=1.5) == 4
creation = CreateOnlyView.create({'count': 7, 'label': 'created', 'fixed': [2, 5], 'optional': 11})
owner = bytearray(16) + bytes(creation.buffer)
borrowed_creation = CreateOnlyView(buffer=owner, offset=16)
assert borrowed_creation.buffer.obj is owner
assert not hasattr(borrowed_creation, '_offset')
unit_fields = UnitFieldsView.create([{'marker': None, 'value': 42}])
assert unit_fields.get_marker(index=0) is None
assert unit_fields.get_value(index=0) == 42

access = AccessModesView.create({'read_only': 1, 'write_only': 2})
assert access.read_only == 1
access.set_write_only(7)
assert access.buffer.tobytes() == b'\x01\0\0\0\x07\0\0\0'
assert not hasattr(access, 'write_only')
try:
    access.read_only = 3
except AttributeError:
    pass
else:
    raise AssertionError('read-only accessor accepted a write')

value = [None, {'x': 1.0, 'y': 2.0}]
size = OptionalPointsView.compute_byte_length(value)
buffer = bytearray(size)
view = OptionalPointsView.create_in_place(buffer, value)
assert view.buffer.obj is buffer
assert view[0] is None
point = view[1]
assert point is not None and point.x == 1.0
point.y = 3.0
assert OptionalPointsView(buffer)[1].y == 3.0

# NestedInput views reuse the storage and its typed casts.
assert point._storage is view._storage
assert point._storage.f32 is view._storage.f32
assert point._storage.f32.obj is buffer

# Native numeric views cover all supported widths without signedness changes.
numeric_value = dict(
    enabled=True, signed8=-(1 << 7), unsigned8=(1 << 8) - 1,
    signed16=-(1 << 15), unsigned16=(1 << 16) - 1,
    signed32=-(1 << 31), unsigned32=(1 << 32) - 1,
    signed64=-(1 << 63), unsigned64=(1 << 64) - 1,
    real32=-1.25, real64=12345.125, label='x',
)
reference = (Path(__file__).resolve().parents[2] / 'target/python-types/numeric.bin').read_bytes()
numeric = NumericValuesView.create(numeric_value)
assert bytes(numeric.buffer) == reference
assert len(reference) % 8 != 0  # The final string does not pad to a numeric width.
for field, expected in numeric_value.items():
    assert getattr(numeric, field) == expected

numeric.enabled = False
numeric.signed8 = (1 << 7) - 1
numeric.signed16 = (1 << 15) - 1
numeric.signed32 = (1 << 31) - 1
numeric.signed64 = (1 << 63) - 1
numeric.unsigned8 = numeric.unsigned16 = numeric.unsigned32 = numeric.unsigned64 = 0
numeric.real32 = numeric.real64 = -0.5
assert not numeric.enabled
assert numeric.signed8 == (1 << 7) - 1
assert numeric.signed16 == (1 << 15) - 1
assert numeric.signed32 == (1 << 31) - 1
assert numeric.signed64 == (1 << 63) - 1
assert numeric.unsigned8 == numeric.unsigned16 == numeric.unsigned32 == numeric.unsigned64 == 0
assert numeric.real32 == numeric.real64 == -0.5

# Borrow a caller's sub-buffer, keeping the surrounding bytes intact.
size = NumericValuesView.compute_byte_length(numeric_value)
alignment = NumericValuesView.ALIGNMENT
owner = bytearray([0xA5]) * (size + alignment * 3)
address = ctypes.addressof(ctypes.c_char.from_buffer(owner))
start = alignment + (-address % alignment)
borrowed = NumericValuesView.create_in_place(memoryview(owner)[start:start + size], numeric_value)
assert borrowed.buffer.obj is owner
assert owner[:start] == bytes([0xA5]) * start
assert owner[start + size:] == bytes([0xA5]) * (len(owner) - start - size)
offset_view = NumericValuesView(owner, start)
assert offset_view.unsigned64 == (1 << 64) - 1
offset_view.real32 = 2.5
assert borrowed.real32 == 2.5

# Read-only and already typed buffers retain the buffer protocol's semantics.
readonly = NumericValuesView(reference)
assert readonly.signed64 == -(1 << 63)
try:
    readonly.real64 = 0.0
except TypeError:
    pass
else:
    raise AssertionError('read-only backing bytes accepted a write')
typed_owner = array('f', [1.5])
typed_view = ScalarView(memoryview(typed_owner))
typed_view.value = 3.5
assert typed_owner[0] == 3.5

# Deferred annotations remain inspectable as well as statically checked.
assert get_type_hints(ScalarView.value.fget)['return'] is float
print('Python API: access modes, numeric widths, shared casts, sub-buffers, and annotations passed')
