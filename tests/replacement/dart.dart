import 'dart:typed_data';
import '../../target/replacement/bindings.dart';

void check(bool value) { if (!value) throw StateError('in-place replacement'); }

void main() {
  final view = MessageView.create(const Message(before:123, fixed:Fixed(maybe:null, nested:Some(null), choice:ChoiceEmpty()), items:[null], text:'preserved', after:456));
  final alias = MessageView(view.bytes);
  final fixed = alias.fixed;
  check(fixed.maybe == null && fixed.nested!.value == null);
  check(fixed.choice.childLeaf == null);
  const leaf = Leaf(number:7, scalar:8);
  final boxed = BoxedLeafView.create(null);
  final boxedAlias = BoxedLeafView(boxed.bytes);
  check(boxedAlias.value == null);
  boxed.set(leaf);
  check(boxedAlias.value!.number == 7);
  boxed.set(null);
  check(boxedAlias.value == null);
  view.fixed.setMaybe(leaf);
  check(fixed.maybe!.number == 7 && identical(fixed.maybe, fixed.maybe));
  view.fixed.setNested(const Some(leaf));
  check(fixed.nested!.value!.number == 7);
  view.fixed.choice.replace(const ChoiceChild(leaf:leaf));
  check(fixed.choice.childLeaf!.number == 7);
  view.fixed.choice.replace(const ChoiceNumbers(values:[1,2,3]));
  check(fixed.choice.childLeaf == null && fixed.choice.numbersValues![2] == 3);
  view.fixed.setMaybe(null);
  view.fixed.setNested(const Some(null));
  check(fixed.maybe == null && fixed.nested!.value == null);
  view.fixed.setNested(null);
  check(fixed.nested == null);
  view.items[0] = leaf;
  check(alias.items[0]!.number == 7);
  view.items[0] = null;
  check(alias.items[0] == null);
  final scalar = MaybeScalarView.create(null);
  final noFactory = NoFactoryView(scalar.bytes);
  noFactory.replace(const NoFactory(value:17));
  check(scalar.value == 17);
  scalar.value = 42;
  check(scalar.value == 42);
  scalar.value = null;
  check(scalar.value == null);
  final nested = NestedView.create(null);
  final other = NestedView(nested.bytes);
  check(other.value == null);
  nested.replace(const Some(leaf));
  check(other.value!.value!.number == 7);
  nested.replace(const Some(null));
  check(other.value!.value == null);
  final emptyPayloads = EmptyPayloadsView.create(const EmptyPayloads(empty: [], nested: [[], []], number: 1));
  final emptyAlias = EmptyPayloadsView(emptyPayloads.bytes);
  const replacement = EmptyPayloads(empty: [], nested: [[], []], number: 2);
  emptyPayloads.replace(replacement);
  check(emptyAlias.number == 2 && emptyAlias.bytes.length == 4);
  check(emptyAlias.bytes[0] == 2 && emptyAlias.bytes.skip(1).every((byte) => byte == 0));
  check(emptyAlias.empty.length == 0 && emptyAlias.nested.length == 2);
  check(emptyAlias.nested[1].length == 0);
  view.fixed.choice.replace(const ChoiceEmptyArrays(data: replacement));
  check(fixed.choice.emptyArraysData!.number == 2);
  check(alias.before == 123 && alias.after == 456 && alias.text == 'preserved');

  // Scalar reads and writes support unaligned storage and record offsets.
  final storage = Uint8List(view.bytes.length + 2)..fillRange(0, view.bytes.length + 2, 0xa5);
  storage.setRange(2, 2 + view.bytes.length, view.bytes);
  final unaligned = MessageView(Uint8List.sublistView(storage, 1), 1);
  unaligned.fixed.setMaybe(leaf);
  unaligned.fixed.choice.replace(const ChoiceNumbers(values:[21,22,23]));
  check(unaligned.fixed.maybe!.number == 7);
  // Typed-array getters require aligned storage; inspect a copy after the write.
  final aligned = MessageView(Uint8List.fromList(unaligned.bytes.sublist(1)));
  check(aligned.fixed.choice.numbersValues![2] == 23);
  check(unaligned.before == 123 && unaligned.after == 456 && unaligned.text == 'preserved');
  check(storage[0] == 0xa5 && storage[1] == 0xa5);
  print('Dart: in-place replacement and shared view caches passed');
}
