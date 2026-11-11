import assert from "node:assert/strict";
import fs from "node:fs";
import { zeroed } from "../../target/node_modules/@intrlay/runtime/dist/index.js";
import { IndexedEnumsView, NumericInputsView, NumericEnumFieldsView, ProjectionEdgesView, EmptyNumericTailView, ZeroSizeSequencesView } from "../../target/regressions/contracts.js";

// Empty f64 projections impose no eight-byte alignment on an otherwise valid buffer.
const tailOwner = new Uint8Array(11).fill(0xa5);
const tailInput = { prefix: new Uint8Array(9).fill(7), trailer: new Float64Array(0) };
const tail = EmptyNumericTailView.createInPlace(tailOwner.subarray(1, 10), tailInput);
tail.replace(tailInput);
assert.deepEqual([...tailOwner], [0xa5, ...new Array(9).fill(7), 0xa5]);
assert.throws(() => EmptyNumericTailView.create({ ...tailInput, trailer: new Float64Array(1) }), RangeError);
const sequenceOwner = new Uint8Array(24).fill(0xa5);
for (const numeric of [new Float64Array(0), zeroed(3)]) {
    const sequence = ZeroSizeSequencesView.createInPlace(sequenceOwner.subarray(4, 20), { numeric, ordinary: zeroed(2) });
    assert.equal(sequence.storage.u32[1], typeof numeric === "number" ? 3 : 0);
    assert.equal(sequence.storage.u32[3], 2);
}
assert.deepEqual([...sequenceOwner.subarray(0, 4), ...sequenceOwner.subarray(20)], new Array(8).fill(0xa5));

const edges = ProjectionEdgesView.create({
    flattened: { kind: 1, payload: {} }, items: [{ kind: 1, payload: {} }],
    cached: { kind: 1, payload: new Float32Array(0) },
    floats: new Float32Array(0), integers: new Uint32Array(0),
});
assert.equal(edges.flattened.tag, 1);
assert.equal(edges.items.getTag(0), 1);
assert.equal(edges.floats.length, 0);
assert.equal(edges.integers.length, 0);
assert.equal(edges.cached.emptyPayload.length, 0);
edges.flattened.replace({ kind: 0 });
edges.cached.replace({ kind: 0 });
assert.equal(edges.flattened.tag, 0);
assert.equal(edges.cached.emptyPayload, undefined);
edges.cached.replace({ kind: 1, payload: new Float32Array(0) });
assert.equal(edges.cached.emptyPayload.length, 0);

const choices = IndexedEnumsView.create({
    items: [
        { kind: 3, label: "payload", vector: [1, 2, 3], child: { value: 7 } },
        { kind: 0 }, { kind: 2, value: null }, { kind: 4, value: null },
    ],
    fixed: [{ kind: 1, value: 9 }, { kind: 2, value: 10 }],
});
assert.deepEqual(choices.storage.bytes, new Uint8Array(fs.readFileSync("target/regressions/indexed-enums.bin")));
const items = choices.items;
assert.equal(items.get, undefined);
assert.equal(items.getTag(0), 3);
assert.equal(items.getDataLabel(0), "payload");
assert.equal(items.getDataVectorComponent(0, 1), 2);
assert.equal(items.getDataChild(0).value, 7);
assert.equal(items.getOptionalValue(2), null);
assert.equal(items.getOptionalValue(1), undefined);
assert.equal(items.getUnitValue(3), null);
assert.equal(items.getUnitValue(1), undefined);
const before = choices.storage.bytes.slice();
items.setNumberValue(0, 99);
items.setDataVectorComponent(1, 0, 99);
items.setDataChild(1, { value: 99 });
assert.deepEqual(choices.storage.bytes, before);
items.setDataVectorComponent(0, 1, 20);
items.getDataChild(0).value = 30;
choices.fixed.setNumberValue(0, 40);
assert.equal(items.getDataVector(0)[1], 20);
assert.equal(items.getDataChild(0).value, 30);
assert.equal(choices.fixed.get(0).numberValue, 40);

const input = {
    matrix: new Float32Array([1, 2, 3, 4]),
    points: new Float32Array([5, 6, 7, 8, 9, 10]),
    tags: new Uint32Array([2, 1]),
    empty: new Float32Array(0),
};
const expected = new Uint8Array(fs.readFileSync("target/regressions/numeric-inputs.bin"));
assert.deepEqual(NumericInputsView.create(input).storage.bytes, expected);
assert.deepEqual(NumericInputsView.createInPlace(new Uint8Array(expected.length), input).storage.bytes, expected);
assert.throws(() => NumericInputsView.create({ ...input, points: new Float32Array(4) }), RangeError);
assert.throws(() => NumericInputsView.create({ ...input, matrix: new Float32Array(3) }), RangeError);
const empty = NumericInputsView.create({ matrix: zeroed(), points: zeroed(2), tags: new Uint32Array(), empty: zeroed() });
assert.deepEqual([...empty.matrix], [0, 0, 0, 0]);
assert.equal(empty.points.length, 6);
assert.deepEqual([...empty.points], [0, 0, 0, 0, 0, 0]);
for (const count of [-1, 0.5, NaN, Infinity, 0x100000000]) {
    assert.throws(() => NumericInputsView.create({ ...input, points: zeroed(count) }), RangeError);
    assert.throws(() => NumericInputsView.createInPlace(new Uint8Array(128), { ...input, points: zeroed(count) }), RangeError);
}
const enumFields = NumericEnumFieldsView.create({ plain: 1, numeric: new Uint32Array([2]) });
assert.equal(enumFields.plain, 1);
assert.equal(enumFields.numeric[0], 2);
enumFields.plain = 2;
assert.equal(enumFields.plain, 2);
