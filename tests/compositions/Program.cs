using System;
using System.Buffers;
using System.IO;
using System.Runtime.CompilerServices;
using IntrlayGenerated.Schema;

static void Check(bool value) { if (!value) throw new Exception("composition regression"); }

var zeroInput = ZeroedArraysInput.FromSources(73, new Intrlay.ZeroedSequence<ZeroedPointInput>(3),
    new Intrlay.ZeroedSequence<bool>(4), new Intrlay.ZeroedArray<FixedArray2<float>>(),
    new IntrlayOptionalArray2OfU32Input<Intrlay.ZeroedArray<uint>>(default),
    new IntrlaySequenceOfSequenceOfU32Input<Intrlay.ZeroedSequence<uint>>(
        new[] { new Intrlay.ZeroedSequence<uint>(2), new Intrlay.ZeroedSequence<uint>(0) }),
    default);
var zeroExpected = File.ReadAllBytes("target/compositions/zeroed.bin");
var zeroOwner = new byte[zeroExpected.Length + 32];
zeroOwner.AsSpan().Fill(0xa5);
var zeroView = ZeroedArraysView.CreateInPlace(zeroOwner.AsMemory(16), in zeroInput);
Check(zeroView.Bytes.Span.SequenceEqual(zeroExpected));
Check(ZeroedArraysView.Create(in zeroInput).Bytes.Span.SequenceEqual(zeroExpected));
Check(zeroOwner.AsSpan(0, 16).IndexOfAnyExcept((byte)0xa5) < 0);
Check(zeroOwner.AsSpan(16 + zeroExpected.Length).IndexOfAnyExcept((byte)0xa5) < 0);

var zeroPayload = ManagedUnionFloatsInput.FromSources(new Intrlay.ZeroedSequence<float>(5));
var boxedPayload = new IntrlayBoxedManagedUnionInput<ManagedUnionFloatsInput<Intrlay.ZeroedSequence<float>>>(zeroPayload);
var nestedPayload = ManagedUnionNestedInput.FromSources(boxedPayload);
var canonicalNested = ManagedUnionInput.FromNested(new Intrlay.Box<ManagedUnionInput>(ManagedUnionInput.FromFloats(new float[5])));
Check(ManagedUnionView.Create(in nestedPayload).Bytes.Span.SequenceEqual(ManagedUnionView.Create(in canonicalNested).Bytes.Span));

var absent = default(IntrlayOptionalArray2OfU32Input<Intrlay.ZeroedArray<uint>>);
var absentInput = ZeroedArraysInput.FromSources(zeroInput.Id, zeroInput.Points, zeroInput.Flags,
    zeroInput.Matrix, absent, zeroInput.Nested, zeroInput.Empty);
Check(ZeroedArraysView.Create(in absentInput).Present is null);
Check(zeroView.Present is { } presentArray && presentArray[0] == 0);

// Isolate steady-state union allocation from tiered-JIT loop transitions.
[MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
static float UseUnions(ReadOnlyMemory<float> memory, FourFloatsInput payload, int count)
{
    var total = 0f;
    for (int i = 0; i < count; ++i)
    {
        total += ManagedUnionInput.FromFloats(memory).Floats.Values.Span[0];
        total += CompactUnionInput.FromH(payload).H.Field0.X;
        total += ReferenceUnionInput.FromH("shared").H.Field0.Length;
    }
    return total;
}

Check(Composition.Cli.ScalarView.Create(1.5f).Value == 1.5f);
var bindingName = Composition.BindingName.RootView.Create(new(new uint[] { 17, 19 }));
Check(bindingName.RootBindings[1] == 19);
Check(Composition.BindingName.RootView.CreateInPlace(new byte[bindingName.Bytes.Length], new(new uint[] { 23 })).RootBindings[0] == 23);

var names = RenamedListView.Create(new RenamedListInput(new RenamedInput[] { new(11, 22) }));
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
var tagged = RenamedVariantView.Create(RenamedVariantInput.FromTagged(34));
Check(tagged.TaggedValue == 34);
var elementAccessInput = new Composition.ElementAccess.ElementAccessCollectionInput(new Composition.ElementAccess.ElementAccessInput[] { new(43) });
var elementAccess = Composition.ElementAccess.ElementAccessCollectionView.Create(in elementAccessInput);
Check(elementAccess.Items[0].Number == 43);
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

Check(Unsafe.SizeOf<CompactUnionInput>() == 20);
Check(Unsafe.SizeOf<ReferenceUnionInput>() == 16);
var payload = new FourFloatsInput(1, 2, 3, 4);
foreach (var value in new[] { CompactUnionInput.FromA(payload), CompactUnionInput.FromB(payload), CompactUnionInput.FromC(payload), CompactUnionInput.FromD(payload), CompactUnionInput.FromE(payload), CompactUnionInput.FromF(payload), CompactUnionInput.FromG(payload), CompactUnionInput.FromH(payload) })
{
    var wire = CompactUnionView.Create(value);
    Check(wire.Tag == value.Tag && wire.Bytes.Length == 20);
    Check(BitConverter.ToSingle(wire.Bytes.Span.Slice(4, 4)) == 1);
    Check(BitConverter.ToSingle(wire.Bytes.Span.Slice(16, 4)) == 4);
}
var managed = ManagedUnionInput.FromRecord(new("shared", new float[] { 1, 2 }, new uint[] { 3, 4 }, NestedChoiceInput.FromValues(new uint[] { 5, 6 })));
var expected = File.ReadAllBytes("target/compositions/managed-union.bin");
Check(ManagedUnionView.Create(managed).Bytes.Span.SequenceEqual(expected));
var nested = ManagedUnionInput.FromNested(new(managed));
Check(nested.Nested.Value.Value.Record.Field0.Optional!.Value.Values.Field0.Span[1] == 6);
using var manager = new TestMemoryManager<float>(new float[] { 0, 10, 20, 30 });
ReadOnlyMemory<float> memory = manager.Memory.Slice(1, 2);
var memoryValue = ManagedUnionInput.FromFloats(memory);
Check(memoryValue.Floats.Values.Equals(memory));
Check(ManagedUnionInput.FromNumbers(new uint[] { 5, 6 }).Numbers.Values.Span[1] == 6);
_ = UseUnions(memory, payload, 10000);
long before = GC.GetAllocatedBytesForCurrentThread();
float checksum = UseUnions(memory, payload, 100000);
long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
Check(allocated == 0 && checksum > 0);
GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
Check(managed.Record.Field0.Label == "shared" && managed.Record.Field0.Counts.Span[1] == 4);
Console.WriteLine($"C# native unions: compact={Unsafe.SizeOf<CompactUnionInput>()}B, references={Unsafe.SizeOf<ReferenceUnionInput>()}B, 100000 constructions/reads={allocated}B allocated");
NativeUnionTests.Run();
ValueEnumTests.Run();

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
