"""Intrlay buffer views for Python 3.12+. Encodings are trusted inputs."""
from __future__ import annotations

import ctypes
import sys
from array import array
from collections.abc import Buffer, Callable, Iterator, Sequence, Sized
from typing import TYPE_CHECKING, NewType, overload
from enum import Enum


ZeroedSequence = NewType("ZeroedSequence", int)
"""A sequence length explicitly requested without intermediate elements."""


class Zeroed:
    """Zero initialization using the fixed array's schema length."""

    __slots__ = ()


_zeroed_array = Zeroed()


@overload
def zeroed() -> Zeroed: ...


@overload
def zeroed(length: int) -> ZeroedSequence: ...


def zeroed(length: int | None = None) -> Zeroed | ZeroedSequence:
    """Request zeroed elements; omit length only for a fixed array."""
    return _zeroed_array if length is None else ZeroedSequence(length)


class CacheState(Enum):
    """An unevaluated child view, distinct from a cached absent value."""

    UNINITIALIZED = 0

if TYPE_CHECKING:
    type IntView = memoryview[int]
    type FloatView = memoryview[float]
else:
    # memoryview became subscriptable at runtime in Python 3.14.
    IntView = memoryview
    FloatView = memoryview


if sys.byteorder != "little":
    raise RuntimeError("Intrlay raw memory views require a little-endian host")

# memoryview.cast uses native C formats. Check their widths once for the module.
if (
    ctypes.sizeof(ctypes.c_short), ctypes.sizeof(ctypes.c_int), ctypes.sizeof(ctypes.c_longlong),
    ctypes.sizeof(ctypes.c_float), ctypes.sizeof(ctypes.c_double),
) != (2, 4, 8, 4, 8):
    raise RuntimeError("Intrlay requires 16/32/64-bit integers and 32/64-bit floats")


class Cursor:
    __slots__ = ("end",)
    end: int

    def __init__(self, end: int) -> None:
        self.end = end

    def reserve(self, size: int, alignment: int) -> int:
        start = (self.end + alignment - 1) & -alignment
        self.end = start + size
        if size < 0 or self.end > 0xffffffff:
            raise OverflowError("Intrlay buffer exceeds u32 capacity")
        return start

def count(value: int | Sized) -> int:
    return value if isinstance(value, int) else len(value)


def try_copy_numeric_array(
    target: IntView | FloatView, start: int, values: Sequence[int] | Sequence[float]
) -> bool:
    """Copy compatible contiguous lanes without an intermediate bytes object.

    Return False for ordinary sequences or different scalar formats so the
    generated element writer retains its numeric conversion and range checks.
    Source and destination must not overlap, as with all encoding inputs.
    """
    if (
        isinstance(values, memoryview)
        and values.format == target.format
        and values.ndim == 1
        and values.c_contiguous
    ) or (
        isinstance(values, array) and values.typecode == target.format
    ) or (
        target.format == "B" and isinstance(values, (bytes, bytearray))
    ):
        target[start:start + len(values)] = values
        return True
    return False


