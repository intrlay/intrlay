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
assert.deepEqual(outbound.OutboundPacketView.create(outboundInput).storage.bytes, outboundExpected);
assert.deepEqual(outbound.OutboundPacketView.createInPlace(new Uint8Array(outboundExpected.length), outboundInput).storage.bytes, outboundExpected);
assert.equal("InternalRecordView" in outbound, false);
assert.equal("ZInternalRecordView" in outbound, false);

const creationInput = { count: 7, label: "created", fixed: [2, 5], optional: 11 };
const creationExpected = new Uint8Array(fs.readFileSync("target/regressions/storage_emission_created.bin"));
for (const created of [storage.CreateOnlyView.create(creationInput),
    storage.CreateOnlyView.createInPlace(new Uint8Array(creationExpected.length), creationInput)]) {
    assert.deepEqual(created.storage.bytes, creationExpected);
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
assert.equal(incoming.state, 9);
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
assert.deepEqual(view.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/contracts.bin")));
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
    value: new Float32Array([1.5]),
    structured: { branch: { leaf: { value: 2.5 } } },
    indexed: [{ value: 3 }],
    ordinary: [{ value: 5 }],
    direct: [{ value: 7 }],
});
assert.equal(projected.value[0], 1.5);
projected.value[0] = 40;
assert.equal(projected.value[0], 40);
assert.equal(projected.structured.branch.leaf.value, 2.5);
assert.equal(projected.indexed.getValue(0), 3);
projected.indexed.setValue(0, 4);
assert.equal(new ProjectedAccessView(projected.storage.bytes).indexed.getValue(0), 4);
assert.equal(projected.indexed.get(0).value, 4);
assert.equal(projected.ordinary.get(0).value, 5);
assert.equal(projected.ordinary.constructor, projected.indexed.constructor);
assert.equal(projected.direct.constructor, projected.indexed.constructor);
projected.ordinary.get(0).value = 6;
assert.equal(new ProjectedAccessView(projected.storage.bytes).ordinary.get(0).value, 6);
assert.equal(projected.ordinary.getValue(0), 6);
projected.direct.setValue(0, 8);
assert.equal(projected.direct.getValue(0), 8);
assert.equal(projected.direct.get(0).value, 8);
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

// Flattening changes only the access path, retaining padded canonical offsets.
const { FlattenedAccessView, SelectiveFlattenView } = await import("../../target/regressions/contracts.js");
const flat = FlattenedAccessView.create({
    min: [1, 2, 3], max: { x: 4, y: 5, z: 6 },
    details: { id: 7, enabled: true, weight: 8.5, fixed: 9 },
    grid: [[10, 11], [12, 13]], label: "mixed fields", maybe: 14,
    structured: { leaf: { value: 15 } }, lanes: new Float32Array([16, 17, 18]),
});
assert.deepEqual(flat.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/flattened.bin")));
assert.deepEqual([flat.min0, flat.maxX, flat.detailsId, flat.detailsEnabled, flat.detailsWeight, flat.detailsFixed, flat.grid11], [1, 4, 7, true, 8.5, 9, 13]);
assert.equal(flat.label, "mixed fields");
assert.equal(flat.structured.leaf.value, 15);
assert.deepEqual(Array.from(flat.lanes), [16, 17, 18]);
assert.equal(flat.max, undefined);
assert.throws(() => { flat.detailsFixed = 0; }, TypeError);
flat.min1 = 20; flat.maxZ = 60; flat.detailsEnabled = false;
flat.detailsWeight = 80.5; flat.grid10 = 120;
assert.deepEqual(flat.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/flattened-changed.bin")));
flat.maybe = null;
assert.equal(flat.maybe, null);
const selective = SelectiveFlattenView.create({ lower: { x: 1, y: 2, z: 3 }, max: { x: 4, y: 5, z: 6 } });
assert.equal(selective.lowerX, 1);
assert.equal(selective.max.z, 6);

const indexedApi = await import("../../target/regressions/contracts.js");
assert.equal(indexedApi.IndexedEntryView, undefined);
const indexedOnly = indexedApi.IndexedOnlyView.create({ items: [{
    id: 7, label: "entry", plain: [1, 2, 3], lanes: [4, 5, 6],
    projected: new Float32Array([7, 8, 9, 0]), direct: new Float32Array([10, 11, 12, 0]),
    child: { value: 13 }, flat: [14, 15],
}] });
assert.deepEqual(indexedOnly.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/indexed-only.bin")));
const only = indexedOnly.items;
assert.equal(only.length, 1);
assert.equal(only.get, undefined);
assert.equal(only.getPlainComponent, undefined);
assert.equal(only.getProjectedComponent, undefined);
assert.equal(only.getPlain(0)[1], 2);
assert.equal(only.getProjected(0)[2], 9);
assert.equal(only.getLabel(0), "entry");
assert.equal(only.getChild(0).value, 13);
only.setId(0, 17);
only.setLanesComponent(0, 1, 20);
only.setDirectComponent(0, 2, 60);
only.setFlat1(0, 30);
assert.deepEqual([only.getId(0), only.getLanes(0)[1], only.getDirectComponent(0, 2), only.getFlat1(0)], [17, 20, 60, 30]);

// Fixed arrays retain field writers even without a Create factory or element view.
const indexedFixed = new indexedApi.IndexedFixedView(new Uint8Array(fs.readFileSync("target/regressions/indexed-fixed.bin")));
assert.equal(indexedApi.IndexedMutableEntryView, undefined);
assert.equal(typeof indexedApi.IndexedValueView, "function");
assert.equal(indexedApi.IndexedFixedView.create, undefined);
const components = indexedFixed.withComponents;
assert.equal(components.length, 1);
assert.equal(components.get, undefined);
assert.equal(components.getPlainComponent, undefined);
assert.equal(components.getChild(0).value, 13);
assert.equal(indexedFixed.withoutComponents.getLanesComponent, undefined);
assert.equal(components.getLanesComponent(0, 1), 2);
components.setChild(0, { value: 31 });
components.setLanesComponent(0, 1, 20);
indexedFixed.withoutComponents.setChild(0, { value: 32 });
assert.deepEqual(indexedFixed.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/indexed-fixed-changed.bin")));

// Navigation getters preserve mutation without restoring whole-element/scalar reads.
const indexedWriteOnly = indexedApi.IndexedWriteOnlyView.create({
    items: [{ id: 7, values: [1, 2] }], fixed: [{ id: 7, values: [1, 2] }],
});
assert.equal(indexedApi.IndexedWriteEntryView, undefined);
for (const collection of [indexedWriteOnly.items, indexedWriteOnly.fixed]) {
    assert.equal(collection.get, undefined);
    assert.equal(collection.getId, undefined);
}
indexedWriteOnly.items.setId(0, 17);
indexedWriteOnly.items.getValues(0).set(1, 20);
indexedWriteOnly.fixed.getValues(0).set(0, 30);
assert.deepEqual(indexedWriteOnly.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/indexed-write-only-changed.bin")));

await import("./projections.mjs");
