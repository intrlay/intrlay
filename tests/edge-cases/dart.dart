import 'dart:io';
import '../../target/edge-cases/bindings.dart' as bindings;
import '../../target/edge-cases/scalar.dart' as scalar;

void check(bool value) { if (!value) throw StateError('boundary value regression'); }

void main() {
  const values = ['', 'hello', '\uFEFF', '\uFEFFhello', '\uFEFF\uFEFFhello', 'a\uFEFFb', '한글😀'];
  final text = bindings.TextView.create(const bindings.TextInput(values: values));
  final golden = File('target/edge-cases/text.bin').readAsBytesSync();
  check(text.storage.bytes.length == golden.length);
  for (var i = 0; i < golden.length; i++) check(text.storage.bytes[i] == golden[i]);
  final root = bindings.EdgeCasesView(File('target/edge-cases/golden.bin').readAsBytesSync());
  for (var i = 0; i < values.length; i++) {
    check(text.values[i] == values[i]);
    check(root.text.values[i] == values[i]);
  }
  check(root.nested.child!.value!.value!.value == 9);
  check(root.recursive.child!.value!.value == 8);
  check(root.zeroes.after == 42 && root.zeroes.units.length == 0 && root.zeroes.elements.length == 2);
  check(root.shadow.custom.value == 13 && root.shadow.optional == 14 && root.shadow.items[0] == 15);
  check(root.choice.valueChild!.value!.value == 16);
  check(root.projection.numeric.length == 4 && root.projection.named == 2);
  for (var i = 0; i < root.projection.many.length; i++) {
    check(root.projection.many.getX(i) == 3);
    root.projection.many.setX(i, 20);
  }
  check(bindings.EdgeCasesView(root.storage.bytes).projection.many.getX(0) == 20);
  check(bindings.NestedView(File('target/edge-cases/absent-0.bin').readAsBytesSync()).child == null);
  check(bindings.NestedView(File('target/edge-cases/absent-1.bin').readAsBytesSync()).child!.value == null);
  check(bindings.NestedView(File('target/edge-cases/absent-2.bin').readAsBytesSync()).child!.value!.value == null);
  check(scalar.IntrlayU32View.create(123).value == 123);
  print('Dart boundary values and composed layouts passed');
}
