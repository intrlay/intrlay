import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as intrlay from '../../target/node_modules/@intrlay/runtime/dist/index.js';
import { FixtureView, DeepRootView, CachedRootView, ChoiceView } from '../../target/reference-fixes/bindings.js';

const some = value => ({ value });
const padded = value => new Float32Array([value, 0, 0, 0]);
const input = {
    custom: { value: 1 }, deep: 7, nested: some(8), absent: null, missingInner: some(null),
    values: [9], items: [{ scalar: 10, child: some({ value: 11 }), projected: some(padded(12)) }],
    cached: some({ value: 13 }), projected: some(padded(14)), lanes: new Float32Array([15, 0, 0, 0, 16, 0, 0, 0]),
    choice: { kind: 1 /* ChoiceKind.Selected */, scalar: some(17), child: some({ value: 18 }) },
    inactive: { kind: 0 /* ChoiceKind.Empty */ }, before: 19, empty: new Float32Array(0), after: 20,
    emptyItems: 2, readOnly: some(21),
};
const owned = FixtureView.create(input);
assert.deepEqual(owned.storage.bytes, new Uint8Array(readFileSync(new URL('../../target/reference-fixes/fixture.bin', import.meta.url))));

class CountingStorage extends intrlay.Storage {
    reads = 0;
    get u32() { ++this.reads; return super.u32; }
}
const storage = new CountingStorage(owned.storage.bytes);
const view = new FixtureView(storage);
function reads(expected, action) {
    storage.reads = 0;
    const result = action();
    assert.equal(storage.reads, expected);
    return result;
}
assert.equal(reads(11, () => view.deep), 7);
assert.equal(reads(13, () => view.nested).value, 8);
assert.equal(view.absent, null);
assert.equal(view.missingInner.value, null);
const values = view.values;
// The immutable sequence base is resolved when its view is constructed.
assert.equal(reads(11, () => values.get(0)), 9);
const items = view.items;
assert.equal(reads(11, () => items.getScalar(0)), 10);
assert.equal(items.getChild(0).value.value, 11);
assert.equal(items.getProjected(0).value[0], 12);
const cached = reads(2, () => view.cached);
assert.equal(reads(0, () => view.cached), cached);
cached.value.value = 23;
assert.equal(new FixtureView(owned.storage.bytes).cached.value.value, 23);
assert.equal(reads(2, () => view.projected).value[0], 14);
const lanes = reads(2, () => view.lanes);
assert.deepEqual([...lanes], [15, 0, 0, 0, 16, 0, 0, 0]);
assert.equal(reads(0, () => view.lanes), lanes);
assert.equal(view.choice.selectedScalar.value, 17);
assert.equal(view.choice.selectedChild.value.value, 18);
assert.equal(view.inactive.selectedChild, undefined);
const noPayload = ChoiceView.create({ kind: 1 /* ChoiceKind.Selected */, scalar: null, child: null });
assert.equal(noPayload.selectedChild, null);
const noInnerPayload = ChoiceView.create({ kind: 1 /* ChoiceKind.Selected */, scalar: some(null), child: some(null) });
assert.equal(noInnerPayload.selectedChild.value, null);
assert.equal(noInnerPayload.selectedChild, noInnerPayload.selectedChild);
assert.equal(view.empty.length, 0);
assert.equal(view.emptyItems.length, 0);
assert.equal(view.before, 19);
assert.equal(view.after, 20);
assert.equal(view.readOnly.value, 21);
assert.equal('setReadOnly' in view, false);
view.setDeep(31);
view.setNested(32);
values.set(0, 33);
items.setScalar(0, 34);
assert.equal(view.deep, 31);
assert.equal(view.nested.value, 32);
assert.equal(values.get(0), 33);
assert.equal(items.get(0).scalar, 34);
assert.equal(DeepRootView.create(35).value, 35);
const root = CachedRootView.create(some({ value: 36 }));
assert.equal(root.value, root.value);
assert.equal(root.value.value.value, 36);
assert.equal(CachedRootView.create(null).value, null);
assert.equal(CachedRootView.create(some(null)).value.value, null);
console.log('TypeScript reference loads: values, mutation, cache identity, bytes, and exact read counts passed');
