<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/static/img/logo-wordmark-dark.svg" />
    <img src="docs/static/img/logo-wordmark.svg" alt="Intrlay" width="360" height="109" />
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

See [Getting started](docs/docs/getting-started.mdx) for the complete example and
installation from a source checkout.

## Languages

Schemas can originate in **Rust or C++**. Either frontend can generate every
target below.

| Target | Runtime package | Guide |
| --- | --- | --- |
| Rust | `intrlay` on crates.io | [Rust](docs/docs/languages/rust.mdx) |
| C++20 | Header-only library | [C++](docs/docs/languages/cpp.mdx) |
| TypeScript / JavaScript | `@intrlay/runtime` on npm | [TypeScript](docs/docs/languages/typescript.mdx) |
| C# / .NET | `Intrlay` on NuGet, including its source generator | [C#](docs/docs/languages/csharp.mdx) |
| Python | `intrlay` on PyPI | [Python](docs/docs/languages/python.mdx) |
| Dart | `intrlay` on pub.dev | [Dart](docs/docs/languages/dart.mdx) |

## Documentation

- [Documentation](https://intrlay.github.io/intrlay/)
- [Feature comparison](docs/docs/feature-comparison.mdx)
- [Tutorial: a drawing message](docs/docs/tutorial.mdx)
- [Values, buffers, and views](docs/docs/concepts/buffers-and-views.mdx)
- [Code generation](docs/docs/reference/code-generation.mdx)
- [Schema annotations](docs/docs/reference/annotations.mdx)
- [Generated names](docs/docs/reference/naming.mdx)

Intrlay targets cooperating producers and consumers that agree on a schema.
Views assume a valid encoding; transport, schema negotiation, and validation of
untrusted input belong at the application's boundary.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions,
and testing.

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.
