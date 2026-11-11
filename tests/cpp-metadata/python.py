"""Whole alternatives preserve layout and cached views across tag changes."""
import sys
from pathlib import Path

directory = Path(__file__).resolve().parents[2] / "target/cpp-metadata"
sys.path.insert(0, str(directory))
from alternatives import WholeChoiceKind, WholeChoiceView
from enum_getters import WholeChoiceKind as NamedWholeKind, WholeChoiceView as NamedWholeView
from enum_getters import OrdinaryChoiceKind as NamedOrdinaryKind, OrdinaryChoiceView as NamedOrdinaryView

named_whole = NamedWholeView.create(dict(kind=NamedWholeKind.Value, payload=dict(value=31, marker=37)))
assert named_whole.read_value is not None and named_whole.read_value.read_value == 31
named_ordinary = NamedOrdinaryView.create(dict(kind=NamedOrdinaryKind.Value, value=23, native_extra=29))
assert named_ordinary.read_value_value is not None and named_ordinary.read_value_value.value == 23

view = WholeChoiceView.create(dict(kind=WholeChoiceKind.Value, payload=dict(value=31, marker=37)))
assert bytes(view.storage.bytes) == (directory / "whole.bin").read_bytes()
alias = WholeChoiceView(view.storage.bytes)
assert alias.value is not None and alias.value.value == 31
assert alias.value is alias.value
view.set_value(dict(value=41, marker=43))
assert alias.value.marker == 43
view.replace(dict(kind=WholeChoiceKind.Nested, payload=[41, None]))
assert alias.value is None and alias.nested is not None
assert list(alias.nested) == [41, None]
assert bytes(view.storage.bytes) == (directory / "nested.bin").read_bytes()
view.replace(dict(kind=WholeChoiceKind.Empty, payload=None))
assert alias.nested is None and alias.empty is not None
view.replace(dict(kind=WholeChoiceKind.Value, payload=dict(value=47, marker=53)))
assert alias.empty is None and alias.value is not None
assert alias.value.marker == 53
