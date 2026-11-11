import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/regressions/bindings.dart';
import '../../target/regressions/arrays.dart';
import '../../target/regressions/alpha.dart';
import '../../target/regressions/numeric-copy.dart' as numeric;
import '../../target/regressions/storage_emission.dart' as storage;
import '../../target/regressions/view_emission.dart' as outbound;
import 'dart:typed_data';

void main() {
  const outboundInput = outbound.OutboundPacketInput(
    records: [outbound.InternalRecordInput(value: 7, label: 'nested')],
    optional: outbound.InternalRecordInput(value: 8, label: 'optional'),
    choice: outbound.InternalChoiceRecordInput(payload: outbound.InternalRecordInput(value: 9, label: 'variant')),
  );
  final packet = outbound.OutboundPacketView.create(outboundInput);
  final packetInPlace = outbound.OutboundPacketView.createInPlace(Uint8List(packet.storage.bytes.length), outboundInput);
  expectBytes(packet.storage.bytes, packetInPlace.storage.bytes);
  final view = ProjectionsView.create(ProjectionsInput(vectors: intrlay.SequenceInput.from([1, 2, 3, 0])));
  final lanes = view.vectors;
  if (lanes.length != 4 || lanes[2] != 3 || lanes[3] != 0) throw StateError('padded projection');
  lanes[1] = 9;
  if (view.vectors[1] != 9) throw StateError('borrowed mutation');
  final arrays = FixedArraysView.create(FixedArraysInput(
    three: intrlay.FixedArrayInput.from([1, 2, 3]), four: intrlay.FixedArrayInput.from([4, 5, 6, 7]), matrix: intrlay.FixedArrayInput.from([intrlay.FixedArrayInput.from([8, 9, 10]), intrlay.FixedArrayInput.from([11, 12, 13])]),
    present: intrlay.FixedArrayInput.from([14, 15, 16]), absent: null, readOnly: intrlay.FixedArrayInput.from([17, 18, 19]),
  ));
  final Float32List three = arrays.three;
  if (three.length != 3 || arrays.four.length != 4 || arrays.matrix.length != 6 || arrays.matrix[5] != 13) {
    throw StateError('fixed extents and nested flattening');
  }
  if (arrays.present![2] != 16 || arrays.absent != null) throw StateError('optional arrays');
  three[1] = 20;
  if (arrays.three[1] != 20) throw StateError('fixed view mutation');
  final cached = AlphaView.create(AlphaInput(values: intrlay.SequenceInput.from([1, 2])));
  if (!identical(cached.values, cached.values)) throw StateError('cached collection view');
  numericCopies();
  const creationInput = storage.CreateOnlyInput(count: 7, label: 'created', fixed: intrlay.FixedArrayInput.from([2, 5]), optional: 11);
  final created = storage.CreateOnlyView.create(creationInput);
  final inPlace = storage.CreateOnlyView.createInPlace(Uint8List(created.storage.bytes.length), creationInput);
  expectBytes(inPlace.storage.bytes, created.storage.bytes);
  if (storage.CreateOnlyView.computeByteLength(creationInput) != created.storage.bytes.length) throw StateError('create-only layout');
  final zeroArray = storage.ZeroArrayView(Uint8List(0));
  zeroArray.setValues(const intrlay.FixedArrayInput<storage.ZeroPayloadInput>.from([]));
  zeroArray.replace(const storage.ZeroArrayInput(values: intrlay.FixedArrayInput.from([])));
  if (zeroArray.values.length != 0) throw StateError('empty array without element writers');
  print('Dart: numeric projections passed');
}

void expectBytes(List<int> actual, List<int> expected) {
  if (actual.length != expected.length) throw StateError('numeric byte length');
  for (var i = 0; i < expected.length; ++i) {
    if (actual[i] != expected[i]) throw StateError('numeric bytes at $i');
  }
}

void checkNumeric<T extends num>(List<T> values, Uint8List Function(List<T>) encode) {
  expectBytes(encode(values), encode(List<T>.of(values)));
  expectBytes(encode(values.sublist(0, 0)), encode(<T>[]));
}

