import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/compositions/bindings.dart';
import '../../target/compositions/setter-names.dart' as setters;
import '../../target/compositions/value-enums.dart' as enums;
import '../../target/compositions/projected-enums-data.dart';
import '../../target/compositions/nested-enums-data.dart';
import 'dart:typed_data';

void check(bool value) { if (!value) throw StateError('composition regression'); }

Float32List padded(double value) => Float32List.fromList([value, 0, 0, 0]);

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
    empty: const intrlay.FixedArrayInput.from([]))).storage.bytes;
  final zeroOwner = Uint8List(zeroExpected.length + 32)..fillRange(0, zeroExpected.length + 32, 0xa5);
  final zeroView = ZeroedArraysView.createInPlace(Uint8List.sublistView(zeroOwner, 16), zeroInput);
  for (var i = 0; i < zeroExpected.length; ++i) { check(zeroView.storage.bytes[i] == zeroExpected[i]); }
  check(zeroOwner.take(16).every((value) => value == 0xa5));
  check(zeroOwner.skip(16 + zeroExpected.length).every((value) => value == 0xa5));
  zeroView.matrix.fillRange(0, 4, 7);
  zeroView.setMatrix(const intrlay.FixedArrayInput.zeroed());
  for (var i = 0; i < zeroExpected.length; ++i) { check(zeroView.storage.bytes[i] == zeroExpected[i]); }
  const enumInput = enums.ValueEnumsInput(choice: enums.ValueChoicePresentInput(
    state: enums.SparseState.Last, optional: enums.SparseState.Middle,
    fixed: [enums.SparseState.First, enums.SparseState.Last],
    states: [enums.SparseState.Middle, enums.SparseState.First], text: 'hello',
  ));
  final enumView = enums.ValueEnumsView.create(enumInput);
  check(enumView.choice.presentState == enums.SparseState.Last && enumView.choice.presentOptional?.value == enums.SparseState.Middle);
  check(enumView.choice.presentFixed?[1] == enums.SparseState.Last && enumView.choice.presentStates?[1] == enums.SparseState.First);
  final enumState = enums.SparseStateView.create(enums.SparseState.Last);
  enumState.replace(enums.SparseState.Middle);
  check(enumState.tag == 37 && enums.StateAliasView.create(enums.StateAlias.First).tag == 3);
  checkEnumProjection();
  checkNestedEnums();
  final numeric = NumericItemsView.create(NumericItemsInput(items: [NumericItemInput(id: 7, position: intrlay.FixedArrayInput.from([1, 2, 3]), matrix: intrlay.FixedArrayInput.from([intrlay.FixedArrayInput.from([4, 5]), intrlay.FixedArrayInput.from([6, 7])]), padded: padded(8), weights: intrlay.SequenceInput.from([9, 10]), wide: intrlay.FixedArrayInput.from([11, 12]), readOnly: intrlay.FixedArrayInput.from([13, 14]), writeOnly: intrlay.FixedArrayInput.from([15, 16]))]));
  final owner = Uint8List(numeric.storage.bytes.length + 16)..setRange(16, numeric.storage.bytes.length + 16, numeric.storage.bytes);
  final items = NumericItemsView(Uint8List.sublistView(owner, 16)).items;
  check(items.getPositionComponent(0, 2) == 3 && items.getMatrixComponent(0, 3) == 7);
  check(items.getPaddedComponent(0, 3) == 0 && items.getWeightsComponent(0, 1) == 10);
  items[0].setPadded(padded(18));
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
  check(settersView.storage.bytes.buffer.asUint32List(settersView.storage.bytes.offsetInBytes, 2)[0] == 3);
  final names = RenamedListView.create(const RenamedListInput(items: intrlay.SequenceInput.from([RenamedInput(first: 11, second: 22)])));
  check(names.items.getFirst(0) == 11);
  names.items.setSecond(0, 23);
  check(names.items[0].second == 23);
  final p = ProjectionSetView.create(ProjectionSetInput(direct: padded(1), optional: padded(2), boxed: padded(3), elements: intrlay.SequenceInput.from([padded(4), padded(5)]), readOnly: padded(6)));
  for (final value in [p.direct, p.optional, p.boxed, p.elements[0], p.readOnly]) {
    check(value is Float32List && value.length == 4);
  }
  check(identical(p.direct, p.direct) && identical(p.optional, p.optional) && identical(p.boxed, p.boxed));
  p.optional![0] = 7;
  check(p.optional![0] == 7);
  p.setDirect(padded(8));
  p.setOptional(padded(9));
  check(p.direct[0] == 8 && p.optional![0] == 9);
  final w = WrappedScalarsView.create(const WrappedScalarsInput(present: 1, absent: null, boxed: 2, nested: Some(3), missingInner: Some(null), items: [4, null], fixed: [5, null], readOnly: 6, writeOnly: 7));
  final length = w.storage.bytes.length;
  w.present = 11; w.absent = 12; w.setBoxed(13); w.setNested(14); w.setMissingInner(15);
  w.items[0] = 16; w.items[1] = 17; w.fixed[0] = 18; w.fixed[1] = 19; w.writeOnly = 20;
  check(w.present == 11 && w.absent == 12 && w.boxed == 13 && w.nested!.value == 14 && w.missingInner!.value == 15);
  check(w.items[0] == 16 && w.items[1] == null && w.fixed[0] == 18 && w.fixed[1] == 19 && w.storage.bytes.length == length);
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

