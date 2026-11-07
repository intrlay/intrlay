using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void Measure(Node node)
    {
        Open($"internal static void @{node.MeasureName}(in {Input(node.Id)} value, ref global::Intrlay.Cursor cursor)");
        switch (node.Kind)
        {
            case "String":
                Line("cursor.Reserve(global::System.Text.Encoding.UTF8.GetByteCount(value), 1);");
                break;
            case "Record":
                foreach (var field in node.Fields.Where(field => Node(field.Type).HasPayload))
                    Line($"@{Node(field.Type).MeasureName}(value.@{field.Name}, ref cursor);");
                break;
            case "Enum":
                Open($"switch (value.{Name("tag")})");
                foreach (var variant in node.Variants)
                {
                    Line($"case {variant.Tag}:");
                    Open("");
                    var fields = node.Fields.Where(field => field.Variant == variant.Tag && Node(field.Type).HasPayload).ToList();
                    if (fields.Count != 0) Line($"var payload = value.@{variant.Name};");
                    foreach (var field in fields)
                        Line($"@{Node(field.Type).MeasureName}(payload.@{field.Name}, ref cursor);");
                    Line("break;");
                    Close();
                }
                Line("default: throw new global::System.ArgumentException(\"Unknown enum tag.\", nameof(value));");
                Close();
                break;
            case "Sequence":
            case "Array":
                var element = Node(node.Value);
                if (node.Kind == "Sequence") Line($"cursor.Reserve(checked(value.Length * {element.Size}), {element.Alignment});");
                if (element.HasPayload)
                {
                    Line($"global::System.ReadOnlySpan<{Input(node.Value)}> items = {(node.Kind == "Array" ? "value" : "value.Span")};");
                    Open("foreach (ref readonly var item in items)");
                    Line($"@{element.MeasureName}(in item, ref cursor);");
                    Close();
                }
                break;
            case "Optional":
                var optional = OptionalInput(node.Value);
                Open($"if ({optional.Present})");
                Line($"@{Node(node.Value).MeasureName}({optional.Value}, ref cursor);");
                Close();
                break;
            case "Boxed":
                var boxed = Node(node.Value);
                Line($"cursor.Reserve({boxed.Size}, {boxed.Alignment});");
                if (boxed.HasPayload) Line($"@{boxed.MeasureName}(value.Value, ref cursor);");
                break;
        }
        Close();
        Line();
    }

    private void Write(Node node)
    {
        if (node.Scalar || (node.Kind == "Record" && node.Fields.Count <= 4)) Inline();
        Open($"internal static void @{node.WriteName}(global::System.Span<byte> buffer, int offset, in {Input(node.Id)} value, ref global::Intrlay.Cursor cursor)");
        if (node.Scalar) Line(Store(node, "buffer", "offset", "value"));
        if (node.Kind == "Record" && node.NativeLayout)
        {
            Open("if (global::System.BitConverter.IsLittleEndian)");
            Line("global::System.Runtime.InteropServices.MemoryMarshal.Write(buffer.Slice(offset), in value);");
            Line("return;");
            Close();
        }
        switch (node.Kind)
        {
            case "String":
                Line("int length = global::System.Text.Encoding.UTF8.GetByteCount(value);");
                Line("int payloadOffset = cursor.Reserve(length, 1);");
                Reference("payloadOffset");
                Line("global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset + 4), (uint)length);");
                Line("global::System.Text.Encoding.UTF8.GetBytes(value, buffer.Slice(payloadOffset, length));");
                break;
            case "Record":
                Line($"var header = buffer.Slice(offset, {node.Size});");
                foreach (var field in node.Fields)
                {
                    var member = Node(field.Type);
                    if (member.Scalar) Line(Store(member, "header", field.Offset.ToString(), $"value.@{field.Name}"));
                    else if (!member.HasPayload)
                        Line($"@{member.WriteName}(header, {field.Offset}, value.@{field.Name}, ref cursor);");
                    else Line($"@{member.WriteName}(buffer, offset + {field.Offset}, value.@{field.Name}, ref cursor);");
                }
                break;
            case "Enum":
                Open($"switch (value.{Name("tag")})");
                foreach (var variant in node.Variants)
                {
                    Line($"case {variant.Tag}:");
                    Open("");
                    Line($"global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset), {variant.Tag});");
                    var fields = node.Fields.Where(field => field.Variant == variant.Tag).ToList();
                    if (fields.Count != 0) Line($"var payload = value.@{variant.Name};");
                    foreach (var field in fields)
                        Line($"@{Node(field.Type).WriteName}(buffer, offset + {field.Offset}, payload.@{field.Name}, ref cursor);");
                    Line("break;");
                    Close();
                }
                Line("default: throw new global::System.ArgumentException(\"Unknown enum tag.\", nameof(value));");
                Close();
                break;
            case "Sequence":
            case "Array":
                var element = Node(node.Value);
                if (node.Kind == "Sequence")
                {
                    Line($"int payloadOffset = cursor.Reserve(checked(value.Length * {element.Size}), {element.Alignment});");
                    Reference("payloadOffset");
                    Line("global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset + 4), (uint)value.Length);");
                }
                else Line("int payloadOffset = offset;");
                Line($"global::System.ReadOnlySpan<{Input(node.Value)}> items = {(node.Kind == "Array" ? "value" : "value.Span")};");
                if (element.NativeLayout)
                {
                    Open("if (global::System.BitConverter.IsLittleEndian)");
                    Line("global::System.Runtime.InteropServices.MemoryMarshal.AsBytes(items).CopyTo(buffer.Slice(payloadOffset));");
                    Line("return;");
                    Close();
                }
                Open("for (int index = 0; index < items.Length; ++index)");
                Line($"@{element.WriteName}(buffer, payloadOffset + index * {element.Size}, in items[index], ref cursor);");
                Close();
                break;
            case "Optional":
                var optional = OptionalInput(node.Value);
                Line($"buffer[offset] = {optional.Present} ? (byte)1 : (byte)0;");
                Open($"if ({optional.Present})");
                Line($"@{Node(node.Value).WriteName}(buffer, offset + {node.Argument}, {optional.Value}, ref cursor);");
                Close();
                break;
            case "Boxed":
                var boxed = Node(node.Value);
                Line($"int payloadOffset = cursor.Reserve({boxed.Size}, {boxed.Alignment});");
                Reference("payloadOffset");
                Line($"@{boxed.WriteName}(buffer, payloadOffset, value.Value, ref cursor);");
                break;
        }
        Close();
        Line();
    }

    private void Reference(string target) => Line($"global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset), checked((uint)({target} - offset)));");
}
