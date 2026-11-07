"""Cross-view mutations must observe current presence and discriminants."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "target/replacement"))
from bindings import MessageView, MaybeScalarView, NestedView, BoxedLeafView, NoFactoryView, EmptyPayloadsView
from intrlay import Some

view = MessageView.create(dict(before=123, fixed=dict(maybe=None, nested=Some(None), choice=dict(kind="Empty")), items=[None], text="preserved", after=456))
alias = MessageView(view.buffer)
fixed = alias.fixed
assert fixed.maybe is None and fixed.nested.value is None
assert fixed.choice.child_leaf is None
leaf = dict(number=7, scalar=8)
boxed = BoxedLeafView.create(None)
boxed_alias = BoxedLeafView(boxed.buffer)
assert boxed_alias.value is None
boxed.set(leaf)
assert boxed_alias.value.number == 7
boxed.set(None)
assert boxed_alias.value is None
view.fixed.set_maybe(leaf)
assert fixed.maybe.number == 7 and fixed.maybe is fixed.maybe
view.fixed.set_nested(Some(leaf))
assert fixed.nested.value.number == 7
view.fixed.choice.replace(dict(kind="Child", leaf=leaf))
assert fixed.choice.child_leaf.number == 7
view.fixed.choice.replace(dict(kind="Numbers", values=[1,2,3]))
assert fixed.choice.child_leaf is None and list(fixed.choice.numbers_values) == [1,2,3]
view.fixed.choice.set_numbers_values(3)
assert list(fixed.choice.numbers_values) == [0,0,0]
view.fixed.set_maybe(None)
view.fixed.set_nested(Some(None))
assert fixed.maybe is None and fixed.nested.value is None
view.fixed.set_nested(None)
assert fixed.nested is None
view.items[0] = leaf
assert alias.items[0].number == 7
view.items[0] = None
assert alias.items[0] is None
scalar = MaybeScalarView.create(None)
no_factory = NoFactoryView(scalar.buffer)
no_factory.replace(dict(value=17))
assert scalar.value == 17
scalar.value = 42
assert scalar.value == 42
scalar.value = None
assert scalar.value is None
nested = NestedView.create(None)
other = NestedView(nested.buffer)
assert other.value is None
nested.replace(Some(leaf))
assert other.value.value.number == 7
nested.replace(Some(None))
assert other.value.value is None
empty_payloads = EmptyPayloadsView.create(dict(empty=[], nested=[[], []], number=1))
empty_alias = EmptyPayloadsView(empty_payloads.buffer)
replacement = dict(empty=[], nested=[[], []], number=2)
empty_payloads.replace(replacement)
assert empty_alias.number == 2 and bytes(empty_alias.buffer) == b"\x02\x00\x00\x00"
assert len(empty_alias.empty) == 0 and len(empty_alias.nested) == 2
assert len(empty_alias.nested[1]) == 0
view.fixed.choice.replace(dict(kind="EmptyArrays", data=replacement))
assert fixed.choice.empty_arrays_data.number == 2
assert alias.before == 123 and alias.after == 456 and alias.text == "preserved"
print("Python: in-place replacement and shared view caches passed")
