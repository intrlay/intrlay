import assert from "node:assert/strict";
import { runInNewContext } from "node:vm";
import { EncodingBuffer, Storage } from "../../packages/typescript/dist/index.js";
import { FixedArraysView } from "../../target/regressions/arrays.js";
import { AlphaView } from "../../target/regressions/alpha.js";
import { UnitSequenceView } from "../../target/compositions/bindings.js";
import * as Numeric from "../../target/regressions/numeric-copy.js";
import { TaggedItemsView } from "../../target/regressions/tagged-defaults.js";

assert.equal(UnitSequenceView.create(0xffffffff).length, 0xffffffff);
assert.equal(UnitSequenceView.create(0).length, 0);
for (const count of [-1, 0.5, NaN, Infinity, 0x100000000]) {
    assert.throws(() => UnitSequenceView.create(count), RangeError);
    assert.throws(() => UnitSequenceView.createInPlace(new Uint8Array(8), count), RangeError);
}

const input = {
    three: [1, 2, 3], four: [4, 5, 6, 7], matrix: [[8, 9, 10], [11, 12, 13]],
    present: [14, 15, 16], absent: null, readOnly: [17, 18, 19],
};
const view = FixedArraysView.create(input);
assert.ok(view.three instanceof Float32Array);
assert.equal(view.three.length, 3);
assert.equal(view.four.length, 4);
assert.deepEqual([...view.matrix], [8, 9, 10, 11, 12, 13]);
assert.deepEqual([...view.present], [14, 15, 16]);
assert.equal(view.absent, null);
view.three[1] = 20;
assert.equal(view.three[1], 20);
assert.throws(() => FixedArraysView.create({ ...input, three: [1, 2] }));
const cached = AlphaView.create({ values: [1, 2] });
assert.equal(cached.values, cached.values);

function changingValues(first, second) {
    let reads = 0;
    return { get values() { return ++reads === 1 ? first : second; } };
}

const backing = new Uint8Array(64).fill(0xa5);
const reusable = new EncodingBuffer(backing.subarray(8, 56));
for (const create of [
    value => AlphaView.create(value),
    value => AlphaView.createInPlace(backing.subarray(8, 56), value),
    value => AlphaView.createInPlace(reusable, value),
]) {
    for (const [first, second] of [[[], [42]], [[42], []]]) {
        assert.throws(() => create(changingValues(first, second)), {
            name: "RangeError", message: "Intrlay layout changed during construction",
        });
    }
    // Different values of the same size remain valid, including after a failed write.
    const equalSize = create(changingValues([1], [42]));
    assert.equal(equalSize.values.length, 1);
    assert.equal(equalSize.values.get(0), 42);
    const stable = create({ values: [7, 11] });
    assert.equal(stable.values.get(1), 11);
}
assert.ok(backing.subarray(0, 8).every(value => value === 0xa5));
assert.ok(backing.subarray(56).every(value => value === 0xa5));

// Typed sources and arbitrary ArrayLike sources retain the same encoding,
// including borrowed input/output slices with nonzero backing-buffer offsets.
for (const [View, ArrayType, values] of [
    [Numeric.I8View, Int8Array, [-128, 127]],
    [Numeric.U8View, Uint8Array, [0, 255]],
    [Numeric.I16View, Int16Array, [-32768, 32767]],
    [Numeric.U16View, Uint16Array, [0, 65535]],
    [Numeric.I32View, Int32Array, [-2147483648, 2147483647]],
    [Numeric.U32View, Uint32Array, [0, 4294967295]],
    [Numeric.I64View, BigInt64Array, [-(1n << 63n), (1n << 63n) - 1n]],
    [Numeric.U64View, BigUint64Array, [0n, (1n << 64n) - 1n]],
    [Numeric.F32View, Float32Array, [1.25, -0]],
    [Numeric.F64View, Float64Array, [Math.PI, -Infinity]],
]) {
    const source = new ArrayType([values[0], ...values, values[0]]).subarray(1, 3);
    const expected = View.create(values).bytes;
    const output = new Uint8Array(expected.length + 16).fill(0xa5);
    const actual = View.createInPlace(output.subarray(8, -8), source);
    assert.deepEqual(actual.bytes, expected);
    assert.deepEqual(View.create({ 0: values[0], 1: values[1], length: 2 }).bytes, expected);
    assert.deepEqual(View.create(new ArrayType(0)).bytes, View.create([]).bytes);
    assert.ok(output.subarray(0, 8).every(value => value === 0xa5));
    assert.ok(output.subarray(-8).every(value => value === 0xa5));
}
const foreign = runInNewContext("Float32Array.of(1.25, -2.5)");
assert.deepEqual(Numeric.F32View.create(foreign).bytes, Numeric.F32View.create([1.25, -2.5]).bytes);
assert.deepEqual(Numeric.F32View.create(Int16Array.of(-123, 456)).bytes,
    Numeric.F32View.create([-123, 456]).bytes);
