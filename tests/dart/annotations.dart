import 'package:intrlay/intrlay.dart' as intrlay;
import 'dart:io';
import '../../target/regressions/contracts.dart';
import '../../target/regressions/input_emission.dart' as inputs;
import '../../target/regressions/storage_emission.dart' as storage;
import '../../target/regressions/view_emission.dart' as outbound;
import 'dart:typed_data';

void main() {
  const outboundInput = outbound.OutboundPacketInput(
    records: [outbound.InternalRecordInput(value: 7, label: 'nested')],
    optional: outbound.InternalRecordInput(value: 8, label: 'optional'),
    choice: outbound.InternalChoiceRecordInput(payload: outbound.InternalRecordInput(value: 9, label: 'variant')),
  );
  final outboundExpected = File('../../target/regressions/view_emission.bin').readAsBytesSync();
  for (final packet in [outbound.OutboundPacketView.create(outboundInput),
      outbound.OutboundPacketView.createInPlace(Uint8List(outboundExpected.length), outboundInput)]) {
    if (packet.bytes.length != outboundExpected.length) throw StateError('encoding-only nested length');
    for (var i = 0; i < outboundExpected.length; ++i) {
      if (packet.bytes[i] != outboundExpected[i]) throw StateError('encoding-only nested byte parity');
    }
  }
  const creationInput = storage.CreateOnlyInput(count: 7, label: 'created', fixed: intrlay.FixedArrayInput.from([2, 5]), optional: 11);
  final creationExpected = File('../../target/regressions/storage_emission_created.bin').readAsBytesSync();
  for (final created in [storage.CreateOnlyView.create(creationInput),
      storage.CreateOnlyView.createInPlace(Uint8List(creationExpected.length), creationInput)]) {
    if (created.bytes.length != creationExpected.length) throw StateError('create-only byte length');
    for (var i = 0; i < creationExpected.length; ++i) {
      if (created.bytes[i] != creationExpected[i]) throw StateError('create-only byte parity');
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
      incoming.event.dataKind != 42 || incoming.state.tag != inputs.ReadOnlyStateInput.On.tag) {
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
  if (expected.length != view.bytes.length) throw StateError('byte length');
  for (var i = 0; i < expected.length; ++i) {
    if (view.bytes[i] != expected[i]) throw StateError('byte parity');
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
    value: NumericAccessInput(branch: NumericBranchInput(leaf: NumericLeafInput(value: 1.5))),
    structured: NumericAccessInput(branch: NumericBranchInput(leaf: NumericLeafInput(value: 2.5))),
    indexed: intrlay.SequenceInput.from([ContractChildInput(value: 3)]),
  ));
  if (projected.value[0] != 1.5) throw StateError('numeric value');
  projected.value[0] = 40;
  if (projected.value[0] != 40) throw StateError('projection mutation');
  if (projected.structured.branch.leaf.value != 2.5) throw StateError('field projection override');
  if (projected.indexed.getValue(0) != 3) throw StateError('indexed projection');
  projected.indexed.setValue(0, 4);
  if (ProjectedAccessView(projected.bytes).indexed.getValue(0) != 4) throw StateError('indexed mutation');
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
  print('Dart: transparent wrappers, byte parity and nested access passed');
}
