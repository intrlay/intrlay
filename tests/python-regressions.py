"""Shared runtime identity, numeric projections, and Python 3.12+ annotations."""
import sys
from array import array
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
assert bytes(outbound.OutboundPacketView.create(outbound_input).storage.bytes) == outbound_expected
assert bytes(outbound.OutboundPacketView.create_in_place(bytearray(len(outbound_expected)), outbound_input).storage.bytes) == outbound_expected
assert not hasattr(outbound, 'InternalRecordView') and not hasattr(outbound, 'ZInternalRecordView')

creation_input: storage.CreateOnlyInput = {'count': 7, 'label': 'created', 'fixed': [2, 5], 'optional': 11}
creation_expected = Path('target/regressions/storage_emission_created.bin').read_bytes()
for created in [storage.CreateOnlyView.create(creation_input),
                storage.CreateOnlyView.create_in_place(bytearray(len(creation_expected)), creation_input)]:
    assert bytes(created.storage.bytes) == creation_expected
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
assert incoming.event.data_kind == 42 and incoming.state == inputs.ReadOnlyState.On
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
assert scalar_setters.WrapperView(scalar.storage.bytes).editable == 17

private_input = {
    'child': {'value': 1}, 'children': [{'value': 2}], 'optional': {'value': 3},
    'boxed': {'value': 4}, 'choice': {'kind': private_names.__ChoiceKind.Present, 'child': {'value': 5}}, 'text': 'hello',
    'status': private_names.__Status.Idle, 'numeric_status': memoryview(array('I', [3])),
    'statuses': memoryview(array('I', [1, 3])),
}
private = private_names.__RootView.create(private_input)
assert private.child.value == 1 and private.children[0].value == 2
assert private.optional.value == 3 and private.boxed.value == 4
assert private.choice.present_child.value == 5 and private.text == 'hello'
assert private_names.__ChildView.create({'value': 7}).value == 7
private.child.value = 11
assert private_names.__RootView(private.storage.bytes).child.value == 11
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
    assert bytes(private_choice.storage.bytes) == (2).to_bytes(4, 'little') + (29).to_bytes(4, 'little')
    keys = get_type_hints(module.PrivateFieldInput)
    assert keys == {'__value': int}
    encoded = module.PrivateFieldView.create(module.PrivateFieldInput(__value=27))
    assert bytes(encoded.storage.bytes) == (27).to_bytes(4, 'little')
    # View members retain normal Python private-attribute spelling.
    assert encoded._PrivateFieldView__value == 27
    private = module.__RootView.create(private_input)
    assert private.text == 'hello' and private.children[0].value == 2
    assert private.status is module.__Status.Idle
    assert list(private.numeric_status) == [3] and list(private.statuses) == [1, 3]
    assert get_type_hints(module.__RootInput)['status'] is module.__Status
    reused = module.__RootView.create_in_place(bytearray(len(private.storage.bytes)), private_input)
    assert bytes(reused.storage.bytes) == bytes(private.storage.bytes)
    private.status = module.__Status.Ready
    private.numeric_status[0] = 1
    private.statuses[1] = 1
    reloaded = module.__RootView(private.storage.bytes)
    assert reloaded.status is module.__Status.Ready
    assert list(reloaded.numeric_status) == [1] and list(reloaded.statuses) == [1, 1]

# String reads borrow the encoded bytes until UTF-8 decoding; no bytes copy is needed.
from array import array
for text in ['', 'ASCII', '\ufeffprefix', '한글 😀', '\x00embedded\x00']:
    encoded = text.encode('utf-8')
    raw = array('I', [8, len(encoded)]).tobytes() + encoded
    for buffer in [raw, bytearray(raw), memoryview(b'1234' + raw)[4:]]:
        assert intrlay.Storage(buffer).string_at(0) == text

