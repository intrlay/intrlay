import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { EdgeCasesView, NestedView, TextView } from "../../target/edge-cases/bindings.ts";
import { IntrlayU32View } from "../../target/edge-cases/scalar.ts";

const read = name => new Uint8Array(readFileSync(new URL(`../../target/edge-cases/${name}.bin`, import.meta.url)));
const values = ["", "hello", "\uFEFF", "\uFEFFhello", "\uFEFF\uFEFFhello", "a\uFEFFb", "한글😀"];
const text = TextView.create({ values });
assert.deepEqual(text.bytes, read("text"));
for (let i = 0; i < values.length; i++) assert.equal(text.values.get(i), values[i]);
const root = new EdgeCasesView(read("golden"));
for (let i = 0; i < values.length; i++) assert.equal(root.text.values.get(i), values[i]);
assert.equal(root.nested.child.value.value.value, 9);
assert.equal(root.recursive.child.value.value, 8);
assert.equal(root.zeroes.after, 42);
assert.equal(root.zeroes.units.length, 0);
assert.equal(root.zeroes.elements.length, 2);
assert.equal(root.shadow.custom.value, 13);
assert.equal(root.shadow.optional, 14);
assert.equal(root.shadow.items.get(0), 15);
assert.equal(root.choice.valueChild.value, 16);
assert.equal(root.projection.numeric.length, 4);
assert.equal(root.projection.named, 2);
for (let i = 0; i < root.projection.many.length; i++) {
    assert.equal(root.projection.many.getX(i), 3);
    root.projection.many.setX(i, 20);
}
assert.equal(new EdgeCasesView(root.bytes).projection.many.getX(0), 20);
assert.equal(new NestedView(read("absent-0")).child, null);
assert.equal(new NestedView(read("absent-1")).child.value, null);
assert.equal(new NestedView(read("absent-2")).child.value.value, null);
assert.equal(IntrlayU32View.create(123).value, 123);
console.log("TypeScript boundary values and composed layouts passed");
