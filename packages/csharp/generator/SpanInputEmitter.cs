using System.Linq;

namespace Intrlay.Generator;

internal sealed partial class Emitter
{
    private bool SpanCollection(Node node) => node.Kind is "Sequence" or "Array"
        && (Node(node.Value).Scalar || Node(node.Value).NativeLayout
            || Node(node.Value).ValueEnum || Node(node.Value).Kind == "Unit");

    private bool SpanCodec(Node node) => node.Kind == "String"
        || SpanCollection(node) || node.SpanInputName.Length != 0;

    private string SpanInputType(Node node)
    {
        if (node.Kind == "String") return "global::System.ReadOnlySpan<char>";
        if (SpanCollection(node)) return $"global::System.ReadOnlySpan<{Input(node.Value)}>";
        return node.SpanInputName.Length != 0 ? node.SpanInputName : Input(node.Id);
    }

    private void SpanInput(Node node)
    {
        Docs(node.Docs + "\nBorrows flat payloads for synchronous encoding, including stack-backed spans. The source and destination must not overlap.");
        Open($"public readonly ref struct {node.SpanInputName}");
        foreach (var field in node.Fields)
        {
            Docs(field.Docs);
            Line($"public {SpanInputType(Node(field.Type))} @{field.Name} {{ get; }}");
            Line();
        }
        var parameters = string.Join(", ", node.Fields.Select(field => $"{SpanInputType(Node(field.Type))} @{field.Name}"));
        Inline();
        Open($"public {node.SpanInputName}({parameters})");
        foreach (var field in node.Fields) Line($"this.@{field.Name} = @{field.Name};");
        Close();
        if (node.Encode && CustomSource(node)) SourceFactory(node.SpanInputName, node.Fields, scoped: true);
        Close();
        Line();
    }
}
