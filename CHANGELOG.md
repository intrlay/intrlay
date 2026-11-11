# Changelog

## [0.1.4]

### Buffer validation

- Rust and C++ provide checked `from_bytes`, `from_parts`, and codec-level view
  entry points, with explicit `_unchecked` counterparts for trusted encodings.
  Checked constructors validate bounds, tags, UTF-8, and referenced payloads
  before returning a view.
- Rust `decode` returns `Result<NativeValue, DecodeError>`; `decode_unchecked`
  reconstructs trusted values. Safe view construction requires `StableBytes`,
  and `into_buffer` is available only for owned storage implementing `OwnedBytes`.
- C++ checked entry points return `Expected<View, ViewError>`. Owned buffers
  provide both checked `view()` and trusted `view_unchecked()` access.

### Adapters

- C++ `Adapter<T>` specializations and `INTRLAY_ADAPT_STRUCT` map external types
  to schemas and encoding inputs, including nested containers.
- Optional Rust features provide glam 0.33/0.34, smol_str, and smallvec adapters.
  C++ headers provide GLM vector, matrix, quaternion, and dual-quaternion adapters.
- Rust remote adapters support native-layout copying when their layout is proven
  compatible; custom adapters can supply an explicit `unsafe(native_layout)` proof.

### Generated APIs

- TypeScript, Python, and Dart numeric projections apply to both inputs and
  views, including nested fixed arrays and payload-free enum tags.
- In those targets, `project = "flatten"` exposes inline leaf fields directly.
  `indexed` and `indexed_only` support payload-enum collections and share
  compatible collection views. Projected APIs retain source documentation.
- The `generate_component_accessors`, `generate_byte_range_accessors`, and
  `generate_value_accessors` options select field-access APIs in those targets.
  Array byte ranges identify the first element and its contiguous region.
- TypeScript, Python, and Dart views expose `storage` for access to their backing
  bytes and numeric storage.
- Payload-free enums retain their schema names in C++, C#, TypeScript, Python,
  and Dart. These targets generate standalone enum views only for explicitly
  exported enums.
- C++ collection views support range iteration and native-compatible `copy_to`.
  Rust `SequenceView::copy_to` decodes into an existing native slice.
- Correct accessor naming under custom policies, preserve navigation through
  transparent indexed collections, and reject generated-name collisions.

### Compatibility

Regenerate bindings and use matching runtime packages. Handle the result returned
by Rust and C++ checked constructors, or select the explicit unchecked API when
its contract is satisfied. In TypeScript, Python, and Dart, access backing bytes
through `view.storage.bytes`. Numeric projections use packed inputs, and numeric
component helpers require `generate_component_accessors = true`. Export C++
out-of-line definitions and explicit specializations through concrete aliases.
For Clang, use version 22 or later. Existing schema wire layouts are unchanged.

## [0.1.3]

### Encoding inputs

- Rust provides derived `TypeInput` types, `EncodeSource`, and `create_from`,
  `compute_layout_from`, and `create_in_place_from` APIs. Nested inputs can borrow
  strings and slices or use repeatable `SequenceInput` iterators without first
  allocating the schema's owned containers. `BoxInput` supplies boxed payloads
  without an intermediate allocation.
- C++ input templates accept owned or borrowed fields, including
  `std::string_view` and `std::span`. C++20 concepts check source compatibility
  recursively for construction and mutation, including exact scalar types.
- C# provides scoped `TypeSpanInput` overloads for records with directly
  borrowable strings and scalar or native-layout collections, including
  `stackalloc` inputs. The package targets .NET 10 and C# 14.
- C# generic field sources specialize value copying and zero initialization
  without a per-value mode flag. Existing `Create` overloads remain available;
  `FromSources` factories compose mixed, nested, and variant-specific inputs.
- Borrowed encoding retains native-layout bulk copies where valid and checks
  inconsistent input lengths and allocation requirements during construction.
