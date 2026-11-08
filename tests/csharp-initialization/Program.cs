using System;
using System.IO;
using Initialization.Generated;

static void Equal(string name, ReadOnlySpan<byte> bytes)
{
    var expected = File.ReadAllBytes($"target/initialization/{name}.bin");
    if (!bytes.SequenceEqual(expected)) throw new InvalidOperationException(name);
}

static void Reused(string name, Func<Memory<byte>, ReadOnlyMemory<byte>> create)
{
    var expected = File.ReadAllBytes($"target/initialization/{name}.bin");
    var buffer = new byte[expected.Length + 33];
    buffer.AsSpan().Fill(0xa5);
    var bytes = create(buffer.AsMemory(1));
    Equal(name, bytes.Span);
    if (buffer[0] != 0xa5 || buffer.AsSpan(1 + expected.Length).IndexOfAnyExcept((byte)0xa5) >= 0)
        throw new InvalidOperationException($"{name}: guard bytes");
}

var frame = new FrameInput(17, new[] { new PackedInput(3, -0.0f), new PackedInput(5, 1.25f) });
var empty = new FrameInput(19, Array.Empty<PackedInput>());
Equal("frame", FrameView.Create(in frame).Bytes.Span);
Equal("empty", FrameView.Create(in empty).Bytes.Span);
Reused("frame", bytes => FrameView.CreateInPlace(bytes, in frame).Bytes);
Reused("empty", bytes => FrameViewReadOnly.CreateInPlace(bytes, in empty).Bytes);

// A shorter frame reuses an existing payload without modifying its old suffix.
var old = FrameView.Create(in frame).Bytes.ToArray();
var suffix = old.AsSpan(FrameView.ComputeByteLength(in empty)).ToArray();
Equal("empty", FrameView.CreateInPlace(old, in empty).Bytes.Span);
if (!old.AsSpan(FrameView.ComputeByteLength(in empty)).SequenceEqual(suffix))
    throw new InvalidOperationException("shortened frame suffix");

Span<byte> stack = stackalloc byte[FrameView.ComputeByteLength(in frame) + 2];
stack.Fill(0xa5);
Equal("frame", FrameSpanView.CreateInPlace(stack.Slice(1, stack.Length - 2), in frame).Bytes);
if (stack[0] != 0xa5 || stack[^1] != 0xa5) throw new InvalidOperationException("span guards");
Equal("frame", FrameSpanViewReadOnly.Create(in frame).Bytes);

var padded = new PaddedInput(7, ulong.MaxValue);
Equal("padded", PaddedView.Create(in padded).Bytes.Span);
Reused("padded", bytes => PaddedView.CreateInPlace(bytes, in padded).Bytes);
var absent = new OptionalValueInput(null);
var present = new OptionalValueInput(23);
Equal("absent", OptionalValueView.Create(in absent).Bytes.Span);
Equal("present", OptionalValueView.Create(in present).Bytes.Span);
Reused("absent", bytes => OptionalValueView.CreateInPlace(bytes, in absent).Bytes);
var option = OptionalValueView.Create(in present).Bytes;
Equal("absent", OptionalValueSpanView.CreateInPlace(option.Span, in absent).Bytes);

var word = VariantInput.FromWord(ulong.MaxValue);
var none = VariantInput.FromEmpty();
Equal("variant-word", VariantView.Create(in word).Bytes.Span);
Equal("variant-empty", VariantView.Create(in none).Bytes.Span);
Reused("variant-empty", bytes => VariantView.CreateInPlace(bytes, in none).Bytes);
var variant = VariantView.Create(in word).Bytes;
Equal("variant-empty", VariantSpanViewReadOnly.CreateInPlace(variant.Span, in none).Bytes);

var dynamicValue = new DynamicInput(new ReadOnlyMemory<uint>[] { new uint[] { 1, 2 }, Array.Empty<uint>(), new uint[] { 3 } });
Equal("dynamic", DynamicView.Create(in dynamicValue).Bytes.Span);
Reused("dynamic", bytes => DynamicView.CreateInPlace(bytes, in dynamicValue).Bytes);
var tree = new TreeInput(new[] { new TreeInput(Array.Empty<TreeInput>()) });
Equal("tree", TreeView.Create(in tree).Bytes.Span);
Reused("tree", bytes => TreeView.CreateInPlace(bytes, in tree).Bytes);

for (int length = 0; length < 17; ++length)
{
    var data = new byte[length];
    data.AsSpan().Fill(0x37);
    var gapped = new GappedInput(data, new ulong[] { ulong.MaxValue, 29 });
    Equal($"gap-{length}", GappedView.Create(in gapped).Bytes.Span);
    Reused($"gap-{length}", bytes => GappedView.CreateInPlace(bytes, in gapped).Bytes);
}
Console.WriteLine("C#: proven initialization elision, padding, stale payloads, recursive values, and span/memory guards passed");
