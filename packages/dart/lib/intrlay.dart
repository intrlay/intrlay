import 'dart:collection';
import 'dart:convert';
import 'dart:typed_data';

import 'src/utf8.dart' as strings;

// Wasm supports JS interop but retains native 64-bit integers and typed lists.
const _isWasm = bool.fromEnvironment('dart.tool.dart2wasm');
const _isJavaScript = bool.fromEnvironment('dart.library.js_interop') && !_isWasm;
const _wordBase = 4294967296;
final _isLittleEndian = Endian.host == Endian.little;

// A bounded, shared source lets typed-data setRange use bulk memory copying.
// Reusing it avoids both per-message allocation and scalar fillRange loops.
final _zeroBytes = Uint8List(16 * 1024);

// Never is a subtype of every element type, so the request can share List<T>'s
// representation without casting real element storage on each access.
final class _ZeroedElements extends ListBase<Never> {
  @override
  final int length;

  const _ZeroedElements(this.length);

  @override
  set length(int value) => throw UnsupportedError('zero-initialization request');

  @override
  Never operator [](int index) => throw UnsupportedError('zero-initialization request');

  @override
  void operator []=(int index, Never value) => throw UnsupportedError('zero-initialization request');
}

/// A typed construction input erased to its existing element storage.
/// Only schemas whose element encoding supports zero initialization use it.
extension type SequenceInput<T>._(List<T> _value) implements Object {
  const SequenceInput.from(List<T> values) : _value = values;
  SequenceInput.zeroed(int length) : _value = _ZeroedElements(length) {
    RangeError.checkNotNegative(length, 'length');
  }

  @pragma('vm:prefer-inline')
  @pragma('dart2js:tryInline')
  int get length => _value.length;

  @pragma('vm:prefer-inline')
  @pragma('dart2js:tryInline')
  List<T>? get values => _value is _ZeroedElements ? null : _value;
}

const _zeroedFixedArray = _ZeroedElements(0);

/// A fixed array's values or zero initialization using the schema's length.
/// The non-null marker remains distinct from an absent optional array.
extension type const FixedArrayInput<T>._(List<T> _value) implements Object {
  const FixedArrayInput.from(List<T> values) : _value = values;
  const FixedArrayInput.zeroed() : _value = _zeroedFixedArray;

  @pragma('vm:prefer-inline')
  @pragma('dart2js:tryInline')
  List<T>? get values => identical(_value, _zeroedFixedArray) ? null : _value;
}

Uint8List _prepareWriteBytes(Uint8List bytes, int size, int alignment) {
  if (bytes.length < size || bytes.offsetInBytes % alignment != 0) {
    throw ArgumentError('buffer capacity or alignment');
  }
  final result = Uint8List.sublistView(bytes, 0, size);
  for (var offset = 0; offset < size; offset += _zeroBytes.length) {
    final remaining = size - offset;
    final count = remaining < _zeroBytes.length ? remaining : _zeroBytes.length;
    result.setRange(offset, offset + count, _zeroBytes);
  }
  return result;
}

/// Reads a signed little-endian integer. JavaScript precision follows Dart's int.
@pragma('vm:prefer-inline')
int readInt64(ByteData data, int offset) => _isJavaScript
    ? data.getInt32(offset + 4, Endian.little) * _wordBase +
        data.getUint32(offset, Endian.little)
    : data.getInt64(offset, Endian.little);

/// Reads an unsigned little-endian integer without a JavaScript-only BigInt API.
@pragma('vm:prefer-inline')
int readUint64(ByteData data, int offset) => _isJavaScript
    ? data.getUint32(offset + 4, Endian.little) * _wordBase +
        data.getUint32(offset, Endian.little)
    : data.getUint64(offset, Endian.little);

/// Writes a signed integer; JavaScript callers must account for lost precision.
@pragma('vm:prefer-inline')
void writeInt64(ByteData data, int offset, int value) {
  if (_isJavaScript) {
    _writeWideJavaScript(data, offset, value);
  } else {
    data.setInt64(offset, value, Endian.little);
  }
}

/// Writes an unsigned integer using the same little-endian representation.
@pragma('vm:prefer-inline')
void writeUint64(ByteData data, int offset, int value) {
  if (_isJavaScript) {
    _writeWideJavaScript(data, offset, value);
  } else {
    data.setUint64(offset, value, Endian.little);
  }
}

