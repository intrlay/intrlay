using System;
using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private bool CustomSource(Node node) => node.SourceInputName.Length != 0;
    private bool ElementSource(Node node) => node.Kind is "Sequence" or "Array" && Node(node.Value).Zeroable;
    private string InputContract(Node node) => $"global::Intrlay.IEncodeInput<{Input(node.Id, qualified: true)}>";
    private string SourceContract(Node node) => ElementSource(node)
        ? $"global::Intrlay.{(node.Kind == "Sequence" ? "ISequenceSource" : "IFixedArraySource")}<{Input(node.Value, qualified: true)}>"
        : InputContract(node);
    private string Binding => $"global::{schema.Namespace}.{schema.Name}";

    private Dictionary<Field, string> SourceParameters(List<Field> fields, string name)
    {
        var names = new NameScope(NativeTypeNames().Concat(fields.Select(field => field.Name)).Append(name));
        return fields.Where(field => CustomSource(Node(field.Type)))
            .ToDictionary(field => field, field => names.Allocate("T" + field.Name + "Source"));
    }

    private static string TypeArguments(IEnumerable<string> types) => "<" + string.Join(", ", types) + ">";

    private void SourceConstraints(Dictionary<Field, string> parameters, bool scoped)
    {
        foreach (var pair in parameters)
            Line($"    where {pair.Value} : struct, {SourceContract(Node(pair.Key.Type))}{(scoped ? ", allows ref struct" : "")}");
    }

    private void SourceInputs()
    {
        foreach (var node in schema.Nodes.Values.Where(node => node.Native && node.Encode && CustomSource(node)))
        {
            if (node.Kind is "Record" or "Enum")
            {
                // Canonical input storage and its existing overloads stay unchanged.
                // Explicit implementations add no fields and preserve native POD layout.
                Open($"public readonly partial struct {node.Name} : {InputContract(node)}");
                CanonicalSourceMethods(node);
                if (node.Kind == "Record") SourceFactory(node.Name, node.Fields, scoped: false);
                Close();
                Line();

                if (node.Kind == "Record")
                {
                    SourceRecord(node, node.Name, node.Fields, scoped: false);
                    if (node.SpanInputName.Length != 0)
                        SourceRecord(node, node.SpanInputName, node.Fields, scoped: true);
                }
                else
                {
                    foreach (var variant in node.Variants)
                    {
                        var fields = node.Fields.Where(field => field.Variant == variant.Tag).ToList();
                        if (!fields.Any(field => CustomSource(Node(field.Type)))) continue;
                        Open($"public readonly partial struct {variant.InputName}");
                        SourceFactory(variant.InputName, fields, scoped: false);
                        Close();
                        Line();
                        SourceRecord(node, variant.InputName, fields, scoped: false, tag: variant.Tag);
                    }
                }
            }
            else SourceWrapper(node);
        }
    }

    private void CanonicalSourceMethods(Node node)
    {
        Inline();
        Open($"void {InputContract(node)}.Measure(ref global::Intrlay.Cursor cursor)");
        if (node.HasPayload) Line($"{Binding}.@{node.MeasureName}(in this, ref cursor);");
        Close();
        Line();
        Inline();
        Open($"void {InputContract(node)}.Write(global::System.Span<byte> buffer, int offset, ref global::Intrlay.Cursor cursor)");
        Line($"{Binding}.@{node.WriteName}(buffer, offset, in this, ref cursor);");
        Close();
        Line();
    }

    private void SourceFactory(string name, List<Field> fields, bool scoped)
    {
        var parameters = SourceParameters(fields, name);
        if (parameters.Count == 0) return;
        string arguments = TypeArguments(parameters.Values);
        string FieldType(Field field) => parameters.TryGetValue(field, out var type)
            ? type : scoped ? SpanInputType(Node(field.Type)) : Input(field.Type);
        Docs("Constructs an input whose field sources select value copying or zero initialization through generic specialization.");
        Inline();
        Line(NativeArguments($"public static {name}{arguments} {Name("from_sources")}{arguments}(",
            fields.Select(field => $"{FieldType(field)} @{field.Name}"), ")"));
        SourceConstraints(parameters, scoped);
        Open("");
        Line($"return new({string.Join(", ", fields.Select(field => "@" + field.Name))});");
        Close();
    }

    private void SourceRecord(Node schemaNode, string name, List<Field> fields, bool scoped, uint? tag = null)
    {
        var parameters = SourceParameters(fields, name);
        if (parameters.Count == 0) return;
        string FieldType(Field field) => parameters.TryGetValue(field, out var type)
            ? type : scoped ? SpanInputType(Node(field.Type)) : Input(field.Type);
        Docs("Encodes the canonical schema with statically selected field sources. Source storage must remain stable and must not overlap the destination.");
        foreach (var pair in parameters)
            Line($"/// <typeparam name=\"{pair.Value}\">The source representation for {pair.Key.Name}.</typeparam>");
        Line($"public readonly {(scoped ? "ref " : "")}struct {name}{TypeArguments(parameters.Values)} : {InputContract(schemaNode)}");
        SourceConstraints(parameters, scoped);
        Open("");
        foreach (var field in fields)
        {
            Docs(field.Docs);
            Line($"public {FieldType(field)} @{field.Name} {{ get; }}");
            Line();
        }
        Inline();
        Open(NativeArguments($"public {name}(", fields.Select(field => $"{FieldType(field)} @{field.Name}"), ")"));
        foreach (var field in fields) Line($"this.@{field.Name} = @{field.Name};");
        Close();
        Line();
        Inline();
        Open($"void {InputContract(schemaNode)}.Measure(ref global::Intrlay.Cursor cursor)");
        MeasureFields(fields, "this", generic: true);
        Close();
        Line();
        Inline();
        Open($"void {InputContract(schemaNode)}.Write(global::System.Span<byte> buffer, int offset, ref global::Intrlay.Cursor cursor)");
        if (tag is { } value)
            Line($"global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset), {value}u);");
        WriteFields(schemaNode, fields, "this", generic: true);
        Close();
        Close();
        Line();
    }

    private void SourceWrapper(Node node)
    {
        bool elements = node.Kind is "Sequence" or "Array" && !ElementSource(node);
        string parameter = elements ? "TElement" : "TSource";
        var child = Node(node.Value);
        string contract = ElementSource(node) ? SourceContract(node) : SourceContract(child);
        string storedType = elements
            ? node.Kind == "Sequence" ? "global::System.ReadOnlyMemory<TElement>" : $"{node.ArrayName}<TElement>"
            : parameter;
        string name = node.SourceInputName;
        Docs("Combines a source with its resolved wire schema without constructing an owned value.");
        Line($"/// <typeparam name=\"{parameter}\">{(elements ? "The element source" : "The nested source")} with the same wire schema as the canonical input.</typeparam>");
        Line($"public readonly struct {name}<{parameter}> : {InputContract(node)}");
        Line($"    where {parameter} : struct, {contract}");
        Open("");
        Line($"private readonly {storedType} _value;");
        if (node.Kind == "Optional") Line("private readonly bool _hasValue;");
        Line();
        Inline();
        Open($"public {name}({storedType} value)");
        Line("_value = value;");
        if (node.Kind == "Optional") Line("_hasValue = true;");
        Close();
        Line();

        foreach (bool writing in new[] { false, true })
        {
            Inline();
            Open(writing
                ? $"void {InputContract(node)}.Write(global::System.Span<byte> buffer, int offset, ref global::Intrlay.Cursor cursor)"
                : $"void {InputContract(node)}.Measure(ref global::Intrlay.Cursor cursor)");
            if (ElementSource(node))
            {
                if (writing) SourceWrite(node, "buffer", "offset", "_value", source: true);
                else SourceMeasure(node, "_value", source: true);
            }
            else if (elements)
            {
                string length = node.Kind == "Sequence" ? "_value.Length" : node.Argument.ToString();
                if (node.Kind == "Sequence")
                {
                    Line($"{(writing ? "int payloadOffset = " : "")}cursor.Reserve(checked({length} * {child.Size}), {child.Alignment});");
                    if (writing)
                    {
                        Reference("payloadOffset");
                        Line($"global::System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(buffer.Slice(offset + 4), (uint){length});");
                    }
                }
                else if (writing) Line("int payloadOffset = offset;");
                if (writing || child.HasPayload)
                {
                    Line($"global::System.ReadOnlySpan<TElement> items = {(node.Kind == "Sequence" ? "_value.Span" : "_value")};");
                    Open("for (int index = 0; index < items.Length; ++index)");
                    if (writing) SourceWrite(child, "buffer", $"payloadOffset + index * {child.Size}", "items[index]", source: true);
                    else SourceMeasure(child, "items[index]", source: true);
                    Close();
                }
            }
            else
            {
                if (node.Kind == "Optional")
                {
                    if (writing) Line("buffer[offset] = _hasValue ? (byte)1 : (byte)0;");
                    Open("if (_hasValue)");
                    if (writing) SourceWrite(child, "buffer", $"offset + {node.Argument}", "_value", source: true);
                    else SourceMeasure(child, "_value", source: true);
                    Close();
                }
                else if (node.Kind == "Boxed")
                {
                    Line($"{(writing ? "int payloadOffset = " : "")}cursor.Reserve({child.Size}, {child.Alignment});");
                    if (writing)
                    {
                        Reference("payloadOffset");
                        SourceWrite(child, "buffer", "payloadOffset", "_value", source: true);
                    }
                    else SourceMeasure(child, "_value", source: true);
                }
                else throw new InvalidOperationException("Unsupported source wrapper " + node.Kind);
            }
            Close();
            Line();
        }
        Close();
        Line();
    }

    private void SourceConstruction(Node node, string view)
    {
        string contract = InputContract(node);
        string size = Name("byte_size");
        string measure = Name("compute_byte_length");
        // A returned span view borrows only its destination. Passing a scoped
        // source by value also bounds any spans stored inside a ref struct.
        string passing = scopedView ? "scoped " : "scoped in ";
        Line();
        Inline();
        Line($"public static int {measure}<TInput>(scoped in TInput value)");
        Line($"    where TInput : struct, {contract}, allows ref struct");
        Open("");
        Line($"var cursor = new global::Intrlay.Cursor({size});");
        if (node.HasPayload) Line("value.Measure(ref cursor);");
        Line("return cursor.End;");
        Close();
        Line();
        Inline();
        Line($"public static {view} {Name("create")}<TInput>({passing}TInput value)");
        Line($"    where TInput : struct, {contract}, allows ref struct");
        Open("");
        Line($"var buffer = new byte[{measure}(in value)];");
        Line($"var cursor = new global::Intrlay.Cursor({size});");
        Line("value.Write(buffer, 0, ref cursor);");
        if (node.HasPayload) Line("cursor.Finish(buffer.Length);");
        Line($"return new {view}(buffer);");
        Close();
        Line();
        Inline();
        Line($"public static {view} {Name("create_in_place")}<TInput>(global::System.{(scopedView ? "Span" : "Memory")}<byte> buffer, {passing}TInput value)");
        Line($"    where TInput : struct, {contract}, allows ref struct");
        Open("");
        Line($"var storage = buffer.Slice(0, {measure}(in value));");
        if (!node.FullyWritten) Line($"{Bytes("storage")}.Clear();");
        Line($"var cursor = new global::Intrlay.Cursor({size});");
        Line($"value.Write({Bytes("storage")}, 0, ref cursor);");
        if (node.HasPayload) Line("cursor.Finish(storage.Length);");
        Line($"return new {view}(storage);");
        Close();
    }
}
