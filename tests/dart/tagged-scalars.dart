// @dart=3.3
import '../../target/dart-writes/tagged-scalars.dart';

void main() {
  final view = TaggedScalarView.create(TaggedScalarValueInput(number: 7, identifier: 11, flag: false));
  view.setValueNumber(13);
  view.setValueIdentifier(17);
  view.setValueFlag(true);
  if (view.valueNumber != 13 || view.valueIdentifier != 17 || view.valueFlag != true) {
    throw StateError('active tagged scalar mutation');
  }
  final absent = TaggedScalarView.create(TaggedScalarEmptyInput());
  absent.setValueNumber(19);
  absent.setValueIdentifier(23);
  absent.setValueFlag(true);
  if (absent.valueNumber != null || absent.valueIdentifier != null || absent.valueFlag != null) {
    throw StateError('inactive tagged scalar mutation');
  }
  print('Dart 3.3 tagged scalar setters passed');
}
