import 'dart:typed_data';
import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/field-helpers/bindings.dart';
import '../../target/naming/enum_properties.dart' as names;
import '../../target/field-helpers/data.dart';

void check(bool value) {
  if (!value) throw StateError('field helper regression');
}

void equalBytes(Uint8List actual, List<int> expected) {
  check(actual.length == expected.length);
  for (var i = 0; i < expected.length; ++i) {
    if (actual[i] != expected[i]) {
      throw StateError('field helper byte mismatch at $i');
    }
  }
}

void absentMember(Object? Function() action) {
  try {
    action();
  } on NoSuchMethodError {
    return;
  }
  throw StateError('suppressed accessor was emitted');
}

void ranges(FieldRecordView value, int base) {
  final actual = [
    [value.matrixByteOffset, value.matrixByteLength],
    [value.paddedByteOffset, value.paddedByteLength],
    [value.labelByteOffset, value.labelByteLength],
    [value.maybeByteOffset, value.maybeByteLength],
    [value.boxedByteOffset, value.boxedByteLength],
    [value.emptyByteOffset, value.emptyByteLength],
    [value.childByteOffset, value.childByteLength],
    [value.flatFirstByteOffset, value.flatFirstByteLength],
    [value.flatRest0ByteOffset, value.flatRest0ByteLength],
    [value.flatRest1ByteOffset, value.flatRest1ByteLength],
  ];
  const expected = [[0, 64], [64, 16], [80, 8], [96, 12], [108, 4],
    [112, 0], [112, 12], [124, 4], [128, 4], [132, 4]];
  for (var i = 0; i < expected.length; ++i) {
    check(actual[i][0] == base + expected[i][0] && actual[i][1] == expected[i][1]);
  }
}

