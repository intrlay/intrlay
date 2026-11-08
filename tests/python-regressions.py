"""Shared runtime identity, numeric projections, and Python 3.12+ annotations."""
import sys
from pathlib import Path
from typing import Literal, get_type_hints

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'target' / 'regressions'))
import bindings
import alpha
import beta
import intrlay
import arrays
import private_names
import private_helpers
import scalar_setters
import input_emission as inputs
import storage_emission as storage
import view_emission as outbound

outbound_input: outbound.OutboundPacketInput = {
    'records': [{'value': 7, 'label': 'nested'}],
    'optional': {'value': 8, 'label': 'optional'},
    'choice': {'kind': outbound.InternalChoiceKind.Record, 'payload': {'value': 9, 'label': 'variant'}},
}
outbound_expected = Path('target/regressions/view_emission.bin').read_bytes()
assert bytes(outbound.OutboundPacketView.create(outbound_input).buffer) == outbound_expected
assert bytes(outbound.OutboundPacketView.create_in_place(bytearray(len(outbound_expected)), outbound_input).buffer) == outbound_expected
assert not hasattr(outbound, 'InternalRecordView') and not hasattr(outbound, 'ZInternalRecordView')

creation_input: storage.CreateOnlyInput = {'count': 7, 'label': 'created', 'fixed': [2, 5], 'optional': 11}
creation_expected = Path('target/regressions/storage_emission_created.bin').read_bytes()
for created in [storage.CreateOnlyView.create(creation_input),
                storage.CreateOnlyView.create_in_place(bytearray(len(creation_expected)), creation_input)]:
    assert bytes(created.buffer) == creation_expected
    assert not hasattr(created, '_offset')
    assert not hasattr(created, '__dict__')
assert not hasattr(storage, 'DisabledNamesView')
unit_bytes = bytes(16) + Path('target/regressions/storage_emission_units.bin').read_bytes()
units = storage.UnitItemsView(unit_bytes, 16).values
assert len(units) == 3 and units[2] is None
assert not hasattr(units, '_offset') and not hasattr(units, '_element_offset')
zero_array = storage.ZeroArrayView(bytearray())
zero_array.set_values([])
zero_array.replace({'values': []})
assert len(zero_array.values) == 0

incoming = inputs.ReadOnlyView(Path('target/regressions/input_emission_read.bin').read_bytes())
assert incoming.label == 'incoming' and incoming.next is None
assert incoming.event.tag == inputs.ReadOnlyEventKind.Data
assert incoming.event.data_kind == 42 and incoming.state.tag == inputs.ReadOnlyStateInput.On
assert type(incoming.event.tag) is int
assert not hasattr(inputs.ReadOnlyView, 'create')
assert inputs.ZReadOnlyView is inputs.ReadOnlyView
for name in ['ReadOnlyInput', '__ReadOnlyInput', 'ZReadOnlyInput', 'ReadOnlyEventInput',
             'ReadOnlyEventDataInput', 'ZReadOnlyEventInput', 'FieldOnlyInput']:
    assert not hasattr(inputs, name), name
get_type_hints(inputs.ReadOnlyView.next.fget)
parent = inputs.WriteParentView.create({
    'leaf': {'value': 1}, 'leaves': [{'value': 2}],
    'event': {'kind': inputs.WriteEventKind.Data, 'leaf': {'value': 3}},
})
assert parent.leaf.value == 1 and parent.leaves[0].value == 2 and parent.event.data_leaf.value == 3
assert not hasattr(inputs.WriteLeafView, 'create')
field_only = inputs.FieldOnlyView(bytearray(Path('target/regressions/input_emission_mutation.bin').read_bytes()))
field_only.set_leaf({'value': 8})
field_only.leaf.replace({'value': 9})
assert field_only.leaf.value == 9 and field_only.label == 'mutable field'
assert not hasattr(inputs.FieldOnlyView, 'create')

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
    'boxed': {'value': 4}, 'choice': {'kind': private_names.__ChoiceKind.Present, 'child': {'value': 5}}, 'text': 'hello',
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
get_type_hints(private_names.__RootInput)

