import 'dart:collection';
import 'dart:typed_data';
import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/regressions/contracts.dart';
import '../../target/regressions/projection-data.dart';

void main() => checkProjections();

void _check(bool value) { if (!value) throw StateError('projection mismatch'); }
void _equal(Uint8List actual, Uint8List expected) {
  _check(actual.length == expected.length);
  for (var i = 0; i < actual.length; ++i) { _check(actual[i] == expected[i]); }
}

void checkProjections() {
  final tailOwner = Uint8List(11)..fillRange(0, 11, 0xa5);
  final tailInput = EmptyNumericTailInput(
    prefix: intrlay.FixedArrayInput.from(Uint8List(9)..fillRange(0, 9, 7)), trailer: Float64List(0),
  );
  final tail = EmptyNumericTailView.createInPlace(Uint8List.sublistView(tailOwner, 1, 10), tailInput);
  tail.replace(tailInput);
  _check(tailOwner.first == 0xa5 && tailOwner.last == 0xa5 && tailOwner.sublist(1, 10).every((v) => v == 7));
  try {
    EmptyNumericTailView.create(EmptyNumericTailInput(prefix: const intrlay.FixedArrayInput.zeroed(), trailer: Float64List(1)));
    throw StateError('nonempty projection accepted');
  } on ArgumentError { }
  final sequenceOwner = Uint8List(24)..fillRange(0, 24, 0xa5);
  for (final numeric in [intrlay.SequenceInput.from(Float64List(0)), intrlay.SequenceInput<double>.zeroed(3)]) {
    final sequence = ZeroSizeSequencesView.createInPlace(Uint8List.sublistView(sequenceOwner, 4, 20),
      ZeroSizeSequencesInput(numeric: numeric, ordinary: intrlay.SequenceInput.zeroed(2)));
    _check(sequence.storage.u32(4) == (numeric.values == null ? 3 : 0) && sequence.storage.u32(12) == 2);
  }
  _check(sequenceOwner.take(4).every((v) => v == 0xa5) && sequenceOwner.skip(20).every((v) => v == 0xa5));
  checkSequenceCounts();
  final edges = ProjectionEdgesView.create(ProjectionEdgesInput(
    flattened: const FlattenedEmptyChoiceEmptyInput(payload: EmptyProjectedInput()),
    items: const [FlattenedEmptyChoiceEmptyInput(payload: EmptyProjectedInput())],
    cached: CachedEmptyChoiceEmptyInput(payload: intrlay.FixedArrayInput.from(Float32List(0))),
    floats: Float32List(0), integers: Uint32List(0),
  ));
  _check(edges.flattened.tag == 1 && edges.items.getTag(0) == 1);
  _check(edges.floats.isEmpty && edges.integers.isEmpty && edges.cached.emptyPayload!.isEmpty);
  edges.flattened.replace(const FlattenedEmptyChoiceAbsentInput());
  edges.cached.replace(const CachedEmptyChoiceAbsentInput());
  _check(edges.flattened.tag == 0 && edges.cached.emptyPayload == null);
  edges.cached.replace(CachedEmptyChoiceEmptyInput(payload: intrlay.FixedArrayInput.from(Float32List(0))));
  _check(edges.cached.emptyPayload!.isEmpty);

  final choices = IndexedEnumsView.create(const IndexedEnumsInput(
    items: [
      IndexedChoiceDataInput(label: 'payload', vector: intrlay.FixedArrayInput.from([1, 2, 3]), child: ContractChildInput(value: 7)),
      IndexedChoiceEmptyInput(), IndexedChoiceOptionalInput(value: null), IndexedChoiceUnitInput(value: null),
    ],
    fixed: [IndexedChoiceNumberInput(value: 9), IndexedChoiceOptionalInput(value: 10)],
  ));
  _equal(choices.storage.bytes, Uint8List.fromList(indexedEnumBytes));
  final items = choices.items;
  _check(items.getTag(0) == 3 && items.getDataLabel(0) == 'payload');
  _check(items.getDataVectorComponent(0, 1) == 2 && items.getDataChild(0)!.value == 7);
  _check(items.getOptionalValue(2)!.value == null && items.getOptionalValue(1) == null);
  _check(items.getUnitValue(3)!.value == null && items.getUnitValue(1) == null);
  final before = Uint8List.fromList(choices.storage.bytes);
  items.setNumberValue(0, 99);
  items.setDataVectorComponent(1, 0, 99);
  items.setDataChild(1, const ContractChildInput(value: 99));
  _equal(choices.storage.bytes, before);
  items.setDataVectorComponent(0, 1, 20);
  items.getDataChild(0)!.value = 30;
  choices.fixed.setNumberValue(0, 40);
  _check(items.getDataVector(0)![1] == 20 && items.getDataChild(0)!.value == 30);
  _check(choices.fixed[0].numberValue == 40);
  try {
    (items as dynamic)[0];
    throw StateError('indexed-only exposed an element getter');
  } on NoSuchMethodError { }

  NumericInputsInput input(int matrixCount, int pointCount) => NumericInputsInput(
    matrix: intrlay.FixedArrayInput.from(Float32List.fromList([1, 2, 3, 4].take(matrixCount).map((v) => v.toDouble()).toList())),
    points: intrlay.SequenceInput.from(Float32List.fromList([5, 6, 7, 8, 9, 10].take(pointCount).map((v) => v.toDouble()).toList())),
    tags: Uint32List.fromList([2, 1]), empty: intrlay.FixedArrayInput.from(Float32List(0)),
  );
  final expected = Uint8List.fromList(numericInputBytes);
  _equal(NumericInputsView.create(input(4, 6)).storage.bytes, expected);
  _equal(NumericInputsView.createInPlace(Uint8List(expected.length), input(4, 6)).storage.bytes, expected);
  for (final invalid in [input(3, 6), input(4, 4)]) {
    try { NumericInputsView.create(invalid); throw StateError('invalid lane count'); } on ArgumentError { }
  }
  final empty = NumericInputsView.create(NumericInputsInput(
    matrix: const intrlay.FixedArrayInput.zeroed(), points: intrlay.SequenceInput.zeroed(2),
    tags: Uint32List(0), empty: const intrlay.FixedArrayInput.zeroed(),
  ));
  _check(empty.matrix.length == 4 && empty.matrix.every((v) => v == 0));
  _check(empty.points.length == 6 && empty.points.every((v) => v == 0));
  final enumFields = NumericEnumFieldsView.create(NumericEnumFieldsInput(
    plain: ZNumericTag.First, numeric: Uint32List.fromList([NumericTag.Second.tag]),
  ));
  _check(enumFields.plain == NumericTag.First && enumFields.numeric[0] == ZNumericTag.Second.tag);
  enumFields.plain = NumericTag.Second;
  _check(enumFields.plain == ZNumericTag.Second);
}