- TypeScript, Dart, and Python numeric-array writers bulk-copy compatible typed
  storage while retaining element writers for other inputs and conversions.
  Small fixed arrays omit bulk-copy dispatch using measured target-specific
  thresholds, including separate Dart native and web paths.
- Array inputs can request zero initialization without allocating intermediate
  elements. Variable sequences take an explicit count; fixed arrays take their
  extent from the schema. The request works recursively in nested inputs and
  clears reused storage as well as new buffers.

### Generated APIs

- Generated construction types use the `Input` suffix across target languages.
  Rust retains its owned schema types alongside derived input and view types.
- Payload-free enums use native enum inputs, including TypeScript `const enum`,
  Python `IntEnum`, and Dart enums with explicit wire tags.
- TypeScript and Python payload enum inputs use numeric `TypeKind` discriminants
  instead of string literals. Declared tags are preserved independently of
  variant order.
- TypeScript view tags return the corresponding enum type without runtime
  conversion. TypeScript, Python, and Dart omit unused record and payload-enum
  input declarations while retaining inputs needed by parent construction or
  mutation and enum constants needed by readers.
- Views omit unused offsets and collection caches, including construction-only
  APIs. Encoding helper reachability is separate from input-type dependencies,
  avoiding unused helpers for empty arrays and inlined scalar writes.
- Nested encoding-only types omit unused View declarations and aliases while
  retaining their Input types and writers. Unused internal parameters use
  underscore prefixes. TypeScript supports `noUnusedParameters`; Python keeps
  public keyword parameter names with narrow lint exceptions where necessary.
- `--csharp-language-version 15` enables custom struct unions with implicit case
  conversions and exhaustive non-boxing patterns on C# 15 and .NET 11. C# 14
  remains the default; both paths retain the same input storage and wire layout.

### Correctness and tooling

- Group target configuration under `cpp` and `csharp` in generation options.
  Use `cpp.namespace`, `csharp.namespace`, and `csharp.language_version`;
  module naming remains shared. Unknown option names are rejected.

- Validate generated names in each language's declaration and member scopes,
  including input aliases, enum companions, inherited members, and runtime
  references. Report collisions rather than silently renaming symbols.
  Input field validation is independent of whether a corresponding View exists.
- Apply collection accessor-generation options consistently, including whole
  element replacement when an element omits its individual field setters.
- Use distinct replacement methods when a numeric projection's borrowed getter
  and construction input have different types.
- Reject uninhabited enums and invalid enum input tags.
- Borrow C# fixed-array constructor arguments to support large inline inputs on
  platforms whose CLR rejects passing them by value.
- Document encoding inputs, generated type families, and ownership contracts;
  use Clang 19 for C++20 input alias deduction in CI.

The wire layout for valid schemas is unchanged. Generated source APIs include
breaking changes: regenerate bindings with 0.1.3, use the matching runtime
packages, and update construction type names and enum discriminants. C++ source
and mutation arguments must match the schema's scalar types; numeric conversions
must be explicit. TypeScript and Python use `zeroed(count)` or `zeroed()` instead
of bare numeric array inputs. Dart uses `.from(list)` and `.zeroed(...)` factories
for eligible collection inputs. C# uses `ZeroedSequence<T>` and `ZeroedArray<T>`
with generic input factories and requires .NET 10.

## [0.1.2]

- Support installing the prebuilt CLI with `cargo binstall intrlay-cli`.
- Verify native archives through cargo-binstall before publication and avoid GNU
  sparse tar entries that binary installers can skip.
- Validate and publish Dart archives outside Git ignore rules, and request the
  publishing token after building the release tooling.
- Update installation instructions and document the Linux binary requirements.
- Fix shared README links on crates.io and refresh API and platform documentation.

## [0.1.1]

- Distribute prebuilt CLI binaries for Windows, Linux, and macOS on x64 and ARM64,
  with C++ headers and SHA-256 checksums in GitHub Releases.
- Publish runtime packages through GitHub Actions using trusted publishing.
- Verify clean release sources and include only current workspace crate archives
  in release artifacts.

## [0.1.0]

- Initial release.
