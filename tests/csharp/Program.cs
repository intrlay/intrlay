using Intrlay;
using System;
using System.IO;
using System.Runtime.CompilerServices;
using IntrlayGenerated.Conformance;

static void Check(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}

// Measure an optimized method body without a tiered-JIT transition inside the loop.
[MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
static double Traverse(MessageView view, int count)
{
    double checksum = 0;
    for (int i = 0; i < count; ++i)
    {
        view.Points[0].SetX(i);
        checksum += view.Points[0].X + view.Tree.Next.GetValueOrDefault().Id;
    }
    return checksum;
}

[MethodImpl(MethodImplOptions.NoInlining | MethodImplOptions.AggressiveOptimization)]
static void Construct(Memory<byte> buffer, in Message value, int count)
{
    for (int i = 0; i < count; ++i)
        _ = MessageView.CreateInPlace(buffer, in value);
}

string directory = args[0];
byte[] bytes = File.ReadAllBytes(Path.Combine(directory, "rust.bin"));
var view = new MessageView(bytes);
Check(view.Sequence == 0x123456789abcdef0UL, "u64");
Check(view.Title == "fixture 🦀 한글", "UTF-8");
Check(view.Points[1].Z == 6, "point");
Check(view.Commands[1].DrawPointSize == 0.75, "variant");
Check(view.Tree.Next.GetValueOrDefault().Id == 8, "recursive box");
Check(!view.Tree.Next.GetValueOrDefault().Next.HasValue, "None");
Check(view.Optional.HasValue && view.Optional.Value == 0, "Some(0)");

var value = new Message(
    0x123456789abcdef0UL,
    "fixture 🦀 한글",
    new[] { new Point(1, 2, 3), new Point(4, 5, 6) },
    new[] { DrawingCommand.FromClear(), DrawingCommand.FromDrawPoint(new Point(7, 8, 9), 0.75, "example") },
    new Link(7, new Box<Link>(new Link(8, default))),
    new Nullable<uint>(0),
    new float[] { 10, 20, 30 });

var created = MessageView.Create(in value);
Check(created.Bytes.Span.SequenceEqual(bytes), "exact Rust bytes");
var backing = new byte[bytes.Length + 32];
Array.Fill(backing, (byte)0xcc);
var borrowed = MessageView.CreateInPlace(backing.AsMemory(16), in value);
Check(borrowed.Bytes.Span.SequenceEqual(bytes), "external subview");
Check(backing[15] == 0xcc && backing[^1] == 0xcc, "external capacity preserved");
borrowed.Points[1].SetZ(99);
borrowed.SetSequence(42);
Check(borrowed.Points[1].Z == 99 && borrowed.Sequence == 42, "mutations");

// Warm the JIT and framework paths before measuring managed allocations.
Construct(backing.AsMemory(16), in value, 1000);
_ = Traverse(borrowed, 1000);
long before = GC.GetAllocatedBytesForCurrentThread();
double checksum = Traverse(borrowed, 100_000);
long accessBytes = GC.GetAllocatedBytesForCurrentThread() - before;
before = GC.GetAllocatedBytesForCurrentThread();
Construct(backing.AsMemory(16), in value, 10_000);
long creationBytes = GC.GetAllocatedBytesForCurrentThread() - before;
Check(accessBytes == 0, $"view traversal allocated {accessBytes} bytes");
Check(creationBytes == 0, $"external construction allocated {creationBytes} bytes");
File.WriteAllBytes(Path.Combine(directory, "csharp.bin"), created.Bytes.Span);
Console.WriteLine($"C#: exact bytes; 100,000 view accesses = {accessBytes} B; 10,000 CreateInPlace = {creationBytes} B; checksum={checksum}");

byte[] nullableBytes = File.ReadAllBytes(Path.Combine(directory, "nullable.bin"));
var nullable = new NullableCasesView(nullableBytes);
Check(nullable.Absent is null && nullable.Zero == 0, "nullable scalar");
Check(nullable.Text == "" && nullable.MissingText is null, "nullable reference");
Check(nullable.AbsentUnit is null && nullable.PresentUnit.HasValue, "nullable unit");
Check(!nullable.NestedAbsent.HasValue, "outer None");
Check(nullable.NestedNone.HasValue && nullable.NestedNone.Value is null, "Some(None)");
Check(nullable.NestedValue.HasValue && nullable.NestedValue.Value == 7, "Some(Some(7))");
Check(nullable.BoxedNone.HasValue && nullable.BoxedNone.Value is null, "Some(Box(None))");
Check(nullable.Triple.HasValue && nullable.Triple.Value.HasValue && nullable.Triple.Value.Value is null, "three option levels");
Check(!nullable.Variants[0].ValueValue.HasValue, "inactive nullable enum field");
Check(nullable.Variants[1].ValueValue.HasValue && nullable.Variants[1].ValueValue.Value is null, "active nullable enum field");
Check(nullable.Variants[2].ValueValue.Value == 9, "active enum payload");
var nullableValue = new NullableCases(
    null, new Nullable<uint>(0), "", null, null, new Nullable<ValueTuple>(default(ValueTuple)),
    default, new Optional<Nullable<uint>>(null), new Optional<Nullable<uint>>(7),
    new Box<Nullable<uint>>(null), new Optional<Optional<Nullable<uint>>>(new Optional<Nullable<uint>>(null)),
    new[] { NullableVariant.FromEmpty(), NullableVariant.FromValue(null), NullableVariant.FromValue(9) });
Check(NullableCasesView.Create(in nullableValue).Bytes.Span.SequenceEqual(nullableBytes), "nullable exact Rust bytes");
Console.WriteLine("C#: nullable values preserve all nested presence states");
