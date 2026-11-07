using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void InlineArrays()
    {
        // One declaration per extent, shared by every element type and nesting path.
        foreach (var node in schema.Nodes.Values.Where(node => node.Kind == "Array" && node.Argument != 0)
            .GroupBy(node => node.ArrayName).Select(group => group.First()))
        {
            Docs($"Stores exactly {node.Argument} inline elements without a separate array allocation.");
            Line($"[global::System.Runtime.CompilerServices.InlineArray({node.Argument})]");
            Open($"public struct {node.ArrayName}<T>");
            Line($"private T {Name("first_element")};");
            Line($"public int {Name("length")} => {node.Argument};");
            Line();
            Open($"public {node.ArrayName}(global::System.ReadOnlySpan<T> values)");
            Line("this = default;");
            Line($"if (values.Length != {node.Argument}) throw new global::System.ArgumentException(\"fixed array length\", nameof(values));");
            Line("values.CopyTo(this);");
            Close();
            Line();
            Line($"public static implicit operator {node.ArrayName}<T>(T[] values) => new(values);");
            Line($"public static implicit operator {node.ArrayName}<T>(global::System.ReadOnlyMemory<T> values) => new(values.Span);");
            Close();
            Line();
        }
    }
}
