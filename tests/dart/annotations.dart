import 'package:intrlay/intrlay.dart' as intrlay;
import 'dart:io';
import '../../target/regressions/contracts.dart';
import '../../target/regressions/input_emission.dart' as inputs;
import '../../target/regressions/storage_emission.dart' as storage;
import '../../target/regressions/view_emission.dart' as outbound;
import 'dart:typed_data';

import 'projections.dart';

void main() {
  checkProjections();
  const outboundInput = outbound.OutboundPacketInput(
    records: [outbound.InternalRecordInput(value: 7, label: 'nested')],
    optional: outbound.InternalRecordInput(value: 8, label: 'optional'),
    choice: outbound.InternalChoiceRecordInput(payload: outbound.InternalRecordInput(value: 9, label: 'variant')),
  );
  final outboundExpected = File('../../target/regressions/view_emission.bin').readAsBytesSync();
  for (final packet in [outbound.OutboundPacketView.create(outboundInput),
      outbound.OutboundPacketView.createInPlace(Uint8List(outboundExpected.length), outboundInput)]) {
    if (packet.storage.bytes.length != outboundExpected.length) throw StateError('encoding-only nested length');
    for (var i = 0; i < outboundExpected.length; ++i) {
      if (packet.storage.bytes[i] != outboundExpected[i]) throw StateError('encoding-only nested byte parity');
    }
  }
  const creationInput = storage.CreateOnlyInput(count: 7, label: 'created', fixed: intrlay.FixedArrayInput.from([2, 5]), optional: 11);
  final creationExpected = File('../../target/regressions/storage_emission_created.bin').readAsBytesSync();
  for (final created in [storage.CreateOnlyView.create(creationInput),
      storage.CreateOnlyView.createInPlace(Uint8List(creationExpected.length), creationInput)]) {
    if (created.storage.bytes.length != creationExpected.length) throw StateError('create-only byte length');
    for (var i = 0; i < creationExpected.length; ++i) {
      if (created.storage.bytes[i] != creationExpected[i]) throw StateError('create-only byte parity');
    }
  }
  final unitsBytes = File('../../target/regressions/storage_emission_units.bin').readAsBytesSync();
  final prefixedUnits = Uint8List(16 + unitsBytes.length)..setRange(16, 16 + unitsBytes.length, unitsBytes);
  final units = storage.UnitItemsView(prefixedUnits, 16).values;
  if (units.length != 3 || units[2] != null) throw StateError('unit collection without element offsets');
  final zeroArray = storage.ZeroArrayView(Uint8List(0));
  zeroArray.setValues(const intrlay.FixedArrayInput<storage.ZeroPayloadInput>.from([]));
  zeroArray.replace(const storage.ZeroArrayInput(values: intrlay.FixedArrayInput.from([])));
  if (zeroArray.values.length != 0) throw StateError('empty array without element writers');
  final incoming = inputs.ReadOnlyView(File('../../target/regressions/input_emission_read.bin').readAsBytesSync());
  if (incoming.label != 'incoming' || incoming.next != null || incoming.event.tag != 17 ||
      incoming.event.dataKind != 42 || incoming.state != inputs.ReadOnlyState.On) {
    throw StateError('receive-only views and numeric tags');
  }
  final parent = inputs.WriteParentView.create(const inputs.WriteParentInput(
    leaf: inputs.WriteLeafInput(value: 1),
    leaves: intrlay.SequenceInput.from([inputs.WriteLeafInput(value: 2)]),
    event: inputs.WriteEventDataInput(leaf: inputs.WriteLeafInput(value: 3)),
  ));
  if (parent.leaf!.value != 1 || parent.leaves[0].value != 2 || parent.event.dataLeaf!.value != 3) {
    throw StateError('transitive construction inputs');
  }
  final fieldOnly = inputs.FieldOnlyView(File('../../target/regressions/input_emission_mutation.bin').readAsBytesSync());
  fieldOnly.setLeaf(const inputs.ReplacementOnlyInput(value: 8));
  fieldOnly.leaf.replace(const inputs.ReplacementOnlyInput(value: 9));
  if (fieldOnly.leaf.value != 9 || fieldOnly.label != 'mutable field') {
    throw StateError('mutation inputs without construction');
  }
  final view = ContractsView.create(ContractsInput(
    editable: ContractChildInput(value: 1), readOnly: ContractChildInput(value: 2), writeOnly: ContractChildInput(value: 3),
    readOnlyArray: intrlay.FixedArrayInput.from([4,5,6]), readOnlyNested: intrlay.SequenceInput.from([ContractChildInput(value: 7)]), fixed: ConstantChildInput(value: 8),
    identifier: 9, identifiers: intrlay.SequenceInput.from([10,11]), maybe: Some(null), label: 'hello',
  ));
  final expected = File('../../target/regressions/contracts.bin').readAsBytesSync();
  if (expected.length != view.storage.bytes.length) throw StateError('byte length');
  for (var i = 0; i < expected.length; ++i) {
    if (view.storage.bytes[i] != expected[i]) throw StateError('byte parity');
  }
  if (IdentifierView.byteSize != 16 || view.identifier != 9 || view.identifiers[1] != 11) throw StateError('transparent');
  view.identifier = 19;
  view.identifiers[1] = 21;
  if (view.identifier != 19 || view.identifiers[1] != 21 || view.label != 'hello' || view.maybe?.value != null) throw StateError('view values');
  view.editable.value = 30;
  view.writeOnly.value = 31;
  view.readOnly.value = 41;
  view.readOnlyNested![0].value = 42;
  for (final dynamic child in [view.fixed]) {
    try { child.value = 40; throw StateError('read-only setter'); } on NoSuchMethodError { }
  }
  view.readOnlyArray[0] = 40;
  if (view.readOnlyArray[0] != 40) throw StateError('array mutation');
  final projected = ProjectedAccessView.create(ProjectedAccessInput(
    value: Float32List.fromList([1.5]),
    structured: NumericAccessFieldsInput(branch: NumericBranchInput(leaf: NumericLeafInput(value: 2.5))),
    indexed: intrlay.SequenceInput.from([ContractChildInput(value: 3)]),
    ordinary: intrlay.SequenceInput.from([ContractChildInput(value: 5)]),
    direct: intrlay.SequenceInput.from([ContractChildInput(value: 7)]),
  ));
  if (projected.value[0] != 1.5) throw StateError('numeric value');
  projected.value[0] = 40;
  if (projected.value[0] != 40) throw StateError('projection mutation');
  if (projected.structured.branch.leaf.value != 2.5) throw StateError('field projection override');
  if (projected.indexed.getValue(0) != 3) throw StateError('indexed projection');
  projected.indexed.setValue(0, 4);
  if (ProjectedAccessView(projected.storage.bytes).indexed.getValue(0) != 4) throw StateError('indexed mutation');
  if (projected.indexed[0].value != 4 || projected.ordinary[0].value != 5) throw StateError('ordinary element access');
  projected.ordinary[0].value = 6;
  if (ProjectedAccessView(projected.storage.bytes).ordinary[0].value != 6) throw StateError('ordinary mutation');
  projected.direct.setValue(0, 8);
  if (projected.direct.getValue(0) != 8) throw StateError('indexed-only mutation');
  if (projected.ordinary.runtimeType != projected.indexed.runtimeType || projected.direct.runtimeType != projected.indexed.runtimeType) throw StateError('shared collection type');
  if (projected.ordinary.getValue(0) != 6 || projected.direct[0].value != 8) throw StateError('merged collection accessors');
  final hints = IndexedHintsView.create(IndexedHintsInput(
    readable: intrlay.SequenceInput.from([ReadIndexedInput(readScore: 7, readVector: intrlay.FixedArrayInput.from([1, 2, 3]))]),
    writable: intrlay.SequenceInput.from([WriteIndexedInput(writeScore: 8, writeVector: intrlay.FixedArrayInput.from([4, 5, 6]))]),
    hidden: intrlay.FixedArrayInput.from([HiddenIndexedInput(hiddenScore: 9, hiddenVector: intrlay.FixedArrayInput.from([7, 8, 9]))]),
  ));
  if (hints.readable.getReadScore(0) != 7 || hints.readable.getReadVectorComponent(0, 1) != 2) throw StateError('indexed getters');
  hints.writable.setWriteScore(0, 10);
  hints.writable.setWriteVectorComponent(0, 1, 20);
  if (hints.writable[0].writeVector[1] != 20) throw StateError('indexed setters');
  final dynamic readable = hints.readable, writable = hints.writable, hidden = hints.hidden;
  for (final access in <void Function()>[
    () { readable.setReadScore(0, 1); },
    () { readable.setReadVectorComponent(0, 0, 1.0); },
    () { writable.getWriteScore(0); },
    () { writable.getWriteVectorComponent(0, 0); },
    () { hidden.getHiddenScore(0); },
    () { hidden.setHiddenVectorComponent(0, 0, 1.0); },
  ]) {
    try { access(); throw StateError('omitted indexed helper'); } on NoSuchMethodError { }
  }
  final flat = FlattenedAccessView.create(FlattenedAccessInput(
    min: intrlay.FixedArrayInput.from([1, 2, 3]),
    max: FlatPointInput(x: 4, y: 5, z: 6),
    details: FlatDetailsInput(id: 7, enabled: true, weight: 8.5, fixed: 9),
    grid: intrlay.FixedArrayInput.from([intrlay.FixedArrayInput.from([10, 11]), intrlay.FixedArrayInput.from([12, 13])]),
    label: 'mixed fields', maybe: 14,
    structured: NumericBranchInput(leaf: NumericLeafInput(value: 15)),
    lanes: intrlay.FixedArrayInput.from([16, 17, 18]),
  ));
  final flatExpected = File('../../target/regressions/flattened.bin').readAsBytesSync();
  for (var i = 0; i < flatExpected.length; ++i) {
    if (flat.storage.bytes[i] != flatExpected[i]) throw StateError('flattened padded byte parity');
  }
  if (flat.min0 != 1 || flat.maxX != 4 || flat.detailsId != 7 || !flat.detailsEnabled || flat.detailsWeight != 8.5 || flat.detailsFixed != 9 || flat.grid11 != 13) throw StateError('flattened leaves');
  if (flat.label != 'mixed fields' || flat.structured.leaf.value != 15 || flat.lanes[2] != 18) throw StateError('flattening overrides');
  flat.min1 = 20; flat.maxZ = 60; flat.detailsEnabled = false;
  flat.detailsWeight = 80.5; flat.grid10 = 120;
  final flatChanged = File('../../target/regressions/flattened-changed.bin').readAsBytesSync();
  for (var i = 0; i < flatChanged.length; ++i) {
    if (flat.storage.bytes[i] != flatChanged[i]) throw StateError('flattened mutation parity');
  }
  flat.maybe = null;
  if (flat.maybe != null) throw StateError('flattened optional mutation');
  final selective = SelectiveFlattenView.create(SelectiveFlattenInput(
    lower: FlatPointInput(x: 1, y: 2, z: 3), max: FlatPointInput(x: 4, y: 5, z: 6),
  ));
  if (selective.lowerX != 1 || selective.max.z != 6) throw StateError('field flattening');
  final indexedOnly = IndexedOnlyView.create(IndexedOnlyInput(items: [
    IndexedEntryInput(
      id: 7, label: 'entry', plain: intrlay.FixedArrayInput.from([1, 2, 3]),
      lanes: intrlay.FixedArrayInput.from([4, 5, 6]),
      projected: Float32List.fromList([7, 8, 9, 0]), direct: Float32List.fromList([10, 11, 12, 0]),
      child: ContractChildInput(value: 13), flat: intrlay.FixedArrayInput.from([14, 15]),
    ),
  ]));
  final indexedExpected = File('../../target/regressions/indexed-only.bin').readAsBytesSync();
  for (var i = 0; i < indexedExpected.length; ++i) {
    if (indexedOnly.storage.bytes[i] != indexedExpected[i]) throw StateError('indexed-only byte parity');
  }
  final only = indexedOnly.items;
  if (only.length != 1 || only.getPlain(0)[1] != 2 || only.getProjected(0)[2] != 9 || only.getLabel(0) != 'entry' || only.getChild(0).value != 13) throw StateError('indexed-only getters');
  only.setId(0, 17);
  only.setLanesComponent(0, 1, 20);
  only.setDirectComponent(0, 2, 60);
  only.setFlat1(0, 30);
  if (only.getId(0) != 17 || only.getLanes(0)[1] != 20 || only.getDirectComponent(0, 2) != 60 || only.getFlat1(0) != 30) throw StateError('indexed-only setters');
  final dynamic omitted = only;
  for (final access in <void Function()>[
    () { omitted[0]; },
    () { omitted.getPlainComponent(0, 0); },
    () { omitted.getProjectedComponent(0, 0); },
  ]) {
    try { access(); throw StateError('omitted accessor'); } on NoSuchMethodError { }
  }
  final indexedFixed = IndexedFixedView(File('../../target/regressions/indexed-fixed.bin').readAsBytesSync());
  final components = indexedFixed.withComponents;
  if (components.length != 1 || components.getLanesComponent(0, 1) != 2) throw StateError('fixed indexed-only getters');
  if (components.getChild(0).value != 13) throw StateError('indexed mutation navigation');
  components.setChild(0, IndexedValueInput(value: 31));
  components.setLanesComponent(0, 1, 20);
  indexedFixed.withoutComponents.setChild(0, IndexedValueInput(value: 32));
  final fixedExpected = File('../../target/regressions/indexed-fixed-changed.bin').readAsBytesSync();
  for (var i = 0; i < fixedExpected.length; ++i) {
    if (indexedFixed.storage.bytes[i] != fixedExpected[i]) throw StateError('fixed indexed-only writer parity');
  }
  final dynamic withComponents = components;
  final dynamic withoutComponents = indexedFixed.withoutComponents;
  for (final access in <void Function()>[
    () { withComponents[0]; },
    () { withComponents.getPlainComponent(0, 0); },
    () { withoutComponents.getLanesComponent(0, 0); },
  ]) {
    try { access(); throw StateError('fixed indexed-only omitted accessor'); } on NoSuchMethodError { }
  }
  final indexedWriteOnly = IndexedWriteOnlyView.create(IndexedWriteOnlyInput(
    items: [IndexedWriteEntryInput(id: 7, values: intrlay.SequenceInput.from([1, 2]))],
    fixed: [IndexedWriteEntryInput(id: 7, values: intrlay.SequenceInput.from([1, 2]))],
  ));
  indexedWriteOnly.items.setId(0, 17);
  indexedWriteOnly.items.getValues(0)[1] = 20;
  indexedWriteOnly.fixed.getValues(0)[0] = 30;
  final writeExpected = File('../../target/regressions/indexed-write-only-changed.bin').readAsBytesSync();
  for (var i = 0; i < writeExpected.length; ++i) {
    if (indexedWriteOnly.storage.bytes[i] != writeExpected[i]) throw StateError('indexed navigation byte parity');
  }
  for (final dynamic collection in [indexedWriteOnly.items, indexedWriteOnly.fixed]) {
    for (final access in <void Function()>[
      () { collection[0]; },
      () { collection.getId(0); },
    ]) {
      try { access(); throw StateError('indexed navigation restored omitted getter'); } on NoSuchMethodError { }
    }
  }
  print('Dart: transparent wrappers, byte parity and nested access passed');
}