void numericCopies() {
  final signed = List<int>.generate(512, (i) => i.isEven ? -1 : 1);
  final positive = List<int>.generate(512, (i) => i % 256);
  final real = List<double>.generate(512, (i) => i.isEven ? -0.0 : i / 3);
  checkNumeric<int>(Int8List.fromList(signed),
      (v) => numeric.I8View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Uint8List.fromList(positive),
      (v) => numeric.U8View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Int16List.fromList(signed),
      (v) => numeric.I16View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Uint16List.fromList(positive),
      (v) => numeric.U16View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Int32List.fromList(signed),
      (v) => numeric.I32View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Uint32List.fromList(positive),
      (v) => numeric.U32View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<double>(Float32List.fromList(real),
      (v) => numeric.F32View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<double>(Float64List.fromList(real),
      (v) => numeric.F64View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  if (!const bool.fromEnvironment('dart.library.js_interop')) {
    checkNumeric<int>(Int64List.fromList(signed),
        (v) => numeric.I64View.create(intrlay.SequenceInput.from(v)).storage.bytes);
    checkNumeric<int>(Uint64List.fromList(positive),
        (v) => numeric.U64View.create(intrlay.SequenceInput.from(v)).storage.bytes);
    expectBytes(numeric.FixedWideShortView.create(intrlay.FixedArrayInput.from(Int64List.fromList([-1, 1]))).storage.bytes,
        numeric.FixedWideShortView.create(const intrlay.FixedArrayInput.from([-1, 1])).storage.bytes);
    expectBytes(numeric.FixedWideLongView.create(intrlay.FixedArrayInput.from(Int64List.fromList(signed.sublist(0, 16)))).storage.bytes,
        numeric.FixedWideLongView.create(intrlay.FixedArrayInput.from(signed.sublist(0, 16))).storage.bytes);
  }

  // A different numeric lane still converts rather than copying its bytes.
  checkNumeric<double>(Float64List.fromList(real),
      (v) => numeric.F32View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Int16List.fromList(signed),
      (v) => numeric.U16View.create(intrlay.SequenceInput.from(v)).storage.bytes);
  checkNumeric<int>(Uint32List.fromList(positive),
      (v) => numeric.I32View.create(intrlay.SequenceInput.from(v)).storage.bytes);

  final padded = Float32List.fromList([99.0, ...real, 99.0]);
  final largeSource = Float32List.sublistView(padded, 1, 513).asUnmodifiableView();
  final largeExpected = numeric.FixedBulkView.create(intrlay.FixedArrayInput.from(List<double>.of(largeSource))).storage.bytes;
  expectBytes(numeric.FixedBulkView.create(intrlay.FixedArrayInput.from(largeSource)).storage.bytes, largeExpected);
  final largeOwner = Uint8List(largeExpected.length + 16)..fillRange(0, largeExpected.length + 16, 0xa5);
  final largeActual = numeric.FixedBulkView.createInPlace(
      Uint8List.sublistView(largeOwner, 8, largeOwner.length - 8), intrlay.FixedArrayInput.from(largeSource));
  expectBytes(largeActual.storage.bytes, largeExpected);
  if (largeOwner.take(8).any((v) => v != 0xa5) || largeOwner.skip(largeOwner.length - 8).any((v) => v != 0xa5)) {
    throw StateError('bulk copy changed bytes outside the output');
  }
  if (Endian.host == Endian.little) {
    final words = Uint32List(512)..fillRange(0, 512, 0x7fa12345);
    final nanValues = Float32List.view(words.buffer);
    expectBytes(numeric.FixedBulkView.create(intrlay.FixedArrayInput.from(nanValues)).storage.bytes,
        words.buffer.asUint8List());
  }
  final source = Float32List.sublistView(padded, 1, 129).asUnmodifiableView();
  final expected = numeric.FixedLongView.create(intrlay.FixedArrayInput.from(List<double>.of(source))).storage.bytes;
  final owner = Uint8List(expected.length + 16)..fillRange(0, expected.length + 16, 0xa5);
  final actual = numeric.FixedLongView.createInPlace(
      Uint8List.sublistView(owner, 8, owner.length - 8), intrlay.FixedArrayInput.from(source));
  expectBytes(actual.storage.bytes, expected);
  if (owner.take(8).any((v) => v != 0xa5) || owner.skip(owner.length - 8).any((v) => v != 0xa5)) {
    throw StateError('bulk copy changed bytes outside the output');
  }
  expectBytes(numeric.FixedShortView.create(intrlay.FixedArrayInput.from(Float32List.fromList([1.25, -0.0]))).storage.bytes,
      numeric.FixedShortView.create(const intrlay.FixedArrayInput.from([1.25, -0.0])).storage.bytes);
  expectBytes(numeric.FixedMediumView.create(intrlay.FixedArrayInput.from(Float32List.sublistView(source, 0, 16))).storage.bytes,
      numeric.FixedMediumView.create(intrlay.FixedArrayInput.from(source.sublist(0, 16))).storage.bytes);
  expectBytes(numeric.FixedBytesShortView.create(intrlay.FixedArrayInput.from(Uint8List.fromList([0, 255]))).storage.bytes,
      numeric.FixedBytesShortView.create(const intrlay.FixedArrayInput.from([0, 255])).storage.bytes);
  expectBytes(numeric.FixedBytesLongView.create(intrlay.FixedArrayInput.from(Uint8List.fromList(positive.sublist(0, 128)))).storage.bytes,
      positive.sublist(0, 128));
  expectBytes(numeric.FixedLongView.create(intrlay.FixedArrayInput.zeroed()).storage.bytes, Uint8List(512));
  try {
    numeric.FixedLongView.create(intrlay.FixedArrayInput.from(Float32List(127)));
    throw StateError('bulk copy skipped fixed extent validation');
  } on ArgumentError {
    // Length validation must precede either copy implementation.
  }
  final nested = numeric.NestedView.create(intrlay.FixedArrayInput.from([
    intrlay.FixedArrayInput.from(source), intrlay.FixedArrayInput.from(source),
  ]));
  expectBytes(nested.storage.bytes, [...expected, ...expected]);

  // Borrowed views can begin at an unaligned offset; their writer uses ByteData.
  final unaligned = Uint8List(519)..fillRange(0, 519, 0xa5);
  final storage = intrlay.Storage(Uint8List.sublistView(unaligned, 1, 518));
  intrlay.writeFloat32Array(storage.writer, 3, source);
  expectBytes(unaligned.sublist(4, 516), expected);
  if (unaligned.take(4).any((v) => v != 0xa5) || unaligned.skip(516).any((v) => v != 0xa5)) {
    throw StateError('unaligned fallback overwrote surrounding bytes');
  }
}