a = alpha.AlphaView.create({'values': [1, 2]})
b = beta.BetaView.create({'values': [3, 4]})
assert type(a.storage) is type(b.storage) is intrlay.Storage
assert a.values[1] == 2 and b.values[0] == 3
assert bindings.ScalarView.create(1.5).value == 1.5
view = bindings.ProjectionsView.create({'vectors': memoryview(array('f', [1, 2, 3, 0]))})
assert list(view.vectors) == [1, 2, 3, 0]
view.vectors[1] = 9
assert view.vectors[1] == 9
get_type_hints(intrlay.Storage)
get_type_hints(bindings.ProjectionsView.vectors.fget)
assert isinstance(bindings.ProjectionsView(bytes(view.storage.bytes)).vectors, memoryview)
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
assert bytes(view.storage.bytes) == (Path(__file__).resolve().parents[1] / 'target/regressions/contracts.bin').read_bytes()
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
    'value': memoryview(array('f', [1.5])),
    'structured': {'branch': {'leaf': {'value': 2.5}}},
    'indexed': [{'value': 3}],
    'ordinary': [{'value': 5}],
    'direct': [{'value': 7}],
})
assert not projected.value.readonly and projected.value[0] == 1.5
projected.value[0] = 40
assert projected.value[0] == 40
assert projected.structured.branch.leaf.value == 2.5
assert projected.indexed.get_value(0) == 3
projected.indexed.set_value(0, 4)
assert contracts.ProjectedAccessView(projected.storage.bytes).indexed.get_value(0) == 4
assert projected.indexed[0].value == 4
assert projected.ordinary[0].value == 5
assert type(projected.ordinary) is type(projected.indexed) is type(projected.direct)
projected.ordinary[0].value = 6
assert contracts.ProjectedAccessView(projected.storage.bytes).ordinary[0].value == 6
assert projected.ordinary.get_value(0) == 6
projected.direct.set_value(0, 8)
assert projected.direct.get_value(0) == 8
assert projected.direct[0].value == 8
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
        expected = bytes(cls.create(list(source)).storage.bytes)
        owner = bytearray([0xa5]) * (len(expected) + 16)
        actual = cls.create_in_place(memoryview(owner)[8:-8], source)
        assert bytes(actual.storage.bytes) == expected, (name, type(source))
        assert owner[:8] == owner[-8:] == b'\xa5' * 8

for source in [bytes(range(128)), bytearray(range(128))]:
    assert bytes(numeric.U8View.create(source).storage.bytes) == bytes(numeric.U8View.create(list(source)).storage.bytes)

converted = array('d', [1 / 3, -2.25])
assert bytes(numeric.F32View.create(converted).storage.bytes) == bytes(numeric.F32View.create(list(converted)).storage.bytes)
for cls, code, size in [
    (numeric.FixedShortView, 'f', 2), (numeric.FixedMediumView, 'f', 16),
    (numeric.FixedLongView, 'f', 128), (numeric.FixedBytesShortView, 'B', 2),
    (numeric.FixedBytesLongView, 'B', 128), (numeric.FixedWideShortView, 'q', 2),
    (numeric.FixedWideLongView, 'q', 16),
]:
    values = array(code, range(size))
    assert bytes(cls.create(values).storage.bytes) == bytes(cls.create(list(values)).storage.bytes)
    assert bytes(cls.create(intrlay.zeroed()).storage.bytes) == b'\0' * len(values.tobytes())
    try:
        cls.create(values[:-1])
    except ValueError:
        pass
    else:
        raise AssertionError('bulk copy skipped fixed array length validation')

nested = [array('f', range(128)), array('f', range(128, 256))]
assert bytes(numeric.NestedView.create(nested).storage.bytes) == b''.join(value.tobytes() for value in nested)
nan_bits = array('I', [0x7fa12345, 0x80000000])
nan_values = memoryview(nan_bits).cast('B').cast('f')
assert bytes(numeric.F32View.create(nan_values).storage.bytes)[8:] == nan_bits.tobytes()
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

