"""Type-check representative consumer code against the generated public API."""

from collections.abc import Iterator
from typing import assert_type

from bindings import (
    DrawingCommand,
    Link,
    LinkView,
    Message,
    MessageView,
    NullableCases,
    NullableCasesView,
    Point,
    PointView,
    Some,
)
from edge_bindings import AccessModes, AccessModesView, OptionalPointsView, ScalarView


def inputs(point: Point, command: DrawingCommand) -> Message:
    if command['kind'] == 'DrawPoint':
        assert_type(command['position'], Point)
        assert_type(command['size'], float)
    return {
        'sequence': 1,
        'title': 'Typed input',
        'points': [point],
        'commands': [command, {'kind': 'Clear'}],
        'tree': {'id': 1, 'next': {'id': 2, 'next': None}},
        'optional': None,
        'fixed': [1, 2, 3],
    }


class DerivedMessageView(MessageView):
    __slots__ = ()


def views(value: Message, nullable_input: NullableCases, recursive: Link) -> None:
    view = MessageView.create(value)
    assert_type(view, MessageView)
    assert_type(DerivedMessageView.create(value), DerivedMessageView)
    assert_type(view.sequence, int)
    assert_type(view.title, str)
    assert_type(view.points[0], float)
    assert_type(iter(view.points), Iterator[float])
    assert_type(view.commands[0].draw_point_position, PointView | None)
    assert_type(view.tree.next, LinkView | None)
    assert_type(LinkView.create(recursive), LinkView)
    view.sequence = 3
    view.points[2] = 1.5
    view.fixed[1] = 2.5
    borrowed = MessageView.create_in_place(bytearray(4096), value)
    assert_type(borrowed, MessageView)
    assert_type(borrowed.buffer, memoryview)
    nullable = NullableCasesView.create(nullable_input)
    assert_type(nullable.nested_value, Some[int | None] | None)
    assert_type(nullable.triple, Some[Some[int | None] | None] | None)
    assert_type(nullable.variants[0].value_value, Some[int | None] | None)


def explicit_exports() -> None:
    scalar = ScalarView.create(1.5)
    assert_type(scalar.value, float)
    scalar.value = 2.0
    array = OptionalPointsView.create([None, {'x': 1.0, 'y': 2.0}])
    assert array[0] is None
    input: AccessModes = {'read_only': 1, 'write_only': 2}
    access = AccessModesView.create(input)
    assert_type(access.read_only, int)
    access.set_write_only(3)
