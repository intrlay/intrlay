import 'dart:typed_data';
import '../../target/reference-scopes/renamed.dart' as renamed;
import '../../target/reference-scopes/omitted.dart' as omitted;
import '../../target/reference-scopes/unrelated_Child.dart' as unrelatedChild;
import '../../target/reference-scopes/unrelated_ChildView.dart' as unrelatedView;
import '../../target/reference-scopes/read_only.dart' as readOnly;
import '../../target/reference-scopes/no_measure.dart' as noMeasure;
import '../../target/reference-scopes/no_mutator.dart' as noMutator;

void check(bool condition) {
  if (!condition) throw StateError('class reference scopes');
}

void main() {
  final view = renamed.PacketView.create(const renamed.Packet(count: 1, item: renamed.Child(number: 2)));
  view.setItem(const renamed.Child(number: 3));
  check(view.count == 1 && view.item.number == 3);
  check(readOnly.PacketView(view.bytes).item.number == 3);
  check(readOnly.checkPrivate(const readOnly.Packet(writePacket: 8, item: readOnly.Child(number: 0)), readOnly.PacketView(view.bytes)) == 9);
  check(omitted.PacketView.create(const omitted.Packet(ChildView: 4, item: omitted.Child(number: 5))).ChildView == 4);
  check(unrelatedChild.PacketView.create(const unrelatedChild.Packet(Child: 6)).Child == 6);
  check(unrelatedView.PacketView.create(const unrelatedView.Packet(ChildView: 7)).ChildView == 7);
  check(noMutator.PacketView.create(const noMutator.Packet(writeChild: 8, item: noMutator.Child(number: 9))).item.number == 9);
  check(noMutator.checkPrivate(const noMutator.Packet(writeChild: 11, item: noMutator.Child(number: 0)), noMutator.PacketView(view.bytes)) == 11);

  final bytes = Uint8List(16);
  final data = ByteData.sublistView(bytes);
  data.setUint32(4, 10, Endian.little);
  data.setUint32(8, 8, Endian.little);
  check(noMeasure.PacketView(bytes).item.number == 10);
  check(noMeasure.PacketView(bytes).text == '');
  check(noMeasure.checkPrivate(const noMeasure.Packet(measurePacket: 12, item: noMeasure.Child(number: 0), text: ''), noMeasure.PacketView(bytes)) == 12);
  print('Dart class reference scopes passed.');
}