# Heterogeneous, padded inline fields expose individual scalar accessors.
flat = contracts.FlattenedAccessView.create({
    'min': [1, 2, 3], 'max': {'x': 4, 'y': 5, 'z': 6},
    'details': {'id': 7, 'enabled': True, 'weight': 8.5, 'fixed': 9},
    'grid': [[10, 11], [12, 13]], 'label': 'mixed fields', 'maybe': 14,
    'structured': {'leaf': {'value': 15}}, 'lanes': memoryview(array('f', [16, 17, 18])),
})
assert bytes(flat.storage.bytes) == Path('target/regressions/flattened.bin').read_bytes()
assert (flat.min_0, flat.max_x, flat.details_id, flat.details_enabled, flat.details_weight, flat.details_fixed, flat.grid_1_1) == (1, 4, 7, True, 8.5, 9, 13)
assert flat.label == 'mixed fields' and flat.structured.leaf.value == 15
assert list(flat.lanes) == [16, 17, 18]
assert not hasattr(flat, 'max')
assert contracts.FlattenedAccessView.details_fixed.fset is None
flat.min_1 = 20
flat.max_z = 60
flat.details_enabled = False
flat.details_weight = 80.5
flat.grid_1_0 = 120
assert bytes(flat.storage.bytes) == Path('target/regressions/flattened-changed.bin').read_bytes()
flat.maybe = None
assert flat.maybe is None
selective = contracts.SelectiveFlattenView.create({'lower': {'x': 1, 'y': 2, 'z': 3}, 'max': {'x': 4, 'y': 5, 'z': 6}})
assert selective.lower_x == 1 and selective.max.z == 6

assert not hasattr(contracts, 'IndexedEntryView')
indexed_only = contracts.IndexedOnlyView.create({'items': [{
    'id': 7, 'label': 'entry', 'plain': [1, 2, 3], 'lanes': [4, 5, 6],
    'projected': memoryview(array('f', [7, 8, 9, 0])), 'direct': memoryview(array('f', [10, 11, 12, 0])),
    'child': {'value': 13}, 'flat': [14, 15],
}]})
assert bytes(indexed_only.storage.bytes) == Path('target/regressions/indexed-only.bin').read_bytes()
only = indexed_only.items
assert len(only) == 1 and not hasattr(only, '__getitem__') and not hasattr(only, '__iter__')
assert not hasattr(only, 'get_plain_component') and not hasattr(only, 'get_projected_component')
assert only.get_plain(0)[1] == 2 and only.get_projected(0)[2] == 9
assert only.get_label(0) == 'entry' and only.get_child(0).value == 13
only.set_id(0, 17)
only.set_lanes_component(0, 1, 20.)
only.set_direct_component(0, 2, 60.)
only.set_flat_1(0, 30)
assert (only.get_id(0), only.get_lanes(0)[1], only.get_direct_component(0, 2), only.get_flat_1(0)) == (17, 20, 60, 30)

indexed_fixed = contracts.IndexedFixedView(bytearray(Path('target/regressions/indexed-fixed.bin').read_bytes()))
assert not hasattr(contracts, 'IndexedMutableEntryView') and hasattr(contracts, 'IndexedValueView')
assert not hasattr(contracts.IndexedFixedView, 'create')
components = indexed_fixed.with_components
assert len(components) == 1 and not hasattr(components, '__getitem__')
assert not hasattr(components, 'get_plain_component') and components.get_child(0).value == 13
assert not hasattr(indexed_fixed.without_components, 'get_lanes_component')
assert components.get_lanes_component(0, 1) == 2
components.set_child(0, {'value': 31})
components.set_lanes_component(0, 1, 20.)
indexed_fixed.without_components.set_child(0, {'value': 32})
assert bytes(indexed_fixed.storage.bytes) == Path('target/regressions/indexed-fixed-changed.bin').read_bytes()

indexed_write_only = contracts.IndexedWriteOnlyView.create({
    'items': [{'id': 7, 'values': [1, 2]}], 'fixed': [{'id': 7, 'values': [1, 2]}],
})
assert not hasattr(contracts, 'IndexedWriteEntryView')
for collection in (indexed_write_only.items, indexed_write_only.fixed):
    assert not hasattr(collection, '__getitem__') and not hasattr(collection, 'get_id')
indexed_write_only.items.set_id(0, 17)
indexed_write_only.items.get_values(0)[1] = 20
indexed_write_only.fixed.get_values(0)[0] = 30
assert bytes(indexed_write_only.storage.bytes) == Path('target/regressions/indexed-write-only-changed.bin').read_bytes()

import projection_cases  # noqa: E402,F401 - Run the shared projection cases.
