import 'dart:typed_data';
import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/reference-scopes/renamed.dart' as renamed;
import '../../target/reference-scopes/omitted.dart' as omitted;
import '../../target/reference-scopes/unrelated_ChildInput.dart' as unrelatedChild;
import '../../target/reference-scopes/unrelated_ChildView.dart' as unrelatedView;
import '../../target/reference-scopes/read_only.dart' as readOnly;
import '../../target/reference-scopes/no_measure.dart' as noMeasure;
import '../../target/reference-scopes/no_mutator.dart' as noMutator;
import '../../target/reference-scopes/collection.dart' as collection;
import '../../target/reference-scopes/indexed_only.dart' as indexedOnly;
import '../../target/reference-scopes/omitted_factories.dart' as omittedFactories;
import '../../target/reference-scopes/numeric_alias.dart' as numericAlias;
import '../../target/reference-scopes/public-storage.dart' as publicStorage;
import '../../target/reference-scopes/numeric_input_names.dart' as numericNames;

void check(bool condition) {
  if (!condition) throw StateError('class reference scopes');
}

void main() {
  check(publicStorage.checkPublicStorage());
  final numeric = numericNames.PacketView.create(
    numericNames.PacketInput(payload: Uint32List.fromList([17, 19, 23])),
  );
  check(numeric.payload.join(',') == '17,19,23');
  check(numeric.storage.bytes.join(',') == '17,0,0,0,19,0,0,0,23,0,0,0');
  numeric.setPayload(Uint32List.fromList([29, 31, 37]));
  check(numeric.payload.join(',') == '29,31,37');
  check(numeric.storage.bytes.join(',') == '29,0,0,0,31,0,0,0,37,0,0,0');
  final packed = numericAlias.PacketView.create(Float32List.fromList([2]));
  check(ByteData.sublistView(packed.storage.bytes).getFloat32(0, Endian.little) == 2);
  check(omittedFactories.ChoiceView.byte_size_create == 4);
  check(omittedFactories.ChoiceView(Uint8List(4)).tag == 0);
  final items = collection.ItemsView.create([collection.Choice.Ready]);
  check(items.getLength == 1 && items[0] == collection.Choice.Ready);
  check(collection.Choice.Ready.getTag == 0);
  items[0] = collection.Choice.Ready;
  final indexed = indexedOnly.RootView.Create(const indexedOnly.RootInput(
    items: intrlay.SequenceInput.from([
      indexedOnly.GetValueInput(valueView: 23), indexedOnly.GetValueInput(valueView: 29),
    ]),
    fixedItems: intrlay.FixedArrayInput.from([
      indexedOnly.GetValueInput(valueView: 31), indexedOnly.GetValueInput(valueView: 37),
    ]),
  ));
  check(indexed.items.GetValueView(0) == 23);
  check(indexed.fixedItems.GetValueView(1) == 37);
  indexed.items.SetValueView(1, 41);
  indexed.fixedItems.SetValueView(0, 43);
  check(indexed.items.GetValueView(1) == 41);
  check(indexed.fixedItems.GetValueView(0) == 43);
  final view = renamed.PacketView.create(const renamed.PacketInput(count: 1, item: renamed.ChildInput(number: 2)));
  view.setItem(const renamed.ChildInput(number: 3));
  check(view.count == 1 && view.item.number == 3);
  check(readOnly.PacketView(view.storage.bytes).item.number == 3);
  check(readOnly.checkPrivate(readOnly.PacketView(view.storage.bytes)) == 1);
  check(omitted.PacketView.create(const omitted.PacketInput(ChildView: 4, item: omitted.ChildInput(number: 5))).ChildView == 4);
  check(unrelatedChild.PacketView.create(const unrelatedChild.PacketInput(ChildInput: 6)).ChildInput == 6);
  check(unrelatedView.PacketView.create(const unrelatedView.PacketInput(ChildView: 7)).ChildView == 7);
  check(noMutator.PacketView.create(const noMutator.PacketInput(writeChild: 8, item: noMutator.ChildInput(number: 9))).item.number == 9);
  check(noMutator.checkPrivate(const noMutator.PacketInput(writeChild: 11, item: noMutator.ChildInput(number: 0)), noMutator.PacketView(view.storage.bytes)) == 11);

  final bytes = Uint8List(16);
  final data = ByteData.sublistView(bytes);
  data.setUint32(4, 10, Endian.little);
  data.setUint32(8, 8, Endian.little);
  check(noMeasure.PacketView(bytes).item.number == 10);
  check(noMeasure.PacketView(bytes).text == '');
  check(noMeasure.checkPrivate(12, noMeasure.PacketView(bytes)) == 12);
  print('Dart class reference scopes passed.');
}
