<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/intrlay/intrlay/main/docs/static/img/logo-wordmark-dark.svg" />
    <img src="https://raw.githubusercontent.com/intrlay/intrlay/main/docs/static/img/logo-wordmark.svg" alt="Intrlay" width="360" height="109" />
  </picture>
</p>

Intrlay generates typed views over binary buffers from Rust and C++ type
definitions. Exchange structured data across FFI, WebAssembly, or IPC boundaries
without rebuilding an object tree just to read it.

Define your types once, then generate bindings for **Rust, C++, TypeScript, C#,
Python, and Dart**.

- **Language-native schemas.** Use Rust derives or C++ annotations with records,
  enums, optional values, arrays, and owned trees.
- **Direct buffer access.** Read fields through views and update fixed-size
  values in place. Construct a buffer with one allocation or use caller-owned
  storage.
- **Idiomatic bindings.** Rust borrows, C++ value-type views, C# structs and
  spans, TypeScript TypedArrays, Python memoryviews, and Dart typed data.
- **Readable generated code.** Keep named fields and documentation, with
  optional numeric projections and indexed access for allocation-sensitive code.

## Quick start

Install the generator and create a schema library:

```sh
cargo install intrlay-cli
cargo new --lib protocol
cargo add --manifest-path protocol/Cargo.toml intrlay
```

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), use
`cargo binstall intrlay-cli` to install a prebuilt generator for Windows, Linux,
or macOS on x64 or ARM64.

Define a type in `protocol/src/lib.rs`:

```rust
use intrlay::Intrlay;

#[derive(Intrlay)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

intrlay::export_types!(Point);
```

Generate TypeScript bindings and install their runtime:

```sh
intrlay generate --manifest-path protocol/Cargo.toml --language typescript --output generated/point.ts
npm install @intrlay/runtime
```

Use the generated view:

```typescript
import { PointView } from "./generated/point.js";

const point = PointView.create({ x: 1, y: 2, z: 3 });
point.x = 4;

const received = new PointView(point.bytes);
console.log(received.x); // 4
```

`received` reads the same buffer. Encoding a native value creates its binary
representation; borrowing an existing buffer does not copy the payload.

See [Getting started](https://intrlay.github.io/intrlay/getting-started/) for the complete example and
installation from a source checkout.

## Languages

Schemas can originate in **Rust or C++**. Either frontend can generate every
target below.

| Target | Runtime package | Guide |
| --- | --- | --- |
| Rust | `intrlay` on crates.io | [Rust](https://intrlay.github.io/intrlay/languages/rust/) |
| C++20 | Header-only library | [C++](https://intrlay.github.io/intrlay/languages/cpp/) |
| TypeScript / JavaScript | `@intrlay/runtime` on npm | [TypeScript](https://intrlay.github.io/intrlay/languages/typescript/) |
| C# / .NET | `Intrlay` on NuGet, including its source generator | [C#](https://intrlay.github.io/intrlay/languages/csharp/) |
| Python | `intrlay` on PyPI | [Python](https://intrlay.github.io/intrlay/languages/python/) |
| Dart | `intrlay` on pub.dev | [Dart](https://intrlay.github.io/intrlay/languages/dart/) |

## Documentation

- [Documentation](https://intrlay.github.io/intrlay/)
- [Feature comparison](https://intrlay.github.io/intrlay/feature-comparison/)
- [Tutorial: a drawing message](https://intrlay.github.io/intrlay/tutorial/)
- [Values, buffers, and views](https://intrlay.github.io/intrlay/concepts/buffers-and-views/)
- [Code generation](https://intrlay.github.io/intrlay/reference/code-generation/)
- [Schema annotations](https://intrlay.github.io/intrlay/reference/annotations/)
- [Generated names](https://intrlay.github.io/intrlay/reference/naming/)

Intrlay targets cooperating producers and consumers that agree on a schema.
Views assume a valid encoding; transport, schema negotiation, and validation of
untrusted input belong at the application's boundary.

## Contributing

See [CONTRIBUTING.md](https://github.com/intrlay/intrlay/blob/main/CONTRIBUTING.md) for development setup, coding conventions,
and testing.

## License

Licensed under either [Apache License 2.0](https://github.com/intrlay/intrlay/blob/main/LICENSE-APACHE) or
[MIT](https://github.com/intrlay/intrlay/blob/main/LICENSE-MIT), at your option.
