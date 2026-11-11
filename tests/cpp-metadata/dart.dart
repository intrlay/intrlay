import '../../target/cpp-metadata/alternatives.dart';
import '../../target/cpp-metadata/enum_getters.dart' as named;

void check(bool condition) {
  if (!condition) throw StateError('whole alternative');
}

void main() {
  final namedWhole = named.WholeChoiceView.create(const named.WholeChoiceValueInput(payload: named.ChildInput(value: 31, marker: 37)));
  check(namedWhole.read_value!.read_value == 31);
  final namedOrdinary = named.OrdinaryChoiceView.create(const named.OrdinaryChoiceValueInput(value: 23, nativeExtra: 29));
  check(namedOrdinary.read_value_value!.value == 23);
  final view = WholeChoiceView.create(const WholeChoiceValueInput(payload: ChildInput(value: 31, marker: 37)));
  final alias = WholeChoiceView(view.storage.bytes);
  check(alias.value!.value == 31 && identical(alias.value, alias.value));
  view.setValue(const ChildInput(value: 41, marker: 43));
  check(alias.value!.marker == 43);
  view.replace(const WholeChoiceNestedInput(payload: [41, null]));
  check(alias.value == null && alias.nested![0] == 41 && alias.nested![1] == null);
  view.replace(const WholeChoiceEmptyInput(payload: null));
  check(alias.nested == null && alias.empty != null);
  view.replace(const WholeChoiceValueInput(payload: ChildInput(value: 47, marker: 53)));
  check(alias.empty == null && alias.value!.marker == 53);
}
