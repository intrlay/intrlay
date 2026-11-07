import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// dart2js uses the web global even for these DOM-independent runtime tests.
globalThis.self = globalThis;
await import(pathToFileURL(resolve(process.argv[2])).href);
