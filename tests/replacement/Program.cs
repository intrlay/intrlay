using System;
using Intrlay;
using IntrlayGenerated.Replacement;

static class Program
{
    static void Check(bool value) { if (!value) throw new Exception("in-place replacement"); }

    static void Main()
    {
        var initial = new MessageInput(123, new FixedInput(null, new Optional<LeafInput?>(null), ChoiceInput.FromEmpty()), new LeafInput?[] { null }, "preserved", 456);
        var view = MessageView.Create(initial);
        var alias = new MessageView(view.Bytes);
        var fixedView = alias.Fixed;
        Check(fixedView.Maybe is null && fixedView.Nested.Value is null);
        Check(fixedView.Choice.ChildLeaf is null);
        var leaf = new LeafInput(7, 8);
        view.Fixed.SetMaybe(leaf);
        Check(fixedView.Maybe!.Value.Number == 7);
        view.Fixed.SetNested(new Optional<LeafInput?>(leaf));
        Check(fixedView.Nested.Value!.Value.Number == 7);
        view.Fixed.Choice.Replace(ChoiceInput.FromChild(leaf));
        Check(fixedView.Choice.ChildLeaf!.Value.Number == 7);
        FixedArray3<uint> numbers = default;
        numbers[0] = 1; numbers[1] = 2; numbers[2] = 3;
        view.Fixed.Choice.AsSpan().Replace(ChoiceInput.FromNumbers(numbers));
        Check(fixedView.Choice.ChildLeaf is null && fixedView.Choice.NumbersValues!.Value[2] == 3);
        view.Fixed.SetMaybe(null);
        view.Fixed.SetNested(new Optional<LeafInput?>(null));
        Check(fixedView.Maybe is null && fixedView.Nested.Value is null);
        view.Fixed.SetNested(default);
        Check(!fixedView.Nested.HasValue);
        view.Items.Set(0, leaf);
        Check(alias.Items[0]!.Value.Number == 7);
        view.Items.Set(0, null);
        Check(alias.Items[0] is null);
        var scalar = MaybeScalarView.Create(null);
        scalar.Set(42);
        Check(scalar.Value == 42);
        scalar.Set(null);
        Check(scalar.Value is null);
        Span<byte> bytes = stackalloc byte[FixedView.ByteSize];
        var scoped = FixedSpanView.CreateInPlace(bytes, initial.Fixed);
        scoped.SetMaybe(leaf);
        Check(scoped.Maybe.HasValue && scoped.Maybe.Value.Number == 7);
        var emptyPayloads = EmptyPayloadsView.Create(new EmptyPayloadsInput(default, default, 1));
        var emptyAlias = new EmptyPayloadsView(emptyPayloads.Bytes);
        var replacement = new EmptyPayloadsInput(default, default, 2);
        emptyPayloads.Replace(replacement);
        Check(emptyAlias.Number == 2 && emptyAlias.Bytes.Span.SequenceEqual(new byte[] { 2, 0, 0, 0 }));
        Check(emptyAlias.Empty.Count == 0 && emptyAlias.Nested.Count == 2 && emptyAlias.Nested[1].Count == 0);
        emptyPayloads.AsSpan().Replace(new EmptyPayloadsInput(default, default, 3));
        Check(emptyAlias.Number == 3);
        view.Fixed.Choice.Replace(ChoiceInput.FromEmptyArrays(replacement));
        Check(fixedView.Choice.EmptyArraysData!.Value.Number == 2);
        view.Fixed.Choice.AsSpan().Replace(ChoiceInput.FromEmptyArrays(new EmptyPayloadsInput(default, default, 4)));
        Check(fixedView.Choice.EmptyArraysData!.Value.Number == 4);
        Check(alias.Before == 123 && alias.After == 456 && alias.Text == "preserved");
        Console.WriteLine("C#: Memory/Span views, variant and presence replacement passed");
    }
}
