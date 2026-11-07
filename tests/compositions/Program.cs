using System;
using System.Buffers;
using System.IO;
using System.Runtime.CompilerServices;
using IntrlayGenerated.Schema;

static void Check(bool value) { if (!value) throw new Exception("composition regression"); }

// Isolate steady-state union allocation from tiered-JIT loop transitions.
[MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
static float UseUnions(ReadOnlyMemory<float> memory, FourFloats payload, int count)
{
    var total = 0f;
    for (int i = 0; i < count; ++i)
    {
        total += ManagedUnion.FromFloats(memory).Floats.Values.Span[0];
        total += CompactUnion.FromH(payload).H.Field0.X;
        total += ReferenceUnion.FromH("shared").H.Field0.Length;
    }
    return total;
}

Check(Composition.Cli.ScalarView.Create(1.5f).Value == 1.5f);
var bindingName = Composition.BindingName.RootView.Create(new(new uint[] { 17, 19 }));
Check(bindingName.RootBindings[1] == 19);
Check(Composition.BindingName.RootView.CreateInPlace(new byte[bindingName.Bytes.Length], new(new uint[] { 23 })).RootBindings[0] == 23);

var names = RenamedListView.Create(new RenamedList(new Renamed[] { new(11, 22) }));
Check(names.Items[0].first == 11);
names.Items[0].SetSecond(23);
Check(names.Items[0].second == 23);
var a = Composition.Alpha.SharedAlphaView.Create(new(new(31)));
var b = Composition.Beta.SharedBetaView.Create(new(new(32)));
Check(a.Shared.Value == 31 && b.Shared.Value == 32);
var naming = Composition.Naming.SharedView.Create(new(33));
Check(naming.Value == 33);
byte[] prefixed = new byte[8];
naming.Bytes.Span.CopyTo(prefixed.AsSpan(4));
Check(new Composition.Naming.SharedView(prefixed, 4).Value == 33);
var tagged = RenamedVariantView.Create(RenamedVariant.FromTagged(34));
Check(tagged.TaggedValue == 34);
var w = WrappedScalarsView.Create(new(1, null, new(2), new(3), new(null), new Intrlay.Box<uint>?[] { new(4), null }, new uint?[] { 5, null }, 6, 7));
var length = w.Bytes.Length;
w.SetReadOnly(100);
Check(w.WriteOnly == 7 && w.ReadOnly == 100);
w.SetPresent(11); w.SetAbsent(12); w.SetBoxed(13); w.SetNested(14); w.SetMissingInner(15);
w.Items.Set(0, 16); w.Items.Set(1, 17); w.Fixed.Set(0, 18); w.Fixed.Set(1, 19); w.SetWriteOnly(20);
Check(w.Present == 11 && w.Absent == 12 && w.Boxed == 13 && w.Nested.Value == 14 && w.MissingInner.HasValue && w.MissingInner.Value == 15);
Check(w.Items[0] == 16 && w.Items[1] == null && w.Fixed[0] == 18 && w.Fixed[1] == 19 && w.Bytes.Length == length);
foreach (var item in new Intrlay.Box<uint?>?[] { null, new(null), new(2) })
{
    var root = OptionalRootView.Create(item);
    root.Set(9);
    Check(item == null ? !root.Value.HasValue : root.Value.HasValue && root.Value.Value == 9);
}
Console.WriteLine("C# composition regressions passed");

Check(Unsafe.SizeOf<CompactUnion>() == 20);
Check(Unsafe.SizeOf<ReferenceUnion>() == 16);
var payload = new FourFloats(1, 2, 3, 4);
foreach (var value in new[] { CompactUnion.FromA(payload), CompactUnion.FromB(payload), CompactUnion.FromC(payload), CompactUnion.FromD(payload), CompactUnion.FromE(payload), CompactUnion.FromF(payload), CompactUnion.FromG(payload), CompactUnion.FromH(payload) })
{
    var wire = CompactUnionView.Create(value);
    Check(wire.Tag == value.Tag && wire.Bytes.Length == 20);
    Check(BitConverter.ToSingle(wire.Bytes.Span.Slice(4, 4)) == 1);
    Check(BitConverter.ToSingle(wire.Bytes.Span.Slice(16, 4)) == 4);
}
var managed = ManagedUnion.FromRecord(new("shared", new float[] { 1, 2 }, new uint[] { 3, 4 }, NestedChoice.FromValues(new uint[] { 5, 6 })));
var expected = File.ReadAllBytes("target/compositions/managed-union.bin");
Check(ManagedUnionView.Create(managed).Bytes.Span.SequenceEqual(expected));
var nested = ManagedUnion.FromNested(new(managed));
Check(nested.Nested.Value.Value.Record.Field0.Optional!.Value.Values.Field0.Span[1] == 6);
using var manager = new TestMemoryManager<float>(new float[] { 0, 10, 20, 30 });
ReadOnlyMemory<float> memory = manager.Memory.Slice(1, 2);
var memoryValue = ManagedUnion.FromFloats(memory);
Check(memoryValue.Floats.Values.Equals(memory));
Check(ManagedUnion.FromNumbers(new uint[] { 5, 6 }).Numbers.Values.Span[1] == 6);
_ = UseUnions(memory, payload, 10000);
long before = GC.GetAllocatedBytesForCurrentThread();
float checksum = UseUnions(memory, payload, 100000);
long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
Check(allocated == 0 && checksum > 0);
GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
Check(managed.Record.Field0.Label == "shared" && managed.Record.Field0.Counts.Span[1] == 4);
Console.WriteLine($"C# native unions: compact={Unsafe.SizeOf<CompactUnion>()}B, references={Unsafe.SizeOf<ReferenceUnion>()}B, 100000 constructions/reads={allocated}B allocated");
NativeUnionTests.Run();

sealed class TestMemoryManager<T>(T[] values, bool exposeArray = false) : MemoryManager<T>
{
    public int SpanReads { get; private set; }
    public void ResetSpanReads() => SpanReads = 0;
    public override Span<T> GetSpan()
    {
        ++SpanReads;
        return values;
    }
    protected override bool TryGetArray(out ArraySegment<T> segment)
    {
        segment = exposeArray ? new(values) : default;
        return exposeArray;
    }
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}
