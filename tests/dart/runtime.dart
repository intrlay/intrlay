import 'dart:convert';
import 'dart:typed_data';
import 'package:intrlay/intrlay.dart';

void main() {
  checkBulkClearing();
  checkNumericWrites();
  checkStringRanges();
  final texts = ['', 'plain ASCII', 'héllo', '한글', '🦀', 'a🦀b',
    '\ud800', '\udc00', '\ud800\ud800', '\udc00\ud800', '\ud800a\udc00'];
  for (final text in texts) {
    if (utf8ByteLength(text) != utf8.encode(text).length) {
      throw StateError('UTF-8 byte count differs from the standard encoder');
    }
  }
  // Cover every code unit and its interaction with a neighbouring surrogate.
  for (var unit = 0; unit <= 0xffff; ++unit) {
    for (final codes in [[unit], [0xd800, unit], [unit, 0xdc00]]) {
      final text = String.fromCharCodes(codes);
      if (utf8ByteLength(text) != utf8.encode(text).length) {
        throw StateError('UTF-8 byte count mismatch: $codes');
      }
    }
  }
  final owner = Uint8List(32)..fillRange(0, 32, 0xcc);
  final storage = Storage.forWrite(Uint8List.sublistView(owner, 8), 12, 4);
  if (storage.bytes.length != 12 || owner[7] != 0xcc || owner[20] != 0xcc ||
      storage.bytes.any((value) => value != 0)) {
    throw StateError('external storage initialization');
  }
  for (var offset = 0; offset < 16; ++offset) {
    for (var size = 0; size <= 41; ++size) {
      for (final alignment in [1, 2, 4, 8]) {
        final bytes = Uint8List(offset + size + 8)..fillRange(0, offset + size + 8, 0xcc);
        if (offset % alignment != 0) {
          try {
            Storage.forWrite(Uint8List.sublistView(bytes, offset), size, alignment);
            throw StateError('accepted unaligned storage');
          } on ArgumentError { /* Expected capacity/alignment rejection. */ }
          continue;
        }
        Storage.forWrite(Uint8List.sublistView(bytes, offset), size, alignment);
        for (var i = 0; i < bytes.length; ++i) {
          if (bytes[i] != (i >= offset && i < offset + size ? 0 : 0xcc)) {
            throw StateError('clear escaped the borrowed region');
          }
        }
      }
    }
  }
  final wideBytes = Uint8List(48)..fillRange(0, 48, 0xcc);
  final wideData = ByteData.sublistView(wideBytes);
  final signed = int64View(wideBytes.buffer, 8, 2);
  final unsigned = uint64View(wideBytes.buffer, 24, 2);
  for (final value in [-1099511627776, -4294967297, -1, 0, 4294967297, 1099511627776]) {
    writeInt64(wideData, 8, value);
    if (readInt64(wideData, 8) != value || signed[0] != value) {
      throw StateError('signed 64-bit read');
    }
    signed[1] = value;
    if (readInt64(wideData, 16) != value) throw StateError('signed 64-bit write');
  }
  for (final value in [0, 1, 4294967295, 4294967296, 4294967297, 1099511627776]) {
    writeUint64(wideData, 24, value);
    if (readUint64(wideData, 24) != value || unsigned[0] != value) {
      throw StateError('unsigned 64-bit read');
    }
    unsigned[1] = value;
    if (readUint64(wideData, 32) != value) throw StateError('unsigned 64-bit write');
  }
  if (wideBytes[7] != 0xcc || wideBytes[40] != 0xcc) throw StateError('64-bit view bounds');
  if (uint64View(wideBytes.buffer, 0, 0).isNotEmpty) throw StateError('empty 64-bit view');
  print('Dart runtime: UTF-8 sizes, borrowed storage, and 64-bit values passed');
}

