# Intrlay for C++

Header-only C++20 support for Intrlay schemas and generated buffer views.
Generated codecs use compile-time metadata and templates; no linked runtime
library is required.

## Installation

Download the C++ header package from [Releases](https://github.com/intrlay/intrlay/releases), or use
`packages/cpp` from a source checkout. With CMake:

```cmake
add_subdirectory(path/to/intrlay/packages/cpp intrlay-runtime)
target_link_libraries(my_app PRIVATE intrlay::intrlay)
```

You can also add the package's `include` directory to your compiler's include
paths. CMake integration requires CMake 3.20 or later.

## Usage

After generating `point.hpp` with `--language cpp --module-name example`:

```cpp
#include "generated/point.hpp"
#include <cassert>

int main() {
    using namespace intrlay_generated::example;

    auto point = PointView::create(Point{1.0f, 2.0f, 3.0f});
    auto view = point.view();
    view.set_x(4.0f);
    assert(view.x() == 4.0f);
}
```

The owned value keeps the buffer alive; `view` borrows it. You can also construct
views over existing byte storage.

C++ schema extraction uses libclang. Compiling and using generated headers does
not require Clang.

## Documentation

- [Getting started](https://intrlay.github.io/intrlay/getting-started/)
- [C++ guide](https://intrlay.github.io/intrlay/languages/cpp/)
- [Code generation](https://intrlay.github.io/intrlay/reference/code-generation/)
- [Project](https://github.com/intrlay/intrlay)

Licensed under Apache-2.0 OR MIT.
