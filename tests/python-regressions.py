"""Shared runtime identity, numeric projections, and Python 3.12+ annotations."""
import sys
from pathlib import Path
from typing import get_type_hints

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'target' / 'regressions'))
import bindings
import alpha
import beta
import intrlay
import arrays
import private_names
import private_helpers
import scalar_setters

scalar = scalar_setters.WrapperView.create({'identifier': 7, 'constant': 11, 'editable': 13})
assert scalar.identifier == 7 and scalar.constant == 11
for name in ['identifier', 'constant']:
    try:
        setattr(scalar, name, 99)
    except AttributeError:
        pass
    else:
        raise AssertionError(f'unrequested nested setter emitted: {name}')
scalar.editable = 17
assert scalar_setters.WrapperView(scalar.buffer).editable == 17

private_input = {
    'child': {'value': 1}, 'children': [{'value': 2}], 'optional': {'value': 3},
    'boxed': {'value': 4}, 'choice': {'kind': 'Present', 'child': {'value': 5}}, 'text': 'hello',
}
private = private_names.__RootView.create(private_input)
assert private.child.value == 1 and private.children[0].value == 2
assert private.optional.value == 3 and private.boxed.value == 4
assert private.choice.present_child.value == 5 and private.text == 'hello'
assert private_names.__ChildView.create({'value': 7}).value == 7
private.child.value = 11
assert private_names.__RootView(private.buffer).child.value == 11
for cls in [private_names.__RootView, private_names.__ChildView, private_names.__ChoiceView]:
    get_type_hints(cls)
    get_type_hints(cls.create)
    get_type_hints(cls.compute_byte_length)
get_type_hints(private_names.__RootView.optional.fget)
get_type_hints(private_names.__Root)

for module in [private_names, private_helpers]:
    keys = get_type_hints(module.PrivateField)
    assert keys == {'__value': int}
    encoded = module.PrivateFieldView.create(module.PrivateField(__value=27))
    assert bytes(encoded.buffer) == (27).to_bytes(4, 'little')
    # View members retain normal Python private-attribute spelling.
    assert encoded._PrivateFieldView__value == 27
    private = module.__RootView.create(private_input)
    assert private.text == 'hello' and private.children[0].value == 2
    reused = module.__RootView.create_in_place(bytearray(len(private.buffer)), private_input)
    assert bytes(reused.buffer) == bytes(private.buffer)

# String reads borrow the encoded bytes until UTF-8 decoding; no bytes copy is needed.
from array import array
for text in ['', 'ASCII', '\ufeffprefix', '한글 😀', '\x00embedded\x00']:
    encoded = text.encode('utf-8')
    raw = array('I', [8, len(encoded)]).tobytes() + encoded
    for buffer in [raw, bytearray(raw), memoryview(b'1234' + raw)[4:]]:
        assert intrlay.Storage(buffer).string_at(0) == text

a = alpha.AlphaView.create({'values': [1, 2]})
b = beta.BetaView.create({'values': [3, 4]})
assert type(a._storage) is type(b._storage) is intrlay.Storage
assert a.values[1] == 2 and b.values[0] == 3
assert bindings.ScalarView.create(1.5).value == 1.5
view = bindings.ProjectionsView.create({'vectors': [{'x': 1, 'y': 2, 'z': 3}]})
assert list(view.vectors) == [1, 2, 3, 0]
view.vectors[1] = 9
assert view.vectors[1] == 9
get_type_hints(intrlay.Storage)
get_type_hints(bindings.ProjectionsView.vectors.fget)
assert isinstance(bindings.ProjectionsView(bytes(view.buffer)).vectors, memoryview)
fixed = arrays.FixedArraysView.create({
    'three': [1, 2, 3], 'four': [4, 5, 6, 7], 'matrix': [[8, 9, 10], [11, 12, 13]],
    'present': [14, 15, 16], 'absent': None, 'read_only': [17, 18, 19],
})
assert isinstance(fixed.three, memoryview) and len(fixed.three) == 3
assert len(fixed.four) == 4 and list(fixed.matrix) == [8, 9, 10, 11, 12, 13]
assert list(fixed.present) == [14, 15, 16] and fixed.absent is None
fixed.three[1] = 20
assert fixed.three[1] == 20
assert a.values is a.values
print('Python: independent modules, numeric projections, and evaluated annotations passed')

import contracts
from intrlay import Some
input = {
    'editable': {'value': 1}, 'read_only': {'value': 2}, 'write_only': {'value': 3},
    'read_only_array': [4,5,6], 'read_only_nested': [{'value': 7}], 'fixed': {'value': 8},
    'identifier': 9, 'identifiers': [10,11], 'maybe': Some(None), 'label': 'hello',
}
view = contracts.ContractsView.create(input)
assert bytes(view.buffer) == (Path(__file__).resolve().parents[1] / 'target/regressions/contracts.bin').read_bytes()
assert contracts.IdentifierView.BYTE_SIZE == 16
assert view.identifier == 9 and view.identifiers[1] == 11
view.identifier = 19
view.identifiers[1] = 21
assert view.identifier == 19 and view.identifiers[1] == 21
assert isinstance(view.maybe, Some) and view.maybe.value is None and view.label == 'hello'
view.editable.value = 30
view.write_only.value = 31
assert view.write_only.value == 31
view.read_only.value = 41
view.read_only_nested[0].value = 42
for child in [view.fixed]:
    try:
        child.value = 40
    except AttributeError:
        pass
    else:
        raise AssertionError('nested write must be unavailable')
assert not view.read_only_array.readonly
view.read_only_array[0] = 40
assert view.read_only_array[0] == 40
projected = contracts.ProjectedAccessView.create({
    'value': {'branch': {'leaf': {'value': 1.5}}},
    'structured': {'branch': {'leaf': {'value': 2.5}}},
    'indexed': [{'value': 3}],
})
assert not projected.value.readonly and projected.value[0] == 1.5
projected.value[0] = 40
assert projected.value[0] == 40
assert projected.structured.branch.leaf.value == 2.5
assert projected.indexed.get_value(0) == 3
projected.indexed.set_value(0, 4)
assert contracts.ProjectedAccessView(projected.buffer).indexed.get_value(0) == 4
print('Python: transparent wrappers, byte parity and nested access passed')

hints = contracts.IndexedHintsView.create({
    'readable': [{'read_score': 7, 'read_vector': [1, 2, 3]}],
    'writable': [{'write_score': 8, 'write_vector': [4, 5, 6]}],
    'hidden': [{'hidden_score': 9, 'hidden_vector': [7, 8, 9]}],
})
assert hints.readable.get_read_score(0) == 7
assert hints.readable.get_read_vector_component(0, 1) == 2
assert not hasattr(hints.readable, 'set_read_score')
assert not hasattr(hints.readable, 'set_read_vector_component')
hints.writable.set_write_score(0, 10)
hints.writable.set_write_vector_component(0, 1, 20)
assert not hasattr(hints.writable, 'get_write_score')
assert not hasattr(hints.writable, 'get_write_vector_component')
assert hints.writable[0].write_vector[1] == 20
assert not hasattr(hints.hidden, 'get_hidden_score')
assert not hasattr(hints.hidden, 'set_hidden_vector_component')
