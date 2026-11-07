import 'dart:io';
import '../../target/regressions/contracts.dart';

void main() {
  final view = ContractsView.create(Contracts(
    editable: ContractChild(value: 1), readOnly: ContractChild(value: 2), writeOnly: ContractChild(value: 3),
    readOnlyArray: [4,5,6], readOnlyNested: [ContractChild(value: 7)], fixed: ConstantChild(value: 8),
    identifier: 9, identifiers: [10,11], maybe: Some(null), label: 'hello',
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
  final projected = ProjectedAccessView.create(ProjectedAccess(
    value: NumericAccess(branch: NumericBranch(leaf: NumericLeaf(value: 1.5))),
    structured: NumericAccess(branch: NumericBranch(leaf: NumericLeaf(value: 2.5))),
    indexed: [ContractChild(value: 3)],
  ));
  if (projected.value[0] != 1.5) throw StateError('numeric value');
  projected.value[0] = 40;
  if (projected.value[0] != 40) throw StateError('projection mutation');
  if (projected.structured.branch.leaf.value != 2.5) throw StateError('field projection override');
  if (projected.indexed.getValue(0) != 3) throw StateError('indexed projection');
  projected.indexed.setValue(0, 4);
  if (ProjectedAccessView(projected.bytes).indexed.getValue(0) != 4) throw StateError('indexed mutation');
  final hints = IndexedHintsView.create(IndexedHints(
    readable: [ReadIndexed(readScore: 7, readVector: [1, 2, 3])],
    writable: [WriteIndexed(writeScore: 8, writeVector: [4, 5, 6])],
    hidden: [HiddenIndexed(hiddenScore: 9, hiddenVector: [7, 8, 9])],
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
