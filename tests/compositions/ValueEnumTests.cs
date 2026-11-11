using System;
using System.IO;
using Composition.ValueEnums;

static class ValueEnumTests
{
    public static void Run()
    {
        FixedArray2<SparseState> fixedStates = default;
        fixedStates[0] = SparseState.First;
        fixedStates[1] = SparseState.Last;
        var choice = ValueChoiceInput.FromPresent(
            SparseState.Last,
            SparseState.Middle,
            fixedStates,
            new SparseState[] { SparseState.Middle, SparseState.First },
            "hello");
        if (choice.Present.State != SparseState.Last || choice.Present.Optional != SparseState.Middle || choice.Present.Fixed[1] != SparseState.Last)
            throw new Exception("native enum union storage lost its sparse tags");
        var input = new ValueEnumsInput(choice);
        var view = ValueEnumsView.Create(in input);
        if (!view.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/compositions/value-enums.bin")))
            throw new Exception("native enum bytes differ from Rust");
        if (view.Choice.PresentState != SparseState.Last || view.Choice.PresentOptional.Value != SparseState.Middle || view.Choice.PresentFixed?[1] != SparseState.Last || view.Choice.PresentStates?[1] != SparseState.First)
            throw new Exception("nested native enum fields lost their sparse tags");
        var scoped = view.AsSpan().Choice;
        if (scoped.PresentState != SparseState.Last || scoped.PresentOptional.Value != SparseState.Middle || scoped.PresentFixed.Value[1] != SparseState.Last || scoped.PresentStates.Value[1] != SparseState.First)
            throw new Exception("scoped native enum fields lost their sparse tags");
        var readOnly = new ValueEnumsViewReadOnly(view.Bytes).Choice;
        if (readOnly.PresentState != SparseState.Last || readOnly.PresentStates?[1] != SparseState.First)
            throw new Exception("read-only enum getters did not return enum values");
        scoped.SetPresentState(SparseState.Middle);
        scoped.SetPresentOptional(null);
        scoped.PresentFixed.Value.Set(1, SparseState.First);
        scoped.PresentStates.Value.Set(1, SparseState.Last);
        if (view.Choice.PresentState != SparseState.Middle || view.Choice.PresentOptional.Value is not null || view.Choice.PresentFixed?[1] != SparseState.First || view.Choice.PresentStates?[1] != SparseState.Last)
            throw new Exception("enum setters did not update the borrowed storage");
        var state = SparseStateView.Create(SparseState.Last);
        state.Replace(SparseState.Middle);
        if (state.Tag != SparseState.Middle || StateAliasView.Create(SparseState.First).Tag != SparseState.First)
            throw new Exception("native enum replacement or export alias failed");
        var stateSpan = SparseStateSpanView.CreateInPlace(stackalloc byte[4], SparseState.Last);
        stateSpan.Replace(SparseState.Middle);
        if (stateSpan.Tag != SparseState.Middle || new SparseStateSpanViewReadOnly(stateSpan.Bytes).Tag != SparseState.Middle)
            throw new Exception("exported enum span views lost their typed tags");
        NestedEnums();
        Console.WriteLine("C# native enum values passed");
    }

    private static void NestedEnums()
    {
        var assembly = typeof(NestedEnumValuesView).Assembly;
        foreach (var name in new[] { "PaddedStateView", "PaddedStateViewReadOnly", "PaddedStateSpanView", "PaddedStateSpanViewReadOnly" })
            if (assembly.GetType("Composition.ValueEnums." + name) is not null)
                throw new Exception("transitive enums must not emit standalone views");

        FixedArray2<PaddedState> fixedStates = default;
        fixedStates[0] = PaddedState.First;
        fixedStates[1] = PaddedState.Last;
        var input = new NestedEnumValuesInput(
            PaddedState.Last,
            PaddedState.Middle,
            new(PaddedState.First),
            fixedStates,
            new PaddedState[] { PaddedState.Middle, PaddedState.First },
            new EnumEntryInput[] { new(PaddedState.Middle, PaddedState.First) },
            fixedStates,
            PaddedState.First,
            PaddedState.Middle);
        var view = NestedEnumValuesView.Create(in input);
        if (!view.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/compositions/nested-enums.bin")))
            throw new Exception("padded enum bytes differ from Rust");
        var scoped = view.AsSpan();
        var readOnly = new NestedEnumValuesViewReadOnly(view.Bytes);
        var scopedReadOnly = readOnly.AsSpan();
        if (view.State != PaddedState.Last || scoped.State != view.State || readOnly.State != view.State || scopedReadOnly.State != view.State
            || view.Optional != PaddedState.Middle || scoped.Optional != view.Optional || readOnly.Optional != view.Optional || scopedReadOnly.Optional != view.Optional
            || view.Boxed != PaddedState.First || scoped.Boxed != view.Boxed || readOnly.Boxed != view.Boxed || scopedReadOnly.Boxed != view.Boxed
            || view.Fixed[1] != PaddedState.Last || scoped.Fixed[1] != view.Fixed[1] || readOnly.Fixed[1] != view.Fixed[1] || scopedReadOnly.Fixed[1] != view.Fixed[1]
            || view.States[1] != PaddedState.First || scoped.States[1] != view.States[1] || readOnly.States[1] != view.States[1] || scopedReadOnly.States[1] != view.States[1]
            || view.Indexed[0].State != PaddedState.Middle || scoped.Indexed[0].Optional != PaddedState.First
            || view.Projected[1] != PaddedState.Last || readOnly.Projected[0] != PaddedState.First)
            throw new Exception("nested enum getters must return native enum values");

        scoped.SetState(PaddedState.Middle);
        scoped.SetOptional(null);
        scoped.SetBoxed(PaddedState.Last);
        scoped.Fixed.Set(1, PaddedState.Middle);
        scoped.States.Set(1, PaddedState.Last);
        scoped.Indexed[0].SetState(PaddedState.First);
        scoped.Projected.Set(0, PaddedState.Middle);
        scoped.SetWriteOnly(PaddedState.Last);
        if (view.State != PaddedState.Middle || view.Optional is not null || view.Boxed != PaddedState.Last
            || view.Fixed[1] != PaddedState.Middle || view.States[1] != PaddedState.Last
            || view.Indexed[0].State != PaddedState.First || view.Projected[0] != PaddedState.Middle
            || view.WriteOnly != PaddedState.Last || view.ReadOnly != PaddedState.Middle)
            throw new Exception("nested enum setters did not update their existing storage");
    }
}
