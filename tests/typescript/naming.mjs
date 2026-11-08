import assert from "node:assert/strict";
import { NamingSampleView } from "../../target/naming/bindings.js";
import { ItemsView as StaticCollectionView } from "../../target/naming/collection-static.js";
import { ItemsView as OmittedCollectionView } from "../../target/naming/collection-omitted.js";

const value = { sampleRate: 120, literalName: 0 };
const view = NamingSampleView.Static_Create(value);
assert.equal(view.sampleRate, 120);
assert.equal(view.literalName, 0);
view.sampleRate = 144;
assert.equal(view.sampleRate, 144);
assert.equal(view.bytes.length, NamingSampleView.Static_ComputeByteLength(value));

const collection = StaticCollectionView.create([0, 0]);
assert.equal(StaticCollectionView.byteSizeLength, 8);
assert.equal(collection.byteSizeLength, 2);
assert.equal(collection.get(1).byteSizeTag, 0);
const strings = OmittedCollectionView.create(["one", "two"]);
assert.equal(strings.setLength, 2);
assert.equal(strings.getLength(1), "two");
console.log("TypeScript: naming rules preserve explicit names and static factories");
