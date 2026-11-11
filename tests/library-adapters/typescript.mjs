import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SampleView } from "../../target/library-adapters/bindings.js";

const bytes = new Uint8Array(readFileSync("target/library-adapters/sample.bin"));
const view = new SampleView(bytes);
assert.equal(view.boundsMinY, -2);
assert.equal(view.boundsMaxZ, 6);
assert.deepEqual([...view.points], [7, 8, 9, 0, 10, 11, 12, 0]);
assert.equal(view.matrix.yAxis.z, 6);
assert.deepEqual([view.mask.x, view.mask.y, view.mask.z], [true, false, true]);
assert.equal(view.labels.get(1), "two 🦀");
const rebuilt = SampleView.create({
    bounds: { min: { x: -1, y: -2, z: -3 }, max: { x: 4, y: 5, z: 6 } },
    points: new Float32Array([7, 8, 9, 0, 10, 11, 12, 0]),
    matrix: { xAxis: { x: 1, y: 2, z: 3 }, yAxis: { x: 4, y: 5, z: 6 }, zAxis: { x: 7, y: 8, z: 9 } },
    mask: { x: true, y: false, z: true },
    labels: ["one", "two 🦀"],
});
assert.deepEqual(rebuilt.storage.bytes, bytes);
view.boundsMinY = -20;
view.points[4] = 100;
assert.equal(view.boundsMinY, -20);
assert.equal(view.points[4], 100);
