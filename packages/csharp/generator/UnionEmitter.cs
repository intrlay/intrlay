using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private void UnionMembers(NativeUnion union)
    {
        // Fixed language-protocol names live in a separate scope. A schema can
        // still have variants named Value, HasValue, Create, or TryGetValue.
        Docs("Supplies C# union conversions and non-boxing pattern matching for this input.");
        Open("public interface IUnionMembers : global::System.Runtime.CompilerServices.IUnion");
        foreach (var (variant, fields) in union.Root.Variants)
        {
            var payload = $"global::{schema.Namespace}.{variant.InputName}";
            var arguments = fields.Select(field => $"value.@{field.Field.Name}");
            Docs($"Constructs the {variant.Name} case without boxing its payload.");
            Inline();
            Line(NativeArguments($"public static {NativeType(union.Node)} Create({payload} value) => {NativeType(union.Node)}.@{variant.Factory}(", arguments, ");"));
            Line();
        }
        Docs("Whether the input contains a declared case, including an empty case.");
        Line("bool HasValue { get; }");
        Line();
        foreach (var variant in union.Node.Variants)
        {
            Docs($"Retrieves the {variant.Name} case without boxing, or returns false.");
            Line($"bool TryGetValue(out global::{schema.Namespace}.{variant.InputName} value);");
            Line();
        }
        Close();
        Line();

        // The object projection is required by the union protocol, but typed
        // patterns use the explicit accessors below and never call it.
        Open("object? global::System.Runtime.CompilerServices.IUnion.Value");
        Open("get");
        Line($"return this.{Name("tag")} switch");
        Open("");
        foreach (var variant in union.Node.Variants)
            Line($"{variant.Tag} => this.@{variant.Name},");
        Line("_ => null,");
        --indentation;
        Line("};");
        Close();
        Close();
        Line();

        Open("bool IUnionMembers.HasValue");
        Inline();
        Line($"get => this.{Name("tag")} is {string.Join(" or ", union.Node.Variants.Select(variant => variant.Tag))};");
        Close();
        Line();
        foreach (var variant in union.Node.Variants)
        {
            Inline();
            Open($"bool IUnionMembers.TryGetValue(out global::{schema.Namespace}.{variant.InputName} value)");
            Line($"value = this.@{variant.Name};");
            Line($"return this.{Name("tag")} == {variant.Tag};");
            Close();
            Line();
        }
    }
}