void checkStringRanges() {
  Storage storageFor(List<int> encoded) {
    // Both the storage and string start inside a larger allocation. Invalid
    // surrounding UTF-8 must not participate in the string's byte range.
    final owner = Uint8List(encoded.length + 32)..fillRange(0, encoded.length + 32, 0xff);
    final bytes = Uint8List.sublistView(owner, 5, owner.length - 5);
    final data = ByteData.sublistView(bytes);
    data.setUint32(3, 16, Endian.little);
    data.setUint32(7, encoded.length, Endian.little);
    bytes.setRange(19, 19 + encoded.length, encoded);
    return Storage(bytes);
  }

  for (final text in ['', 'ASCII', 'héllo', '한글 🦀', '\u0000', '\uFEFF', '\uFEFFtext', '\uFEFF\uFEFFtext']) {
    final storage = storageFor(utf8.encode(text));
    if (storage.stringAt(3) != text) throw StateError('UTF-8 range or BOM preservation');
  }
  for (final encoded in [[0xc3], [0xff], [0xc0, 0xaf], [0xed, 0xa0, 0x80], [0xf4, 0x90, 0x80, 0x80]]) {
    try {
      storageFor(encoded).stringAt(3);
      throw StateError('accepted malformed UTF-8');
    } on FormatException { /* Expected strict decoding. */ }
  }
  final truncated = storageFor([0xc3, 0xa9]);
  truncated.data.setUint32(7, 1, Endian.little);
  try {
    truncated.stringAt(3);
    throw StateError('UTF-8 decoder read beyond the declared string');
  } on FormatException { /* The continuation byte is outside the string. */ }
  truncated.data.setUint32(7, truncated.bytes.length, Endian.little);
  try {
    truncated.stringAt(3);
    throw StateError('accepted an out-of-bounds string');
  } on RangeError { /* The byte range must fit in the borrowed storage. */ }
}

void checkBulkClearing() {
  for (final prepare in [Storage.forWrite, WriteStorage.forWrite]) {
    for (final size in [0, 1, 7, 16383, 16384, 16385, 32768, 32769, 65539]) {
      for (final offset in [0, 1, 8, 15]) {
        final owner = Uint8List(offset + size + 17)
          ..fillRange(0, offset + size + 17, 0xa5);
        final result = prepare(Uint8List.sublistView(owner, offset), size, 1);
        if (result.bytes.length != size) throw StateError('borrowed size');
        for (var i = 0; i < owner.length; ++i) {
          final expected = i >= offset && i < offset + size ? 0 : 0xa5;
          if (owner[i] != expected) throw StateError('bulk clear boundaries');
        }
      }
    }
    try {
      prepare(Uint8List(8), 9, 1);
      throw StateError('accepted insufficient capacity');
    } on ArgumentError { /* Expected capacity rejection. */ }
    try {
      prepare(Uint8List.sublistView(Uint8List(16), 1), 8, 8);
      throw StateError('accepted unaligned storage');
    } on ArgumentError { /* Expected alignment rejection. */ }
  }
  if (WriteStorage.allocate(0).bytes.isNotEmpty) throw StateError('empty allocation');
}

void checkNumericWrites() {
  for (final value in [-0.0, 1.0 / 3.0, double.infinity, double.negativeInfinity, double.nan]) {
    final expectedBytes = Uint8List(67);
    final expected = ByteData.sublistView(expectedBytes);
    expected.setUint16(0, 0xffff, Endian.little);
    expected.setInt16(2, -32768, Endian.little);
    expected.setUint32(4, 0xffffffff, Endian.little);
    expected.setInt32(8, -2147483648, Endian.little);
    expected.setFloat32(12, value, Endian.little);
    expected.setFloat64(16, value, Endian.little);
    writeInt64(expected, 24, -4294967297);
    writeUint64(expected, 32, 4294967297);
    expected.setUint16(40, 0x10001, Endian.little);
    expected.setUint32(44, 0x100000001, Endian.little);

    for (final offset in [0, 8, 16]) {
      final owner = Uint8List(offset + 67 + 11)..fillRange(0, offset + 67 + 11, 0xcc);
      final writers = [
        WriteStorage.allocate(67),
        WriteStorage.forWrite(Uint8List.sublistView(owner, offset), 67, 8),
      ];
      for (final writer in writers) {
        // Repeated writes exercise cached lanes without allocating per scalar.
        for (var pass = 0; pass < 2; ++pass) {
          writer.writeUint16(0, 0xffff);
          writer.writeInt16(2, -32768);
          writer.writeUint32(4, 0xffffffff);
          writer.writeInt32(8, -2147483648);
          writer.writeFloat32(12, value);
          writer.writeFloat64(16, value);
          writer.writeInt64(24, -4294967297);
          writer.writeUint64(32, 4294967297);
          writer.writeUint16(40, 0x10001);
          writer.writeUint32(44, 0x100000001);
          for (var i = 0; i < expectedBytes.length; ++i) {
            if (writer.bytes[i] != expectedBytes[i]) throw StateError('numeric writer byte $i');
          }
        }
        // Borrowed ByteData access continues to alias numeric construction lanes.
        writer.data.setFloat32(12, 9, Endian.little);
        writer.writeFloat32(12, 11);
        if (writer.data.getFloat32(12, Endian.little) != 11) throw StateError('writer alias');
      }
      if (owner.take(offset).any((value) => value != 0xcc) ||
          owner.skip(offset + 67).any((value) => value != 0xcc)) {
        throw StateError('writer escaped borrowed region');
      }
    }
  }
}