class Storage:
    """Shared byte storage with lazily cached native numeric views."""

    __slots__ = (
        "bytes", "_i8", "_i16", "_u16",
        "_i32", "_u32", "_i64", "_u64",
        "_f32", "_f64",
    )

    bytes: IntView
    _i8: IntView | None
    _i16: IntView | None
    _u16: IntView | None
    _i32: IntView | None
    _u32: IntView | None
    _i64: IntView | None
    _u64: IntView | None
    _f32: FloatView | None
    _f64: FloatView | None

    def __init__(self, buffer: Buffer) -> None:
        view = buffer if isinstance(buffer, memoryview) else memoryview(buffer)
        self.bytes = view if view.format == "B" and view.ndim == 1 else view.cast("B")
        self._i8 = None
        self._i16 = None
        self._u16 = None
        self._i32 = None
        self._u32 = None
        self._i64 = None
        self._u64 = None
        self._f32 = None
        self._f64 = None

    @property
    def u8(self) -> IntView:
        return self.bytes

    @property
    def i8(self) -> IntView:
        if self._i8 is None:
            self._i8 = self.bytes.cast("b")
        return self._i8

    @property
    def i16(self) -> IntView:
        if self._i16 is None:
            end = len(self.bytes) // 2 * 2
            self._i16 = self.bytes[:end].cast("h")
        return self._i16

    @property
    def u16(self) -> IntView:
        if self._u16 is None:
            end = len(self.bytes) // 2 * 2
            self._u16 = self.bytes[:end].cast("H")
        return self._u16

    @property
    def i32(self) -> IntView:
        if self._i32 is None:
            end = len(self.bytes) // 4 * 4
            self._i32 = self.bytes[:end].cast("i")
        return self._i32

    @property
    def u32(self) -> IntView:
        if self._u32 is None:
            end = len(self.bytes) // 4 * 4
            self._u32 = self.bytes[:end].cast("I")
        return self._u32

    @property
    def i64(self) -> IntView:
        if self._i64 is None:
            end = len(self.bytes) // 8 * 8
            self._i64 = self.bytes[:end].cast("q")
        return self._i64

    @property
    def u64(self) -> IntView:
        if self._u64 is None:
            end = len(self.bytes) // 8 * 8
            self._u64 = self.bytes[:end].cast("Q")
        return self._u64

    @property
    def f32(self) -> FloatView:
        if self._f32 is None:
            end = len(self.bytes) // 4 * 4
            self._f32 = self.bytes[:end].cast("f")
        return self._f32

    @property
    def f64(self) -> FloatView:
        if self._f64 is None:
            end = len(self.bytes) // 8 * 8
            self._f64 = self.bytes[:end].cast("d")
        return self._f64

    def string_at(self, offset: int) -> str:
        integers = self.u32
        start = offset + integers[offset // 4]
        length = integers[offset // 4 + 1]
        return str(self.bytes[start:start + length], "utf-8")


class BufferView:
    """Backing storage for views whose generated API needs no record offset."""

    __slots__ = ("_storage",)
    _storage: Storage

    def __init__(self, buffer: Buffer | Storage, offset: int = 0) -> None:  # noqa: ARG002 - Preserve the view constructor's offset keyword.
        self._storage = buffer if isinstance(buffer, Storage) else Storage(buffer)


class View(BufferView):
    """Backing storage and record offset for generated accessors."""

    __slots__ = ("_offset",)
    _offset: int

    def __init__(self, buffer: Buffer | Storage, offset: int = 0) -> None:
        self._storage = buffer if isinstance(buffer, Storage) else Storage(buffer)
        self._offset = offset


def allocate(size: int, alignment: int) -> IntView:
    owner = bytearray(size + alignment - 1)
    if not owner:
        return memoryview(owner)
    address = ctypes.addressof(ctypes.c_char.from_buffer(owner))
    start = -address % alignment
    return memoryview(owner)[start:start + size]

def initialize[View: BufferView, Value](
    cls: type[View],
    buffer: Buffer,
    value: Value,
    size: int,
    header_size: int,
    alignment: int,
    writer: Callable[[Storage, int, Value, Cursor], None],
    *,
    check_extent: bool = False,
) -> View:
    view = memoryview(buffer).cast("B")
    if len(view) < size:
        raise ValueError("insufficient buffer capacity")
    if view.readonly:
        raise TypeError("construction requires writable storage")
    if size and ctypes.addressof(ctypes.c_char.from_buffer(view)) % alignment:
        raise ValueError("misaligned buffer")
    view = view[:size]
    # Clear padding in native code without a second payload-sized allocation.
    if size:
        ctypes.memset(ctypes.addressof(ctypes.c_char.from_buffer(view)), 0, size)
    storage = Storage(view)
    cursor = Cursor(header_size)
    writer(storage, 0, value, cursor)
    if check_extent and cursor.end != size:
        raise ValueError("Intrlay layout changed during construction")
    return cls(storage)


class Some[T]:
    """Present optional whose payload can itself be None."""

    __slots__ = ("_value",)
    _value: T

    def __init__(self, value: T) -> None:
        self._value = value

    @property
    def value(self) -> T:
        """Returns the fixed payload reference; a contained view can still be writable."""
        return self._value


from .fields import (
    Field as Field,
    ReadField as ReadField,
    read_field as read_field,
    bool_field as bool_field,
    i8_field as i8_field,
    u8_field as u8_field,
    i16_field as i16_field,
    u16_field as u16_field,
    i32_field as i32_field,
    u32_field as u32_field,
    i64_field as i64_field,
    u64_field as u64_field,
    f32_field as f32_field,
    f64_field as f64_field,
)
