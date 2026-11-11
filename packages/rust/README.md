# Intrlay for Rust

Define schemas in Rust and read or construct their binary representation through
typed views. The `intrlay` crate includes the `Intrlay` derive and schema export
support used by the Intrlay CLI.

## Installation

Requires Rust 1.95 or later.

```sh
cargo add intrlay
```

## Usage

```rust
use intrlay::Intrlay;

#[derive(Intrlay)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut point = PointView::create(&Point { x: 1.0, y: 2.0, z: 3.0 })?;
    point.x_mut().set(4.0);
    assert_eq!(point.x(), 4.0);
    Ok(())
}
```

The derive creates `PointView` and `PointInput` alongside `Point`. Export schema roots with
`intrlay::export_types!(Point);` at a library crate's root to generate bindings
for other languages. Rust can also consume types generated from C++ schemas.

Optional `glam-0-33`, `glam-0-34`, `smol_str`, and `smallvec` features provide library codecs
through the ordinary `#[intrlay(with = ...)]` adapter mechanism. See
[Library adapters](https://soulart.github.io/intrlay/reference/library-adapters/).

## Documentation

- [Getting started](https://soulart.github.io/intrlay/getting-started/)
- [Rust guide](https://soulart.github.io/intrlay/languages/rust/)
- [Schema annotations](https://soulart.github.io/intrlay/reference/annotations/)
- [Project](https://github.com/soulart/intrlay)

Licensed under Apache-2.0 OR MIT.
