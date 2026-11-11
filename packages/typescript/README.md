# Intrlay for TypeScript

TypedArray storage and construction helpers for generated Intrlay bindings.
Use typed accessors over a binary buffer in JavaScript or TypeScript without
decoding the entire value into objects.

## Installation

```sh
npm install @intrlay/runtime
```

This package uses ES modules. Generate your bindings with the Intrlay CLI's
`--language typescript` option, then compile them with your TypeScript project.

## Usage

Given the generated `PointView` from the getting-started example:

```typescript
import { PointView } from "./generated/point.js";

const point = PointView.create({ x: 1, y: 2, z: 3 });
point.x = 4;

const received = new PointView(point.storage.bytes);
console.log(received.x); // 4
```

`point.storage.bytes` is a `Uint8Array`. The generated view reads and writes its backing
storage directly. Caller-provided buffers are supported by `createInPlace`.

## Documentation

- [Getting started](https://soulart.github.io/intrlay/getting-started/)
- [TypeScript guide](https://soulart.github.io/intrlay/languages/typescript/)
- [Buffers and views](https://soulart.github.io/intrlay/concepts/buffers-and-views/)
- [Project](https://github.com/soulart/intrlay)

Licensed under Apache-2.0 OR MIT.