void _writeWideJavaScript(ByteData data, int offset, int value) {
  data.setUint32(offset, value, Endian.little);
  data.setUint32(offset + 4, (value / _wordBase).floor(), Endian.little);
}

/// Counts encoded bytes without allocating a temporary UTF-8 buffer.
/// Unpaired UTF-16 surrogates use the same replacement as [utf8.encode].
int utf8ByteLength(String value) {
  var length = value.length;
  for (var index = 0; index < value.length; ++index) {
    final unit = value.codeUnitAt(index);
    if (unit < 0x80) continue;
    if (unit < 0x800) {
      ++length;
    } else {
      length += 2;
      if (unit >= 0xd800 && unit <= 0xdbff && index + 1 < value.length) {
        final next = value.codeUnitAt(index + 1);
        if (next >= 0xdc00 && next <= 0xdfff) ++index;
      }
    }
  }
  return length;
}

class Cursor {
  int end;

  Cursor(this.end);

  int reserve(int size, int alignment) {
    final start = ((end + alignment - 1) ~/ alignment) * alignment;
    end = start + size;
    if (size < 0 || end > 0xffffffff) {
      throw RangeError('Intrlay buffer exceeds u32 capacity');
    }
    return start;
  }

  /// Rejects an input whose layout changed between measurement and writing.
  @pragma('vm:prefer-inline')
  void finish(int byteLength) {
    if (end != byteLength) {
      _throwLayoutChanged();
    }
  }
}

// Keep exception construction outside the successful completion path.
@pragma('vm:never-inline')
Never _throwLayoutChanged() =>
    throw StateError('Intrlay layout changed during construction');

class Storage {
  final Uint8List bytes;
  final ByteData data;
  /// Reuses numeric write views for in-place updates without clearing any bytes.
  late final WriteStorage writer = WriteStorage._(bytes, aligned: false);
  Storage(this.bytes)
      : data = ByteData.sublistView(bytes);

  /// Borrows exactly [size] bytes and clears all padding before encoding.
  factory Storage.forWrite(Uint8List bytes, int size, int alignment) =>
      Storage(_prepareWriteBytes(bytes, size, alignment));

  int u32(int offset) => data.getUint32(offset, Endian.little);

  @pragma('vm:prefer-inline')
  @pragma('dart2js:tryInline')
  String stringAt(int offset) =>
      strings.decodeUtf8(bytes, offset + u32(offset), u32(offset + 4));
}

/// Numeric write storage with cached lanes for aligned construction buffers.
///
/// Generated encoders supply offsets aligned to each scalar's size. Borrowed
/// views use [Storage.writer], which preserves unaligned ByteData access.
/// Native and Wasm encoders create only the numeric views they use, once per
/// storage.
/// JavaScript uses ByteData for scalar stores and byte views for bulk copies.
final class WriteStorage extends Storage {
  @override
  WriteStorage get writer => this;
  final bool _aligned;
  WriteStorage._(super.bytes, {bool aligned = true}) : _aligned = aligned;

  /// Allocates zero-initialized storage without a second clearing pass.
  factory WriteStorage.allocate(int size) => WriteStorage._(Uint8List(size));

  /// Borrows and clears the encoded region, preserving the caller's other bytes.
  factory WriteStorage.forWrite(Uint8List bytes, int size, int alignment) =>
      WriteStorage._(_prepareWriteBytes(bytes, size, alignment));

  late final Uint16List _uint16 = bytes.buffer.asUint16List(
    bytes.offsetInBytes, bytes.length ~/ 2,
  );
  late final Uint32List _uint32 = bytes.buffer.asUint32List(
    bytes.offsetInBytes, bytes.length ~/ 4,
  );
  late final Uint64List _uint64 = bytes.buffer.asUint64List(
    bytes.offsetInBytes, bytes.length ~/ 8,
  );
  late final Float32List _float32 = bytes.buffer.asFloat32List(
    bytes.offsetInBytes, bytes.length ~/ 4,
  );
  late final Float64List _float64 = bytes.buffer.asFloat64List(
    bytes.offsetInBytes, bytes.length ~/ 8,
  );

