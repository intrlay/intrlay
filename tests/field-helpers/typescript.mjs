import assert from "node:assert/strict";
import fs from "node:fs";
import * as api from "../../target/field-helpers/bindings.js";

const bytes = name => new Uint8Array(fs.readFileSync(`target/field-helpers/${name}.bin`));
const rangeFields = [
    ["matrix", 0, 64], ["padded", 64, 16], ["label", 80, 8],
    ["maybe", 96, 12], ["boxed", 108, 4], ["empty", 112, 0], ["child", 112, 12],
    ["flatFirst", 124, 4], ["flatRest0", 128, 4], ["flatRest1", 132, 4],
];
const data = view => new DataView(view.storage.bytes.buffer, view.storage.bytes.byteOffset, view.storage.bytes.byteLength);
const record = new api.FieldRecordView(bytes("record"));
assert.equal(api.FieldRecordView.byteSize, 144);
for (const [name, offset, length] of rangeFields) {
    assert.equal(record[`${name}ByteOffset`], offset, name);
    assert.equal(record[`${name}ByteLength`], length, name);
}
for (const name of ["matrix", "padded", "label", "values", "maybe", "boxed", "empty", "child", "flat"]) {
    assert.equal(name in record, false, `suppressed value accessor ${name}`);
}
assert.equal("flatByteOffset" in record, false);
assert.equal("setMatrix" in record, false);
assert.equal(Object.keys(record).some(name => name.startsWith("_cached")), false);
assert.equal(data(record).getFloat32(record.matrixByteOffset + 60, true), 16);
assert.equal(record.valuesByteLength, 16);
assert.ok(record.valuesByteOffset >= api.FieldRecordView.byteSize);
assert.equal(data(record).getFloat32(record.valuesByteOffset, true), 21);
assert.equal(data(record).getFloat32(record.valuesByteOffset + 12, true), 24);
assert.equal(data(record).getUint32(record.labelByteOffset + 4, true), 11);
assert.equal(record.storage.bytes[record.maybeByteOffset], 1);
assert.deepEqual(Array.from(record.storage.bytes.subarray(record.paddedByteOffset + 12, record.paddedByteOffset + 16)), [0, 0, 0, 0]);
assert.equal(data(record).getUint32(record.childByteOffset, true), 29);
assert.equal(data(record).getFloat32(record.flatRest1ByteOffset, true), 32);
assert.equal(record.getPaddedComponent, undefined);
assert.deepEqual([record.getMatrixComponent(15), record.getValuesComponent(2), record.getMaybeComponent(1), record.getBoxedComponent(0)], [16, 23, 26, 27]);
record.setMatrixComponent(15, 99);
data(record).setFloat32(record.paddedByteOffset + 4, 61, true);
record.setValuesComponent(2, 51);
record.setMaybeComponent(1, 41);
record.setBoxedComponent(0, 71);
assert.deepEqual(record.storage.bytes, bytes("changed"));

// The helper offset is relative to Storage.bytes, whose own buffer offset is separate.
const original = bytes("record");
const backing = new Uint8Array(original.length + 64);
backing.set(original, 32);
const shifted = new api.FieldRecordView(backing.subarray(16, 32 + original.length), 16);
assert.equal(shifted.storage.bytes.byteOffset, 16);
for (const [name, offset, length] of rangeFields) {
    assert.equal(shifted[`${name}ByteOffset`], 16 + offset);
    assert.equal(shifted[`${name}ByteLength`], length);
}
assert.equal(data(shifted).getFloat32(shifted.matrixByteOffset + 60, true), 16);
assert.equal(shifted.valuesByteOffset, record.valuesByteOffset + 16);
assert.equal(shifted.valuesByteLength, 16);
assert.equal(data(shifted).getFloat32(shifted.valuesByteOffset, true), 21);
shifted.setBoxedComponent(0, 72);
assert.equal(shifted.getBoxedComponent(0), 72);
assert.deepEqual(Array.from(backing.subarray(0, 32)), new Array(32).fill(0));
const absent = new api.FieldRecordView(bytes("absent"));
const absentBefore = absent.storage.bytes.slice();
assert.equal(absent.maybeByteOffset, 96);
assert.equal(absent.maybeByteLength, 12);
assert.equal(absent.storage.bytes[absent.maybeByteOffset], 0);
assert.ok(absent.getMaybeComponent(0) == null);
absent.setMaybeComponent(0, 99);
assert.deepEqual(absent.storage.bytes, absentBefore);

