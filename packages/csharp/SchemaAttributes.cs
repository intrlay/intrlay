using System;
using System.Diagnostics;

namespace Intrlay;

/// <summary>Identifies a resolved schema consumed by the Intrlay source generator.</summary>
/// <remarks>Schema attributes are omitted from assemblies unless INTRLAY_INCLUDE_SCHEMA is defined.</remarks>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class)]
public sealed class GenerateBindingsAttribute : Attribute
{
    /// <summary>Minimum consumer language version. C# 15 adds non-boxing payload-enum patterns.</summary>
    public int CSharpLanguageVersion { get; set; } = 14;
}

/// <summary>A wire representation whose offsets have already been resolved by Intrlay.</summary>
public enum WireKind
{
    Bool, I8, U8, I16, U16, I32, U32, I64, U64, F32, F64,
    Unit, String, Record, Enum, Sequence, Array, Optional, Boxed,
}

/// <summary>Preserves a generated API name after applying the module's naming policy.</summary>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class BindingNameAttribute(string role, string name) : Attribute
{
    public string Role { get; } = role;
    public string Name { get; } = name;
}

/// <summary>Describes one node of the resolved wire type graph.</summary>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class WireTypeAttribute(
    int id, WireKind kind, string name, int size, int alignment, int bufferAlignment) : Attribute
{
    public int Id { get; } = id;
    public WireKind Kind { get; } = kind;
    public string Name { get; } = name;
    public int Size { get; } = size;
    public int Alignment { get; } = alignment;
    public int BufferAlignment { get; } = bufferAlignment;
    public int Value { get; set; } = -1;
    public int Argument { get; set; }
    public bool Read { get; set; }
    public bool Write { get; set; }
    public bool Create { get; set; }
    public bool HasPayload { get; set; }
    public bool Encode { get; set; }
    public bool Native { get; set; }
    public bool NativeLayout { get; set; }
    /// <summary>The resolved encoder writes every byte, including all alignment gaps.</summary>
    public bool FullyWritten { get; set; }
    public bool Zeroable { get; set; }
    public string ViewName { get; set; } = "";
    public string SpanName { get; set; } = "";
    /// <summary>Optional stack-scoped input for a record with directly borrowable payloads.</summary>
    public string SpanInputName { get; set; } = "";
    public string SourceInputName { get; set; } = "";
    public string ArrayName { get; set; } = "";
    public string NativeReadName { get; set; } = "";
    public string MeasureName { get; set; } = "";
    public string WriteName { get; set; } = "";
    public string Documentation { get; set; } = "";
}

/// <summary>Describes an inline field, including its effective borrowing capabilities.</summary>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class WireFieldAttribute(
    int owner, int type, string name, string property, int offset) : Attribute
{
    public int Owner { get; } = owner;
    public int Type { get; } = type;
    public string Name { get; } = name;
    public string Property { get; } = property;
    public int Offset { get; } = offset;
    public bool Read { get; set; }
    public bool Write { get; set; }
    public long Variant { get; set; } = -1;
    public string SetterName { get; set; } = "";
    /// <summary>Candidate backing-field name, used only when CLR recursion requires erased collection storage.</summary>
    public string NativeStorageName { get; set; } = "";
    public string Documentation { get; set; } = "";
}

/// <summary>Names a payload variant and its native construction method.</summary>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class WireVariantAttribute(int owner, uint tag, string name, string factory) : Attribute
{
    public int Owner { get; } = owner;
    public uint Tag { get; } = tag;
    public string Name { get; } = name;
    public string Factory { get; } = factory;
    public string Documentation { get; set; } = "";
    public string InputName { get; set; } = "";
}

/// <summary>Requests a named buffer view of a wire type.</summary>
[Conditional("INTRLAY_INCLUDE_SCHEMA")]
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class WireViewAttribute(int type, string name) : Attribute
{
    public int Type { get; } = type;
    public string Name { get; } = name;
    public bool ReadOnly { get; set; }
    public string SpanName { get; set; } = "";
}