  /// Writes the low sixteen bits, shared by signed and unsigned encodings.
  @pragma('vm:prefer-inline')
  void writeUint16(int offset, int value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _uint16[offset >>> 1] = value;
    } else {
      data.setUint16(offset, value, Endian.little);
    }
  }

  /// Writes a signed sixteen-bit value using its two's-complement bits.
  @pragma('vm:prefer-inline')
  void writeInt16(int offset, int value) => writeUint16(offset, value);

  /// Writes the low thirty-two bits, shared by signed and unsigned encodings.
  @pragma('vm:prefer-inline')
  void writeUint32(int offset, int value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _uint32[offset >>> 2] = value;
    } else {
      data.setUint32(offset, value, Endian.little);
    }
  }

  /// Writes a signed thirty-two-bit value using its two's-complement bits.
  @pragma('vm:prefer-inline')
  void writeInt32(int offset, int value) => writeUint32(offset, value);

  /// Writes signed lanes, retaining JavaScript integer conversion where needed.
  @pragma('vm:prefer-inline')
  void writeInt64(int offset, int value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _uint64[offset >>> 3] = value;
    } else if (_isJavaScript) {
      _writeWideJavaScript(data, offset, value);
    } else {
      data.setInt64(offset, value, Endian.little);
    }
  }

  /// Writes unsigned lanes, retaining JavaScript integer conversion where needed.
  @pragma('vm:prefer-inline')
  void writeUint64(int offset, int value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _uint64[offset >>> 3] = value;
    } else if (_isJavaScript) {
      _writeWideJavaScript(data, offset, value);
    } else {
      data.setUint64(offset, value, Endian.little);
    }
  }

  /// Rounds to float32 directly in the cached numeric view.
  @pragma('vm:prefer-inline')
  void writeFloat32(int offset, double value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _float32[offset >>> 2] = value;
    } else {
      data.setFloat32(offset, value, Endian.little);
    }
  }

  /// Writes a float64 without changing its representation.
  @pragma('vm:prefer-inline')
  void writeFloat64(int offset, double value) {
    if (!_isJavaScript && _isLittleEndian && _aligned) {
      _float64[offset >>> 3] = value;
    } else {
      data.setFloat64(offset, value, Endian.little);
    }
  }
}

// Array writers resolve native byte order and the cached destination once.
// Inlining on JavaScript preserves fixed-length loops in the generated caller.
// Dart 3.13 AOT/dart2js measurements: small loops beat range-copy dispatch.
// Wasm uses the native strategy rather than JavaScript's number conversions.
// Code generation applies the same limits statically for fixed-length arrays.
const _arrayLoopLimit = _isJavaScript ? 128 : 4;
// Byte lists already expose the source view, so JavaScript breaks even sooner.
const _byteArrayLoopLimit = _isJavaScript ? 64 : 4;

// JavaScript borrows source bytes only for a bulk copy. Using native lane caches
// here would keep every lazy lane field in every WriteStorage allocation,
// including messages that only write scalars or short arrays.
// Wasm float32 copies also use bytes: setRange between different backing array
// types can convert elements through double, changing float32 NaN payloads.
@pragma('dart2js:tryInline')
void _copyTypedArrayBytes(WriteStorage storage, int offset, TypedData values) {
  final bytes = Uint8List.view(values.buffer, values.offsetInBytes, values.lengthInBytes);
  storage.bytes.setRange(offset, offset + bytes.length, bytes);
}

/// Writes signed 8-bit array elements using their two's-complement bits.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt8Array(WriteStorage storage, int offset, List<int> values) =>
    writeUint8Array(storage, offset, values);

/// Bulk-copies typed byte lists; other inputs retain direct element stores.
@pragma('dart2js:tryInline')
void writeUint8Array(WriteStorage storage, int offset, List<int> values) {
  if (values.length > _byteArrayLoopLimit && (values is Uint8List || values is Int8List)) {
    storage.bytes.setRange(offset, offset + values.length, values);
  } else {
    writeUint8ArrayElements(storage, offset, values);
  }
}

/// Writes a short signed byte array without range-copy setup.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt8ArrayElements(WriteStorage storage, int offset, List<int> values) =>
    writeUint8ArrayElements(storage, offset, values);

/// Writes a short byte array without range-copy setup.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeUint8ArrayElements(WriteStorage storage, int offset, List<int> values) {
  final target = storage.bytes;
  for (var index = 0; index < values.length; ++index) {
    target[offset + index] = values[index];
  }
}

