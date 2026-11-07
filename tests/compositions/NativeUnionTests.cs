using System;
using System.Buffers;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Composition.Regression;
using IntrlayGenerated.Schema;

static class NativeUnionTests
{
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception(message);
    }

    public static void Run()
    {
        Names();
        Owners();
        ManagerWindows();
        NestedPayloads();
        Console.WriteLine("C# union names, owner lifetime, and single payload reads passed");
    }

    private static void Names()
    {
        var storage = Storage.FromValue(31, 32);
        var helper = HelperNames.FromStorage(storage);
        Check(helper.Storage.Field0.Value.storage == 31, "helper type hides native Storage");
        Check(helper.Storage.Field0.Value.storage2 == 32, "factory local hides parameter");
        var numeric = HelperNames.FromNumericData(NumericData.FromComposite(storage));
        Check(numeric.NumericData.Field0.Composite.Field0.Value.storage == 31, "numeric helper hides native enum");
        Check(NumericData.FromNumber(42).Number.Field0 == 42, "numeric union storage");
        Check(PrivateHelper.FromStorage(37)._storage.Field0 == 37, "storage field hides variant");
        Check(RenamedPositionView.Create(new(39)).Coordinate == 39, "renamed native property");
        Check(HelperNamesView.Create(helper).Bytes.Length > 0, "helper-name wire encoding");
    }

    private static void Owners()
    {
        foreach (bool exposeArray in new[] { false, true })
        {
            using var manager = new TestMemoryManager<float>(new float[] { 0, 10, 20, 30 }, exposeArray);
            foreach (int length in new[] { 0, 2 })
            {
                ReadOnlyMemory<float> memory = manager.Memory.Slice(1, length);
                var restored = ManagedUnion.FromFloats(memory).Floats.Values;
                Check(restored.Equals(memory), "memory owner and slice identity");
                Check(MemoryMarshal.TryGetMemoryManager<float, MemoryManager<float>>(restored, out var owner)
                    && ReferenceEquals(owner, manager), "manager replaced by array");
            }
        }
        using var emptyManager = new TestMemoryManager<float>(Array.Empty<float>(), true);
        ReadOnlyMemory<float> emptyMemory = emptyManager.Memory;
        Check(ManagedUnion.FromFloats(emptyMemory).Floats.Values.Equals(emptyMemory), "empty manager owner");
        Check(ManagedUnion.FromFloats(default).Floats.Values.Equals(default(ReadOnlyMemory<float>)), "default memory");
        ReadOnlyMemory<float> array = new float[] { 1, 2, 3 };
        array = array.Slice(1, 1);
        Check(ManagedUnion.FromFloats(array).Floats.Values.Equals(array), "array slice identity");

        foreach (int length in new[] { 0, 1 })
        {
            // No manager or original memory remains reachable outside the union.
            var (value, owner) = CreateRetainedManager(length);
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
            Check(owner.IsAlive, "union must retain a manager even when it exposes an array");
            Check(value.Floats.Values.Length == length, "retained slice length");
            GC.KeepAlive(value);
        }
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static (ManagedUnion Value, WeakReference Owner) CreateRetainedManager(int length)
    {
        var manager = new TestMemoryManager<float>(new float[] { 0, 10, 20 }, true);
        return (ManagedUnion.FromFloats(manager.Memory.Slice(1, length)), new WeakReference(manager));
    }

    private static void NestedPayloads()
    {
        using var manager = new TestMemoryManager<float>(new float[] { 0, 10, 20, 30 });
        ReadOnlyMemory<float> memory = manager.Memory.Slice(1, 2);
        var pair = Buffers.FromPair(memory, memory);
        manager.ResetSpanReads();
        var nested = NestedBuffers.FromInner(pair, Buffers.FromEmpty(), 7);
        Check(manager.SpanReads == 0, "nested factory must copy storage without restoring memory owners");
        Check(NestedBuffersView.Create(nested).Bytes.Span.SequenceEqual(File.ReadAllBytes("target/compositions/nested-buffers.bin")),
            "nested factory changed wire values");
        manager.ResetSpanReads();
        var both = NestedBuffers.FromInner(pair, pair, 8);
        Check(manager.SpanReads == 0, "separate nested cases must copy their existing storage");
        Check(both.Inner.storage.Pair.A.Equals(memory) && both.Inner.payload.Pair.B.Equals(memory)
            && both.Inner.payload2 == 8, "nested factory locals hide parameters");

        _ = UseNested(pair, 10000);
        long before = GC.GetAllocatedBytesForCurrentThread();
        float checksum = UseNested(pair, 100000);
        long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
        Check(allocated == 0 && checksum > 0, "nested native union allocation");
    }

    private static void ManagerWindows()
    {
        using var manager = new WindowMemoryManager();
        foreach (var memory in new ReadOnlyMemory<float>[] {
            manager.Memory.Slice(1, 2), manager.Whole.Slice(0, 1),
            manager.Whole.Slice(5, 1), manager.Whole.Slice(6, 0),
        })
        {
            manager.Start = 1;
            manager.Count = 4;
            var value = ManagedUnion.FromFloats(memory);
            // Retention must not depend on the window exposed at access time.
            manager.Start = 2;
            manager.Count = 1;
            Check(value.Floats.Values.Equals(memory), "overridden manager slice identity");
            Check(value.Floats.Values.Span.SequenceEqual(memory.Span), "overridden manager slice contents");
            var view = ManagedUnionView.Create(value);
            var values = view.FloatsValues!.Value;
            Check(values.Count == memory.Length, "overridden manager encoded length");
            for (int i = 0; i < memory.Length; ++i)
                Check(values[i] == memory.Span[i], "overridden manager encoded values");
        }
        using var ordinary = new TestMemoryManager<float>(new float[] { 10, 20 });
        ReadOnlyMemory<float> original = ordinary.Memory;
        _ = ManagedUnion.FromFloats(original); // Warm the per-type policy once.
        long before = GC.GetAllocatedBytesForCurrentThread();
        for (int i = 0; i < 1000; ++i)
            Check(ManagedUnion.FromFloats(original).Floats.Values.Equals(original), "ordinary manager identity");
        Check(GC.GetAllocatedBytesForCurrentThread() == before, "ordinary manager factories remain allocation-free");
        using MemoryManager<float> hidden = new HiddenMemoryManager();
        ReadOnlyMemory<float> hiddenMemory = hidden.Memory;
        Check(ManagedUnion.FromFloats(hiddenMemory).Floats.Values.Equals(hiddenMemory),
            "unrelated hidden Memory property must not break retention");
        ReadOnlyMemory<float>[] mixed = { original, manager.Whole.Slice(1, 2), hiddenMemory };
        System.Threading.Tasks.Parallel.For(0, 1000, i =>
        {
            var current = mixed[i % mixed.Length];
            Check(ManagedUnion.FromFloats(current).Floats.Values.Equals(current),
                "concurrent mixed manager policies must retain each slice");
        });
    }

    [MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
    private static float UseNested(Buffers pair, int count)
    {
        float total = 0;
        for (int i = 0; i < count; ++i)
        {
            var value = NestedBuffers.FromInner(pair, pair, 7).Inner;
            total += value.storage.Pair.A.Span[0] + value.payload.Pair.B.Span[0];
        }
        return total;
    }
}

sealed class WindowMemoryManager : MemoryManager<float>
{
    private readonly float[] values = { 10, 20, 30, 40, 50, 60 };
    public int Start = 1, Count = 4;
    public Memory<float> Whole => CreateMemory(values.Length);
    public override Memory<float> Memory => CreateMemory(Start, Count);
    public override Span<float> GetSpan() => values;
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}

sealed class HiddenMemoryManager : MemoryManager<float>
{
    private readonly float[] values = { 1, 2 };
    public new int Memory => 0;
    public override Span<float> GetSpan() => values;
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}
