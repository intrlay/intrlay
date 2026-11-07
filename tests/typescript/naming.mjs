import assert from "node:assert/strict";
import { NamingSampleView } from "../../target/naming/bindings.ts";

const value = { sampleRate: 120, literalName: 0 };
const view = NamingSampleView.Static_Create(value);
assert.equal(view.sampleRate, 120);
assert.equal(view.literalName, 0);
view.sampleRate = 144;
assert.equal(view.sampleRate, 144);
assert.equal(view.bytes.length, NamingSampleView.Static_ComputeByteLength(value));
console.log("TypeScript: naming rules preserve explicit names and static factories");
