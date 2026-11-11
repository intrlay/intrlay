"""Valid boundary values preserve their bytes and nested presence states."""
from pathlib import Path
import sys

base = Path(__file__).resolve().parents[2] / 'target/edge-cases'
sys.path.insert(0, str(base))
from bindings import EdgeCasesView, NestedView, TextView
from scalar import IntrlayU32View

values = ['', 'hello', '\uFEFF', '\uFEFFhello', '\uFEFF\uFEFFhello', 'a\uFEFFb', '한글😀']
text = TextView.create({'values': values})
assert bytes(text.storage.bytes) == (base / 'text.bin').read_bytes()
assert list(text.values) == values
root = EdgeCasesView(bytearray((base / 'golden.bin').read_bytes()))
assert list(root.text.values) == values
assert root.nested.child.value.value.value == 9
assert root.recursive.child.value.value == 8
assert root.zeroes.after == 42 and len(root.zeroes.units) == 0 and len(root.zeroes.elements) == 2
assert root.shadow.custom.value == 13 and root.shadow.optional == 14 and root.shadow.items[0] == 15
assert root.choice.value_child.value.value == 16
assert len(root.projection.numeric) == 4 and root.projection.named == 2
for i in range(len(root.projection.many)):
    assert root.projection.many.get_x(i) == 3
    root.projection.many.set_x(i, 20)
assert EdgeCasesView(root.storage.bytes).projection.many.get_x(0) == 20
assert NestedView((base / 'absent-0.bin').read_bytes()).child is None
assert NestedView((base / 'absent-1.bin').read_bytes()).child.value is None
assert NestedView((base / 'absent-2.bin').read_bytes()).child.value.value is None
assert IntrlayU32View.create(123).value == 123
print('Python boundary values and composed layouts passed')
