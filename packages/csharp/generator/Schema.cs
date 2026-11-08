using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.CodeAnalysis;

namespace Intrlay.Generator;

// This model consumes resolved metadata; it does not calculate wire layout.
internal sealed class Schema
{
    public string Namespace = "";
    public string Name = "";
    public int CSharpLanguageVersion = 14;
    public readonly Dictionary<string, string> Names = new();
    public readonly Dictionary<int, Node> Nodes = new();
    public readonly List<View> Views = new();
    public string HintName => Namespace + "." + Name + ".g.cs";

    public static Schema Read(INamedTypeSymbol symbol)
    {
        var schema = new Schema { Name = symbol.Name, Namespace = symbol.ContainingNamespace.ToDisplayString() };
        var attributes = symbol.GetAttributes();
        foreach (var attribute in attributes)
        {
            var data = new Metadata(attribute);
            if (data.Is("GenerateBindings"))
            {
                schema.CSharpLanguageVersion = data.Named("CSharpLanguageVersion", 14);
            }
            else if (data.Is("WireType"))
            {
                var node = new Node
                {
                    Id = data.At<int>(0), Kind = data.Enum(1), Name = data.At<string>(2),
                    Size = data.At<int>(3), Alignment = data.At<int>(4), BufferAlignment = data.At<int>(5),
                    Value = data.Named("Value", -1), Argument = data.Named("Argument", 0),
                    Read = data.Named("Read", false), Write = data.Named("Write", false),
                    Create = data.Named("Create", false), HasPayload = data.Named("HasPayload", false),
                    Encode = data.Named("Encode", false), Native = data.Named("Native", false),
                    NativeLayout = data.Named("NativeLayout", false),
                    FullyWritten = data.Named("FullyWritten", false),
                    Zeroable = data.Named("Zeroable", false),
                    ViewName = data.Named("ViewName", ""), MeasureName = data.Named("MeasureName", ""),
                    SpanName = data.Named("SpanName", ""), ArrayName = data.Named("ArrayName", ""),
                    SpanInputName = data.Named("SpanInputName", ""),
                    SourceInputName = data.Named("SourceInputName", ""),
                    NativeReadName = data.Named("NativeReadName", ""),
                    WriteName = data.Named("WriteName", ""), Docs = data.Named("Documentation", ""),
                };
                schema.Nodes.Add(node.Id, node);
            }
            else if (data.Is("BindingName"))
            {
                schema.Names.Add(data.At<string>(0), data.At<string>(1));
            }
        }
        foreach (var attribute in attributes)
        {
            var data = new Metadata(attribute);
            if (data.Is("WireField"))
            {
                schema.Nodes[data.At<int>(0)].Fields.Add(new Field
                {
                    Type = data.At<int>(1), Name = data.At<string>(2), Property = data.At<string>(3),
                    Offset = data.At<int>(4), Read = data.Named("Read", false), Write = data.Named("Write", false),
                    Variant = data.Named("Variant", -1L), Setter = data.Named("SetterName", ""), Docs = data.Named("Documentation", ""),
                    NativeStorageName = data.Named("NativeStorageName", ""),
                });
            }
            else if (data.Is("WireVariant"))
            {
                schema.Nodes[data.At<int>(0)].Variants.Add(new Variant
                {
                    Tag = data.At<uint>(1), Name = data.At<string>(2), Factory = data.At<string>(3),
                    InputName = data.Named("InputName", ""),
                    Docs = data.Named("Documentation", ""),
                });
            }
            else if (data.Is("WireView"))
            {
                schema.Views.Add(new View { Type = data.At<int>(0), Name = data.At<string>(1), ReadOnly = data.Named("ReadOnly", false), SpanName = data.Named("SpanName", "") });
            }
        }
        return schema;
    }
}

internal sealed class Node
{
    public int Id, Size, Alignment, BufferAlignment, Value, Argument;
    public string Kind = "", Name = "", ViewName = "", MeasureName = "", WriteName = "", Docs = "";
    public string SpanName = "", ArrayName = "", NativeReadName = "";
    public string SpanInputName = "";
    public string SourceInputName = "";
    public bool Read, Write, Create, HasPayload, Encode, Native, NativeLayout, FullyWritten, Zeroable;
    public readonly List<Field> Fields = new();
    public readonly List<Variant> Variants = new();
    public bool ValueEnum => Kind == "Enum" && Fields.Count == 0;
    public bool Scalar => Kind is "Bool" or "I8" or "U8" or "I16" or "U16" or "I32" or "U32" or "I64" or "U64" or "F32" or "F64";
}

internal sealed class Field
{
    public int Type, Offset;
    public long Variant;
    public string Name = "", Property = "", Setter = "", Docs = "";
    public string NativeStorageName = "";
    public bool Read, Write;
}

internal sealed class Variant
{
    public uint Tag;
    public string Name = "", Factory = "", Docs = "", InputName = "";
}

internal sealed class View
{
    public int Type;
    public string Name = "";
    public string SpanName = "";
    public bool ReadOnly;
}

internal readonly struct Metadata(AttributeData data)
{
    public bool Is(string name) => data.AttributeClass?.ToDisplayString() == "Intrlay." + name + "Attribute";
    public T At<T>(int index) => (T)data.ConstructorArguments[index].Value!;
    public T Named<T>(string name, T fallback)
    {
        foreach (var pair in data.NamedArguments)
            if (pair.Key == name) return (T)pair.Value.Value!;
        return fallback;
    }
    public string Enum(int index)
    {
        var value = data.ConstructorArguments[index];
        return value.Type!.GetMembers().OfType<IFieldSymbol>()
            .First(field => field.HasConstantValue && Equals(field.ConstantValue, value.Value)).Name;
    }
}