// Suppression on an indexed collection preserves navigation to its field helpers.
const rows = new api.FieldRowsView(bytes("rows"));
assert.equal(rows.rows.length, 2);
assert.equal(rows.rowsByteOffset, 16);
assert.equal(rows.rowsByteLength, 288);
assert.equal(rows.rows.get, undefined);
assert.equal(rows.rows.getMatrix, undefined);
assert.equal(rows.rows.setMatrix, undefined);
assert.equal(rows.rows.getFlatByteOffset, undefined);
for (const [name, offset, length] of rangeFields) {
    const suffix = name[0].toUpperCase() + name.slice(1);
    assert.equal(rows.rows[`get${suffix}ByteOffset`](0), 16 + offset);
    assert.equal(rows.rows[`get${suffix}ByteOffset`](1), 160 + offset);
    assert.equal(rows.rows[`get${suffix}ByteLength`](1), length);
}
for (let index = 0; index < 2; ++index) {
    assert.equal(rows.rows.getValuesByteLength(index), 16);
    assert.equal(data(rows).getFloat32(rows.rows.getValuesByteOffset(index), true), 21);
}
rows.rows.setMatrixComponent(1, 15, 99);
assert.equal(rows.rows.getPaddedComponent, undefined);
data(rows).setFloat32(rows.rows.getPaddedByteOffset(1) + 4, 61, true);
rows.rows.setValuesComponent(1, 2, 51);
rows.rows.setMaybeComponent(1, 1, 41);
rows.rows.setBoxedComponent(1, 0, 71);
assert.equal(rows.rows.getMatrixComponent(0, 15), 16);
assert.equal(rows.rows.getMatrixComponent(1, 15), 99);
assert.equal(Object.keys(rows.rows).some(name => name.startsWith("_cached")), false);
const visible = rows.visible.get(0);
assert.equal(visible.lanes, visible.lanes);
assert.equal(visible.getLanesComponent, undefined);
assert.equal(rows.visible.getLanesComponent, undefined);
assert.notEqual(rows.visible.getLanes(0), rows.visible.getLanes(0));
visible.lanes[0] = 70;
assert.deepEqual(rows.storage.bytes, bytes("rowsChanged"));

// Raw payload ranges exist even when its variant is inactive or an option is absent.
const inactive = new api.RawChoiceView(bytes("inactive"));
for (const [name, offset, length] of [["Value", 4, 8], ["Label", 12, 8], ["Maybe", 20, 12], ["Boxed", 32, 4]]) {
    assert.equal(inactive[`data${name}ByteOffset`], offset);
    assert.equal(inactive[`data${name}ByteLength`], length);
}
assert.ok(inactive.getDataValueComponent(0) == null);
assert.ok(inactive.getDataBoxedComponent(0) == null);
const inactiveBefore = inactive.storage.bytes.slice();
inactive.setDataBoxedComponent(0, 99);
assert.deepEqual(inactive.storage.bytes, inactiveBefore);
const active = new api.RawChoiceView(bytes("active"));
assert.equal(active.dataMaybeByteLength, 12);
assert.ok(active.getDataMaybeComponent(0) == null);
const choices = new api.RawChoicesView(bytes("choices"));
assert.equal(choices.items.getDataValueByteOffset(0), 12);
assert.equal(choices.items.getDataBoxedByteOffset(0), 40);
assert.equal(choices.items.getDataValueByteOffset(1), 48);
assert.equal(choices.items.getDataValueByteLength(0), 8);
assert.ok(choices.items.getDataValueComponent(0, 0) == null);
const choicesBefore = choices.storage.bytes.slice();
choices.items.setDataValueComponent(0, 0, 99);
assert.deepEqual(choices.storage.bytes, choicesBefore);
choices.items.setDataValueComponent(1, 1, 90);
choices.items.setDataBoxedComponent(1, 0, 80);
assert.deepEqual(choices.storage.bytes, bytes("choicesChanged"));

