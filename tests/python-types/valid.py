"""Type-check representative consumer code against the generated public API."""

from collections.abc import Iterator
from typing import assert_type
from intrlay import zeroed

from bindings import (
    DrawingCommandInput,
    DrawingCommandKind,
    LinkInput,
    LinkView,
    MessageInput,
    MessageView,
    NullableCasesInput,
    NullableCasesView,
    PointInput,
    PointView,
    Some,
)
from edge_bindings import (
    AccessModesInput, AccessModesView, CreateOnlyView, OptionalPointsView,
    ScalarView, UnitFieldsView,
)


def inputs(point: PointInput, command: DrawingCommandInput) -> MessageInput:
    if command['kind'] == DrawingCommandKind.DrawPoint:
        assert_type(command['position'], PointInput)
        assert_type(command['size'], float)
    return {
        'sequence': 1,
        'title': 'Typed input',
        'points': zeroed(3),
        'commands': [command, {'kind': DrawingCommandKind.Clear}],
        'tree': {'id': 1, 'next': {'id': 2, 'next': None}},
        'optional': None,
        'fixed': zeroed(),
    }


class DerivedMessageView(MessageView):
    __slots__ = ()


def views(value: MessageInput, nullable_input: NullableCasesInput, recursive: LinkInput) -> None:
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
    assert_type(borrowed.storage.bytes, memoryview)
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
    input: AccessModesInput = {'read_only': 1, 'write_only': 2}
    access = AccessModesView.create(input)
    assert_type(access.read_only, int)
    access.set_write_only(3)


def keyword_arguments(buffer: bytearray) -> None:
    assert_type(ScalarView.compute_byte_length(value=1.5), int)
    assert_type(CreateOnlyView(buffer=buffer, offset=4), CreateOnlyView)
    assert_type(UnitFieldsView(buffer=buffer, offset=4).values.get_marker(index=0), None)
