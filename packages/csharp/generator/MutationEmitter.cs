namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    // Offsets are resolved by Rust; this only emits traversal of existing storage.
    private Node? MutationPayload(Node node)
    {
        if (!node.Write) return null;
        if (!node.HasPayload) return node;
        return node.Kind is "Optional" or "Boxed" ? MutationPayload(Node(node.Value)) : null;
    }

    private void StorePayload(Node node, string buffer, string offset)
    {
        if (node.Scalar)
        {
            Line(Store(node, Bytes(buffer), offset, "value"));
            return;
        }
        Line($"var position = {offset};");
        while (node.HasPayload)
        {
            if (node.Kind == "Optional")
            {
                Line($"if ({Bytes(buffer)}[position] == 0) return;");
                Line($"position += {node.Argument};");
            }
            else Line($"position = global::Intrlay.Reader.Dereference({Bytes(buffer)}, position);");
            node = Node(node.Value);
        }
        if (node.Scalar) Line(Store(node, Bytes(buffer), "position", "value"));
        else
        {
            Line("var cursor = new global::Intrlay.Cursor(0);");
            Line($"global::{schema.Namespace}.{schema.Name}.@{node.WriteName}({Bytes(buffer)}, position, in value, ref cursor);");
        }
    }
}
