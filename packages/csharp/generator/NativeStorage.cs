using System;
using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private readonly Dictionary<int, NativeUnion> nativeUnions = new();
    private string[]? nativeTypeNames;

    private string[] NativeTypeNames() => nativeTypeNames ??= schema.Nodes.Values.Where(type => type.Native)
        .SelectMany(type => new[] { type.Name }.Concat(type.Variants.Select(variant => type.Name + variant.Name)))
        .ToArray();

    private NativeUnion NativeLayout(Node node)
    {
        if (!nativeUnions.TryGetValue(node.Id, out var union))
        {
            var names = new NameScope(NativeTypeNames()
                .Concat(node.Variants.SelectMany(variant => new[] { variant.Name, variant.Factory }))
                .Append(schema.Names["tag"]));
            union = new NativeUnion(this, node, names);
            nativeUnions.Add(node.Id, union);
        }
        return union;
    }

    private string NativeType(Node node) => $"global::{schema.Namespace}.{node.Name}";

    // Each enum has one native layout. Nested enums retain their unmanaged data
    // as an opaque block and share rebased GC-visible slots with other variants.
    // Planning and copying never expand a child's alternatives again.
    private sealed class NativeUnion
    {
        public readonly Node Node;
        public readonly NativeStorage Storage;
        public readonly Part Root;
        public readonly string StorageType, NumericType, StorageField, ReadStorage, CopyStorage;
        public readonly Dictionary<int, NativeArray> Arrays = new();
        private readonly Emitter emitter;
        private readonly NameScope names;

        public NativeUnion(Emitter emitter, Node node, NameScope names)
        {
            this.emitter = emitter;
            this.names = names;
            Node = node;
            StorageType = names.Allocate("Storage");
            NumericType = names.Allocate("NumericData");
            StorageField = names.Allocate("_storage");
            ReadStorage = names.Allocate("ReadStorage");
            CopyStorage = names.Allocate("CopyStorage");
            Storage = new NativeStorage(emitter, this);
            Root = Storage.BuildRoot(node);
        }

        public NativeArray ArrayLayout(Node node)
        {
            if (!Arrays.TryGetValue(node.Id, out var array))
            {
                array = new NativeArray(emitter, this, node, names);
                Arrays.Add(node.Id, array);
            }
            return array;
        }
    }

    // The element layout is emitted once, regardless of the fixed array length.
    // Collections retain their owners through MemorySlot instead of closing a
    // CLR generic value-type cycle through the containing enum.
    private sealed class NativeArray
    {
        public readonly NativeStorage Storage;
        public readonly Part Element;
        public readonly string StorageType, NumericType, Type;

        public NativeArray(Emitter emitter, NativeUnion owner, Node node, NameScope names)
        {
            StorageType = names.Allocate("Array" + node.Id + "ElementStorage");
            NumericType = names.Allocate("Array" + node.Id + "NumericData");
            Type = node.ArrayName + "<" + emitter.NativeType(owner.Node) + "." + StorageType + ">";
            Storage = new NativeStorage(emitter, owner);
            Element = Storage.Build(emitter.Node(node.Value));
        }
    }

    private bool RecursiveArray(Node array, Node owner)
    {
        var pending = new Stack<int>();
        var visited = new HashSet<int>();
        pending.Push(array.Value);
        while (pending.Count != 0)
        {
            int id = pending.Pop();
            if (id == owner.Id) return true;
            if (!visited.Add(id)) continue;
            var node = Node(id);
            switch (node.Kind)
            {
                case "Record":
                case "Enum":
                    foreach (var field in node.Fields) pending.Push(field.Type);
                    break;
                case "Sequence":
                case "Optional":
                case "Array":
                    pending.Push(node.Value);
                    break;
                // Boxes are reference types and already break native value cycles.
            }
        }
        return false;
    }

    // This is CLR storage, independent of the wire layout resolved by Rust.
    // Only unmanaged fields overlap. References and memory owners never do.
    private sealed class NativeStorage(Emitter emitter, NativeUnion owner)
    {
        public readonly Dictionary<(string Type, int Offset), string> Fields = new();
        // Managed arrays cannot overlap numeric storage. Recursive arrays use
        // internal element storage; acyclic arrays retain their native type.
        public readonly Dictionary<string, List<string>> Values = new();
        private Dictionary<string, int> valueCounts = new();
        public int Bytes, Alignment = 1, References, Memories;
        public int Size => (Bytes + Alignment - 1) & -Alignment;

        public Part BuildRoot(Node node)
        {
            var part = new Part(node) { Slot = Number("U32") };
            var start = (Bytes, References, Memories);
            var valueStart = new Dictionary<string, int>(valueCounts);
            var end = start;
            foreach (var variant in node.Variants)
            {
                (Bytes, References, Memories) = start;
                valueCounts = new Dictionary<string, int>(valueStart);
                var fields = new List<(Field, Part)>();
                foreach (var field in node.Fields.Where(field => field.Variant == variant.Tag))
                    fields.Add((field, Build(emitter.Node(field.Type))));
                part.Variants.Add((variant, fields));
                end = (Math.Max(end.Item1, Bytes), Math.Max(end.Item2, References), Math.Max(end.Item3, Memories));
            }
            (Bytes, References, Memories) = end;
            valueCounts = Values.ToDictionary(pair => pair.Key, pair => pair.Value.Count);
            return part;
        }

        public Part Build(Node node)
        {
            var part = new Part(node);
            if (node.Scalar) part.Slot = Number(node.Kind);
            else switch (node.Kind)
            {
                case "String":
                case "Boxed":
                    part.Slot = "Reference" + References++;
                    break;
                case "Sequence":
                    part.Slot = "Memory" + Memories++;
                    break;
                case "Array":
                    // Nested empty arrays also carry no native payload state.
                    if (node.Size == 0) break;
                    if (emitter.RecursiveArray(node, owner.Node))
                    {
                        part.Array = owner.ArrayLayout(node);
                        part.Slot = ValueSlot(part.Array.Type);
                        break;
                    }
                    var type = emitter.Input(node.Id);
                    part.Slot = node.NativeLayout
                        ? DataField(type, node.Size, node.Alignment, "Array" + node.Id + "At")
                        : ValueSlot(type);
                    break;
                case "Record":
                    foreach (var field in node.Fields)
                        part.Fields.Add((field, Build(emitter.Node(field.Type))));
                    break;
                case "Optional":
                    var child = emitter.Node(node.Value);
                    if (child.Kind is not ("String" or "Boxed")) part.Slot = Number("Bool");
                    part.Child = Build(child);
                    break;
                case "Enum":
                    var union = emitter.NativeLayout(node);
                    part.Union = union;
                    part.Slot = DataField(emitter.NativeType(node) + "." + union.NumericType,
                        union.Storage.Size, union.Storage.Alignment, "Enum" + node.Id + "At");
                    part.ReferenceStart = References;
                    part.MemoryStart = Memories;
                    References += union.Storage.References;
                    Memories += union.Storage.Memories;
                    foreach (var pair in union.Storage.Values)
                        foreach (var unused in pair.Value) part.ValueSlots.Add(ValueSlot(pair.Key));
                    break;
            }
            return part;
        }

        private string Number(string kind)
        {
            int size = kind switch
            {
                "Bool" or "U8" or "I8" => 1,
                "U16" or "I16" => 2,
                "U32" or "I32" or "F32" => 4,
                _ => 8,
            };
            return DataField(Scalar(kind), size, size, kind + "At");
        }

        private string ValueSlot(string type)
        {
            valueCounts.TryGetValue(type, out int index);
            valueCounts[type] = index + 1;
            if (!Values.TryGetValue(type, out var fields)) Values.Add(type, fields = new());
            if (index == fields.Count) fields.Add("Value" + Values.Sum(pair => pair.Value.Count));
            return fields[index];
        }

        private string DataField(string type, int size, int alignment, string prefix)
        {
            int offset = (Bytes + alignment - 1) & -alignment;
            Bytes = offset + size;
            Alignment = Math.Max(Alignment, alignment);
            var key = (type, offset);
            if (!Fields.TryGetValue(key, out var name)) Fields.Add(key, name = prefix + offset);
            return "Data." + name;
        }
    }

    private sealed class Part(Node node)
    {
        public readonly Node Node = node;
        public string Slot = "";
        public Part? Child;
        public NativeUnion? Union;
        public NativeArray? Array;
        public int ReferenceStart, MemoryStart;
        public readonly List<string> ValueSlots = new();
        public readonly List<(Field Field, Part Value)> Fields = new();
        public readonly List<(Variant Variant, List<(Field Field, Part Value)> Fields)> Variants = new();
    }

    private void NativeEnum(Node node)
    {
        var union = NativeLayout(node);
        var storage = union.Storage;
        var root = union.Root;
        Open($"public readonly partial struct {node.Name}");
        Line("// Mutually exclusive payloads share numeric, reference, and memory-owner slots.");
        Line($"private readonly {union.StorageType} {union.StorageField};");
        Line($"private {node.Name}({union.StorageType} storage) => {union.StorageField} = storage;");
        Line($"public uint {Name("tag")} => {union.StorageField}.{root.Slot};");
        Line();
        foreach (var (variant, fields) in root.Variants)
        {
            NativePayload(union, variant, fields);
            var arguments = fields.Select(field => $"{Input(field.Field.Type)} @{field.Field.Name}");
            var locals = new NameScope(NativeTypeNames().Concat(fields.Select(field => field.Field.Name)).Append(union.StorageType));
            var storageLocal = locals.Allocate("storage");
            Docs(variant.Docs);
            Open(NativeArguments($"public static {node.Name} @{variant.Factory}(", arguments, ")"));
            Line($"{union.StorageType} {storageLocal} = default;");
            Line($"{storageLocal}.{root.Slot} = {variant.Tag};");
            foreach (var (field, part) in fields) NativeWrite(part, "@" + field.Name, storageLocal, locals);
            Line($"return new({storageLocal});");
            Close();
            Line();
        }
        NativeTransfers(union);
        NativeStorageTypes(storage, union.StorageType, union.NumericType, "private");
        foreach (var array in union.Arrays.Values)
            NativeStorageTypes(array.Storage, array.StorageType, array.NumericType, "internal");
        Close();
        Line();
    }

    private void NativeStorageTypes(NativeStorage storage, string storageType, string numericType, string visibility)
    {
        Open($"{visibility} struct {storageType}");
        if (storage.Fields.Count != 0) Line($"public {numericType} Data;");
        for (int i = 0; i < storage.References; ++i) Line($"public object? Reference{i};");
        for (int i = 0; i < storage.Memories; ++i) Line($"public global::Intrlay.MemorySlot Memory{i};");
        foreach (var pair in storage.Values)
            foreach (var field in pair.Value) Line($"public {pair.Key} {field};");
        Close();
        Line();
        if (storage.Fields.Count == 0) return;
        Line($"[global::System.Runtime.InteropServices.StructLayout(global::System.Runtime.InteropServices.LayoutKind.Explicit, Size = {storage.Size})]");
        Open($"internal struct {numericType}");
        foreach (var pair in storage.Fields.OrderBy(pair => pair.Key.Offset).ThenBy(pair => pair.Key.Type))
        {
            Line($"[global::System.Runtime.InteropServices.FieldOffset({pair.Key.Offset})]");
            Line($"public {pair.Key.Type} {pair.Value};");
        }
        Close();
        Line();
    }

    private void NativePayload(NativeUnion union, Variant variant, List<(Field Field, Part Value)> fields)
    {
        var type = union.Node.Name + variant.Name;
        Docs(variant.Docs);
        if (fields.Count == 0)
        {
            ExpressionGetter($"public {type} @{variant.Name}", $"{Name("tag")} == {variant.Tag} ? new {type}() : default");
            return;
        }
        Open($"public {type} @{variant.Name}");
        Open("get");
        Line($"if ({Name("tag")} != {variant.Tag}) return default;");
        var locals = new NameScope(NativeTypeNames().Concat(schema.Names.Values).Append(union.StorageField));
        var arguments = fields.Select(field => NativeRead(field.Value, union.StorageField, locals)).ToArray();
        Line(NativeArguments($"return new {type}(", arguments, ");"));
        Close();
        Close();
    }

    private void NativeTransfers(NativeUnion union)
    {
        var parameters = new List<(string Type, string Name, string Field)> { (union.NumericType, "data", "Data") };
        for (int i = 0; i < union.Storage.References; ++i) parameters.Add(("object?", "reference" + i, "Reference" + i));
        for (int i = 0; i < union.Storage.Memories; ++i) parameters.Add(("global::Intrlay.MemorySlot", "memory" + i, "Memory" + i));
        foreach (var pair in union.Storage.Values)
            foreach (var field in pair.Value) parameters.Add((pair.Key, field, field));
        Line("// Nested enums transfer their existing storage without reading payloads or memory owners.");
        Open(NativeArguments($"internal void {union.CopyStorage}(",
            parameters.Select(parameter => $"out {parameter.Type} {parameter.Name}"), ")"));
        foreach (var parameter in parameters) Line($"{parameter.Name} = this.{union.StorageField}.{parameter.Field};");
        Close();
        Line();
        Open(NativeArguments($"internal static {union.Node.Name} {union.ReadStorage}(",
            parameters.Select((parameter, index) => $"{(index == 0 ? "in " : "")}{parameter.Type} {parameter.Name}"), ")"));
        var locals = new NameScope(NativeTypeNames().Concat(parameters.Select(parameter => parameter.Name)).Append(union.StorageType));
        var storage = locals.Allocate("storage");
        Line($"{union.StorageType} {storage} = default;");
        foreach (var parameter in parameters) Line($"{storage}.{parameter.Field} = {parameter.Name};");
        Line($"return new({storage});");
        Close();
        Line();
    }

    private void NativeWrite(Part part, string value, string storage, NameScope locals)
    {
        var node = part.Node;
        if (node.Scalar) Line($"{storage}.{part.Slot} = {value};");
        else switch (node.Kind)
        {
            case "String":
            case "Boxed":
                Line($"{storage}.{part.Slot} = {value};");
                break;
            case "Sequence":
                Line($"{storage}.{part.Slot} = global::Intrlay.MemorySlot.From({value});");
                break;
            case "Array":
                if (node.Size == 0) break;
                if (part.Array is null) Line($"{storage}.{part.Slot} = {value};");
                else
                {
                    var index = locals.Allocate("index");
                    Open($"for (int {index} = 0; {index} < {node.Argument}; ++{index})");
                    NativeWrite(part.Array.Element, $"{value}[{index}]", $"{storage}.{part.Slot}[{index}]", locals);
                    Close();
                }
                break;
            case "Record":
                foreach (var (field, child) in part.Fields) NativeWrite(child, $"{value}.@{field.Name}", storage, locals);
                break;
            case "Optional":
                if (part.Slot.Length == 0) NativeWrite(part.Child!, value, storage, locals);
                else
                {
                    Line($"{storage}.{part.Slot} = {value}.HasValue;");
                    Open($"if ({value}.HasValue)");
                    NativeWrite(part.Child!, value + "!.Value", storage, locals);
                    Close();
                }
                break;
            case "Enum":
                Line(NativeArguments($"{value}.@{part.Union!.CopyStorage}(",
                    NativeSlots(part, storage).Select(slot => "out " + slot), ");"));
                break;
        }
    }

    private string NativeRead(Part part, string storage, NameScope locals)
    {
        var node = part.Node;
        var slot = storage + "." + part.Slot;
        if (node.Scalar) return slot;
        switch (node.Kind)
        {
            case "Unit": return "default(global::System.ValueTuple)";
            case "String":
            case "Boxed": return $"({Input(node.Id)}){slot}!";
            case "Sequence": return $"{slot}.As<{Input(node.Value)}>()";
            case "Array":
                if (node.Size == 0) return $"default({Input(node.Id)})";
                if (part.Array is null) return slot;
                var array = locals.Allocate("array");
                Line($"{Input(node.Id)} {array} = default;");
                var index = locals.Allocate("index");
                Open($"for (int {index} = 0; {index} < {node.Argument}; ++{index})");
                var element = NativeRead(part.Array.Element, $"{slot}[{index}]", locals);
                Line($"{array}[{index}] = {element};");
                Close();
                return array;
            case "Record":
                return $"new {node.Name}({string.Join(", ", part.Fields.Select(field => NativeRead(field.Value, storage, locals)))})";
            case "Optional":
                if (part.Slot.Length == 0) return NativeRead(part.Child!, storage, locals);
                var optional = locals.Allocate("optional");
                Line($"{Input(node.Id)} {optional} = default;");
                Open($"if ({slot})");
                var payload = NativeRead(part.Child!, storage, locals);
                Line($"{optional} = new {Input(node.Id)}({payload});");
                Close();
                return optional;
            case "Enum":
                var nested = locals.Allocate("nested");
                Line(NativeArguments($"var {nested} = {NativeType(node)}.@{part.Union!.ReadStorage}(",
                    NativeSlots(part, storage).Select((value, index) => index == 0 ? "in " + value : value), ");"));
                return nested;
            default: throw new InvalidOperationException("Unsupported native storage kind " + node.Kind);
        }
    }

    private static IEnumerable<string> NativeSlots(Part part, string storage)
    {
        yield return storage + "." + part.Slot;
        for (int i = 0; i < part.Union!.Storage.References; ++i) yield return storage + ".Reference" + (part.ReferenceStart + i);
        for (int i = 0; i < part.Union.Storage.Memories; ++i) yield return storage + ".Memory" + (part.MemoryStart + i);
        foreach (var slot in part.ValueSlots) yield return storage + "." + slot;
    }

    private string NativeArguments(string prefix, IEnumerable<string> arguments, string suffix)
    {
        var values = arguments.ToArray();
        var inline = prefix + string.Join(", ", values) + suffix;
        return values.Length == 0 || inline.Length + indentation * 4 <= 116
            ? inline
            : prefix + "\n    " + string.Join(",\n    ", values) + suffix;
    }
}
