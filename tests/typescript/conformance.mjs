import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const directory = resolve(process.argv[2]);
const { MessageView, NullableCasesView } = await import(pathToFileURL(resolve(directory, "bindings.js")));
const rustBytes = new Uint8Array(readFileSync(resolve(directory, "rust.bin")));
const nullableBytes = new Uint8Array(readFileSync(resolve(directory, "nullable.bin")));
const nullable = new NullableCasesView(nullableBytes);
assert.equal(nullable.absent, null);
assert.equal(nullable.zero, 0);
assert.equal(nullable.text, "");
assert.equal(nullable.missingText, null);
assert.equal(nullable.absentUnit, null);
assert.deepEqual(nullable.presentUnit, { value: null });
assert.equal(nullable.nestedAbsent, null);
assert.deepEqual(nullable.nestedNone, { value: null });
assert.deepEqual(nullable.nestedValue, { value: 7 });
assert.deepEqual(nullable.boxedNone, { value: null });
assert.deepEqual(nullable.triple, { value: { value: null } });
assert.equal(nullable.variants.get(0).valueValue, undefined);
assert.equal(nullable.variants.get(1).valueValue, null);
assert.equal(nullable.variants.get(2).valueValue, 9);
assert.deepEqual(NullableCasesView.create({
    absent: null, zero: 0, text: "", missingText: null,
    absentUnit: null, presentUnit: { value: null },
    nestedAbsent: null, nestedNone: { value: null }, nestedValue: { value: 7 },
    boxedNone: { value: null }, triple: { value: { value: null } },
    variants: [{ kind: 0 /* NullableVariantKind.Empty */ }, { kind: 1 /* NullableVariantKind.Value */, value: null }, { kind: 1 /* NullableVariantKind.Value */, value: 9 }],
}).storage.bytes, nullableBytes);
const view = new MessageView(rustBytes);
assert.equal(view.sequence, 0x123456789abcdef0n);
assert.equal(view.title, "fixture 🦀 한글");
assert.deepEqual([...view.points], [1, 2, 3, 0, 4, 5, 6, 0]);
assert.equal(view.commands.get(1).drawPointSize, 0.75);
assert.equal(view.commands.get(1).drawPointLabel, "example");
assert.equal(view.tree.next.id, 8);
assert.equal(view.tree.next.next, null);
assert.equal(view.optional, 0);
assert.equal(view.fixed[2], 30);

const value = {
    sequence: 0x123456789abcdef0n,
    title: "fixture 🦀 한글",
    points: new Float32Array([1, 2, 3, 0, 4, 5, 6, 0]),
    commands: [{ kind: 0 /* DrawingCommandKind.Clear */ }, { kind: 1 /* DrawingCommandKind.DrawPoint */, position: { x: 7, y: 8, z: 9 }, size: 0.75, label: "example" }],
    tree: { id: 7, next: { id: 8, next: null } },
    optional: 0,
    fixed: [10, 20, 30],
};
const created = MessageView.create(value);
assert.deepEqual(created.storage.bytes, rustBytes);
const parent = new Uint8Array(created.storage.bytes.byteLength + 32);
parent.fill(0xcc);
const borrowed = MessageView.createInPlace(parent.subarray(16), value);
assert.deepEqual(borrowed.storage.bytes, rustBytes);
assert.equal(parent[15], 0xcc);
assert.equal(parent.at(-1), 0xcc);
borrowed.sequence = 42n;
borrowed.points[6] = 99;
assert.equal(new MessageView(parent.subarray(16)).sequence, 42n);
assert.equal(new MessageView(parent.subarray(16)).points[6], 99);
writeFileSync(resolve(directory, "typescript.bin"), created.storage.bytes);
console.log("TypeScript: bytes, variants, recursive boxes, raw padded arrays, external subview mutation passed");
