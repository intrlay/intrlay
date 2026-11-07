# Intrlay for Dart

Typed-data storage for generated Intrlay bindings on Dart native and web.
Generated views expose named getters and setters over binary buffers, alongside
typed input values for construction.

## Installation

Requires Dart 3.3 or later.

```sh
dart pub add intrlay
```

For a Flutter project, use `flutter pub add intrlay`. Generate bindings with
the Intrlay CLI's `--language dart` option.

## Usage

Given the generated `Point` and `PointView` from the getting-started example:

```dart
import 'generated/point.dart';

void main() {
  final point = PointView.create(const Point(x: 1, y: 2, z: 3));
  point.x = 4;

  assert(PointView(point.bytes).x == 4);
}
```

`point.bytes` is a `Uint8List`, suitable for passing through an application's
FFI or web transport. `createInPlace` can use storage allocated by the caller.

## Documentation

- [Getting started](https://intrlay.github.io/intrlay/getting-started/)
- [Dart guide](https://intrlay.github.io/intrlay/languages/dart/)
- [Buffers and views](https://intrlay.github.io/intrlay/concepts/buffers-and-views/)
- [Project](https://github.com/intrlay/intrlay)

Licensed under Apache-2.0 OR MIT.
