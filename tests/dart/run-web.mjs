import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// dart2js uses the web global even for these DOM-independent runtime tests.
globalThis.self = globalThis;
const path = resolve(process.argv[2]);
if (path.endsWith(".wasm")) {
    const { compile } = await import(pathToFileURL(path.replace(/\.wasm$/, ".mjs")).href);
    const module = await compile(await readFile(path));
    const instance = await module.instantiate();
    instance.invokeMain();
} else {
    await import(pathToFileURL(path).href);
}
