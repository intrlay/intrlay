using System;
using Intrlay;
using NativeViews.Generated;

static class GenericInputTests
{
    public static void Run()
    {
        var sameName = TInput.FromSources(new ZeroedSequence<uint>(2));
        Check(TView.Create(in sameName).Values.Count == 2, "generic parameter names cannot shadow schema names");
        Span<VertexInput> vertices = stackalloc VertexInput[] { new(9, default, default) };
        Span<float> samples = stackalloc float[] { 1, 2, 3 };
        Span<uint> color = stackalloc uint[] { 5, 6, 7 };
        var mixed = FlatPacketSpanInput.FromSources(23, SequenceSpanInput<VertexInput>.From(vertices),
            new ZeroedSequence<float>(3), "generic".AsSpan(), SequenceSpanInput<uint>.From(color), true);
        var expected = new FlatPacketInput(23, vertices.ToArray(), new float[3], "generic", new(color), true);
        byte[] expectedBytes = FlatPacketView.Create(in expected).Bytes.ToArray();
        Span<byte> buffer = stackalloc byte[expectedBytes.Length + 2];
        buffer.Fill(0xa5);
        var result = FlatPacketSpanView.CreateInPlace(buffer[1..^1], mixed);
        Check(result.Bytes.SequenceEqual(expectedBytes) && buffer[0] == 0xa5 && buffer[^1] == 0xa5,
            "mixed borrowed and zeroed sources match canonical bytes with unaligned storage");

        var memory = FlatPacketInput.FromSources(23, new ZeroedSequence<VertexInput>(2),
            SequenceInput<float>.From(samples.ToArray()), "mixed", new ZeroedArray<uint>(), true);
        var memoryExpected = new FlatPacketInput(23, new VertexInput[2], samples.ToArray(), "mixed", default, true);
        Check(FlatPacketView.Create(in memory).Bytes.Span.SequenceEqual(FlatPacketView.Create(in memoryExpected).Bytes.Span),
            "Memory input mixes value copies with zeroed sequences and fixed extents");

        // The public overloads and generic codecs retain the allocation-free reuse path.
        for (int i = 0; i < 1000; ++i) FlatPacketSpanView.CreateInPlace(buffer, mixed);
        long allocated = GC.GetAllocatedBytesForCurrentThread();
        for (int i = 0; i < 1000; ++i) FlatPacketSpanView.CreateInPlace(buffer, mixed);
        Check(GC.GetAllocatedBytesForCurrentThread() == allocated, "generic ref struct sources must not box");
        Check(CreateFromStack().Color[2] == 7 && CreateReadOnlyFromStack().Color[2] == 7,
            "allocating span factories do not retain generic stack sources");
        Check(CreateInPlaceFromStack(buffer).Color[2] == 7
            && CreateReadOnlyInPlaceFromStack(buffer).Color[2] == 7,
            "in-place span factories borrow only the caller's destination");

        var badExtent = FlatPacketSpanInput.FromSources(23, new ZeroedSequence<VertexInput>(0),
            new ZeroedSequence<float>(0), default, SequenceSpanInput<uint>.From(color[..2]), false);
        try { FlatPacketView.Create(in badExtent); throw new Exception("invalid extent accepted"); }
        catch (ArgumentException) { }
        try { _ = new ZeroedSequence<float>(-1); throw new Exception("negative extent accepted"); }
        catch (ArgumentOutOfRangeException) { }

        var changed = FlatPacketInput.FromSources(0, new ZeroedSequence<VertexInput>(0),
            new ShrinkingSource(new ReadCount()), "", new ZeroedArray<uint>(), false);
        try { FlatPacketView.Create(in changed); throw new Exception("changed input extent accepted"); }
        catch (InvalidOperationException) { }

        // Typed variant sources carry one variant without allocating a native union payload.
        var variant = PayloadAgainInput.FromSources(new ZeroedArray<uint>(), new FixedArray2<string>(new[] { "a", "b" }));
        var union = PayloadInput.FromAgain(default, new[] { "a", "b" });
        Check(PayloadView.Create(in variant).Bytes.Span.SequenceEqual(PayloadView.Create(in union).Bytes.Span),
            "generic variant preserves its declared tag and payload");
        var variants = new IntrlaySequenceOfPayloadInput<PayloadAgainInput<ZeroedArray<uint>>>(new[] { variant, variant });
        Check(SequenceOfPayloadView.Create(in variants).Count == 2, "arrays of variant sources retain the union schema");

        Console.WriteLine("C#: generic value/zero sources, scoped reuse, variant arrays, and extent checks passed");
    }

    private sealed class ReadCount { public int Value; }

    private static FlatPacketSpanView CreateFromStack()
    {
        Span<uint> color = stackalloc uint[] { 5, 6, 7 };
        var input = FlatPacketSpanInput.FromSources(23, new ZeroedSequence<VertexInput>(0),
            new ZeroedSequence<float>(0), default, SequenceSpanInput<uint>.From(color), true);
        return FlatPacketSpanView.Create(input);
    }

    private static FlatPacketSpanViewReadOnly CreateReadOnlyFromStack()
    {
        Span<uint> color = stackalloc uint[] { 5, 6, 7 };
        var input = FlatPacketSpanInput.FromSources(23, new ZeroedSequence<VertexInput>(0),
            new ZeroedSequence<float>(0), default, SequenceSpanInput<uint>.From(color), true);
        return FlatPacketSpanViewReadOnly.Create(input);
    }

    private static FlatPacketSpanView CreateInPlaceFromStack(Span<byte> storage)
    {
        Span<uint> color = stackalloc uint[] { 5, 6, 7 };
        var input = FlatPacketSpanInput.FromSources(23, new ZeroedSequence<VertexInput>(0),
            new ZeroedSequence<float>(0), default, SequenceSpanInput<uint>.From(color), true);
        return FlatPacketSpanView.CreateInPlace(storage, input);
    }

    private static FlatPacketSpanViewReadOnly CreateReadOnlyInPlaceFromStack(Span<byte> storage)
    {
        Span<uint> color = stackalloc uint[] { 5, 6, 7 };
        var input = FlatPacketSpanInput.FromSources(23, new ZeroedSequence<VertexInput>(0),
            new ZeroedSequence<float>(0), default, SequenceSpanInput<uint>.From(color), true);
        return FlatPacketSpanViewReadOnly.CreateInPlace(storage, input);
    }

    private readonly struct ShrinkingSource(ReadCount count) : ISequenceSource<float>
    {
        public static bool IsZeroed => false;
        public int Length => count.Value++ == 0 ? 3 : 0;
        public ReadOnlySpan<float> Span => default;
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
