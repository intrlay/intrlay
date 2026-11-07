"""Whole alternatives preserve layout and cached views across tag changes."""
import sys
from pathlib import Path

directory = Path(__file__).resolve().parents[2] / "target/cpp-metadata"
sys.path.insert(0, str(directory))
from alternatives import WholeChoiceView

view = WholeChoiceView.create(dict(kind="Value", payload=dict(value=31, marker=37)))
assert bytes(view.buffer) == (directory / "whole.bin").read_bytes()
alias = WholeChoiceView(view.buffer)
assert alias.value is not None and alias.value.value == 31
assert alias.value is alias.value
view.set_value(dict(value=41, marker=43))
assert alias.value.marker == 43
view.replace(dict(kind="Nested", payload=[41, None]))
assert alias.value is None and alias.nested is not None
assert list(alias.nested) == [41, None]
assert bytes(view.buffer) == (directory / "nested.bin").read_bytes()
view.replace(dict(kind="Empty", payload=None))
assert alias.nested is None and alias.empty is not None
view.replace(dict(kind="Value", payload=dict(value=47, marker=53)))
assert alias.empty is None and alias.value is not None
assert alias.value.marker == 53
