using System;
using System.IO;
using System.Threading;
using NativeViews.Generated;

static class SpanInputTests
{
    public static void Run()
    {
        Span<float> position = stackalloc float[] { 1, 2, 3 };
        Span<float> color = stackalloc float[] { 0, 0, 0, 1 };
        var vertex = new VertexInput(7, new FixedArray3<float>(position), new FixedArray4<float>(color));
        Span<VertexInput> vertices = stackalloc VertexInput[] { vertex, vertex };
        Span<float> samples = stackalloc float[] { 1.5f, -2, 3.25f };
        Span<uint> channels = stackalloc uint[] { 11, 13, 17 };
        Span<char> text = stackalloc char[11];
        "borrowed 🦀".AsSpan().CopyTo(text);
        var input = new FlatPacketSpanInput(17, vertices, samples, text, channels, true);
        var expected = File.ReadAllBytes("target/native-views/flat.bin");
        var owned = new FlatPacketInput(17, vertices.ToArray(), samples.ToArray(), new string(text),
            new FixedArray3<uint>(channels), true);

        Check(FlatPacketView.ComputeByteLength(input) == expected.Length, "span input measurement");
        Check(FlatPacketView.Create(input).Bytes.Span.SequenceEqual(expected), "span input Rust byte parity");
        Check(FlatPacketView.Create(in owned).Bytes.Span.SequenceEqual(expected), "owned overload byte parity");
        Span<byte> storage = stackalloc byte[expected.Length + 2];
        storage.Fill(0xa5);
        var view = FlatPacketSpanView.CreateInPlace(storage.Slice(1, expected.Length), input);
        Check(view.Bytes.SequenceEqual(expected) && storage[0] == 0xa5 && storage[^1] == 0xa5,
            "stack sources and unaligned destination retain guard bytes");
        Check(view.Vertices[1].ToValue().Position[2] == 3 && view.Label == "borrowed 🦀", "span payload contents");

        // Capture allocation costs after warming both public overloads.
        for (int i = 0; i < 100; ++i)
        {
            FlatPacketSpanView.CreateInPlace(storage, input);
            FlatPacketSpanView.CreateInPlace(storage, in owned);
        }
        long before = GC.GetAllocatedBytesForCurrentThread();
        for (int i = 0; i < 1000; ++i)
        {
            FlatPacketSpanView.CreateInPlace(storage, input);
            FlatPacketSpanView.CreateInPlace(storage, in owned);
        }
        Check(GC.GetAllocatedBytesForCurrentThread() == before, "both input paths allocate no intermediate objects");

        var zeroed = FlatPacketSpanInput.FromSources(17, new Intrlay.ZeroedSequence<VertexInput>(2),
            new Intrlay.ZeroedSequence<float>(3), text, Intrlay.SequenceSpanInput<uint>.From(channels), true);
        var zeroExpected = new FlatPacketInput(17, new VertexInput[2], new float[3], new string(text),
            new FixedArray3<uint>(channels), true);
        storage.Fill(0xa5);
        var zeroView = FlatPacketSpanView.CreateInPlace(storage.Slice(1, expected.Length), zeroed);
        Check(zeroView.Bytes.SequenceEqual(FlatPacketView.Create(in zeroExpected).Bytes.Span)
            && storage[0] == 0xa5 && storage[^1] == 0xa5,
            "zeroed span sources clear reused payloads and retain guard bytes");

        var invalid = new FlatPacketSpanInput(17, vertices, samples, text, channels[..2], true);
        try
        {
            FlatPacketView.Create(invalid);
            throw new InvalidOperationException("incorrect fixed extent was accepted");
        }
        catch (ArgumentException) { }

        Check(typeof(SceneInput).Assembly.GetType("NativeViews.Generated.SceneSpanInput") is null,
            "nested payloads retain the owned input path");
        Check(typeof(VertexInput).Assembly.GetType("NativeViews.Generated.VertexSpanInput") is null,
            "fixed POD inputs retain their existing native copy path");
        CheckLargeOwnedInput();
        Check(CreateLocalVertex().ToValue().Id == 7 && CreateLocalReadOnlyVertex().ToValue().Id == 7,
            "allocating span factories do not borrow canonical local inputs");
        Check(CreateLocalPacket().Label == "local" && CreateLocalReadOnlyPacket().Label == "local",
            "canonical payload inputs do not escape through returned views");
        Console.WriteLine("C#: borrowed SpanInput, byte parity, fixed extents and zero-allocation reuse passed");
    }

    private sealed class LargeHolder
    {
        public LargeInlinePacketInput Value;
    }

    private static VertexSpanView CreateLocalVertex()
    {
        var input = new VertexInput(7, default, default);
        return VertexSpanView.Create(in input);
    }

    private static VertexSpanViewReadOnly CreateLocalReadOnlyVertex()
    {
        var input = new VertexInput(7, default, default);
        return VertexSpanViewReadOnly.Create(in input);
    }

    private static FlatPacketSpanView CreateLocalPacket()
    {
        var input = new FlatPacketInput(7, Array.Empty<VertexInput>(), Array.Empty<float>(), "local", default, true);
        return FlatPacketSpanView.Create(in input);
    }

    private static FlatPacketSpanViewReadOnly CreateLocalReadOnlyPacket()
    {
        var input = new FlatPacketInput(7, Array.Empty<VertexInput>(), Array.Empty<float>(), "local", default, true);
        return FlatPacketSpanViewReadOnly.Create(in input);
    }

    private static void CheckLargeOwnedInput()
    {
        var holder = new LargeHolder();
        // Initialize on a generous stack; the input itself stays on the heap.
        var initialize = new Thread(() =>
        {
            FixedArray32768<uint> left = default;
            FixedArray32768<uint> right = default;
            left[0] = 17;
            right[32767] = 29;
            holder.Value = new LargeInlinePacketInput(in left, in right, "payload");
        }, 8 * 1024 * 1024);
        initialize.Start();
        initialize.Join();

        int length = 0;
        var measure = new Thread(() => length = LargeInlinePacketView.ComputeByteLength(in holder.Value), 128 * 1024);
        measure.Start();
        measure.Join();
        Check(length == LargeInlinePacketView.ByteSize + 7, "measurement does not copy fixed-array storage");

        byte[] storage = new byte[length];
        // The optimized writer reuses one property temporary. A Debug JIT keeps
        // separate locals alive even for the original sequential write path.
#if DEBUG
        const int writeStackSize = 1024 * 1024;
#else
        const int writeStackSize = 256 * 1024;
#endif
        var write = new Thread(() => LargeInlinePacketSpanView.CreateInPlace(storage, in holder.Value), writeStackSize);
        write.Start();
        write.Join();
        var result = new LargeInlinePacketView(storage);
        Check(result.Left[0] == 17 && result.Right[32767] == 29 && result.Label == "payload",
            "owned encoding does not retain all inline-array copies together");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