void checkEnumProjection() {
  final input = enums.ProjectedEnumsInput(
    states: Uint32List.fromList([enums.SparseState.Middle.tag, enums.SparseState.First.tag, enums.SparseState.Last.tag]),
    fixed: Uint32List.fromList([enums.SparseState.First.tag, enums.SparseState.Last.tag]),
    empty: Uint32List(0),
    current: Uint32List.fromList([enums.SparseState.Last.tag]),
    mode: Uint32List.fromList([enums.ProjectedMode.Idle.tag]),
    modes: [Uint32List.fromList([enums.ProjectedMode.Idle.tag]), Uint32List.fromList([enums.ProjectedMode.Active.tag])],
  );
  final owner = Uint8List(projectedEnumsBytes.length + 32)..fillRange(0, projectedEnumsBytes.length + 32, 0xa5);
  final view = enums.ProjectedEnumsView.createInPlace(Uint8List.sublistView(owner, 16), input);
  check(view.storage.bytes.length == projectedEnumsBytes.length);
  for (var i = 0; i < projectedEnumsBytes.length; ++i) {
    check(view.storage.bytes[i] == projectedEnumsBytes[i]);
  }
  for (final region in [view.states, view.fixed, view.empty, view.current, view.mode, view.modes[1]]) {
    check(region is Uint32List);
  }
  check(view.states[2] == 0xffffffff && view.fixed[0] == 3 && view.empty.isEmpty);
  check(view.current[0] == 0xffffffff && view.mode?[0] == 3 && view.modes[1][0] == 255);
  view.states[1] = enums.SparseState.Last.tag;
  view.fixed[0] = enums.SparseState.Middle.tag;
  view.current[0] = enums.SparseState.First.tag;
  view.mode![0] = enums.ProjectedMode.Active.tag;
  view.modes[1][0] = enums.ProjectedMode.Idle.tag;
  check(view.storage.bytes.length == updatedProjectedEnumsBytes.length);
  for (var i = 0; i < updatedProjectedEnumsBytes.length; ++i) {
    check(view.storage.bytes[i] == updatedProjectedEnumsBytes[i]);
  }
  check(owner.take(16).every((value) => value == 0xa5));
  check(owner.skip(16 + projectedEnumsBytes.length).every((value) => value == 0xa5));
  final reopened = enums.ProjectedEnumsView(view.storage.bytes);
  check(reopened.states[1] == 0xffffffff && reopened.fixed[0] == 37 && reopened.mode?[0] == 255);
}

void checkNestedEnums() {
  final input = enums.NestedEnumValuesInput(
    state: enums.PaddedState.Last, optional: enums.PaddedState.Middle, boxed: enums.PaddedState.First,
    fixed: [enums.PaddedState.First, enums.PaddedState.Last],
    states: [enums.PaddedState.Middle, enums.PaddedState.First],
    indexed: [enums.EnumEntryInput(state: enums.PaddedState.Middle, optional: enums.PaddedState.First)],
    projected: Uint32List.fromList([enums.PaddedState.First.tag, 0, 0, 0, enums.PaddedState.Last.tag, 0, 0, 0]),
    writeOnly: enums.PaddedState.First, readOnly: enums.PaddedState.Middle,
  );
  final view = enums.NestedEnumValuesView.create(input);
  final enums.PaddedState state = view.state;
  check(state == enums.PaddedState.Last && view.optional == enums.PaddedState.Middle);
  check(view.boxed == enums.PaddedState.First && view.fixed[1] == enums.PaddedState.Last);
  check(view.states[0] == enums.PaddedState.Middle && view.indexed.getState(0) == enums.PaddedState.Middle);
  check(view.indexed.getOptional(0) == enums.PaddedState.First);
  check(view.storage.bytes.length == nestedEnumsBytes.length);
  for (var i = 0; i < nestedEnumsBytes.length; ++i) {
    check(view.storage.bytes[i] == nestedEnumsBytes[i]);
  }
  check(view.projected.length == 8 && view.projected[0] == 3 && view.projected[4] == 0xffffffff);
  for (var i = 0; i < view.projected.length; ++i) {
    if (i % 4 != 0) check(view.projected[i] == 0);
  }
  check(enums.PaddedState.fromTag(0xffffffff) == enums.PaddedState.Last);
  view.state = enums.PaddedState.First;
  view.optional = null;
  check(view.optional == null);
  view.optional = enums.PaddedState.Last;
  view.setBoxed(enums.PaddedState.Middle);
  view.fixed[1] = enums.PaddedState.Middle;
  view.states[0] = enums.PaddedState.Last;
  view.indexed.setState(0, enums.PaddedState.Last);
  view.indexed.setOptional(0, null);
  check(view.indexed.getOptional(0) == null);
  view.indexed.setOptional(0, enums.PaddedState.Middle);
  view.writeOnly = enums.PaddedState.Last;
  view.projected[4] = enums.PaddedState.Middle.tag;
  final reopened = enums.NestedEnumValuesView(view.storage.bytes);
  check(reopened.state == enums.PaddedState.First && reopened.optional == enums.PaddedState.Last);
  check(reopened.boxed == enums.PaddedState.Middle && reopened.fixed[1] == enums.PaddedState.Middle);
  check(reopened.states[0] == enums.PaddedState.Last && reopened.indexed.getState(0) == enums.PaddedState.Last);
  check(reopened.indexed.getOptional(0) == enums.PaddedState.Middle && reopened.readOnly == enums.PaddedState.Middle);
  check(reopened.projected[4] == enums.PaddedState.Middle.tag);
}
