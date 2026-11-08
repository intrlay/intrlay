# Intrlay for C#

Value-type buffer views for .NET, with a Roslyn incremental source generator.
The `Intrlay` package includes both the runtime and the generator; add the
CLI-generated declarations to your project and the C# compiler supplies their
constructors, views, and codecs.

## Installation

Requires .NET 10 or later and C# 14 or later.
Optional C# 15 union-pattern generation requires C# 15 and .NET 11.

```sh
dotnet add package Intrlay
```

## Usage

After generating `Point.cs` with `--language csharp --module-name example`:

```csharp
using System;
using IntrlayGenerated.Example;

var value = new PointInput(1, 2, 3);
var point = PointView.Create(in value);
point.SetX(4);

Console.WriteLine(new PointView(point.Bytes).X); // 4
```

Memory-backed views can be retained in ordinary objects. Generated span views
also support scoped access to stack or pooled buffers. Normal use of the
generated API does not require enabling unsafe code in the consuming project.

## Documentation

- [Getting started](https://intrlay.github.io/intrlay/getting-started/)
- [C# guide](https://intrlay.github.io/intrlay/languages/csharp/)
- [Code generation](https://intrlay.github.io/intrlay/reference/code-generation/)
- [Project](https://github.com/intrlay/intrlay)

Licensed under Apache-2.0 OR MIT.
