using System;
using System.Buffers;
using System.IO;
using System.Runtime.CompilerServices;
using EdgeCases.Generated;

static void Check(bool condition) { if (!condition) throw new Exception("boundary value regression"); }
static byte[] Read(string name) => File.ReadAllBytes($"target/edge-cases/{name}.bin");

[MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
static uint ReadNested(NestedView view, int count)
{
    uint sum = 0;
    for (int i = 0; i < count; ++i) sum += view.Child.Value.Value!.Value.Value;
    return sum;
}

var values = new[] { "", "hello", "\uFEFF", "\uFEFFhello", "\uFEFF\uFEFFhello", "a\uFEFFb", "한글😀" };
var text = TextView.Create(new Text(values));
Check(text.Bytes.Span.SequenceEqual(Read("text")));
var root = new EdgeCasesView(Read("golden"));
for (int i = 0; i < values.Length; ++i) Check(text.Values[i] == values[i] && root.Text.Values[i] == values[i]);
Check(root.Nested.Child.Value.Value!.Value.Value == 9);
Check(root.Recursive.Child.Value!.Value.Value == 8);
Check(root.Zeroes.After == 42 && root.Zeroes.Units.Count == 0 && root.Zeroes.Elements.Count == 2);
Check(root.Shadow.Custom.Value == 13 && root.Shadow.Optional == 14 && root.Shadow.Items[0] == 15);
Check(root.Choice.ValueChild.Value!.Value.Value == 16);
Check(root.Projection.Numeric.Field0 == 1 && root.Projection.Named.Field0 == 2);
root.Projection.Many[0].SetX(20);
Check(new EdgeCasesViewReadOnly(root.Bytes).Projection.Many[0].X == 20);
Check(!new NestedView(Read("absent-0")).Child.HasValue);
Check(!new NestedView(Read("absent-1")).Child.Value.HasValue);
Check(!new NestedView(Read("absent-2")).Child.Value.Value.HasValue);
Check(EdgeCases.Scalar.IntrlayU32View.Create(123).Value == 123);

var intrinsics = new EdgeCases.Intrinsics.IntrinsicProperties(1, 2, 3, 4);
var intrinsicView = EdgeCases.Intrinsics.IntrinsicPropertiesView.Create(intrinsics);
Check(intrinsicView.@__arglist == 1 && intrinsicView.@__makeref == 2);
Check(intrinsicView.@__reftype == 3 && intrinsicView.@__refvalue == 4);

var names = new EdgeCases.Hygiene.HygieneView(Read("hygiene"));
Check(names.Result.Value == 1 && names.Selected!.Value.Value == 8);
Check(names.Unit.Equals(default(ValueTuple)) && names.Text == "standard names");
var nativeNames = new EdgeCases.Hygiene.Hygiene(
    new EdgeCases.Hygiene.BinaryPrimitives(1), new EdgeCases.Hygiene.Encoding(2),
    new EdgeCases.Hygiene.ValueTuple(3), new EdgeCases.Hygiene.ArgumentException(4),
    new EdgeCases.Hygiene.System(5), default(ValueTuple), "standard names", new uint[] { 6, 7 },
    new EdgeCases.Hygiene.BinaryPrimitives(8));
Check(EdgeCases.Hygiene.HygieneView.Create(nativeNames).Bytes.Span.SequenceEqual(Read("hygiene")));

using var owner = new CountingMemoryManager(Read("golden"));
var tracked = new EdgeCasesView(owner.Memory);
owner.Reset();
var child = tracked.Nested.Child;
Check(owner.SpanReads == 1 && child.Value.Value!.Value.Value == 9);
_ = ReadNested(root.Nested, 10000);
long before = GC.GetAllocatedBytesForCurrentThread();
uint sum = ReadNested(root.Nested, 100000);
long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
Check(allocated == 0 && sum == 900000);
Console.WriteLine($"C# nested getter: one GetSpan, {allocated} bytes allocated for 100000 reads");

sealed class CountingMemoryManager(byte[] bytes) : MemoryManager<byte>
{
    public int SpanReads { get; private set; }
    public void Reset() => SpanReads = 0;
    public override Span<byte> GetSpan() { ++SpanReads; return bytes; }
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}