void checkSequenceCounts() {
  ZeroSizeSequencesInput zeroed(int numeric, int ordinary) => ZeroSizeSequencesInput(
    numeric: intrlay.SequenceInput.zeroed(numeric),
    ordinary: intrlay.SequenceInput.zeroed(ordinary),
  );
  _equal(ZeroSizeSequencesView.create(zeroed(2, 3)).storage.bytes,
      Uint8List.fromList(zeroSizeSequenceBytes));
  _equal(ZeroSizeSequencesView.create(const ZeroSizeSequencesInput(
    numeric: intrlay.SequenceInput.from([]),
    ordinary: intrlay.SequenceInput.from([EmptyProjectedInput()]),
  )).storage.bytes, ZeroSizeSequencesView.create(zeroed(0, 1)).storage.bytes);

  for (final count in [0, 1, 0xffffffff]) {
    final input = zeroed(count, count);
    _check(ZeroSizeSequencesView.computeByteLength(input) == 16);
    final owner = Uint8List(48)..fillRange(0, 48, 0xa5);
    for (final view in [
      ZeroSizeSequencesView.create(input),
      ZeroSizeSequencesView.createInPlace(Uint8List.sublistView(owner, 16, 32), input),
    ]) {
      _check(view.storage.bytes.length == 16);
      _check(view.storage.u32(4) == count && view.storage.u32(12) == count);
      _check(view.numeric.isEmpty && view.ordinary.length == count);
    }
    _check(owner.take(16).every((value) => value == 0xa5));
    _check(owner.skip(32).every((value) => value == 0xa5));
  }

  for (final input in [
    zeroed(0x100000000, 0), zeroed(0, 0x100000000),
    for (final count in [-1, 0x100000000])
      ZeroSizeSequencesInput(
        numeric: const intrlay.SequenceInput.from([]),
        ordinary: intrlay.SequenceInput.from(_LengthOnlyList<EmptyProjectedInput>(count)),
      ),
  ]) {
    _rejectCount(input, ZeroSizeSequencesView.computeByteLength,
        ZeroSizeSequencesView.create, ZeroSizeSequencesView.createInPlace);
  }
  _expectRangeError(() => intrlay.SequenceInput<double>.zeroed(-1));

  NumericInputsInput points(intrlay.SequenceInput<double> values) => NumericInputsInput(
    matrix: const intrlay.FixedArrayInput.zeroed(), points: values,
    tags: Uint32List(0), empty: const intrlay.FixedArrayInput.zeroed(),
  );
  for (final count in [0, 1]) {
    final view = NumericInputsView.create(points(intrlay.SequenceInput.zeroed(count)));
    _check(view.points.length == count * 3 && view.points.every((value) => value == 0));
  }
  for (final input in [
    for (final count in [0xffffffff, 0x100000000])
      points(intrlay.SequenceInput.zeroed(count)),
    for (final count in [-1, 0x100000000])
      points(intrlay.SequenceInput.from(_LengthOnlyList<double>(count * 3))),
  ]) {
    _rejectCount(input, NumericInputsView.computeByteLength,
        NumericInputsView.create, NumericInputsView.createInPlace);
  }
}

void _expectRangeError(void Function() action) {
  try {
    action();
  } on RangeError {
    return;
  }
  throw StateError('accepted an unrepresentable sequence count');
}

void _rejectCount<T>(T input, int Function(T) measure, Object Function(T) create,
    Object Function(Uint8List, T) createInPlace) {
  _expectRangeError(() => measure(input));
  _expectRangeError(() => create(input));
  final owner = Uint8List(96)..fillRange(0, 96, 0xa5);
  _expectRangeError(() => createInPlace(Uint8List.sublistView(owner, 16, 80), input));
  _check(owner.every((value) => value == 0xa5));
}

// Invalid counts must be rejected before indexing or allocating their elements.
class _LengthOnlyList<T> extends ListBase<T> {
  @override
  final int length;

  _LengthOnlyList(this.length);

  @override
  set length(int value) => throw UnsupportedError('length-only list');

  @override
  T operator [](int index) => throw StateError('length-only list was read');

  @override
  void operator []=(int index, T value) => throw UnsupportedError('length-only list');
}