for (const [View, ArrayType, size] of [
    [Numeric.FixedShortView, Float32Array, 2], [Numeric.FixedMediumView, Float32Array, 16],
    [Numeric.FixedLongView, Float32Array, 128], [Numeric.FixedBytesShortView, Uint8Array, 2],
    [Numeric.FixedBytesLongView, Uint8Array, 128], [Numeric.FixedWideShortView, BigInt64Array, 2],
    [Numeric.FixedWideLongView, BigInt64Array, 16],
]) {
    const values = Array.from({ length: size }, (_, i) => ArrayType === BigInt64Array ? BigInt(i) : i);
    const typed = new ArrayType(values);
    assert.deepEqual(View.create(typed).bytes, View.create(values).bytes);
    assert.throws(() => View.create(typed.subarray(1)), RangeError);
}
const nestedNumeric = [Float32Array.from({ length: 128 }, (_, i) => i), new Float32Array(128).fill(-1)];
assert.deepEqual(Numeric.NestedView.create(nestedNumeric).bytes,
    Numeric.NestedView.create(nestedNumeric.map(values => [...values])).bytes);
const typedFixed = FixedArraysView.create({ ...input,
    three: Float32Array.of(1, 2, 3), four: Float32Array.of(4, 5, 6, 7),
    matrix: [Float32Array.of(8, 9, 10), Float32Array.of(11, 12, 13)],
});
assert.deepEqual(typedFixed.bytes, FixedArraysView.create(input).bytes);
assert.throws(() => FixedArraysView.create({ ...input, three: new Float32Array(4) }), RangeError);
assert.throws(() => TaggedItemsView.create({ items: Uint32Array.of(1, 0) }), RangeError);

// Same-lane copies preserve the IEEE payload as well as numeric values.
const floatBits = Uint32Array.of(0x7fa12345, 0x80000000);
const exact = Numeric.F32View.create(new Float32Array(floatBits.buffer));
assert.deepEqual(exact.bytes.subarray(8), new Uint8Array(floatBits.buffer));

// Sparse numeric lanes exercise the u32 address boundary without a multi-GiB allocation.
// The generated view still follows its sequence header and reads/writes each element.
for (const offset of [0x80000000, 0xffffffe0]) {
    for (const [View, lane, size, values] of [
        [Numeric.U16View, "u16", 2, [123, 65535]],
        [Numeric.U32View, "u32", 4, [123, 0xffffffff]],
        [Numeric.U64View, "u64", 8, [123n, 0xffffffffffffffffn]],
    ]) {
        const storage = new Storage(new Uint8Array(0));
        const headers = Object.create(null);
        const elements = lane === "u32" ? headers : Object.create(null);
        headers[offset / 4] = 8;
        headers[offset / 4 + 1] = values.length;
        elements[(offset + 8) / size] = values[0];
        elements[(offset + 8) / size + 1] = values[1];
        Object.defineProperty(storage, "u32", { value: headers });
        if (lane !== "u32") Object.defineProperty(storage, lane, { value: elements });
        const high = new View(storage, offset);
        assert.equal(high.length, values.length);
        assert.equal(high.get(0), values[0]);
        assert.equal(high.get(1), values[1]);
        high.set(1, values[0]);
        assert.equal(elements[(offset + 8) / size + 1], values[0]);
        assert.equal(high.get(1), values[0]);
        assert.ok(Object.keys(elements).every(index => Number(index) >= 0));
    }
}
console.log("TypeScript: fixed arrays use borrowed typed arrays and retain encoded extents");
