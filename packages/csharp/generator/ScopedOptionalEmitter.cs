using System.Collections.Generic;
using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private readonly Dictionary<(int Type, bool ReadOnly), string> scopedOptionalNames = new();
    private readonly List<(int Type, bool ReadOnly, string Name)> scopedOptionals = new();
    private NameScope? scopedNames;

    private bool IsScopedOutput(int id)
    {
        var node = Node(id);
        return node.Kind switch
        {
            "Record" or "Enum" or "Array" or "Sequence" => true,
            "Optional" or "Boxed" => IsScopedOutput(node.Value),
            _ => false,
        };
    }

    private string ScopedOptionalName(int id, bool readOnly)
    {
        var key = (id, readOnly);
        if (scopedOptionalNames.TryGetValue(key, out var name)) return name;
        scopedNames ??= new NameScope(NativeTypeNames()
            .Concat(schema.Nodes.Values.Select(node => node.ArrayName))
            .Concat(schema.Views.SelectMany(view => new[] { view.Name, view.SpanName }))
            .Append(schema.Name)
            .Concat(schema.Names.Values));
        var payload = OptionalPayload(id, input: false);
        var baseName = payload.Kind == "Optional" ? ScopedOptionalName(payload.Value, readOnly) : Output(payload.Id, readOnly);
        name = scopedNames.Allocate(baseName + "Optional");
        scopedOptionalNames.Add(key, name);
        scopedOptionals.Add((id, readOnly, name));
        return name;
    }

    private void ScopedOptionals()
    {
        // Nullable<T> cannot contain ref structs on the minimum C#/.NET target.
        // One wrapper per distinct payload view retains nested Some(None) states.
        scopedView = true;
        for (int i = 0; i < scopedOptionals.Count; ++i)
        {
            var optional = scopedOptionals[i];
            var type = Output(optional.Type, optional.ReadOnly);
            Docs("An optional scoped view. Presence is independent of any optional state inside its payload.");
            Open($"public readonly ref struct {optional.Name}");
            Line($"private readonly {type} {Name("optional_storage")};");
            Line($"public bool {Name("optional_has_value")} {{ get; }}");
            Line();
            Inline();
            Open($"internal {optional.Name}({type} value)");
            Line($"{Name("optional_storage")} = value;");
            Line($"{Name("optional_has_value")} = true;");
            Close();
            Line();
            ExpressionGetter($"public {type} {Name("optional_value")}",
                $"{Name("optional_has_value")} ? {Name("optional_storage")} : throw new global::System.InvalidOperationException(\"Optional has no value.\")");
            Inline();
            Line($"public {type} {Name("get_value_or_default")}() => {Name("optional_storage")};");
            Close();
            Line();
        }
        scopedView = false;
    }
}
