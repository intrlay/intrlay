// @dart=3.3
import 'dart:typed_data';
import '../../target/construction-extents/bindings.dart';

class ChangingExtent extends ExtentInput {
  final EmptyInput? first;
  final EmptyInput? second;
  int reads = 0;

  ChangingExtent(this.first, this.second) : super(value: first);

  @override
  EmptyInput? get value => ++reads == 1 ? first : second;
}

void main() {
  for (final grow in [true, false]) {
    final EmptyInput? first = grow ? null : const EmptyInput();
    final EmptyInput? second = grow ? const EmptyInput() : null;
    final backing = Uint8List(96)..fillRange(0, 96, 0xa5);
    for (final create in <ExtentView Function(ExtentInput)>[
      ExtentView.create,
      (value) => ExtentView.createInPlace(Uint8List.sublistView(backing, 16, 80), value),
      (value) => ExtentView.createInPlace(
          ExtentView.create(const ExtentInput(value: EmptyInput())).storage.bytes, value),
    ]) {
      var rejected = false;
      try {
        create(ChangingExtent(first, second));
      } on StateError catch (error) {
        rejected = error.message == 'Intrlay layout changed during construction';
      }
      if (!rejected) throw StateError('changed extent was published');
    }
    if (backing.take(16).any((value) => value != 0xa5) ||
        backing.skip(80).any((value) => value != 0xa5)) {
      throw StateError('borrowed buffer boundary');
    }
    final recovered = ExtentView.createInPlace(
        Uint8List.sublistView(backing, 16, 80), const ExtentInput(value: EmptyInput()));
    if (recovered.storage.bytes.length != 16 || recovered.value == null) {
      throw StateError('recovery after rejection');
    }
  }
  final stable = ExtentView.create(ChangingExtent(const EmptyInput(), const EmptyInput()));
  if (stable.value == null) throw StateError('equal-size getter result');
  print('Dart: dynamic extents, borrowed slices, reuse and recovery passed');
}
