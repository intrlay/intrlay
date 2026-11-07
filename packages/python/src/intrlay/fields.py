"""Typed factories for built-in property descriptors over Intrlay storage.

Descriptors and accessor closures are created once, when a view class is defined.
Instances retain only their shared storage, byte offset, and declared cache slots.
"""
from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING, Never, Protocol, Self, cast, overload

if TYPE_CHECKING:
    from . import View


class ReadField[T](Protocol):
    """A borrowed value that cannot be replaced through the descriptor."""

    @overload
    def __get__(self, instance: None, owner: type | None = None) -> Self: ...
    @overload
    def __get__(self, instance: View, owner: type | None = None) -> T: ...

    def __set__(self, instance: View, value: Never) -> None: ...


class Field[T](Protocol):
    """A fixed-size scalar supporting reads and in-place writes."""

    @overload
    def __get__(self, instance: None, owner: type | None = None) -> Self: ...
    @overload
    def __get__(self, instance: View, owner: type | None = None) -> T: ...

    def __set__(self, instance: View, value: T) -> None: ...


def read_field[T](get: Callable[[View], T], *, doc: str | None = None) -> ReadField[T]:
    """Expose an existing getter without a Python-level descriptor dispatch."""
    return cast("ReadField[T]", property(get, doc=doc))


def bool_field(offset: int, *, doc: str | None = None) -> Field[bool]:
    """Declare a bool scalar at the given byte offset."""
    def get(instance: View) -> bool:
        return instance._storage.u8[instance._offset + offset] != 0

    def set_(instance: View, value: bool) -> None:
        instance._storage.u8[instance._offset + offset] = value

    return cast("Field[bool]", property(get, set_, doc=doc))


def i8_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a i8 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.i8[instance._offset + offset]

    def set_(instance: View, value: int) -> None:
        instance._storage.i8[instance._offset + offset] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def u8_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a u8 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.u8[instance._offset + offset]

    def set_(instance: View, value: int) -> None:
        instance._storage.u8[instance._offset + offset] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def i16_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a i16 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.i16[(instance._offset + offset) // 2]

    def set_(instance: View, value: int) -> None:
        instance._storage.i16[(instance._offset + offset) // 2] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def u16_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a u16 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.u16[(instance._offset + offset) // 2]

    def set_(instance: View, value: int) -> None:
        instance._storage.u16[(instance._offset + offset) // 2] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def i32_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a i32 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.i32[(instance._offset + offset) // 4]

    def set_(instance: View, value: int) -> None:
        instance._storage.i32[(instance._offset + offset) // 4] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def u32_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a u32 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.u32[(instance._offset + offset) // 4]

    def set_(instance: View, value: int) -> None:
        instance._storage.u32[(instance._offset + offset) // 4] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def i64_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a i64 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.i64[(instance._offset + offset) // 8]

    def set_(instance: View, value: int) -> None:
        instance._storage.i64[(instance._offset + offset) // 8] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def u64_field(offset: int, *, doc: str | None = None) -> Field[int]:
    """Declare a u64 scalar at the given byte offset."""
    def get(instance: View) -> int:
        return instance._storage.u64[(instance._offset + offset) // 8]

    def set_(instance: View, value: int) -> None:
        instance._storage.u64[(instance._offset + offset) // 8] = value

    return cast("Field[int]", property(get, set_, doc=doc))


def f32_field(offset: int, *, doc: str | None = None) -> Field[float]:
    """Declare a f32 scalar at the given byte offset."""
    def get(instance: View) -> float:
        return instance._storage.f32[(instance._offset + offset) // 4]

    def set_(instance: View, value: float) -> None:
        instance._storage.f32[(instance._offset + offset) // 4] = value

    return cast("Field[float]", property(get, set_, doc=doc))


def f64_field(offset: int, *, doc: str | None = None) -> Field[float]:
    """Declare a f64 scalar at the given byte offset."""
    def get(instance: View) -> float:
        return instance._storage.f64[(instance._offset + offset) // 8]

    def set_(instance: View, value: float) -> None:
        instance._storage.f64[(instance._offset + offset) // 8] = value

    return cast("Field[float]", property(get, set_, doc=doc))
