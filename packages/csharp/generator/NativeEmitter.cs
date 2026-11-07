using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void Native(Node node)
    {
        if (node.Kind == "Record")
        {
            NativeRecord(node.Name, node.Fields, node);
        }
        else if (node.Kind == "Enum")
        {
            foreach (var variant in node.Variants)
                NativeRecord(node.Name + variant.Name, node.Fields.Where(field => field.Variant == variant.Tag).ToList());

            NativeEnum(node);
        }
    }

    private void NativeRecord(string name, List<Field> fields, Node? record = null)
    {
        if (fields.Count == 0) return;
        var storage = RecordStorageNames(name, fields, record);
        if (record?.NativeLayout == true)
            Line($"[global::System.Runtime.InteropServices.StructLayout(global::System.Runtime.InteropServices.LayoutKind.Explicit, Size = {record.Size})]");
        Open($"public readonly partial struct {name}");
        foreach (var field in fields)
        {
            if (storage.TryGetValue(field, out var backing))
            {
                StoredRecordField(field, backing);
            }
            else
            {
                Docs(field.Docs);
                if (record?.NativeLayout == true)
                    Line($"[field: global::System.Runtime.InteropServices.FieldOffset({field.Offset})]");
                Line($"public {Input(field.Type)} @{field.Name} {{ get; }}");
            }
            Line();
        }
        var parameters = string.Join(", ", fields.Select(field => $"{Input(field.Type)} @{field.Name}"));
        Open($"public {name}({parameters})");
        foreach (var field in fields) Line($"this.@{field.Name} = @{field.Name};");
        Close();
        Close();
        Line();
    }
}
