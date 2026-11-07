import 'dart:io';
import '../../target/reference-fixes/bindings.dart';

void check(bool condition) {
  if (!condition) throw StateError('reference load regression');
}

void main() {
  final input = Fixture(
    custom: AEnumerate(value: 1), deep: 7, nested: Some(8), absent: null,
    missingInner: Some(null), values: [9],
    items: [Item(scalar: 10, child: Some(Child(value: 11)), projected: Some(12.0))],
    cached: Some(Child(value: 13)), projected: Some(14.0), lanes: [15.0, 16.0],
    choice: ChoiceSelected(scalar: Some(17), child: Some(Child(value: 18))),
    inactive: ChoiceEmpty(), before: 19, empty: Empty(), after: 20,
    emptyItems: [Empty(), Empty()], readOnly: Some(21),
  );
  final view = FixtureView.create(input);
  final expected = File('target/reference-fixes/fixture.bin').readAsBytesSync();
  check(view.bytes.length == expected.length);
  for (var index = 0; index < expected.length; ++index) {
    check(view.bytes[index] == expected[index]);
  }
  check(view.deep == 7 && view.nested!.value == 8);
  check(view.absent == null && view.missingInner!.value == null);
  check(view.values[0] == 9 && view.items.getScalar(0) == 10);
  check(view.items.getChild(0)!.value!.value == 11);
  check(view.items.getProjected(0)!.value![0] == 12.0);
  final cached = view.cached;
  check(identical(cached, view.cached));
  cached!.value!.value = 23;
  check(FixtureView(view.bytes).cached!.value!.value == 23);
  check(view.projected!.value![0] == 14.0);
  check(identical(view.projected, view.projected));
  check(view.lanes.length == 8 && view.lanes[0] == 15 && view.lanes[4] == 16);
  check(identical(view.lanes, view.lanes));
  check(view.choice.selectedScalar!.value!.value == 17);
  check(view.choice.selectedChild!.value!.value!.value == 18);
  check(view.inactive.selectedChild == null);
  final noPayload = ChoiceView.create(ChoiceSelected(scalar: null, child: null));
  check(noPayload.selectedChild != null && noPayload.selectedChild!.value == null);
  final noInner = ChoiceView.create(ChoiceSelected(scalar: Some(null), child: Some(null)));
  check(noInner.selectedChild!.value!.value == null);
  check(identical(noInner.selectedChild, noInner.selectedChild));
  check(view.empty.isEmpty && view.emptyItems.isEmpty);
  check(view.before == 19 && view.after == 20 && view.readOnly!.value == 21);
  view.setDeep(31);
  view.setNested(32);
  view.values[0] = 33;
  view.items.setScalar(0, 34);
  check(view.deep == 31 && view.nested!.value == 32);
  check(view.values[0] == 33 && view.items[0].scalar == 34);
  check(DeepRootView.create(35).value == 35);
  final root = CachedRootView.create(Some(Child(value: 36)));
  check(identical(root.value, root.value));
  check(root.value!.value!.value == 36);
  check(CachedRootView.create(null).value == null);
  check(CachedRootView.create(Some(null)).value!.value == null);
  print('Dart reference loads: values, mutation, cache identity, and bytes passed');
}