for module in [private_names, private_helpers]:
    kind = module.__ChoiceKind
    assert get_type_hints(module.__ChoicePresentInput)['kind'] == Literal[kind.Present]
    assert get_type_hints(module.__ChoicePrivateInput) == {'kind': Literal[kind.Private], '__number': int}
    private_choice = module.__ChoiceView.create({'kind': kind.Private, '__number': 29})
    assert bytes(private_choice.buffer) == (2).to_bytes(4, 'little') + (29).to_bytes(4, 'little')
    keys = get_type_hints(module.PrivateFieldInput)
    assert keys == {'__value': int}
    encoded = module.PrivateFieldView.create(module.PrivateFieldInput(__value=27))
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

# Compatible buffer inputs copy their native lanes; ordinary and strided
# sequences keep element conversion. Both paths preserve caller-owned slices.
from importlib import import_module
numeric = import_module('numeric-copy')
for name, code, pair in [
    ('I8', 'b', [-128, 127]), ('U8', 'B', [0, 255]),
    ('I16', 'h', [-32768, 32767]), ('U16', 'H', [0, 65535]),
    ('I32', 'i', [-(1 << 31), (1 << 31) - 1]), ('U32', 'I', [0, (1 << 32) - 1]),
    ('I64', 'q', [-(1 << 63), (1 << 63) - 1]), ('U64', 'Q', [0, (1 << 64) - 1]),
    ('F32', 'f', [1.25, -0.0]), ('F64', 'd', [1 / 3, -float('inf')]),
]:
    cls = getattr(numeric, name + 'View')
    values = array(code, pair * 64)
    for source in [values, memoryview(values), memoryview(values).toreadonly(),
                   memoryview(values)[1:-1], memoryview(values)[::2],
                   memoryview(values)[::-1], list(values), tuple(values), array(code)]:
        expected = bytes(cls.create(list(source)).buffer)
        owner = bytearray([0xa5]) * (len(expected) + 16)
        actual = cls.create_in_place(memoryview(owner)[8:-8], source)
        assert bytes(actual.buffer) == expected, (name, type(source))
        assert owner[:8] == owner[-8:] == b'\xa5' * 8

for source in [bytes(range(128)), bytearray(range(128))]:
    assert bytes(numeric.U8View.create(source).buffer) == bytes(numeric.U8View.create(list(source)).buffer)

converted = array('d', [1 / 3, -2.25])
assert bytes(numeric.F32View.create(converted).buffer) == bytes(numeric.F32View.create(list(converted)).buffer)
for cls, code, size in [
    (numeric.FixedShortView, 'f', 2), (numeric.FixedMediumView, 'f', 16),
    (numeric.FixedLongView, 'f', 128), (numeric.FixedBytesShortView, 'B', 2),
    (numeric.FixedBytesLongView, 'B', 128), (numeric.FixedWideShortView, 'q', 2),
    (numeric.FixedWideLongView, 'q', 16),
]:
    values = array(code, range(size))
    assert bytes(cls.create(values).buffer) == bytes(cls.create(list(values)).buffer)
    assert bytes(cls.create(intrlay.zeroed()).buffer) == b'\0' * len(values.tobytes())
    try:
        cls.create(values[:-1])
    except ValueError:
        pass
    else:
        raise AssertionError('bulk copy skipped fixed array length validation')

nested = [array('f', range(128)), array('f', range(128, 256))]
assert bytes(numeric.NestedView.create(nested).buffer) == b''.join(value.tobytes() for value in nested)
nan_bits = array('I', [0x7fa12345, 0x80000000])
nan_values = memoryview(nan_bits).cast('B').cast('f')
assert bytes(numeric.F32View.create(nan_values).buffer)[8:] == nan_bits.tobytes()
try:
    numeric.U32View.create(array('i', [-1]))
except (ValueError, OverflowError):
    pass
else:
    raise AssertionError('different signedness bypassed scalar range checks')
tagged_items = import_module('tagged-defaults')
try:
    tagged_items.TaggedItemsView.create({'items': array('I', [99])})
except ValueError:
    pass
else:
    raise AssertionError('numeric buffer bypassed enum tag validation')
print('Python: numeric bulk copies, fixed extents, conversion fallbacks and slices passed')