/// Writes signed 16-bit array elements using their two's-complement bits.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt16Array(WriteStorage storage, int offset, List<int> values) =>
    writeUint16Array(storage, offset, values);

/// Bulk-copies compatible integer lanes, retaining conversion for other lists.
@pragma('dart2js:tryInline')
void writeUint16Array(WriteStorage storage, int offset, List<int> values) {
  if (values.length > _arrayLoopLimit && _isLittleEndian && storage._aligned &&
      (values is Uint16List || values is Int16List)) {
    if (_isJavaScript) {
      _copyTypedArrayBytes(storage, offset, values as TypedData);
    } else {
      final start = offset >>> 1;
      storage._uint16.setRange(start, start + values.length, values);
    }
  } else {
    writeUint16ArrayElements(storage, offset, values);
  }
}

/// Writes signed 16-bit elements without bulk-copy dispatch.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt16ArrayElements(WriteStorage storage, int offset, List<int> values) =>
    writeUint16ArrayElements(storage, offset, values);

/// Resolves the 16-bit destination once and writes elements individually.
@pragma('dart2js:tryInline')
void writeUint16ArrayElements(WriteStorage storage, int offset, List<int> values) {
  if (!_isJavaScript && _isLittleEndian && storage._aligned) {
    final lanes = storage._uint16;
    final start = offset >>> 1;
    for (var index = 0; index < values.length; ++index) {
      lanes[start + index] = values[index];
    }
  } else {
    final target = storage.data;
    for (var index = 0; index < values.length; ++index) {
      target.setUint16(offset + index * 2, values[index], Endian.little);
    }
  }
}

/// Writes signed 32-bit array elements using their two's-complement bits.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt32Array(WriteStorage storage, int offset, List<int> values) =>
    writeUint32Array(storage, offset, values);

/// Bulk-copies compatible integer lanes, retaining conversion for other lists.
@pragma('dart2js:tryInline')
void writeUint32Array(WriteStorage storage, int offset, List<int> values) {
  if (values.length > _arrayLoopLimit && _isLittleEndian && storage._aligned &&
      (values is Uint32List || values is Int32List)) {
    if (_isJavaScript) {
      _copyTypedArrayBytes(storage, offset, values as TypedData);
    } else {
      final start = offset >>> 2;
      storage._uint32.setRange(start, start + values.length, values);
    }
  } else {
    writeUint32ArrayElements(storage, offset, values);
  }
}

/// Writes signed 32-bit elements without bulk-copy dispatch.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt32ArrayElements(WriteStorage storage, int offset, List<int> values) =>
    writeUint32ArrayElements(storage, offset, values);

/// Resolves the 32-bit destination once and writes elements individually.
@pragma('dart2js:tryInline')
void writeUint32ArrayElements(WriteStorage storage, int offset, List<int> values) {
  if (!_isJavaScript && _isLittleEndian && storage._aligned) {
    final lanes = storage._uint32;
    final start = offset >>> 2;
    for (var index = 0; index < values.length; ++index) {
      lanes[start + index] = values[index];
    }
  } else {
    final target = storage.data;
    for (var index = 0; index < values.length; ++index) {
      target.setUint32(offset + index * 4, values[index], Endian.little);
    }
  }
}

/// Writes signed 64-bit array elements using their two's-complement bits.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt64Array(WriteStorage storage, int offset, List<int> values) =>
    writeUint64Array(storage, offset, values);

/// Bulk-copies native integer lanes; JavaScript keeps its integer conversion.
@pragma('dart2js:tryInline')
void writeUint64Array(WriteStorage storage, int offset, List<int> values) {
  if (!_isJavaScript && values.length > _arrayLoopLimit && _isLittleEndian && storage._aligned &&
      (values is Uint64List || values is Int64List)) {
    final start = offset >>> 3;
    storage._uint64.setRange(start, start + values.length, values);
  } else {
    writeUint64ArrayElements(storage, offset, values);
  }
}

/// Writes signed 64-bit elements without bulk-copy dispatch.
@pragma('vm:prefer-inline')
@pragma('dart2js:tryInline')
void writeInt64ArrayElements(WriteStorage storage, int offset, List<int> values) =>
    writeUint64ArrayElements(storage, offset, values);

