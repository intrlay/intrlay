import assert from "node:assert/strict";
import fs from "node:fs";
import { ContractsView, IdentifierView, IndexedHintsView, ProjectedAccessView, RenamedVariantsView } from "../../target/regressions/contracts.ts";

const input = {
    editable: { value: 1 }, readOnly: { value: 2 }, writeOnly: { value: 3 },
    readOnlyArray: [4,5,6], readOnlyNested: [{ value: 7 }], fixed: { value: 8 },
    identifier: 9, identifiers: [10,11], maybe: { value: null }, label: "hello",
};
const view = ContractsView.create(input);
assert.deepEqual(view.bytes, new Uint8Array(fs.readFileSync("target/regressions/contracts.bin")));
assert.equal(IdentifierView.byteSize, 16);
assert.equal(view.identifier, 9);
view.identifier = 19;
assert.equal(view.identifier, 19);
assert.equal(view.identifiers.get(1), 11);
view.identifiers.set(1, 21);
assert.equal(view.identifiers.get(1), 21);
assert.deepEqual(view.maybe, { value: null });
assert.equal(view.label, "hello");
view.editable.value = 30;
view.writeOnly.value = 31;
assert.equal(view.writeOnly.value, 31);
assert.equal(view.editable.value, 30);
view.readOnly.value = 40;
assert.equal(view.readOnly.value, 40);
view.readOnlyNested.get(0).value = 40;
assert.equal(view.readOnlyNested.get(0).value, 40);
assert.throws(() => { view.fixed.value = 40; }, TypeError);
const renamed = RenamedVariantsView.create({kind: "Specific", value: 8});
assert.equal(renamed.specificValue, 8);
const projected = ProjectedAccessView.create({
    value: { branch: { leaf: { value: 1.5 } } },
    structured: { branch: { leaf: { value: 2.5 } } },
    indexed: [{ value: 3 }],
});
assert.equal(projected.value[0], 1.5);
projected.value[0] = 40;
assert.equal(projected.value[0], 40);
assert.equal(projected.structured.branch.leaf.value, 2.5);
assert.equal(projected.indexed.getValue(0), 3);
projected.indexed.setValue(0, 4);
assert.equal(new ProjectedAccessView(projected.bytes).indexed.getValue(0), 4);
view.readOnlyArray.forEach((_value, index, array) => { array[index] = 41; });
assert.deepEqual(Array.from(view.readOnlyArray), [41, 41, 41]);
console.log("TypeScript: transparent wrappers, byte parity, nested access, variant names passed");

const indexedHints = IndexedHintsView.create({
    readable: [{ readScore: 7, readVector: [1, 2, 3] }],
    writable: [{ writeScore: 8, writeVector: [4, 5, 6] }],
    hidden: [{ hiddenScore: 9, hiddenVector: [7, 8, 9] }],
});
assert.equal(indexedHints.readable.getReadScore(0), 7);
assert.equal(indexedHints.readable.getReadVectorComponent(0, 1), 2);
assert.equal(indexedHints.readable.setReadScore, undefined);
assert.equal(indexedHints.readable.setReadVectorComponent, undefined);
indexedHints.writable.setWriteScore(0, 10);
indexedHints.writable.setWriteVectorComponent(0, 1, 20);
assert.equal(indexedHints.writable.getWriteScore, undefined);
assert.equal(indexedHints.writable.getWriteVectorComponent, undefined);
assert.equal(indexedHints.writable.get(0).writeVector[1], 20);
assert.equal(indexedHints.hidden.getHiddenScore, undefined);
assert.equal(indexedHints.hidden.setHiddenVectorComponent, undefined);
