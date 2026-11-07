import assert from "node:assert/strict";
import { EncodingBuffer } from "../../packages/typescript/dist/index.js";
import { FixedArraysView } from "../../target/regressions/arrays.ts";
import { AlphaView } from "../../target/regressions/alpha.ts";
import { UnitSequenceView } from "../../target/compositions/bindings.ts";

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
console.log("TypeScript: fixed arrays use borrowed typed arrays and retain encoded extents");
