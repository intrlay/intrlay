using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    // Keep ordinary fields compact. Nested wrappers use named offsets and
    // payloads so each pointer/presence byte is read once on the selected path.
    private void Getter(string signature, int id, string offset, bool readOnly, string buffer,
        bool method = false, long variant = -1, string? indexCount = null)
    {
        var nested = Node(id).Kind is "Optional" or "Boxed";
        if (!nested && variant < 0 && indexCount is null)
        {
            var expression = LoadInline(id, offset, readOnly, buffer, Bytes(buffer));
            ExpressionGetter(signature, expression, method);
            return;
        }

        if (method) Inline();
        Open(signature);
        if (!method) { Inline(); Open("get"); }
        if (indexCount is not null)
            Line($"if ((uint)index >= (uint){indexCount}) throw new global::System.ArgumentOutOfRangeException(nameof(index));");
        var names = new NameScope(schema.Names.Values
            .Concat(schema.Nodes.Values.Select(node => node.Name))
            .Concat(schema.Views.Select(view => view.Name))
            .Concat(new[] { "buffer", "offset", "index" }));
        var bytes = names.Allocate("bytes");
        Line($"var {bytes} = {Bytes(buffer)};");
        if (variant >= 0)
            Line($"if (global::System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian({bytes}.Slice({Name("offset")})) != {variant}) return default;");
        var value = ReadValue(id, offset, readOnly, buffer, bytes, names);
        if (variant >= 0) value = WrapOptional(id, value, readOnly);
        Line($"return {value};");
        if (!method) Close();
        Close();
    }

    private string ReadValue(int id, string offset, bool readOnly, string buffer, string bytes, NameScope names)
    {
        var node = Node(id);
        if (node.Kind == "Boxed")
        {
            var payloadOffset = names.Allocate("payloadOffset");
            Line($"var {payloadOffset} = global::Intrlay.Reader.Dereference({bytes}, {offset});");
            return ReadValue(node.Value, payloadOffset, readOnly, buffer, bytes, names);
        }
        if (node.Kind == "Optional")
        {
            var valueOffset = names.Allocate("valueOffset");
            var optional = names.Allocate("optional");
            Line($"var {valueOffset} = {offset};");
            Line($"{Output(id, readOnly)} {optional} = default;");
            Open($"if ({bytes}[{valueOffset}] != 0)");
            var value = ReadValue(node.Value, $"{valueOffset} + {node.Argument}", readOnly, buffer, bytes, names);
            Line($"{optional} = {WrapOptional(node.Value, value, readOnly)};");
            Close();
            return optional;
        }
        return LoadInline(id, offset, readOnly, buffer, bytes);
    }

    private string WrapOptional(int value, string expression, bool readOnly) =>
        OptionalPayload(value, input: false).Kind == "String"
            ? expression
            : $"new {OptionalType(value, input: false, readOnly)}({expression})";

    private void ExpressionGetter(string signature, string expression, bool method = false)
    {
        if (method && signature.Length + expression.Length < 110)
        {
            Inline();
            Line(signature + " => " + expression + ";");
        }
        else
        {
            if (method) Inline();
            Open(signature);
            if (!method) { Inline(); Open("get"); }
            Line("return " + expression + ";");
            if (!method) Close();
            Close();
        }
    }
}
