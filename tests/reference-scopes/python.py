from pathlib import Path
import sys
from typing import cast, get_type_hints

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'target/reference-scopes'))
import renamed
import omitted
import unrelated_Child
import unrelated_ChildView
import _write_packet
import _write_child
import _measure_packet

view = renamed.PacketView.create({'count': 1, 'item': {'number': 2}})
view.set_item({'number': 3})
assert view.count == 1 and view.item.number == 3
getter = cast(property, renamed.PacketView.__dict__['item']).fget
assert getter is not None and get_type_hints(getter)['return'] is renamed.ChildView
assert omitted.PacketView.create({'ChildView': 4, 'item': {'number': 5}}).ChildView == 4
assert unrelated_Child.PacketView.create({'Child': 6}).Child == 6
assert unrelated_ChildView.PacketView.create({'ChildView': 7}).ChildView == 7

writer = _write_packet.PacketView.create({'_write_packet': 8, 'item': {'number': 9}, 'text': 'ok'})
assert writer._write_packet == 8 and writer.text == 'ok'
setter = _write_child.PacketView.create({'_write_child': 10, 'item': {'number': 11}, 'text': 'ok'})
setter.set_item({'number': 12})
assert setter._write_child == 10 and setter.item.number == 12
measure = _measure_packet.PacketView.create({'_measure_packet': 13, 'item': {'number': 14}, 'text': 'ok'})
assert measure._measure_packet == 13 and measure.item.number == 14
print('Python class reference scopes passed.')
