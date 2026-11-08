from pathlib import Path
import sys
from typing import cast, get_type_hints

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'target/reference-scopes'))
import renamed
import omitted
import unrelated_ChildInput
import unrelated_ChildView
import _write_packet
import _write_child
import _measure_packet
import native_enum_names
import collection
import omitted_factories
import object_names

view = renamed.PacketView.create({'count': 1, 'item': {'number': 2}})
view.set_item({'number': 3})
assert view.count == 1 and view.item.number == 3
getter = cast(property, renamed.PacketView.__dict__['item']).fget
assert getter is not None and get_type_hints(getter)['return'] is renamed.ChildView
assert omitted.PacketView.create({'ChildView': 4, 'item': {'number': 5}}).ChildView == 4
assert unrelated_ChildInput.PacketView.create({'ChildInput': 6}).ChildInput == 6
assert unrelated_ChildView.PacketView.create({'ChildView': 7}).ChildView == 7

writer = _write_packet.PacketView.create({'_write_packet': 8, 'item': {'number': 9}, 'text': 'ok'})
assert writer._write_packet == 8 and writer.text == 'ok'
setter = _write_child.PacketView.create({'_write_child': 10, 'item': {'number': 11}, 'text': 'ok'})
setter.set_item({'number': 12})
assert setter._write_child == 10 and setter.item.number == 12
measure = _measure_packet.PacketView.create({'_measure_packet': 13, 'item': {'number': 14}, 'text': 'ok'})
assert measure._measure_packet == 13 and measure.item.number == 14
items = collection.ItemsView.create([collection.ChoiceInput.Ready])
assert len(items) == 1 and items[0].getTag == 0
items[0] = collection.ChoiceInput.Ready
assert [item.getTag for item in items] == [0]
assert omitted_factories.ChoiceView.byte_size_create == 4
assert omitted_factories.ChoiceView(bytes(4)).tag == 0

input_only = object_names.InputOnlyView.create({'__repr__': 3, '__str__': 5, '__eq__': 7})
setter_only = object_names.SetterOnlyView.create({'__repr__': 3, '__str__': 5, '__eq__': 7})
setter_only.set_repr(11)
setter_only.set_str(13)
setter_only.set_eq(17)
assert list(input_only.buffer.cast('I')) == [3, 5, 7]
assert list(setter_only.buffer.cast('I')) == [11, 13, 17]
for object_view in (input_only, setter_only):
    assert repr(object_view) == object.__repr__(object_view)
    assert str(object_view) == repr(object_view)
    assert object_view == object_view
    assert hash(object_view) == object.__hash__(object_view)
print('Python class reference scopes passed.')