/// Resolves the 64-bit destination once and writes elements individually.
@pragma('dart2js:tryInline')
void writeUint64ArrayElements(WriteStorage storage, int offset, List<int> values) {
  if (!_isJavaScript && _isLittleEndian && storage._aligned) {
    final lanes = storage._uint64;
    final start = offset >>> 3;
    for (var index = 0; index < values.length; ++index) {
      lanes[start + index] = values[index];
    }
  } else {
    final target = storage.data;
    for (var index = 0; index < values.length; ++index) {
      writeUint64(target, offset + index * 8, values[index]);
    }
  }
}

/// Bulk-copies Float32List lanes, retaining rounding for other numeric lists.
@pragma('dart2js:tryInline')
void writeFloat32Array(WriteStorage storage, int offset, List<double> values) {
  if (values.length > _arrayLoopLimit && _isLittleEndian && storage._aligned && values is Float32List) {
    if (_isJavaScript || _isWasm) {
      _copyTypedArrayBytes(storage, offset, values);
    } else {
      final start = offset >>> 2;
      storage._float32.setRange(start, start + values.length, values);
    }
  } else {
    writeFloat32ArrayElements(storage, offset, values);
  }
}

/// Writes float32 elements with one destination lookup and no bulk dispatch.
@pragma('dart2js:tryInline')
void writeFloat32ArrayElements(WriteStorage storage, int offset, List<double> values) {
  if (!_isJavaScript && _isLittleEndian && storage._aligned) {
    final lanes = storage._float32;
    final start = offset >>> 2;
    for (var index = 0; index < values.length; ++index) {
      lanes[start + index] = values[index];
    }
  } else {
    final target = storage.data;
    for (var index = 0; index < values.length; ++index) {
      target.setFloat32(offset + index * 4, values[index], Endian.little);
    }
  }
}

/// Bulk-copies Float64List lanes, retaining conversion for other numeric lists.
@pragma('dart2js:tryInline')
void writeFloat64Array(WriteStorage storage, int offset, List<double> values) {
  if (values.length > _arrayLoopLimit && _isLittleEndian && storage._aligned && values is Float64List) {
    if (_isJavaScript) {
      _copyTypedArrayBytes(storage, offset, values);
    } else {
      final start = offset >>> 3;
      storage._float64.setRange(start, start + values.length, values);
    }
  } else {
    writeFloat64ArrayElements(storage, offset, values);
  }
}

/// Writes float64 elements with one destination lookup and no bulk dispatch.
@pragma('dart2js:tryInline')
void writeFloat64ArrayElements(WriteStorage storage, int offset, List<double> values) {
  if (!_isJavaScript && _isLittleEndian && storage._aligned) {
    final lanes = storage._float64;
    final start = offset >>> 3;
    for (var index = 0; index < values.length; ++index) {
      lanes[start + index] = values[index];
    }
  } else {
    final target = storage.data;
    for (var index = 0; index < values.length; ++index) {
      target.setFloat64(offset + index * 8, values[index], Endian.little);
    }
  }
}

/// Borrows signed 64-bit lanes. JavaScript retains Dart's int precision limits.
List<int> int64View(ByteBuffer buffer, int offset, int length) => _isJavaScript
    ? _Int64View(ByteData.view(buffer, offset, length * 8), true)
    : buffer.asInt64List(offset, length);

/// Borrows unsigned 64-bit lanes, with a ByteData fallback on JavaScript targets.
List<int> uint64View(ByteBuffer buffer, int offset, int length) => _isJavaScript
    ? _Int64View(ByteData.view(buffer, offset, length * 8), false)
    : buffer.asUint64List(offset, length);

class _Int64View extends ListBase<int> {
  final ByteData _data;
  final bool _signed;

  _Int64View(this._data, this._signed);

  @override
  int get length => _data.lengthInBytes ~/ 8;

  @override
  set length(int value) => throw UnsupportedError('fixed-length buffer view');

  @override
  int operator [](int index) => _signed
      ? readInt64(_data, index * 8)
      : readUint64(_data, index * 8);

  @override
  void operator []=(int index, int value) {
    if (_signed) {
      writeInt64(_data, index * 8, value);
    } else {
      writeUint64(_data, index * 8, value);
    }
  }
}

/// Presence wrapper for an optional whose payload can itself be null.
final class Some<T> {
  final T value;
  const Some(this.value);
}
