using System;
using System.IO;
using IntrlayGenerated.Schema;

static class Program
{
    static void Check(bool condition)
    {
        if (!condition) throw new Exception("whole alternative");
    }

    static void Main()
    {
        var view = WholeChoiceView.Create(WholeChoice.FromValue(new Child(31, 37)));
        Check(view.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/cpp-metadata/whole.bin")));
        var alias = new WholeChoiceView(view.Bytes);
        Check(alias.Value!.Value.Value == 31);
        view.SetValue(new Child(41, 43));
        Check(alias.AsSpan().Value.Value.Marker == 43);
        FixedArray2<uint?> nested = default;
        nested[0] = 41;
        view.AsSpan().Replace(WholeChoice.FromNested(nested));
        Check(alias.Value is null && alias.Nested!.Value[0] == 41 && alias.Nested.Value[1] is null);
        Check(view.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/cpp-metadata/nested.bin")));
        view.Replace(WholeChoice.FromEmpty(default));
        Check(alias.Nested is null && alias.Empty.HasValue);
        view.Replace(WholeChoice.FromValue(new Child(47, 53)));
        Check(alias.Empty is null && alias.Value!.Value.Marker == 53);
    }
}
