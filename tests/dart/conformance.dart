import 'package:intrlay/intrlay.dart' as intrlay;
import 'dart:io';
import 'dart:typed_data';
import '../../target/conformance/bindings.dart';

void check(bool condition, String label) {
  if (!condition) throw StateError(label);
}

void main(List<String> arguments) {
  final directory = arguments[0];
  final nullableBytes = File('$directory/nullable.bin').readAsBytesSync();
  final nullable = NullableCasesView(nullableBytes);
  check(nullable.absent == null && nullable.zero == 0, 'nullable scalar');
  check(nullable.text == '' && nullable.missingText == null, 'nullable string');
  check(nullable.absentUnit == null && nullable.presentUnit != null, 'optional unit');
  check(nullable.nestedAbsent == null && nullable.nestedNone != null && nullable.nestedNone!.value == null, 'nested None');
  check(nullable.nestedValue!.value == 7, 'nested value');
  check(nullable.boxedNone != null && nullable.boxedNone!.value == null, 'boxed None');
  check(nullable.triple!.value!.value == null, 'three option levels');
  check(nullable.variants[0].valueValue == null, 'inactive nullable variant');
  check(nullable.variants[1].valueValue != null && nullable.variants[1].valueValue!.value == null, 'active nullable variant');
  check(nullable.variants[2].valueValue!.value == 9, 'active variant value');
  const nullableValue = NullableCasesInput(
    absent: null, zero: 0, text: '', missingText: null,
    absentUnit: null, presentUnit: Some(null),
    nestedAbsent: null, nestedNone: Some(null), nestedValue: Some(7),
    boxedNone: Some(null), triple: Some(Some(null)),
    variants: [NullableVariantEmptyInput(), NullableVariantValueInput(value: null), NullableVariantValueInput(value: 9)],
  );
  final nullableCreated = NullableCasesView.create(nullableValue);
  check(nullableCreated.bytes.length == nullableBytes.length, 'nullable length');
  for (var i = 0; i < nullableBytes.length; ++i) {
    check(nullableCreated.bytes[i] == nullableBytes[i], 'nullable byte $i');
  }
  final bytes = File('$directory/rust.bin').readAsBytesSync();
  final view = MessageView(bytes);
  check(view.sequence == 0x123456789abcdef0, 'u64');
  check(view.title == 'fixture 🦀 한글', 'UTF-8');
  final Float32List pointLanes = view.points;
  check(pointLanes.length == 8 && pointLanes[3] == 0 && pointLanes[7] == 0, 'numeric projection with padding');
  check(pointLanes[6] == 6, 'point');
  check(view.commands[1].drawPointSize == 0.75, 'variant');
  check(view.tree.next!.id == 8 && view.tree.next!.next == null, 'recursive box');
  check(view.optional == 0, 'Some(0)');
  const value = MessageInput(
    sequence: 0x123456789abcdef0,
    title: 'fixture 🦀 한글',
    points: intrlay.SequenceInput.from([PointInput(x: 1, y: 2, z: 3), PointInput(x: 4, y: 5, z: 6)]),
    commands: [DrawingCommandClearInput(), DrawingCommandDrawPointInput(position: PointInput(x: 7, y: 8, z: 9), size: 0.75, label: 'example')],
    tree: LinkInput(id: 7, next: LinkInput(id: 8, next: null)),
    optional: 0,
    fixed: intrlay.FixedArrayInput.from([10, 20, 30]),
  );
  final created = MessageView.create(value);
  check(created.bytes.length == bytes.length, 'length');
  for (var i = 0; i < bytes.length; ++i) { check(created.bytes[i] == bytes[i], 'byte $i'); }
  final owner = Uint8List(bytes.length + 32)..fillRange(0, bytes.length + 32, 0xcc);
  final borrowed = MessageView.createInPlace(Uint8List.sublistView(owner, 16), value);
  borrowed.sequence = 42;
  borrowed.points[6] = 99;
  check(borrowed.sequence == 42 && MessageView(borrowed.bytes).points[6] == 99, 'mutation');
  check(owner[15] == 0xcc && owner.last == 0xcc, 'external capacity');
  File('$directory/dart.bin').writeAsBytesSync(created.bytes);
  print('Dart: exact bytes, recursive views, numeric projection mutation, external subviews passed');
}
