using System;
using System.IO;
using Composition.ValueEnums;

static class ValueEnumTests
{
    public static void Run()
    {
        FixedArray2<SparseStateInput> fixedStates = default;
        fixedStates[0] = SparseStateInput.First;
        fixedStates[1] = SparseStateInput.Last;
        var choice = ValueChoiceInput.FromPresent(
            SparseStateInput.Last,
            SparseStateInput.Middle,
            fixedStates,
            new SparseStateInput[] { SparseStateInput.Middle, SparseStateInput.First },
            "hello");
        if (choice.Present.State != SparseStateInput.Last || choice.Present.Optional != SparseStateInput.Middle || choice.Present.Fixed[1] != SparseStateInput.Last)
            throw new Exception("native enum union storage lost its sparse tags");
        var input = new ValueEnumsInput(choice);
        var view = ValueEnumsView.Create(in input);
        if (!view.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/compositions/value-enums.bin")))
            throw new Exception("native enum bytes differ from Rust");
        if (view.Choice.PresentState?.Tag != uint.MaxValue || view.Choice.PresentOptional.Value?.Tag != 37 || view.Choice.PresentFixed?[1].Tag != uint.MaxValue || view.Choice.PresentStates?[1].Tag != 3)
            throw new Exception("nested native enum fields lost their sparse tags");
        var state = SparseStateView.Create(SparseStateInput.Last);
        state.Replace(SparseStateInput.Middle);
        if (state.Tag != 37 || StateAliasView.Create(SparseStateInput.First).Tag != 3)
            throw new Exception("native enum replacement or export alias failed");
        Console.WriteLine("C# native enum inputs passed");
    }
}
