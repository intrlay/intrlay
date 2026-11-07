"""Each marked statement must fail; generated annotations must not leak Any."""

from bindings import DrawingCommand, MessageView, Point
from edge_bindings import AccessModesView, ScalarView


def invalid_inputs() -> None:
    point: Point = {'x': 'not a float', 'y': 0.0, 'z': 0.0}  # EXPECT: typeddict-item
    command: DrawingCommand = {'kind': 'Missing'}  # EXPECT: typeddict-item
    ScalarView.create('not a float')  # EXPECT: arg-type


def invalid_access(view: MessageView, access: AccessModesView) -> None:
    name: str = view.sequence  # EXPECT: assignment
    view.points[2] = 'not a float'  # EXPECT: call-overload
    access.read_only = 3  # EXPECT: assignment
    access.set_write_only('not an int')  # EXPECT: arg-type
    print(access.write_only)  # EXPECT: attr-defined