const read = api.ReadHelperView.create({ lanes: [1, 2] });
assert.equal(read.getLanesComponent(1), 2);
assert.equal(read.lanesByteLength, 8);
assert.equal(read.setLanesComponent, undefined);
const write = api.WriteHelperView.create({ lanes: [1, 2] });
write.setLanesComponent(1, 9);
assert.equal(data(write).getFloat32(4, true), 9);
assert.equal(write.getLanesComponent, undefined);
assert.equal(write.lanesByteOffset, undefined);
const create = api.CreateHelperView.create({ lanes: [1, 2] });
assert.equal(data(create).getFloat32(4, true), 2);
assert.equal(create.getLanesComponent, undefined);
assert.equal(create.setLanesComponent, undefined);
assert.equal(create.lanesByteOffset, undefined);
const filtered = api.LanguageHelperView.create({ filtered: [3, 4] });
assert.equal(filtered.filtered[1], 4);
assert.equal(filtered.getFilteredComponent(1), 4);
assert.equal(filtered.filteredByteOffset, undefined);

const arrays = new api.ArrayRangesView(bytes("arrays"));
const arrayData = data(arrays);
assert.equal(arrays.recordsByteLength, 24);
assert.equal(arrayData.getUint32(arrays.recordsByteOffset, true), 101);
assert.equal(arrayData.getUint32(arrays.recordsByteOffset + 12, true), 202);
assert.equal(arrays.fixedByteLength, 24);
assert.equal(arrayData.getUint32(arrays.fixedByteOffset, true), 303);
assert.equal(arrayData.getUint32(arrays.fixedByteOffset + 12, true), 404);
assert.equal(arrays.paddedByteLength, 32);
assert.equal(arrayData.getFloat32(arrays.paddedByteOffset + 16, true), 4);
assert.equal(arrayData.getUint32(arrays.paddedByteOffset + 12, true), 0);
assert.equal(arrays.nestedByteLength, 16); // Element headers, not their indirect data.
const inner = arrays.nestedByteOffset + arrayData.getUint32(arrays.nestedByteOffset, true);
assert.equal(arrayData.getUint32(inner, true), 11);
assert.equal(arrays.emptyByteLength, 0);
assert.ok(arrays.emptyByteOffset <= arrays.storage.bytes.length);
assert.equal(arrays.unitsByteLength, 0);
assert.ok(arrays.unitsByteOffset <= arrays.storage.bytes.length);
assert.equal(arrays.foldedByteLength, 8);
assert.equal(arrayData.getUint32(arrays.foldedByteOffset, true), 51);
const arrayChoices = new api.ArrayChoicesView(bytes("arrayChoices"));
data(arrayChoices).setUint32(12, 0xffffffff, true); // Inactive payload bytes need not be references.
assert.equal(arrayChoices.items.getDataRecordsByteOffset(0), undefined);
assert.equal(arrayChoices.items.getDataRecordsByteLength(0), undefined);
assert.equal(arrayChoices.items.getDataUnitsByteLength(0), undefined);
assert.equal(arrayChoices.items.getDataRecordsByteLength(1), 12);
assert.equal(arrayChoices.items.getDataUnitsByteLength(1), 0);
assert.equal(data(arrayChoices).getUint32(arrayChoices.items.getDataRecordsByteOffset(1), true), 505);
const foldedHelpers = api.FoldedHelpersView.create({ samples: [{ lanes: [3, 7] }] });
assert.deepEqual(foldedHelpers.storage.bytes, bytes("foldedHelpers"));
assert.equal(foldedHelpers.samples.getLanesComponent(0, 1), 7);
assert.equal(foldedHelpers.samples, foldedHelpers.samples);
assert.equal(foldedHelpers.samples.get, undefined);
assert.equal(foldedHelpers.samples.getLanes, undefined);
foldedHelpers.samples.setLanesComponent(0, 1, 11);
assert.deepEqual(foldedHelpers.storage.bytes, bytes("foldedHelpersChanged"));
console.log("TypeScript: field ranges, component-only access, policies, and byte parity passed");
