import '../../target/cpp-metadata/alternatives.dart';

void check(bool condition) {
  if (!condition) throw StateError('whole alternative');
}

void main() {
  final view = WholeChoiceView.create(const WholeChoiceValue(payload: Child(value: 31, marker: 37)));
  final alias = WholeChoiceView(view.bytes);
  check(alias.value!.value == 31 && identical(alias.value, alias.value));
  view.setValue(const Child(value: 41, marker: 43));
  check(alias.value!.marker == 43);
  view.replace(const WholeChoiceNested(payload: [41, null]));
  check(alias.value == null && alias.nested![0] == 41 && alias.nested![1] == null);
  view.replace(const WholeChoiceEmpty(payload: null));
  check(alias.nested == null && alias.empty != null);
  view.replace(const WholeChoiceValue(payload: Child(value: 47, marker: 53)));
  check(alias.empty == null && alias.value!.marker == 53);
}
