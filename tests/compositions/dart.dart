import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/compositions/bindings.dart';
import '../../target/compositions/setter-names.dart' as setters;
import '../../target/compositions/value-enums.dart' as enums;
import 'dart:typed_data';

void check(bool value) { if (!value) throw StateError('composition regression'); }

void main() {
  final zeroInput = ZeroedArraysInput(id: 73,
    points: intrlay.SequenceInput.zeroed(3), flags: intrlay.SequenceInput.zeroed(4),
    matrix: const intrlay.FixedArrayInput.zeroed(), present: const intrlay.FixedArrayInput.zeroed(),
    nested: [intrlay.SequenceInput.zeroed(2), intrlay.SequenceInput.zeroed(0)],
    empty: const intrlay.FixedArrayInput.zeroed());
  final zeroExpected = ZeroedArraysView.create(ZeroedArraysInput(id: 73,
    points: intrlay.SequenceInput.from(List.filled(3, const ZeroedPointInput(x: 0, y: 0, z: 0))),
    flags: intrlay.SequenceInput.from(List.filled(4, false)),
    matrix: intrlay.FixedArrayInput.from(List.filled(2, const intrlay.FixedArrayInput.from([0.0, 0.0]))),
    present: const intrlay.FixedArrayInput.from([0, 0]),
    nested: const [intrlay.SequenceInput.from([0, 0]), intrlay.SequenceInput.from([])],
    empty: const intrlay.FixedArrayInput.from([]))).bytes;
  final zeroOwner = Uint8List(zeroExpected.length + 32)..fillRange(0, zeroExpected.length + 32, 0xa5);
  final zeroView = ZeroedArraysView.createInPlace(Uint8List.sublistView(zeroOwner, 16), zeroInput);
  for (var i = 0; i < zeroExpected.length; ++i) { check(zeroView.bytes[i] == zeroExpected[i]); }
  check(zeroOwner.take(16).every((value) => value == 0xa5));
  check(zeroOwner.skip(16 + zeroExpected.length).every((value) => value == 0xa5));
  zeroView.matrix.fillRange(0, 4, 7);
  zeroView.setMatrix(const intrlay.FixedArrayInput.zeroed());
  for (var i = 0; i < zeroExpected.length; ++i) { check(zeroView.bytes[i] == zeroExpected[i]); }
  const enumInput = enums.ValueEnumsInput(choice: enums.ValueChoicePresentInput(
    state: enums.SparseStateInput.Last, optional: enums.SparseStateInput.Middle,
    fixed: [enums.SparseStateInput.First, enums.SparseStateInput.Last],
    states: [enums.SparseStateInput.Middle, enums.SparseStateInput.First], text: 'hello',
  ));
  final enumView = enums.ValueEnumsView.create(enumInput);
  check(enumView.choice.presentState?.tag == 0xffffffff && enumView.choice.presentOptional?.value?.tag == 37);
  check(enumView.choice.presentFixed?[1].tag == 0xffffffff && enumView.choice.presentStates?[1].tag == 3);
  final enumState = enums.SparseStateView.create(enums.SparseStateInput.Last);
  enumState.replace(enums.SparseStateInput.Middle);
  check(enumState.tag == 37 && enums.StateAliasView.create(enums.StateAliasInput.First).tag == 3);
  final numeric = NumericItemsView.create(const NumericItemsInput(items: [NumericItemInput(id: 7, position: intrlay.FixedArrayInput.from([1, 2, 3]), matrix: intrlay.FixedArrayInput.from([intrlay.FixedArrayInput.from([4, 5]), intrlay.FixedArrayInput.from([6, 7])]), padded: 8, weights: intrlay.SequenceInput.from([9, 10]), wide: intrlay.FixedArrayInput.from([11, 12]), readOnly: intrlay.FixedArrayInput.from([13, 14]), writeOnly: intrlay.FixedArrayInput.from([15, 16]))]));
  final owner = Uint8List(numeric.bytes.length + 16)..setRange(16, numeric.bytes.length + 16, numeric.bytes);
  final items = NumericItemsView(Uint8List.sublistView(owner, 16)).items;
  check(items.getPositionComponent(0, 2) == 3 && items.getMatrixComponent(0, 3) == 7);
  check(items.getPaddedComponent(0, 3) == 0 && items.getWeightsComponent(0, 1) == 10);
  items[0].setPadded(18);
  check(items[0].padded[0] == 18 && items[0].padded[3] == 0);
  items.setPositionComponent(0, 1, 22);
  items.setWeightsComponent(0, 0, 29);
  items.setWideComponent(0, 1, 123456789);
  items.setWriteOnlyComponent(0, 0, -123);
  check(items[0].position[1] == 22 && items[0].weights[0] == 29);
  check(items[0].wide[1] == 123456789 && items[0].writeOnly[0] == -123);
  items[0].position[2] = 33;
  check(items.getPositionComponent(0, 2) == 33);
  final settersView = setters.SetterNamesView.create(const setters.SetterNamesInput(value: 1, setValue: 2));
  settersView.value = 3;
  settersView.setValue = 4;
  check(settersView.setValue == 4);
  check(settersView.bytes.buffer.asUint32List(settersView.bytes.offsetInBytes, 2)[0] == 3);
  final names = RenamedListView.create(const RenamedListInput(items: intrlay.SequenceInput.from([RenamedInput(first: 11, second: 22)])));
  check(names.items.getFirst(0) == 11);
  names.items.setSecond(0, 23);
  check(names.items[0].second == 23);
  final p = ProjectionSetView.create(const ProjectionSetInput(direct: 1, optional: 2, boxed: 3, elements: intrlay.SequenceInput.from([4, 5]), readOnly: 6));
  for (final value in [p.direct, p.optional, p.boxed, p.elements[0], p.readOnly]) {
    check(value is Float32List && value.length == 4);
  }
  check(identical(p.direct, p.direct) && identical(p.optional, p.optional) && identical(p.boxed, p.boxed));
  p.optional![0] = 7;
  check(p.optional![0] == 7);
  p.setDirect(8);
  p.setOptional(9);
  check(p.direct[0] == 8 && p.optional![0] == 9);
  final w = WrappedScalarsView.create(const WrappedScalarsInput(present: 1, absent: null, boxed: 2, nested: Some(3), missingInner: Some(null), items: [4, null], fixed: [5, null], readOnly: 6, writeOnly: 7));
  final length = w.bytes.length;
  w.present = 11; w.absent = 12; w.setBoxed(13); w.setNested(14); w.setMissingInner(15);
  w.items[0] = 16; w.items[1] = 17; w.fixed[0] = 18; w.fixed[1] = 19; w.writeOnly = 20;
  check(w.present == 11 && w.absent == 12 && w.boxed == 13 && w.nested!.value == 14 && w.missingInner!.value == 15);
  check(w.items[0] == 16 && w.items[1] == null && w.fixed[0] == 18 && w.fixed[1] == 19 && w.bytes.length == length);
  for (final item in <Some<int?>?>[null, const Some(null), const Some(2)]) {
    final root = OptionalRootView.create(item);
    root.set(9);
    check(item == null ? root.value == null : root.value != null && root.value!.value == 9);
  }
  final c = CachedChildrenView.create(const CachedChildrenInput(child: SharedInput(value: 1), absent: null, boxed: SharedInput(value: 2), nested: Some(SharedInput(value: 3)), choice: CachedChoiceValueInput(child: SharedInput(value: 4))));
  check(identical(c.child, c.child) && identical(c.boxed, c.boxed) && identical(c.nested, c.nested));
  check(identical(c.choice.valueChild, c.choice.valueChild));
  c.child!.value = 21; c.nested!.value!.value = 22; c.choice.valueChild!.value!.value = 23;
  check(c.child!.value == 21 && c.nested!.value!.value == 22 && c.choice.valueChild!.value!.value == 23);
  print('Dart composition regressions passed');
  final root = CachedRootView.create(const SharedInput(value: 1));
  check(identical(root.value, root.value));
  root.value!.value = 2;
  check(root.value!.value == 2);
}
