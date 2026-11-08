import 'package:intrlay/intrlay.dart' as intrlay;
import 'dart:typed_data';
import '../../target/dart-writes/bindings.dart';
import '../../target/dart-writes/expected.dart';

void main() {
  const inputs = [
    FrameInput(
      enabled: true,
      choice: ChoiceNumbersInput(field0: NumbersInput(
        signedByte: -127, unsignedByte: 255,
        signedShort: -32768, unsignedShort: 65535,
        signedWord: -2147483648, unsignedWord: 4294967295,
        signedLong: -4294967297, unsignedLong: 4294967297,
        single: -0.0, double: 1.0 / 3.0,
      )),
      optional: intrlay.FixedArrayInput.from([1.25, -3.5]), label: '한글 🦀', values: intrlay.SequenceInput.from([1.0 / 3.0, -2.5, 7.25]),
      arrays: NumericArraysInput(
        i8Values: intrlay.SequenceInput.from([-128, -1, 127]), u8Values: intrlay.SequenceInput.from([0, 127, 255]),
        i16Values: intrlay.SequenceInput.from([-32768, -1, 32767]), u16Values: intrlay.SequenceInput.from([0, 32768, 65535]),
        i32Values: intrlay.SequenceInput.from([-2147483648, -1, 2147483647]), u32Values: intrlay.SequenceInput.from([0, 2147483648, 4294967295]),
        i64Values: intrlay.SequenceInput.from([-4294967297, -1, 4294967297]), u64Values: intrlay.SequenceInput.from([0, 4294967295, 4294967297]),
        f32Values: intrlay.SequenceInput.from([-0.0, 1.0 / 3.0, double.infinity]),
        f64Values: intrlay.SequenceInput.from([-0.0, 1.0 / 3.0, double.negativeInfinity]),
        flags: intrlay.SequenceInput.from([false, true, false]), nested: [intrlay.SequenceInput.from([1.25]), intrlay.SequenceInput.from([]), intrlay.SequenceInput.from([-2.5, 3.75])],
      ),
    ),
    FrameInput(
      enabled: false, choice: ChoiceEmptyInput(), optional: null, label: '', values: intrlay.SequenceInput.from([]),
      arrays: NumericArraysInput(
        i8Values: intrlay.SequenceInput.from([]), u8Values: intrlay.SequenceInput.from([]), i16Values: intrlay.SequenceInput.from([]), u16Values: intrlay.SequenceInput.from([]),
        i32Values: intrlay.SequenceInput.from([]), u32Values: intrlay.SequenceInput.from([]), i64Values: intrlay.SequenceInput.from([]), u64Values: intrlay.SequenceInput.from([]),
        f32Values: intrlay.SequenceInput.from([]), f64Values: intrlay.SequenceInput.from([]), flags: intrlay.SequenceInput.from([]), nested: [],
      ),
    ),
  ];
  final owner = Uint8List(expectedFrames[0].length + 32);
  for (var repeat = 0; repeat < 3; ++repeat) {
    for (var index = 0; index < inputs.length; ++index) {
      final expected = expectedFrames[index];
      final fresh = FrameView.create(inputs[index]);
      final previous = Uint8List.fromList(owner);
      final reused = FrameView.createInPlace(Uint8List.sublistView(owner, 16), inputs[index]);
      for (final view in [fresh, reused]) {
        if (view.bytes.length != expected.length) throw StateError('encoded length');
        for (var i = 0; i < expected.length; ++i) {
          if (view.bytes[i] != expected[i]) throw StateError('Rust byte parity at $i');
        }
        view.enabled = !inputs[index].enabled;
        if (FrameView(view.bytes).enabled != !inputs[index].enabled) throw StateError('mutable view');
      }
      for (var i = 0; i < owner.length; ++i) {
        if ((i < 16 || i >= 16 + expected.length) && owner[i] != previous[i]) {
          throw StateError('external prefix or suffix changed');
        }
      }
      // Exercise clearing stale active payloads, padding, and caller-owned bytes.
      owner.fillRange(0, owner.length, 0xa5);
    }
  }
  print('Dart writers: Rust byte parity, reuse, padding, optional/variant state, and aliases passed');
}
