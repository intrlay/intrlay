using System;
using System.Buffers;
using System.IO;
using System.Runtime.CompilerServices;
using NativeViews.Generated;

static void Check(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}

static Vertex MakeVertex() => new(7, new float[] { 1, 2, 3 }, new float[] { 0, 0, 0, 1 });

var vertex = MakeVertex();
var data = Payload.FromData(new uint[] { 10, 20, 30 }, new[] { "a", "b" }, new[] { vertex, vertex });
var again = Payload.FromAgain(new uint[] { 40, 50, 60 }, new[] { "c", "d" });
var nested = Payload.FromNested(new[] { Nested.FromNumber(456), Nested.FromText("nested") });
var value = new Scene(
    new[] { vertex, vertex },
    new FixedArray2<uint>[] { new uint[] { 1, 2 }, new uint[] { 3, 4 } },
    null, vertex, new Intrlay.Optional<Nullable<Vertex>>(null),
    new[] { "first", "second" }, new FixedArray3<uint>[] { new uint[] { 5, 6, 7 } },
    Array.Empty<uint>(), new ValueTuple[] { default, default }, new Padded(9, 123), true,
    new[] { data, again, nested });

var expected = File.ReadAllBytes("target/native-views/rust.bin");
var view = SceneView.Create(in value);
Check(view.Bytes.Span.SequenceEqual(expected), "native inline inputs produce exact Rust bytes");
Check(Unsafe.SizeOf<Vertex>() == 32 && !RuntimeHelpers.IsReferenceOrContainsReferences<Vertex>(), "inline POD native layout");
Check(RuntimeHelpers.IsReferenceOrContainsReferences<FixedArray2<string>>(), "managed inline storage remains GC-visible");
Check(data.Data.Numbers[2] == 30 && data.Data.Texts[1] == "b" && data.Data.Vertices[1].Id == 7, "native enum arrays");
Check(again.Again.Texts[1] == "d" && nested.Nested.Items[1].Text.Field0 == "nested", "native managed array alternatives");

var scoped = view.AsSpan();
var readOnly = new SceneViewReadOnly(view.Bytes).AsSpan();
Check(scoped.Vertices[1].Position[2] == 3 && readOnly.Vertices[0].Color[3] == 1, "typed span descendants");
Check(!scoped.Absent.HasValue && scoped.Present.Value.ToValue().Id == 7, "optional span values");
Check(scoped.Nested.HasValue && !scoped.Nested.Value.HasValue, "nested Some(None) scoped presence");
Check(readOnly.Variants[0].DataNumbers.Value[1] == 20 && !readOnly.Variants[1].DataNumbers.HasValue, "scoped enum field presence");
Check(scoped.Words[1] == "second" && scoped.Variants[2].NestedItems.Value[1].TextField0 == "nested", "managed descendants");
Check(scoped.Matrix[1][0] == 3 && scoped.Empty.Count == 0 && scoped.Units.Count == 2, "nested and zero-sized arrays");
Check(scoped.Padded.Value == 123 && scoped.Flag, "non-POD typed fallback");
Check(typeof(PaddedView).GetMethod("ToValue") is null, "padding excludes POD API");

var cachedVertices = view.Vertices;
Check(scoped.Vertices.TryGetNativeSpan(out var nativeVertices), "aligned POD span");
Check(nativeVertices[1].Position[2] == 3, "native array elements");
nativeVertices[1] = new Vertex(99, new float[] { 4, 5, 6 }, new float[] { 0, 0, 0, 1 });
Check(cachedVertices[1].Id == 99 && readOnly.Vertices[1].Position[2] == 6, "cached metadata retains live mutations");
scoped.Vertices[0].Position.Set(1, 12);
Check(nativeVertices[0].Position[1] == 12, "typed and POD views share storage");
GC.Collect();
Check(nativeVertices[1].Id == 99, "POD spans remain tracked across a GC");

var borrowed = new byte[expected.Length + 2];
borrowed.AsSpan().Fill(0xa5);
var unaligned = SceneSpanView.CreateInPlace(borrowed.AsSpan(1, expected.Length), in value);
Check(borrowed.AsSpan(1, expected.Length).SequenceEqual(expected), "scoped external construction");
Check(borrowed[0] == 0xa5 && borrowed[^1] == 0xa5, "external guard bytes");
Check(!unaligned.Vertices.TryGetNativeSpan(out _) && unaligned.Vertices[1].ToValue().Id == 7, "unaligned POD copy fallback");
Check(unaligned.Batches[0].ToValue()[2] == 7, "native fixed-array copy");
Span<byte> stackStorage = stackalloc byte[32];
// Use safe stack-backed spans; no native pointer or pinning is required by callers.
var stackVertex = VertexSpanView.CreateInPlace(stackStorage, in vertex);
Check(stackVertex.ToValue().Id == 7, "stack-backed construction");

using var owner = new CountingMemoryManager(expected);
var owned = new SceneView(owner.Memory);
owner.Reset();
var ownedVertices = owned.Vertices;
Check(owner.SpanReads == 1, "collection layout resolves its memory owner once");
for (int i = 0; i < 100; ++i) Check(ownedVertices.Count == 2, "cached collection count");
Check(owner.SpanReads == 1, "cached metadata does not resolve memory again");
var borrowedVertices = ownedVertices.AsSpan();
Check(owner.SpanReads == 2, "scoped conversion resolves its memory owner once");
for (int i = 0; i < 100; ++i) Check(borrowedVertices[i & 1].Position[2] == 3, "borrowed descendants");
Check(owner.SpanReads == 2, "scoped descendants retain the borrowed span");

long before = GC.GetAllocatedBytesForCurrentThread();
ulong sum = 0;
for (int i = 0; i < 1000; ++i)
{
    var current = Payload.FromData(data.Data.Numbers, data.Data.Texts, data.Data.Vertices);
    sum += current.Data.Numbers[2] + current.Data.Vertices[0].Id;
    sum += readOnly.Vertices[0].ToValue().Id + readOnly.Present.Value.Id;
}
Check(GC.GetAllocatedBytesForCurrentThread() == before, "native/scoped traversal and enum factories allocate no objects");
Console.WriteLine($"C#: inline arrays, POD, scoped views, optional/enum composition and aliasing passed; {sum}");

sealed class CountingMemoryManager(byte[] bytes) : MemoryManager<byte>
{
    public int SpanReads { get; private set; }
    public void Reset() => SpanReads = 0;
    public override Span<byte> GetSpan() { ++SpanReads; return bytes; }
    public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
    public override void Unpin() { }
    protected override void Dispose(bool disposing) { }
}
