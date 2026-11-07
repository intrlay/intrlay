import '../../target/compositions/bindings.dart';
import '../../target/compositions/setter-names.dart' as setters;
import 'dart:typed_data';

void check(bool value) { if (!value) throw StateError('composition regression'); }

void main() {
  final numeric = NumericItemsView.create(const NumericItems(items: [NumericItem(id: 7, position: [1, 2, 3], matrix: [[4, 5], [6, 7]], padded: 8, weights: [9, 10], wide: [11, 12], readOnly: [13, 14], writeOnly: [15, 16])]));
  final owner = Uint8List(numeric.bytes.length + 16)..setRange(16, numeric.bytes.length + 16, numeric.bytes);
  final items = NumericItemsView(Uint8List.sublistView(owner, 16)).items;
  check(items.getPositionComponent(0, 2) == 3 && items.getMatrixComponent(0, 3) == 7);
  check(items.getPaddedComponent(0, 3) == 0 && items.getWeightsComponent(0, 1) == 10);
  items.setPositionComponent(0, 1, 22);
  items.setWeightsComponent(0, 0, 29);
  items.setWideComponent(0, 1, 123456789);
  items.setWriteOnlyComponent(0, 0, -123);
  check(items[0].position[1] == 22 && items[0].weights[0] == 29);
  check(items[0].wide[1] == 123456789 && items[0].writeOnly[0] == -123);
  items[0].position[2] = 33;
  check(items.getPositionComponent(0, 2) == 33);
  final settersView = setters.SetterNamesView.create(const setters.SetterNames(value: 1, setValue: 2));
  settersView.value = 3;
  settersView.setValue = 4;
  check(settersView.setValue == 4);
  check(settersView.bytes.buffer.asUint32List(settersView.bytes.offsetInBytes, 2)[0] == 3);
  final names = RenamedListView.create(const RenamedList(items: [Renamed(first: 11, second: 22)]));
  check(names.items.getFirst(0) == 11);
  names.items.setSecond(0, 23);
  check(names.items[0].second == 23);
  final p = ProjectionSetView.create(const ProjectionSet(direct: 1, optional: 2, boxed: 3, elements: [4, 5], readOnly: 6));
  for (final value in [p.direct, p.optional, p.boxed, p.elements[0], p.readOnly]) {
    check(value is Float32List && value.length == 4);
  }
  check(identical(p.direct, p.direct) && identical(p.optional, p.optional) && identical(p.boxed, p.boxed));
  p.optional![0] = 7;
  check(p.optional![0] == 7);
  final w = WrappedScalarsView.create(const WrappedScalars(present: 1, absent: null, boxed: 2, nested: Some(3), missingInner: Some(null), items: [4, null], fixed: [5, null], readOnly: 6, writeOnly: 7));
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
  final c = CachedChildrenView.create(const CachedChildren(child: Shared(value: 1), absent: null, boxed: Shared(value: 2), nested: Some(Shared(value: 3)), choice: CachedChoiceValue(child: Shared(value: 4))));
  check(identical(c.child, c.child) && identical(c.boxed, c.boxed) && identical(c.nested, c.nested));
  check(identical(c.choice.valueChild, c.choice.valueChild));
  c.child!.value = 21; c.nested!.value!.value = 22; c.choice.valueChild!.value!.value = 23;
  check(c.child!.value == 21 && c.nested!.value!.value == 22 && c.choice.valueChild!.value!.value == 23);
  print('Dart composition regressions passed');
  final root = CachedRootView.create(const Shared(value: 1));
  check(identical(root.value, root.value));
  root.value!.value = 2;
  check(root.value!.value == 2);
}
