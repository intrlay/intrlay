using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void Measure(Node node, bool spanInput = false)
    {
        if (spanInput) Inline();
        var type = spanInput ? SpanInputType(node) : Input(node.Id);
        Open($"internal static void @{node.MeasureName}({(spanInput ? "scoped " : "")}in {type} value, ref global::Intrlay.Cursor cursor)");
        switch (node.Kind)
        {
            case "String":
                Line("cursor.Reserve(global::System.Text.Encoding.UTF8.GetByteCount(value), 1);");
                break;
            case "Record":
                MeasureFields(node.Fields, "value");
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
        // Measurement only visits payload fields. In particular, an owned
        // record's inline arrays must not be copied just to measure its size.
        if (!spanInput && SpanCodec(node)) Measure(node, spanInput: true);
    }

    private void Write(Node node, bool spanInput = false)
    {
        if (node.Scalar || (node.Kind == "Record" && node.Fields.Count <= 4)) Inline();
        var type = spanInput ? SpanInputType(node) : Input(node.Id);
        Open($"internal static void @{node.WriteName}(global::System.Span<byte> buffer, int offset, {(spanInput ? "scoped " : "")}in {type} value, ref global::Intrlay.Cursor cursor)");
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
                WriteFields(node, node.Fields, "value");
                break;
            case "Enum":
                if (node.ValueEnum)
                {
                    Open("switch (value)");
                    foreach (var variant in node.Variants)
                        Line($"case {Input(node.Id)}.@{variant.Name}:");
                    Line("    break;");
                    Line("default: throw new global::System.ArgumentException(\"Unknown enum tag.\", nameof(value));");
                    Close();
                    Line("global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset), (uint)value);");
                    break;
                }
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
                WriteCollection(node, spanInput, source: false);
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
        // Generate both overloads from the same algorithm. Keeping the owned
        // path direct preserves its JIT shape and avoids retaining temporary
        // inline-array copies together when converting a complete record.
        if (!spanInput && SpanCodec(node)) Write(node, spanInput: true);
    }

    private void MeasureFields(IEnumerable<Field> fields, string owner, bool generic = false)
    {
        foreach (var field in fields.Where(field => Node(field.Type).HasPayload))
            SourceMeasure(Node(field.Type), $"{owner}.@{field.Name}", generic && CustomSource(Node(field.Type)));
    }

    private void WriteFields(Node node, IEnumerable<Field> fields, string owner, bool generic = false)
    {
        Line($"var header = buffer.Slice(offset, {node.Size});");
        foreach (var field in fields)
        {
            var member = Node(field.Type);
            string value = $"{owner}.@{field.Name}";
            if (member.Scalar) Line(Store(member, "header", field.Offset.ToString(), value));
            else SourceWrite(member, member.HasPayload ? "buffer" : "header",
                member.HasPayload ? $"offset + {field.Offset}" : field.Offset.ToString(), value,
                generic && CustomSource(member));
        }
    }

    private void SourceMeasure(Node node, string value, bool source)
    {
        if (!node.HasPayload) return;
        Line(source && !ElementSource(node)
            ? $"{value}.Measure(ref cursor);"
            : $"{Binding}.@{node.MeasureName}({value}, ref cursor);");
    }

    private void SourceWrite(Node node, string buffer, string offset, string value, bool source)
    {
        Line(source && !ElementSource(node)
            ? $"{value}.Write({buffer}, {offset}, ref cursor);"
            : $"{Binding}.@{node.WriteName}({buffer}, {offset}, {value}, ref cursor);");
    }

    private void Reference(string target) => Line($"global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset), checked((uint)({target} - offset)));");

    private void SourceCollectionCodec(Node node)
    {
        if (node.HasPayload)
        {
            Inline();
            Line($"internal static void @{node.MeasureName}<TSource>(scoped in TSource value, ref global::Intrlay.Cursor cursor)");
            Line($"    where TSource : struct, {SourceContract(node)}, allows ref struct");
            Open("");
            Line($"cursor.Reserve(checked(value.Length * {Node(node.Value).Size}), {Node(node.Value).Alignment});");
            Close();
            Line();
        }
        Inline();
        Line($"internal static void @{node.WriteName}<TSource>(global::System.Span<byte> buffer, int offset, scoped in TSource value, ref global::Intrlay.Cursor cursor)");
        Line($"    where TSource : struct, {SourceContract(node)}, allows ref struct");
        Open("");
        WriteCollection(node, spanInput: false, source: true);
        Close();
        Line();
    }

    // Owned, span, and generic sources all use the same layout and copy rules.
    private void WriteCollection(Node node, bool spanInput, bool source)
    {
        var element = Node(node.Value);
        if (spanInput && node.Kind == "Array")
            Line($"if (value.Length != {node.Argument}) throw new global::System.ArgumentException(\"fixed array length\", nameof(value));");
        if (node.Kind == "Sequence")
        {
            Line($"int payloadOffset = cursor.Reserve(checked(value.Length * {element.Size}), {element.Alignment});");
            Reference("payloadOffset");
            Line("global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset + 4), (uint)value.Length);");
        }
        else Line("int payloadOffset = offset;");
        if (source)
        {
            Open("if (TSource.IsZeroed)");
            Line($"buffer.Slice(payloadOffset, {(node.Kind == "Array" ? node.Size.ToString() : $"checked(value.Length * {element.Size})")}).Clear();");
            Line("return;");
            Close();
        }
        Line($"global::System.ReadOnlySpan<{Input(node.Value)}> items = {(source ? "value.Span" : node.Kind == "Array" || spanInput ? "value" : "value.Span")};");
        if (source && node.Kind == "Array")
            Line($"if (items.Length != {node.Argument}) throw new global::System.ArgumentException(\"fixed array length\", nameof(value));");
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
    }
}
