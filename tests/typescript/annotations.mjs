import assert from "node:assert/strict";
import fs from "node:fs";
import { ContractsView, IdentifierView, IndexedHintsView, ProjectedAccessView, RenamedVariantsView } from "../../target/regressions/contracts.js";
import * as inputs from "../../target/regressions/input_emission.js";
import * as storage from "../../target/regressions/storage_emission.js";
import * as outbound from "../../target/regressions/view_emission.js";

const outboundInput = {
    records: [{ value: 7, label: "nested" }],
    optional: { value: 8, label: "optional" },
    choice: { kind: 1, payload: { value: 9, label: "variant" } },
};
const outboundExpected = new Uint8Array(fs.readFileSync("target/regressions/view_emission.bin"));
assert.deepEqual(outbound.OutboundPacketView.create(outboundInput).bytes, outboundExpected);
assert.deepEqual(outbound.OutboundPacketView.createInPlace(new Uint8Array(outboundExpected.length), outboundInput).bytes, outboundExpected);
assert.equal("InternalRecordView" in outbound, false);
assert.equal("ZInternalRecordView" in outbound, false);

const creationInput = { count: 7, label: "created", fixed: [2, 5], optional: 11 };
const creationExpected = new Uint8Array(fs.readFileSync("target/regressions/storage_emission_created.bin"));
for (const created of [storage.CreateOnlyView.create(creationInput),
    storage.CreateOnlyView.createInPlace(new Uint8Array(creationExpected.length), creationInput)]) {
    assert.deepEqual(created.bytes, creationExpected);
    assert.equal(Object.hasOwn(created, "_offset"), false);
}
assert.equal("DisabledNamesView" in storage, false);
const unitsBytes = new Uint8Array(fs.readFileSync("target/regressions/storage_emission_units.bin"));
const prefixedUnits = new Uint8Array(16 + unitsBytes.length);
prefixedUnits.set(unitsBytes, 16);
const units = new storage.UnitItemsView(prefixedUnits, 16).values;
assert.equal(units.length, 3);
assert.equal(units.get(2), null);
assert.equal(Object.hasOwn(units, "_offset"), false);
assert.equal(Object.hasOwn(units, "_elementOffset"), false);
const zeroArray = new storage.ZeroArrayView(new Uint8Array(0));
zeroArray.setValues([]);
zeroArray.replace({ values: [] });
assert.equal(zeroArray.values.length, 0);

const incoming = new inputs.ReadOnlyView(new Uint8Array(fs.readFileSync("target/regressions/input_emission_read.bin")));
assert.equal(incoming.label, "incoming");
assert.equal(incoming.event.tag, 17);
assert.equal(incoming.event.dataKind, 42);
assert.equal(incoming.state.tag, 9);
assert.equal(incoming.next, null);
assert.equal(inputs.ReadOnlyView.create, undefined);
assert.equal(inputs.ZReadOnlyView, inputs.ReadOnlyView);
const inputParent = inputs.WriteParentView.create({
    leaf: { value: 1 }, leaves: [{ value: 2 }],
    event: { kind: 1 /* WriteEventKind.Data */, leaf: { value: 3 } },
});
assert.equal(inputParent.leaf.value, 1);
assert.equal(inputParent.leaves.get(0).value, 2);
assert.equal(inputParent.event.dataLeaf.value, 3);
assert.equal(inputs.WriteLeafView.create, undefined);
const fieldOnly = new inputs.FieldOnlyView(new Uint8Array(fs.readFileSync("target/regressions/input_emission_mutation.bin")));
fieldOnly.setLeaf({ value: 8 });
fieldOnly.leaf.replace({ value: 9 });
assert.equal(fieldOnly.leaf.value, 9);
assert.equal(fieldOnly.label, "mutable field");
assert.equal(inputs.FieldOnlyView.create, undefined);

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
const renamed = RenamedVariantsView.create({kind: 0 /* RenamedVariantsKind.Specific */, value: 8});
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
