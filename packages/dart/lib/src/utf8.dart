import 'dart:convert';
import 'dart:typed_data';

/// Decodes a string's byte range without allocating an intermediate byte view.
String decodeUtf8(Uint8List bytes, int start, int length) {
  final value = const Utf8Decoder().convert(bytes, start, start + length);
  // Dart consumes one leading BOM; Intrlay preserves it as part of the value.
  if (length >= 3 && bytes[start] == 0xef &&
      bytes[start + 1] == 0xbb && bytes[start + 2] == 0xbf) {
    return '\uFEFF$value';
  }
  return value;
}
