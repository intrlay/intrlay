# Intrlay for Python

Typed buffer views for Python 3.12 and later. Generated classes use memoryviews
and properties to access binary data directly, with type hints for both input
values and borrowed views.

## Installation

```sh
python -m pip install intrlay
```

Generate bindings with the Intrlay CLI's `--language python` option.

## Usage

Given the generated `PointView` from the getting-started example:

```python
from generated.point import PointView

point = PointView.create({"x": 1.0, "y": 2.0, "z": 3.0})
point.x = 4.0

received = PointView(point.storage.bytes)
assert received.x == 4.0
```

`point.storage.bytes` is a memoryview. Borrowed views keep the backing buffer accessible;
mutating through them requires writable storage.

## Documentation

- [Getting started](https://intrlay.github.io/intrlay/getting-started/)
- [Python guide](https://intrlay.github.io/intrlay/languages/python/)
- [Buffers and views](https://intrlay.github.io/intrlay/concepts/buffers-and-views/)
- [Project](https://github.com/intrlay/intrlay)

Licensed under Apache-2.0 OR MIT.
