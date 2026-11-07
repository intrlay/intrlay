using System;
using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    // A record normally keeps its public property type as native storage. CLR
    // generic value-type cycles through extra wrappers require MemorySlot instead.
    // Enum storage already erases collection owners and breaks those cycles.
    private Dictionary<Field, string> RecordStorageNames(string name, List<Field> fields, Node? record)
    {
        var storage = new Dictionary<Field, string>();
        if (record is null) return storage;

        var members = new HashSet<string>(fields.Select(field => field.Name), StringComparer.Ordinal)
        {
            name, "Equals", "Finalize", "GetHashCode", "GetType", "MemberwiseClone", "ReferenceEquals", "ToString",
        };
        foreach (var field in fields)
        {
            if (!NeedsErasedRecordMemory(record, field.Type)) continue;
            string backing = field.NativeStorageName;
            if (!PortableIdentifier(backing) || !members.Add(backing))
                throw new InvalidOperationException($"{name}: recursive collection storage name '{backing}' is invalid or conflicts with another member");
            storage.Add(field, backing);
        }
        return storage;
    }

    private static bool PortableIdentifier(string name)
    {
        static bool Letter(char value) => value is (>= 'a' and <= 'z') or (>= 'A' and <= 'Z') or '_';
        if (name.Length == 0 || !Letter(name[0])) return false;
        return name.All(value => Letter(value) || value is >= '0' and <= '9');
    }

    private bool NeedsErasedRecordMemory(Node owner, int field)
    {
        if (!ContainsInlineSequence(field)) return false;
        var pending = new Stack<(int Type, bool Wrapped, bool Sequence)>();
        var visited = new HashSet<(int, bool, bool)>();
        pending.Push((field, false, false));
        while (pending.Count != 0)
        {
            var entry = pending.Pop();
            if (entry.Type == owner.Id && entry.Wrapped) return true;
            if (!visited.Add(entry)) continue;
            var node = Node(entry.Type);
            switch (node.Kind)
            {
                case "Record":
                    foreach (var child in node.Fields) pending.Push((child.Type, entry.Wrapped, false));
                    break;
                case "Sequence":
                    pending.Push((node.Value, entry.Wrapped || entry.Sequence, true));
                    break;
                case "Optional":
                case "Array":
                    pending.Push((node.Value, true, entry.Sequence));
                    break;
                // A Box is a reference type. Enum storage also breaks the cycle.
            }
        }
        return false;
    }

    private bool ContainsInlineSequence(int id)
    {
        var node = Node(id);
        return node.Kind switch
        {
            "Sequence" => true,
            "Optional" or "Array" => ContainsInlineSequence(node.Value),
            _ => false,
        };
    }

    private string RecordStorageType(int id)
    {
        var node = Node(id);
        return node.Kind switch
        {
            "Sequence" => "global::Intrlay.MemorySlot",
            "Array" => $"{node.ArrayName}<{RecordStorageType(node.Value)}>",
            "Optional" => Node(node.Value).Kind switch
            {
                "String" or "Boxed" => RecordStorageType(node.Value) + "?",
                "Optional" => $"global::Intrlay.Optional<{RecordStorageType(node.Value)}>",
                _ => $"global::System.Nullable<{RecordStorageType(node.Value)}>",
            },
            _ => Input(id),
        };
    }

    private void StoredRecordField(Field field, string backing)
    {
        Line($"private readonly {RecordStorageType(field.Type)} @{backing};");
        Docs(field.Docs);
        Open($"public {Input(field.Type)} @{field.Name}");
        foreach (bool writing in new[] { false, true })
        {
            Open(writing ? "private init" : "get");
            int next = 0;
            var value = ConvertRecordStorage(field.Type, writing ? "value" : $"this.@{backing}", writing, ref next);
            Line(writing ? $"this.@{backing} = {value};" : $"return {value};");
            Close();
        }
        Close();
    }

    // Conversions preserve the original memory owner, offset, and count. Only
    // inline wrappers are copied; no arrays or boxes are allocated at runtime.
    private string ConvertRecordStorage(int id, string source, bool writing, ref int next)
    {
        var node = Node(id);
        if (node.Kind == "Sequence")
            return writing ? $"global::Intrlay.MemorySlot.From({source})" : $"{source}.As<{Input(node.Value)}>()";
        if (!ContainsInlineSequence(id)) return source;

        int number = next++;
        string result = "result" + number;
        string type = writing ? RecordStorageType(id) : Input(id);
        Line($"{type} {result} = default;");
        if (node.Kind == "Array" && node.Argument == 0) return result;

        if (node.Kind == "Optional")
        {
            Open($"if ({source}.HasValue)");
            // Nullable flow analysis does not retain repeated inline-array indexing.
            var value = ConvertRecordStorage(node.Value, source + "!.Value", writing, ref next);
            Line($"{result} = new {type}({value});");
        }
        else if (node.Kind == "Array")
        {
            string index = "index" + number;
            Open($"for (int {index} = 0; {index} < {node.Argument}; ++{index})");
            var value = ConvertRecordStorage(node.Value, $"{source}[{index}]", writing, ref next);
            Line($"{result}[{index}] = {value};");
        }
        else
        {
            throw new InvalidOperationException("Only inline wrappers enclose a stored collection");
        }
        Close();
        return result;
    }
}
