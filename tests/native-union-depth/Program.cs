using System;
using System.Buffers;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using NativeUnionDepth.Generated;

static class Program
{
    private const string Fixture = "target/native-union-depth/";
#if INTRLAY_CSHARP15
    private const string Bindings = "target/native-union-depth-csharp15/";
#else
    private const string Bindings = Fixture;
#endif

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception(message);
    }

    private static void Bytes(ReadOnlyMemory<byte> actual, string name) =>
        Check(actual.Span.SequenceEqual(File.ReadAllBytes(Fixture + name)), "wire bytes: " + name);

    private static void Main()
    {
        using var manager = new CountingManager(new uint[] { 0, 10, 20, 30 }, true);
        ReadOnlyMemory<uint> memory = manager.Memory.Slice(1, 2);
        Variants(memory, manager, false);
        Variants(memory, manager, true);
        Owners();
        ZeroedPayload();
        Allocation(memory, manager);
        SourceGrowth();
        Console.WriteLine("C# nested native unions: depths 2-7 copy/read 0 owners, terminal read 1 owner, exact bytes and linear source passed");
    }

    private static void Variants(ReadOnlyMemory<uint> memory, CountingManager manager, bool selectedB)
    {
        var one = selectedB ? Level1Input.FromB(memory, "leaf") : Level1Input.FromA(memory, "leaf");
        manager.Calls = 0;
        var two = selectedB ? Level2Input.FromB(one) : Level2Input.FromA(one);
        var three = selectedB ? Level3Input.FromB(two) : Level3Input.FromA(two);
        var four = selectedB ? Level4Input.FromB(three) : Level4Input.FromA(three);
        var five = selectedB ? Level5Input.FromB(four) : Level5Input.FromA(four);
        var six = selectedB ? Level6Input.FromB(five) : Level6Input.FromA(five);
        var seven = selectedB ? Level7Input.FromB(six) : Level7Input.FromA(six);
        Check(manager.Calls == 0, "constructing nested enums must not restore owners");
        uint tag = selectedB ? 1u : 0u;
        OuterRead(manager, () => selectedB ? two.B.Child.Tag : two.A.Child.Tag, tag, 2);
        OuterRead(manager, () => selectedB ? three.B.Child.Tag : three.A.Child.Tag, tag, 3);
        OuterRead(manager, () => selectedB ? four.B.Child.Tag : four.A.Child.Tag, tag, 4);
        OuterRead(manager, () => selectedB ? five.B.Child.Tag : five.A.Child.Tag, tag, 5);
        OuterRead(manager, () => selectedB ? six.B.Child.Tag : six.A.Child.Tag, tag, 6);
        OuterRead(manager, () => selectedB ? seven.B.Child.Tag : seven.A.Child.Tag, tag, 7);
        OuterRead(manager, () => selectedB ? seven.A.Child.Tag : seven.B.Child.Tag, 0, 7);
        TerminalRead(manager, () => Values(Leaf(two)), memory, 2);
        TerminalRead(manager, () => Values(Leaf(three)), memory, 3);
        TerminalRead(manager, () => Values(Leaf(four)), memory, 4);
        TerminalRead(manager, () => Values(Leaf(five)), memory, 5);
        TerminalRead(manager, () => Values(Leaf(six)), memory, 6);
        TerminalRead(manager, () => Values(Leaf(seven)), memory, 7);

        string suffix = selectedB ? "b" : "a";
        Bytes(Level2View.Create(two).Bytes, $"level2-{suffix}.bin");
        Bytes(Level3View.Create(three).Bytes, $"level3-{suffix}.bin");
        Bytes(Level4View.Create(four).Bytes, $"level4-{suffix}.bin");
        Bytes(Level5View.Create(five).Bytes, $"level5-{suffix}.bin");
        Bytes(Level6View.Create(six).Bytes, $"level6-{suffix}.bin");
        Bytes(Level7View.Create(seven).Bytes, $"level7-{suffix}.bin");

        var first = Level3Input.FromA(Level2Input.FromB(Level1Input.FromB(new uint[] { 31, 32 }, "first")));
        var extra = Level2Input.FromA(Level1Input.FromA(new uint[] { 51 }, "extra"));
        var mixed = MixedInput.FromPair("prefix", new PayloadInput(selectedB ? first : null, three, 0x1122334455667788), selectedB ? extra : null);
        manager.Calls = 0;
        var pair = mixed.Pair;
        Check(manager.Calls == 0 && pair.Payload.Second.Tag == tag, "record/optional transfers restore owners");
        Check(pair.Payload.First.HasValue == selectedB && pair.Extra.HasValue == selectedB, "nested optional presence");
        Check(pair.Prefix == "prefix" && pair.Payload.Marker == 0x1122334455667788, "reference/numeric slot rebasing");
        if (selectedB)
        {
            var restoredFirst = Leaf(pair.Payload.First!.Value).B;
            var restoredExtra = Leaf(pair.Extra!.Value).A;
            Check(restoredFirst.Label == "first" && restoredFirst.Values.Span.SequenceEqual(new uint[] { 31, 32 }), "first sibling slots");
            Check(restoredExtra.Label == "extra" && restoredExtra.Values.Span[0] == 51, "extra sibling slots");
        }
        Check(mixed.Scalar.Value == 0 && MixedInput.FromScalar(12.5).Pair.Prefix is null, "inactive payloads");
        Bytes(MixedView.Create(mixed).Bytes, $"mixed-{suffix}.bin");
        Bytes(MixedView.Create(MixedInput.FromScalar(12.5)).Bytes, "mixed-scalar.bin");
        var optional = OptionalStatesInput.FromValues(default, new Intrlay.Optional<Level3Input?>(null), new Intrlay.Optional<Level3Input?>(three));
        manager.Calls = 0;
        var states = optional.Values;
        Check(!states.OuterAbsent.HasValue && states.InnerAbsent.HasValue && !states.InnerAbsent.Value.HasValue
            && states.Present.Value!.Value.Tag == tag && manager.Calls == 0, "nested optional enum presence/storage");
        Bytes(OptionalStatesView.Create(optional).Bytes, $"optional-{suffix}.bin");
        var readName = TransferParentInput.FromValue(TransferNamesInput.FromReadStorage(two));
        var copyName = TransferParentInput.FromValue(TransferNamesInput.FromCopyStorage(two));
        Check(readName.Value.Child.ReadStorage.storage.Tag == tag && copyName.Value.Child.CopyStorage.data.Tag == tag,
            "transfer helpers collide with source variant/field names");

        int[] sizes = { Unsafe.SizeOf<Level1Input>(), Unsafe.SizeOf<Level2Input>(), Unsafe.SizeOf<Level3Input>(), Unsafe.SizeOf<Level4Input>(),
            Unsafe.SizeOf<Level5Input>(), Unsafe.SizeOf<Level6Input>(), Unsafe.SizeOf<Level7Input>() };
        for (int i = 1; i < sizes.Length; ++i)
            Check(sizes[i] >= sizes[i - 1] && sizes[i] - sizes[i - 1] <= 8, "nested native size must grow linearly");
        Check(sizes[^1] <= sizes[0] + 32, "nested alternatives must share native storage");
        Console.WriteLine($"C# native depth {suffix}: sizes={string.Join(',', sizes)}");
    }

    private static void OuterRead(CountingManager manager, Func<uint> read, uint expected, int depth)
    {
        manager.Calls = 0;
        Check(read() == expected && manager.Calls == 0, $"depth {depth} outer read restores owners");
    }

    private static void TerminalRead(CountingManager manager, Func<ReadOnlyMemory<uint>> read, ReadOnlyMemory<uint> expected, int depth)
    {
        manager.Calls = 0;
        Check(read().Equals(expected) && manager.Calls == 1, $"depth {depth} must restore one terminal owner");
    }

    private static Level7Input Nest(Level1Input leaf) =>
        Level7Input.FromA(Level6Input.FromA(Level5Input.FromA(Level4Input.FromA(Level3Input.FromA(Level2Input.FromA(leaf))))));

    private static Level1Input Leaf(Level2Input value) => value.Tag == 0 ? value.A.Child : value.B.Child;
    private static Level1Input Leaf(Level3Input value) => Leaf(value.Tag == 0 ? value.A.Child : value.B.Child);
    private static Level1Input Leaf(Level4Input value) => Leaf(value.Tag == 0 ? value.A.Child : value.B.Child);
    private static Level1Input Leaf(Level5Input value) => Leaf(value.Tag == 0 ? value.A.Child : value.B.Child);
    private static Level1Input Leaf(Level6Input value) => Leaf(value.Tag == 0 ? value.A.Child : value.B.Child);
    private static Level1Input Leaf(Level7Input value) => Leaf(value.Tag == 0 ? value.A.Child : value.B.Child);
    private static ReadOnlyMemory<uint> Values(Level1Input value) => value.Tag == 0 ? value.A.Values : value.B.Values;

    private static void ZeroedPayload()
    {
        var zeroed = Level7AInput.FromSources(Level6AInput.FromSources(Level5AInput.FromSources(
            Level4AInput.FromSources(Level3AInput.FromSources(Level2AInput.FromSources(
                Level1AInput.FromSources(new Intrlay.ZeroedSequence<uint>(3), "zero")))))));
        var values = Nest(Level1Input.FromA(new uint[3], "zero"));
        Check(Level7View.Create(in zeroed).Bytes.Span.SequenceEqual(Level7View.Create(in values).Bytes.Span),
            "nested zero requests match ordinary element inputs");
    }

    private static void Owners()
    {
        foreach (bool exposeArray in new[] { false, true })
        {
            using var manager = new CountingManager(new uint[] { 0, 10, 20 }, exposeArray);
            foreach (int length in new[] { 0, 2 })
            {
                ReadOnlyMemory<uint> memory = manager.Memory.Slice(1, length);
                var restored = Leaf(Nest(Level1Input.FromA(memory, "owner"))).A.Values;
                Check(restored.Equals(memory), "manager owner/slice identity");
                Check(MemoryMarshal.TryGetMemoryManager<uint, MemoryManager<uint>>(restored, out var owner)
                    && ReferenceEquals(owner, manager), "manager owner replaced during storage transfer");
            }
        }
        ReadOnlyMemory<uint> array = new uint[] { 0, 10, 20 };
        array = array.Slice(1, 2);
        Check(Leaf(Nest(Level1Input.FromA(array, "array"))).A.Values.Equals(array), "array slice identity");
        Check(Leaf(Nest(Level1Input.FromA(default, "default"))).A.Values.Equals(default(ReadOnlyMemory<uint>)), "default memory identity");
        foreach (int length in new[] { 0, 2 })
        {
            var (value, owner, label) = Retained(length);
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
            Check(owner.IsAlive && label.IsAlive, "nested storage lost a managed owner/reference");
            var payload = Leaf(value).A;
            Check(payload.Values.Length == length && payload.Label == "managed", "retained managed payload");
            GC.KeepAlive(value);
        }
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static (Level7Input Value, WeakReference Owner, WeakReference LabelInput) Retained(int length)
    {
        var manager = new CountingManager(new uint[] { 0, 10, 20 }, true);
        string label = new("managed".ToCharArray());
        return (Nest(Level1Input.FromA(manager.Memory.Slice(1, length), label)), new WeakReference(manager), new WeakReference(label));
    }

    private static void Allocation(ReadOnlyMemory<uint> memory, CountingManager manager)
    {
        var leaf = Level1Input.FromA(memory, "leaf");
        _ = UseDeep(leaf, 10000);
        manager.Calls = 0;
        long before = GC.GetAllocatedBytesForCurrentThread();
        long checksum = UseDeep(leaf, 100000);
        long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
        Check(allocated == 0 && checksum == 1400000, "nested construction/read allocations");
        Check(manager.Calls == 200000, "each iteration needs one restoration and one requested Span");
        Console.WriteLine($"C# native depth 7: 100000 constructions/reads={allocated}B allocated");
    }

    [MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
    private static long UseDeep(Level1Input leaf, int count)
    {
        long total = 0;
        for (int i = 0; i < count; ++i)
        {
            var payload = Leaf(Nest(leaf)).A;
            total += payload.Values.Span[0] + payload.Label.Length;
        }
        return total;
    }

    private static void SourceGrowth()
    {
        string file = Directory.GetFiles(Bindings + "expanded",
            "*.g.cs", SearchOption.AllDirectories).Single();
        string source = File.ReadAllText(file).Replace("\r", "");
        int[] lengths = new int[6];
        for (int depth = 2; depth <= 7; ++depth)
        {
            int start = source.IndexOf($"public readonly partial struct Level{depth}Input", StringComparison.Ordinal);
            Check(start >= 0, "missing native declaration");
            int end = source.IndexOf('{', start), nesting = 1;
            while (nesting != 0)
            {
                char value = source[++end];
                if (value == '{') ++nesting;
                if (value == '}') --nesting;
            }
            string body = source.Substring(start, end - start + 1);
            Check(!body.Contains(".As<", StringComparison.Ordinal), "nested native access must reuse child storage");
#if INTRLAY_CSHARP15
            Check(body.Split("switch", StringSplitOptions.None).Length == 2,
                "only the IUnion object projection dispatches alternatives");
#else
            Check(!body.Contains("switch", StringComparison.Ordinal), "nested alternatives were expanded again");
#endif
            lengths[depth - 2] = body.Length;
        }
        Check(lengths.Max() - lengths.Min() < 400, "per-level native implementation must stay bounded");
        Console.WriteLine($"C# native depth 2-7 source characters={string.Join(',', lengths)}");
    }
}

sealed class CountingManager(uint[] values, bool exposeArray) : MemoryManager<uint>
{
    public int Calls;
    public override Span<uint> GetSpan() { ++Calls; return values; }
    protected override bool TryGetArray(out ArraySegment<uint> segment)
    {
        segment = exposeArray ? new(values) : default;
        return exposeArray;
    }
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}