void main() {
  final record = FieldRecordView(Uint8List.fromList(recordBytes));
  check(FieldRecordView.byteSize == 144);
  ranges(record, 0);
  final dynamic hidden = record;
  for (final action in <Object? Function()>[
    () => hidden.matrix, () => hidden.padded, () => hidden.label, () => hidden.values,
    () => hidden.maybe, () => hidden.boxed, () => hidden.empty, () => hidden.child,
    () => hidden.flat, () => hidden.flatByteOffset, () => hidden.setMatrix,
  ]) {
    absentMember(action);
  }
  check(record.storage.data.getFloat32(record.matrixByteOffset + 60, Endian.little) == 16);
  check(record.valuesByteLength == 16 && record.valuesByteOffset >= FieldRecordView.byteSize);
  check(record.storage.data.getFloat32(record.valuesByteOffset, Endian.little) == 21);
  check(record.storage.data.getFloat32(record.valuesByteOffset + 12, Endian.little) == 24);
  check(record.storage.data.getUint32(record.labelByteOffset + 4, Endian.little) == 11);
  check(record.storage.bytes[record.maybeByteOffset] == 1);
  equalBytes(Uint8List.sublistView(record.storage.bytes, 76, 80), [0, 0, 0, 0]);
  check(record.storage.data.getUint32(record.childByteOffset, Endian.little) == 29);
  check(record.storage.data.getFloat32(record.flatRest1ByteOffset, Endian.little) == 32);
  check(record.getMatrixComponent(15) == 16);
  absentMember(() => hidden.getPaddedComponent);
  check(record.getValuesComponent(2) == 23 && record.getMaybeComponent(1) == 26);
  check(record.getBoxedComponent(0) == 27);
  record.setMatrixComponent(15, 99);
  record.storage.data.setFloat32(record.paddedByteOffset + 4, 61, Endian.little);
  record.setValuesComponent(2, 51);
  record.setMaybeComponent(1, 41);
  record.setBoxedComponent(0, 71);
  equalBytes(record.storage.bytes, changedBytes);

  // Storage's underlying byte offset and the root's logical offset are independent.
  final backing = Uint8List(recordBytes.length + 64);
  backing.setRange(32, 32 + recordBytes.length, recordBytes);
  final shifted = FieldRecordView(Uint8List.sublistView(backing, 16, 32 + recordBytes.length), 16);
  check(shifted.storage.bytes.offsetInBytes == 16);
  ranges(shifted, 16);
  check(shifted.valuesByteOffset == record.valuesByteOffset + 16 && shifted.valuesByteLength == 16);
  check(shifted.storage.data.getFloat32(shifted.valuesByteOffset, Endian.little) == 21);
  check(shifted.storage.data.getFloat32(shifted.matrixByteOffset + 60, Endian.little) == 16);
  shifted.setBoxedComponent(0, 72);
  check(shifted.getBoxedComponent(0) == 72);
  equalBytes(Uint8List.sublistView(backing, 0, 32), List.filled(32, 0));
  final absent = FieldRecordView(Uint8List.fromList(absentBytes));
  check(absent.maybeByteOffset == 96 && absent.maybeByteLength == 12);
  check(absent.storage.bytes[absent.maybeByteOffset] == 0);
  check(absent.getMaybeComponent(0) == null);
  absent.setMaybeComponent(0, 99);
  equalBytes(absent.storage.bytes, absentBytes);

  final rows = FieldRowsView(Uint8List.fromList(rowsBytes));
  check(rows.rows.length == 2);
  check(rows.rowsByteOffset == 16 && rows.rowsByteLength == 288);
  absentMember(() => (rows.rows as dynamic)[0]);
  absentMember(() => (rows.rows as dynamic).getMatrix);
  absentMember(() => (rows.rows as dynamic).setMatrix);
  absentMember(() => (rows.rows as dynamic).getFlatByteOffset);
  for (var i = 0; i < 2; ++i) {
    final base = 16 + i * 144;
    check(rows.rows.getMatrixByteOffset(i) == base && rows.rows.getMatrixByteLength(i) == 64);
    check(rows.rows.getPaddedByteOffset(i) == base + 64 && rows.rows.getPaddedByteLength(i) == 16);
    check(rows.rows.getLabelByteOffset(i) == base + 80 && rows.rows.getLabelByteLength(i) == 8);
    check(rows.rows.getValuesByteLength(i) == 16);
    check(rows.storage.data.getFloat32(rows.rows.getValuesByteOffset(i), Endian.little) == 21);
    check(rows.rows.getMaybeByteOffset(i) == base + 96 && rows.rows.getMaybeByteLength(i) == 12);
    check(rows.rows.getBoxedByteOffset(i) == base + 108 && rows.rows.getBoxedByteLength(i) == 4);
    check(rows.rows.getEmptyByteOffset(i) == base + 112 && rows.rows.getEmptyByteLength(i) == 0);
    check(rows.rows.getChildByteOffset(i) == base + 112 && rows.rows.getChildByteLength(i) == 12);
    check(rows.rows.getFlatFirstByteOffset(i) == base + 124 && rows.rows.getFlatFirstByteLength(i) == 4);
    check(rows.rows.getFlatRest0ByteOffset(i) == base + 128 && rows.rows.getFlatRest0ByteLength(i) == 4);
    check(rows.rows.getFlatRest1ByteOffset(i) == base + 132 && rows.rows.getFlatRest1ByteLength(i) == 4);
  }
  rows.rows.setMatrixComponent(1, 15, 99);
  absentMember(() => (rows.rows as dynamic).getPaddedComponent);
  rows.storage.data.setFloat32(rows.rows.getPaddedByteOffset(1) + 4, 61, Endian.little);
  rows.rows.setValuesComponent(1, 2, 51);
  rows.rows.setMaybeComponent(1, 1, 41);
  rows.rows.setBoxedComponent(1, 0, 71);
  check(rows.rows.getMatrixComponent(0, 15) == 16 && rows.rows.getMatrixComponent(1, 15) == 99);
  final visible = rows.visible[0];
  check(identical(visible.lanes, visible.lanes));
  absentMember(() => (visible as dynamic).getLanesComponent);
  absentMember(() => (rows.visible as dynamic).getLanesComponent);
  check(!identical(rows.visible.getLanes(0), rows.visible.getLanes(0)));
  visible.lanes[0] = 70;
  equalBytes(rows.storage.bytes, rowsChangedBytes);

  final inactive = RawChoiceView(Uint8List.fromList(inactiveBytes));
  check(inactive.dataValueByteOffset == 4 && inactive.dataValueByteLength == 8);
  check(inactive.dataLabelByteOffset == 12 && inactive.dataLabelByteLength == 8);
  check(inactive.dataMaybeByteOffset == 20 && inactive.dataMaybeByteLength == 12);
  check(inactive.dataBoxedByteOffset == 32 && inactive.dataBoxedByteLength == 4);
  check(inactive.getDataValueComponent(0) == null && inactive.getDataBoxedComponent(0) == null);
  inactive.setDataBoxedComponent(0, 99);
  equalBytes(inactive.storage.bytes, inactiveBytes);
  final active = RawChoiceView(Uint8List.fromList(activeBytes));
  check(active.dataMaybeByteLength == 12 && active.getDataMaybeComponent(0) == null);
  final choices = RawChoicesView(Uint8List.fromList(choicesBytes));
  check(choices.items.getDataValueByteOffset(0) == 12 && choices.items.getDataBoxedByteOffset(0) == 40);
  check(choices.items.getDataValueByteOffset(1) == 48 && choices.items.getDataValueByteLength(0) == 8);
  check(choices.items.getDataValueComponent(0, 0) == null);
  choices.items.setDataValueComponent(0, 0, 99);
  equalBytes(choices.storage.bytes, choicesBytes);
  choices.items.setDataValueComponent(1, 1, 90);
  choices.items.setDataBoxedComponent(1, 0, 80);
  equalBytes(choices.storage.bytes, choicesChangedBytes);

  const lanes = intrlay.FixedArrayInput<double>.from([1, 2]);
  final read = ReadHelperView.create(const ReadHelperInput(lanes: lanes));
  check(read.getLanesComponent(1) == 2 && read.lanesByteLength == 8);
  absentMember(() => (read as dynamic).setLanesComponent);
  final write = WriteHelperView.create(const WriteHelperInput(lanes: lanes));
  write.setLanesComponent(1, 9);
  check(write.storage.data.getFloat32(4, Endian.little) == 9);
  absentMember(() => (write as dynamic).getLanesComponent);
  absentMember(() => (write as dynamic).lanesByteOffset);
  final created = CreateHelperView.create(const CreateHelperInput(lanes: lanes));
  check(created.storage.data.getFloat32(4, Endian.little) == 2);
  absentMember(() => (created as dynamic).getLanesComponent);
  absentMember(() => (created as dynamic).setLanesComponent);
  absentMember(() => (created as dynamic).lanesByteOffset);
  final filtered = LanguageHelperView.create(const LanguageHelperInput(filtered: lanes));
  check(filtered.filtered[1] == 2 && filtered.getFilteredComponent(1) == 2);
  check(filtered.filteredByteOffset == 0 && filtered.filteredByteLength == 8);
  final arrays = ArrayRangesView(Uint8List.fromList(arraysBytes));
  final arrayData = arrays.storage.data;
  check(arrays.recordsByteLength == 24);
  check(arrayData.getUint32(arrays.recordsByteOffset, Endian.little) == 101);
  check(arrayData.getUint32(arrays.recordsByteOffset + 12, Endian.little) == 202);
  check(arrays.fixedByteLength == 24);
  check(arrayData.getUint32(arrays.fixedByteOffset, Endian.little) == 303);
  check(arrayData.getUint32(arrays.fixedByteOffset + 12, Endian.little) == 404);
  check(arrays.paddedByteLength == 32);
  check(arrayData.getFloat32(arrays.paddedByteOffset + 16, Endian.little) == 4);
  check(arrayData.getUint32(arrays.paddedByteOffset + 12, Endian.little) == 0);
  check(arrays.nestedByteLength == 16);
  final inner = arrays.nestedByteOffset + arrayData.getUint32(arrays.nestedByteOffset, Endian.little);
  check(arrayData.getUint32(inner, Endian.little) == 11);
  check(arrays.emptyByteLength == 0 && arrays.emptyByteOffset <= arrays.storage.bytes.length);
  check(arrays.unitsByteLength == 0 && arrays.unitsByteOffset <= arrays.storage.bytes.length);
  check(arrays.foldedByteLength == 8);
  check(arrayData.getUint32(arrays.foldedByteOffset, Endian.little) == 51);
  final arrayChoices = ArrayChoicesView(Uint8List.fromList(arrayChoicesBytes));
  arrayChoices.storage.data.setUint32(12, 0xffffffff, Endian.little);
  check(arrayChoices.items.getDataRecordsByteOffset(0) == null);
  check(arrayChoices.items.getDataRecordsByteLength(0) == null);
  check(arrayChoices.items.getDataUnitsByteLength(0) == null);
  check(arrayChoices.items.getDataRecordsByteLength(1) == 12);
  check(arrayChoices.items.getDataUnitsByteLength(1) == 0);
  check(arrayChoices.storage.data.getUint32(arrayChoices.items.getDataRecordsByteOffset(1)!, Endian.little) == 505);
  final foldedHelpers = FoldedHelpersView.create(const FoldedHelpersInput(
    samples: intrlay.SequenceInput<IndexedSampleInput>.from([
      IndexedSampleInput(lanes: intrlay.FixedArrayInput<double>.from([3, 7])),
    ]),
  ));
  equalBytes(foldedHelpers.storage.bytes, foldedHelpersBytes);
  check(foldedHelpers.samples.getLanesComponent(0, 1) == 7);
  check(identical(foldedHelpers.samples, foldedHelpers.samples));
  absentMember(() => (foldedHelpers.samples as dynamic).get);
  absentMember(() => (foldedHelpers.samples as dynamic).getLanes);
  foldedHelpers.samples.setLanesComponent(0, 1, 11);
  equalBytes(foldedHelpers.storage.bytes, foldedHelpersChangedBytes);
  final payload = names.PayloadEventView.create(const names.PayloadEventVariantTaskDoneInput(input_request_id: 7, input_saved_id: 11));
  check(payload.read_task_done_request_id == 7);
  payload.setReadTaskDoneRequestId(19);
  check(payload.read_task_done_request_id == 19 && payload.read_task_done_saved_id == 11);
  absentMember(() => (payload as dynamic).setReadTaskDoneSavedId);
  payload.replace(const names.PayloadEventDeliveredInput(Ticket: 23));
  check(payload.read_delivered_ticket == 23);
  payload.setReadDeliveredTicket(29);
  check(payload.read_delivered_ticket == 29 && payload.read_task_done_request_id == null);
  payload.replace(const names.PayloadEventVariantFlattenedInput(input_payload: names.PayloadRecordInput(input_request_id: 31)));
  check(payload.read_flattened_payload_request_id == 31);
  payload.setReadFlattenedPayloadRequestId(37);
  check(payload.read_flattened_payload_request_id == 37);
  payload.replace(const names.PayloadEventVariantSetOnlyInput(input_request_id: 41));
  payload.setReadSetOnlyRequestId(43);
  check(payload.input_storage.u32(4) == 43);
  absentMember(() => (payload as dynamic).read_set_only_request_id);
  print('Dart: field ranges, component-only access, policies, and byte parity passed');
}
