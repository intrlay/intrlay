using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void NativeRead(Node node)
    {
        var type = Input(node.Id);
        Inline();
        Open($"internal static {type} @{node.NativeReadName}(global::System.ReadOnlySpan<byte> buffer, int offset)");
        if (node.Scalar)
            Line($"return {LoadInline(node.Id, "offset", true, "buffer", "buffer")};");
        else
        {
            // The resolved schema proves every offset and excludes padding and
            // managed fields. MemoryMarshal.Read supports unaligned storage.
            Open($"if (global::System.BitConverter.IsLittleEndian && global::System.Runtime.CompilerServices.Unsafe.SizeOf<{type}>() == {node.Size})");
            Line($"return global::System.Runtime.InteropServices.MemoryMarshal.Read<{type}>(buffer.Slice(offset, {node.Size}));");
            Close();
            if (node.Kind == "Record")
            {
                var values = node.Fields.Select(field => $"@{Node(field.Type).NativeReadName}(buffer, offset + {field.Offset})");
                Line(NativeArguments($"return new {type}(", values, ");"));
            }
            else
            {
                Line($"{type} value = default;");
                Open($"for (int index = 0; index < {node.Argument}; ++index)");
                Line($"value[index] = @{Node(node.Value).NativeReadName}(buffer, offset + index * {Node(node.Value).Size});");
                Close();
                Line("return value;");
            }
        }
        Close();
        Line();
    }
}
