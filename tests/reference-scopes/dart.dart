import 'dart:typed_data';
import '../../target/reference-scopes/renamed.dart' as renamed;
import '../../target/reference-scopes/omitted.dart' as omitted;
import '../../target/reference-scopes/unrelated_ChildInput.dart' as unrelatedChild;
import '../../target/reference-scopes/unrelated_ChildView.dart' as unrelatedView;
import '../../target/reference-scopes/read_only.dart' as readOnly;
import '../../target/reference-scopes/no_measure.dart' as noMeasure;
import '../../target/reference-scopes/no_mutator.dart' as noMutator;
import '../../target/reference-scopes/collection.dart' as collection;
import '../../target/reference-scopes/omitted_factories.dart' as omittedFactories;

void check(bool condition) {
  if (!condition) throw StateError('class reference scopes');
}

void main() {
  check(omittedFactories.ChoiceView.byte_size_create == 4);
  check(omittedFactories.ChoiceView(Uint8List(4)).tag == 0);
  final items = collection.ItemsView.create([collection.ChoiceInput.Ready]);
  check(items.getLength == 1 && items[0].getTag == 0);
  items[0] = collection.ChoiceInput.Ready;
  final view = renamed.PacketView.create(const renamed.PacketInput(count: 1, item: renamed.ChildInput(number: 2)));
  view.setItem(const renamed.ChildInput(number: 3));
  check(view.count == 1 && view.item.number == 3);
  check(readOnly.PacketView(view.bytes).item.number == 3);
  check(readOnly.checkPrivate(readOnly.PacketView(view.bytes)) == 1);
  check(omitted.PacketView.create(const omitted.PacketInput(ChildView: 4, item: omitted.ChildInput(number: 5))).ChildView == 4);
  check(unrelatedChild.PacketView.create(const unrelatedChild.PacketInput(ChildInput: 6)).ChildInput == 6);
  check(unrelatedView.PacketView.create(const unrelatedView.PacketInput(ChildView: 7)).ChildView == 7);
  check(noMutator.PacketView.create(const noMutator.PacketInput(writeChild: 8, item: noMutator.ChildInput(number: 9))).item.number == 9);
  check(noMutator.checkPrivate(const noMutator.PacketInput(writeChild: 11, item: noMutator.ChildInput(number: 0)), noMutator.PacketView(view.bytes)) == 11);

  final bytes = Uint8List(16);
  final data = ByteData.sublistView(bytes);
  data.setUint32(4, 10, Endian.little);
  data.setUint32(8, 8, Endian.little);
  check(noMeasure.PacketView(bytes).item.number == 10);
  check(noMeasure.PacketView(bytes).text == '');
  check(noMeasure.checkPrivate(12, noMeasure.PacketView(bytes)) == 12);
  print('Dart class reference scopes passed.');
}
