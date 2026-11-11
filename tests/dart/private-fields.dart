// @dart=3.3
import 'dart:typed_data';
import '../../target/dart-writes/private-fields.dart';

void main() {
  const input = PrivateFieldsInput(payload: 7, label: 'hello', visible: 11);
  final view = PrivateFieldsView.create(input);
  final reused = PrivateFieldsView.createInPlace(Uint8List(view.storage.bytes.length), input);
  if (view.visible != 11 || ByteData.sublistView(view.storage.bytes).getUint32(0, Endian.little) != 7) {
    throw StateError('private input fields were not encoded');
  }
  for (var index = 0; index < view.storage.bytes.length; ++index) {
    if (view.storage.bytes[index] != reused.storage.bytes[index]) throw StateError('private input reuse');
  }
  checkPrivateFields(view);
  final choice = PrivateChoiceView.create(const PrivateChoiceValueInput(payload: 13));
  if (choice.valuePayload != 13) throw StateError('private enum input');
  print('Dart: private constructor fields support language version 3.3');
}
