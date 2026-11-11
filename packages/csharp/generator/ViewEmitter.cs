namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void View(View view, bool scoped = false)
    {
        scopedView = scoped;
        var node = Node(view.Type);
        var memory = $"global::System.{(view.ReadOnly ? "ReadOnly" : "")}{(scoped ? "Span" : "Memory")}<byte>";
        var viewName = scoped ? view.SpanName : view.Name;
        var buffer = Name("buffer");
        var offset = Name("offset");
        Docs(node.Docs);
        Open($"public readonly {(scoped ? "ref " : "")}struct {viewName}");
        Line($"public const int {Name("byte_size")} = {node.Size};");
        Line($"public const int {Name("alignment")} = {node.BufferAlignment};");
        Line($"private readonly {memory} {buffer};");
        Line(scoped && !node.HasPayload ? $"private const int {offset} = 0;" : $"private readonly int {offset};");
        if (node.Kind == "Sequence")
        {
            Line($"private readonly int {Name("element_offset")};");
            Line($"private readonly int {Name("element_count")};");
        }
        Line();
        Line($"public {memory} {Name("bytes")} => {buffer};");
        Line();
        Inline();
        Open($"public {viewName}({memory} buffer, int offset = 0)");
        if (scoped && !node.HasPayload)
        {
            // A fixed encoding has no references outside its header. Bounding
            // this span once makes the field checks visible to the JIT.
            Line($"this.{buffer} = buffer.Slice(offset, {node.Size});");
        }
        else
        {
            Line($"this.{buffer} = buffer;");
            Line($"this.{offset} = offset;");
        }
        if (node.Kind == "Sequence")
        {
            Line($"var bytes = {Bytes("buffer")};");
            Line($"this.{Name("element_offset")} = global::Intrlay.Reader.Dereference(bytes, offset);");
            Line($"this.{Name("element_count")} = (int)global::System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian(bytes.Slice(offset + 4));");
        }
        Close();
        if (!scoped)
        {
            Line();
            Docs("Borrows a typed span view for scoped access without copying the encoded bytes.");
            Inline();
            Line($"public {view.SpanName} {Name("as_span")}() => new({buffer}.Span, {offset});");
        }
        if (node.Create)
        {
            Construction(node, viewName);
            if (CustomSource(node)) SourceConstruction(node, viewName);
        }
        if (!view.ReadOnly && node.Write && !node.HasPayload)
        {
            Line();
            Docs("Replaces the inline value without reallocating its backing storage.");
            Inline();
            Open($"public void {Name("replace")}(in {Input(node.Id)} value)");
            StorePayload(node, buffer, offset);
            Close();
        }
        if (node.NativeLayout)
        {
            Line();
            Docs("Copies the fixed encoding to its native value; compatible layouts use a single unaligned read.");
            Inline();
            Line($"public {Input(node.Id)} {Name("to_value")}() => global::{schema.Namespace}.{schema.Name}.@{node.NativeReadName}({Bytes(buffer)}, {offset});");
        }

        switch (node.Kind)
        {
            case "Record":
            case "Enum":
                if (node.Kind == "Enum" && node.Read)
                {
                    if (node.ValueEnum)
                        Getter($"public {Input(node.Id)} {Name("tag")}", node.Id, offset, view.ReadOnly, buffer);
                    else
                        ExpressionGetter($"public uint {Name("tag")}", $"global::System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian({Bytes(buffer)}.Slice({offset}))");
                }
                foreach (var field in node.Fields) Property(node, field, view.ReadOnly);
                break;
            case "Array":
            case "Sequence":
                Collection(node, view.ReadOnly);
                break;
            default:
                if (node.Read)
                    Getter($"public {Output(node.Id, view.ReadOnly)} {Name("value")}", node.Id, offset, view.ReadOnly, buffer);
                if (!view.ReadOnly && MutationPayload(node) is { } scalar)
                {
                    Open($"public void {Name("set")}({Input(scalar.Id)} value)");
                    StorePayload(node, buffer, offset);
                    Close();
                }
                break;
        }
        Close();
        Line();
    }

    private void Construction(Node node, string view, bool spanInput = false)
    {
        var borrowed = spanInput;
        var input = borrowed ? node.SpanInputName : Input(node.Id);
        // scoped values bound their contained spans as well as the parameter.
        // scoped in only bounds the reference to the struct, so it cannot prove
        // that a returned span view is independent of stack-backed input fields.
        var passing = borrowed ? "scoped " : "scoped in ";
        var size = Name("byte_size");
        var measure = Name("compute_byte_length");
        Line();
        Open($"public static int {measure}({passing}{input} value)");
        if (node.HasPayload)
        {
            Line($"var cursor = new global::Intrlay.Cursor({size});");
            Line($"global::{schema.Namespace}.{schema.Name}.@{node.MeasureName}(in value, ref cursor);");
            Line("return cursor.End;");
        }
        else Line($"return {size};");
        Close();
        Line();
        Open($"public static {view} {Name("create")}({passing}{input} value)");
        Line($"var buffer = new byte[{measure}({(borrowed ? "" : "in ")}value)];");
        Line($"var cursor = new global::Intrlay.Cursor({size});");
        Line($"global::{schema.Namespace}.{schema.Name}.@{node.WriteName}(buffer, 0, in value, ref cursor);");
        if (node.HasPayload) Line("cursor.Finish(buffer.Length);");
        Line($"return new {view}(buffer);");
        Close();
        Line();
        Open($"public static {view} {Name("create_in_place")}(global::System.{(scopedView ? "Span" : "Memory")}<byte> buffer, scoped {(borrowed ? "" : "in ")}{input} value)");
        Line($"var storage = buffer.Slice(0, {measure}({(borrowed ? "" : "in ")}value));");
        if (!node.FullyWritten) Line($"{Bytes("storage")}.Clear();");
        Line($"var cursor = new global::Intrlay.Cursor({size});");
        Line($"global::{schema.Namespace}.{schema.Name}.@{node.WriteName}({Bytes("storage")}, 0, in value, ref cursor);");
        if (node.HasPayload) Line("cursor.Finish(storage.Length);");
        Line($"return new {view}(storage);");
        Close();
        Line();
        if (!spanInput && node.SpanInputName.Length != 0) Construction(node, view, spanInput: true);
    }

    private void Property(Node owner, Field field, bool readOnly)
    {
        var node = Node(field.Type);
        var offset = $"{Name("offset")} + {field.Offset}";
        var buffer = Name("buffer");
        var tagged = field.Variant >= 0;
        var present = $"global::System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian({Bytes(buffer)}.Slice({Name("offset")})) == {field.Variant}";
        if (field.Read || (!readOnly && field.Write && Borrows(field.Type)))
        {
            var type = tagged ? OptionalType(field.Type, input: false, readOnly) : Output(field.Type, readOnly);
            Docs(field.Docs);
            Getter($"public {type} @{field.Property}", field.Type, offset, readOnly, buffer, variant: field.Variant);
        }
        if (!readOnly && field.Write && MutationPayload(node) is { } scalar)
        {
            Docs(field.Docs);
            Open($"public void @{field.Setter}({Input(scalar.Id)} value)");
            if (tagged) Line($"if (!({present})) return;");
            StorePayload(node, buffer, offset);
            Close();
        }
    }

    private void Collection(Node node, bool readOnly)
    {
        var element = Node(node.Value);
        var buffer = Name("buffer");
        var offset = Name("offset");
        var position = node.Kind == "Array" ? offset : Name("element_offset");
        var count = node.Kind == "Array" ? node.Argument.ToString() : Name("element_count");
        Line($"public int {Name("count")} => {count};");
        if (node.Read || (!readOnly && node.Write && Borrows(node.Value)))
        {
            var type = Output(node.Value, readOnly);
            var prefix = readOnly ? "ReadOnly" : "";
            if (scopedView)
                Getter($"public {type} this[int index]", node.Value, $"{position} + index * {element.Size}", readOnly, buffer, indexCount: count);
            else
            {
                Open($"private readonly struct ElementAccess : global::Intrlay.I{prefix}ElementAccess<{type}>");
                Line($"public static int Stride => {element.Size};");
                Getter($"public static {type} Read(global::System.{prefix}Memory<byte> buffer, int offset)", node.Value, "offset", readOnly, "buffer", method: true);
                Close();
                ExpressionGetter($"public {type} this[int index]", $"new global::Intrlay.{prefix}SequenceView<{type}, ElementAccess>({buffer}, {position}, {count})[index]");
            }
        }
        if (!readOnly && node.Write && MutationPayload(element) is { } scalar)
        {
            Open($"public void {Name("set")}(int index, {Input(scalar.Id)} value)");
            StorePayload(element, buffer, $"({position}) + index * {element.Size}");
            Close();
        }
        if (element.NativeLayout)
        {
            Line();
            Docs("Borrows native elements when byte order, alignment, and native stride match; otherwise use the typed indexer.");
            var prefix = readOnly ? "ReadOnly" : "";
            Inline();
            Line($"public bool {Name("try_get_native_span")}(out global::System.{prefix}Span<{Input(node.Value)}> values) =>");
            Line($"    global::Intrlay.NativeLayout.TryGetSpan({Bytes(buffer)}.Slice({position}, checked({count} * {element.Size})), {element.Size}, {element.Alignment}, out values);");
        }
    }

    private void Inline() => Line("[global::System.Runtime.CompilerServices.MethodImpl(global::System.Runtime.CompilerServices.MethodImplOptions.AggressiveInlining)]");

}
