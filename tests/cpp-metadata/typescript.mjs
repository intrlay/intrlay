import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { WholeChoiceView } from "../../target/cpp-metadata/alternatives.js";
import { WholeChoiceView as NamedWholeChoiceView, OrdinaryChoiceView as NamedOrdinaryChoiceView } from "../../target/cpp-metadata/enum_getters.js";

const namedWhole = NamedWholeChoiceView.create({ kind: 1, payload: { value: 31, marker: 37 } });
assert.equal(namedWhole.read_value.read_value, 31);
const namedOrdinary = NamedOrdinaryChoiceView.create({ kind: 0, value: 23, nativeExtra: 29 });
assert.equal(namedOrdinary.read_value_value, 23);

const view = WholeChoiceView.create({ kind: 1 /* WholeChoiceKind.Value */, payload: { value: 31, marker: 37 } });
assert.deepEqual([...view.storage.bytes], [...readFileSync("target/cpp-metadata/whole.bin")]);
const alias = new WholeChoiceView(view.storage.bytes);
assert.equal(alias.value.value, 31);
assert.equal(alias.value, alias.value);
view.setValue({ value: 41, marker: 43 });
assert.equal(alias.value.marker, 43);
view.replace({ kind: 2 /* WholeChoiceKind.Nested */, payload: [41, null] });
assert.equal(alias.value, undefined);
assert.deepEqual([alias.nested.get(0), alias.nested.get(1)], [41, null]);
assert.deepEqual([...view.storage.bytes], [...readFileSync("target/cpp-metadata/nested.bin")]);
view.replace({ kind: 0 /* WholeChoiceKind.Empty */, payload: null });
assert.equal(alias.nested, undefined);
assert.equal(alias.empty, null);
view.replace({ kind: 1 /* WholeChoiceKind.Value */, payload: { value: 47, marker: 53 } });
assert.equal(alias.empty, undefined);
assert.equal(alias.value.marker, 53);
