# Intrlay runtime packages

Generated bindings use the runtime package for their target language. The
generator and both source frontends live in [`crates/`](../crates).

| Language | Package | Role |
| --- | --- | --- |
| [Rust](rust/README.md) | `intrlay` | Derives, schema exports, buffer ownership, and typed views |
| [C++](cpp/README.md) | `intrlay::intrlay` | C++20 headers for annotations and generated codecs |
| [TypeScript](typescript/README.md) | `@intrlay/runtime` | TypedArray storage and construction helpers |
| [C#](csharp/README.md) | `Intrlay` | Value-type views, span support, and Roslyn source generator |
| [Python](python/README.md) | `intrlay` | Typed memoryviews and generated property support |
| [Dart](dart/README.md) | `intrlay` | Typed-data storage and construction helpers |

Use the [getting-started guide](../docs/docs/getting-started.mdx) to define a schema and
generate bindings. Package READMEs describe installation and basic use; the
[language guides](../docs/docs/index.mdx#language-guides) cover each target's API.
