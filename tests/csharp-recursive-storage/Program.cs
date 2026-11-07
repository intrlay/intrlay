using System;
using System.IO;
using System.Runtime.InteropServices;
using RecursiveStorage;

static void Check(bool condition)
{
    if (!condition) throw new Exception("recursive native storage");
}

OptionalNode?[] nodes = { null, new(2, default) };
var source = new OptionalNode(1, nodes.AsMemory());
var encoded = OptionalNodeView.Create(source);
Check(encoded.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/csharp-recursive-storage/optional.bin")));
Check(encoded.Children.Count == 2 && encoded.Children[0] is null && encoded.Children[1]!.Value.Id == 2);
Check(MemoryMarshal.TryGetArray(source.Children, out var retained) && ReferenceEquals(retained.Array, nodes));
var slice = new OptionalNode(3, nodes.AsMemory(1, 1));
Check(slice.Children.Length == 1 && slice.Children.Span[0]!.Value.Id == 2);
Check(MemoryMarshal.TryGetArray(slice.Children, out var retainedSlice) && retainedSlice.Offset == 1);
var decoded = new OptionalNodeSpanViewReadOnly(encoded.Bytes.Span);
Check(decoded.Id == 1 && decoded.Children[1]!.Value.Id == 2);

FixedArray2<ArrayNode> pair = new ArrayNode[] { new(default), new(default) };
var arrays = ArrayNodeView.Create(new ArrayNode(new[] { pair }));
Check(arrays.Children.Count == 1 && arrays.Children[0][1].Children.Count == 0);
var nested = NestedNodeView.Create(new NestedNode(new ReadOnlyMemory<NestedNode>[] { new[] { new NestedNode(default) } }));
Check(nested.Children[0][0].Children.Count == 0);
var absent = WrappedNodeView.Create(new WrappedNode(null));
Check(absent.Children is null);
var wrapped = WrappedNodeView.Create(new WrappedNode(new WrappedNode?[] { new(null) }.AsMemory()));
Check(wrapped.Children!.Value[0]!.Value.Children is null);
FixedArray2<ReadOnlyMemory<InlineNode?>> inline = new ReadOnlyMemory<InlineNode?>[] { default, new InlineNode?[] { null } };
var inlineView = InlineNodeView.Create(new InlineNode(inline));
Check(inlineView.Children[0].Count == 0 && inlineView.Children[1][0] is null);
Check(EmptyNodeView.Create(new EmptyNode(default)).Children.Count == 0);
var mutual = MutualParentView.Create(new MutualParent(new MutualChild[] { new(new MutualParent(default)) }));
Check(mutual.Children[0].Parent!.Value.Children.Count == 0);
Check(PlainNodeView.Create(new PlainNode(new[] { new PlainNode(default) })).Children[0].Children.Count == 0);
Check(BoxedNodeView.Create(new BoxedNode(new[] { new Intrlay.Box<BoxedNode>(new(default)) })).Children[0].Children.Count == 0);

// Candidate backing names do not reserve members when direct storage is valid.
var directChildren = new DirectChildren(new[] { new DirectChildren(default, 7) }, 3);
var directChildrenView = DirectChildrenView.Create(directChildren);
Check(directChildrenView._childrenMemory == 3 && directChildrenView.Children[0]._childrenMemory == 7);

// Nullable inline elements convert between public collections and erased storage
// in both directions, including absent and present-but-empty collections.
FixedArray2<ReadOnlyMemory<NullableInlineNode>?> leafChildren = default;
leafChildren[1] = ReadOnlyMemory<NullableInlineNode>.Empty;
NullableInlineNode[] nullableNodes = { new(default), new(leafChildren), new(default) };
FixedArray2<ReadOnlyMemory<NullableInlineNode>?> nullableChildren = default;
nullableChildren[0] = nullableNodes.AsMemory(1, 1);
var nullableNode = new NullableInlineNode(nullableChildren);
var nullableStored = nullableNode.Children;
Check(MemoryMarshal.TryGetArray(nullableStored[0]!.Value, out var nullableOwner));
Check(ReferenceEquals(nullableOwner.Array, nullableNodes) && nullableOwner.Offset == 1 && nullableOwner.Count == 1);
var nullableView = NullableInlineNodeView.Create(nullableNode);
Check(nullableView.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/csharp-recursive-storage/record-nullable-inline.bin")));
Check(nullableView.Children[1] is null);
Check(nullableView.Children[0]!.Value[0].Children[0] is null);
Check(nullableView.Children[0]!.Value[0].Children[1]!.Value.Count == 0);

NestedNullableInlineNode[] nestedNullableNodes = { new(default), new(default), new(default) };
FixedArray2<ReadOnlyMemory<NestedNullableInlineNode>> nestedNullablePair = default;
nestedNullablePair[0] = nestedNullableNodes.AsMemory(1, 1);
FixedArray2<FixedArray2<ReadOnlyMemory<NestedNullableInlineNode>>?> nestedNullableChildren = default;
nestedNullableChildren[0] = nestedNullablePair;
var nestedNullableNode = new NestedNullableInlineNode(nestedNullableChildren);
var nestedNullableStored = nestedNullableNode.Children;
Check(MemoryMarshal.TryGetArray(nestedNullableStored[0]!.Value[0], out var nestedNullableOwner));
Check(ReferenceEquals(nestedNullableOwner.Array, nestedNullableNodes) && nestedNullableOwner.Offset == 1 && nestedNullableOwner.Count == 1);
var nestedNullableView = NestedNullableInlineNodeView.Create(nestedNullableNode);
Check(nestedNullableView.Bytes.Span.SequenceEqual(File.ReadAllBytes("target/csharp-recursive-storage/record-nested-nullable-inline.bin")));
Check(nestedNullableView.Children[0]!.Value[0][0].Children[0] is null && nestedNullableView.Children[1] is null);

for (int i = 0; i < 1000; ++i)
{
    _ = new NullableInlineNode(nullableChildren).Children;
    _ = new NestedNullableInlineNode(nestedNullableChildren).Children;
}
long nullableBefore = GC.GetAllocatedBytesForCurrentThread();
for (int i = 0; i < 1000; ++i)
{
    Check(new NullableInlineNode(nullableChildren).Children[0]!.Value.Length == 1);
    Check(new NestedNullableInlineNode(nestedNullableChildren).Children[0]!.Value[0].Length == 1);
}
Check(GC.GetAllocatedBytesForCurrentThread() == nullableBefore);

// Array-backed construction retains owners without adding a box allocation.
for (int i = 0; i < 1000; ++i) _ = new OptionalNode(1, nodes.AsMemory()).Children;
long before = GC.GetAllocatedBytesForCurrentThread();
for (int i = 0; i < 1000; ++i) Check(new OptionalNode(1, nodes.AsMemory()).Children.Length == 2);
Check(GC.GetAllocatedBytesForCurrentThread() == before);

// Enum arrays keep inline storage, but their recursive collection owners must
// not become recursively constructed CLR generic value types.
static void CheckEnum(string name, RecursiveEnum value)
{
    var bytes = RecursiveEnumView.Create(value).Bytes;
    Check(bytes.Span.SequenceEqual(File.ReadAllBytes("target/csharp-recursive-storage/" + name + ".bin")));
}

var empty = RecursiveEnum.FromEmpty();
RecursiveEnum[] enumNodes = { empty, empty, empty };
var enumSlice = enumNodes.AsMemory(1, 1);
FixedArray2<ReadOnlyMemory<RecursiveEnum>> direct = new ReadOnlyMemory<RecursiveEnum>[] { enumSlice, default };
var directEnum = RecursiveEnum.FromDirect(direct);
CheckEnum("enum-direct", directEnum);
var directPayload = directEnum.Direct.Items;
Check(MemoryMarshal.TryGetArray(directPayload[0], out var enumRetained));
Check(ReferenceEquals(enumRetained.Array, enumNodes) && enumRetained.Offset == 1 && enumRetained.Count == 1);
var directView = RecursiveEnumView.Create(directEnum);
Check(directView.DirectItems!.Value[0][0].Tag == 0 && directView.DirectItems.Value[1].Count == 0);
Check(new RecursiveEnumSpanViewReadOnly(directView.Bytes.Span).DirectItems!.Value[0][0].Tag == 0);

FixedArray2<ReadOnlyMemory<RecursiveEnum?>> optionalElements = new ReadOnlyMemory<RecursiveEnum?>[]
{
    new RecursiveEnum?[] { null, empty }, default,
};
var optionalElementsEnum = RecursiveEnum.FromOptionalElements(optionalElements);
CheckEnum("enum-optional-elements", optionalElementsEnum);
Check(optionalElementsEnum.OptionalElements.Items[0].Span[0] is null);
Check(optionalElementsEnum.OptionalElements.Items[0].Span[1]!.Value.Tag == 0);
var optionalArray = RecursiveEnum.FromOptionalArray(direct);
CheckEnum("enum-optional-array", optionalArray);
Check(optionalArray.OptionalArray.Items!.Value[0].Span[0].Tag == 0);
Check(RecursiveEnum.FromOptionalArray(null).OptionalArray.Items is null);
FixedArray2<ReadOnlyMemory<RecursiveEnum>?> optionalLists = new ReadOnlyMemory<RecursiveEnum>?[] { null, enumSlice };
var optionalListsEnum = RecursiveEnum.FromOptionalLists(optionalLists);
CheckEnum("enum-optional-lists", optionalListsEnum);
Check(optionalListsEnum.OptionalLists.Items[0] is null && optionalListsEnum.OptionalLists.Items[1]!.Value.Length == 1);

FixedArray2<FixedArray2<ReadOnlyMemory<RecursiveEnum>>> nestedArrays = new[] { direct, default };
var nestedArraysEnum = RecursiveEnum.FromNestedArrays(nestedArrays);
CheckEnum("enum-nested-arrays", nestedArraysEnum);
Check(nestedArraysEnum.NestedArrays.Items[0][0].Span[0].Tag == 0);
FixedArray2<RecursiveBranch> branches = new RecursiveBranch[] { new(7, enumSlice), new(9, default) };
var recordsEnum = RecursiveEnum.FromRecords(branches);
CheckEnum("enum-records", recordsEnum);
Check(recordsEnum.Records.Items[0].Id == 7 && recordsEnum.Records.Items[1].Id == 9);
Check(MemoryMarshal.TryGetArray(recordsEnum.Records.Items[0].Children, out var branchRetained));
Check(ReferenceEquals(branchRetained.Array, enumNodes) && branchRetained.Offset == 1);
FixedArray2<ReadOnlyMemory<RecursiveParent>> parents = new ReadOnlyMemory<RecursiveParent>[]
{
    new[] { RecursiveParent.FromChild(empty) }, default,
};
var mutualEnum = RecursiveEnum.FromMutual(parents);
CheckEnum("enum-mutual", mutualEnum);
Check(mutualEnum.Mutual.Items[0].Span[0].Child.Child!.Value.Tag == 0);

// Nested enum storage transfers preserve internal array slots without decoding
// payload alternatives or allocating collection copies.
var envelope = RecursiveEnvelope.FromChild(directEnum);
var envelopeBytes = RecursiveEnvelopeView.Create(envelope).Bytes;
Check(envelopeBytes.Span.SequenceEqual(File.ReadAllBytes("target/csharp-recursive-storage/enum-envelope.bin")));
Check(envelope.Child.Child.Direct.Items[0].Length == 1);
Check(RecursiveEnumView.Create(RecursiveEnum.FromZero(default)).ZeroItems!.Value.Count == 0);
Check(RecursiveEnumView.Create(RecursiveEnum.FromNestedZero(default)).NestedZeroItems!.Value[1].Count == 0);
Check(RecursiveEnum.FromPlain(enumSlice).Plain.Items.Length == 1);
FixedArray2<ReadOnlyMemory<uint>> acyclic = new ReadOnlyMemory<uint>[] { new uint[] { 3, 4 }, default };
Check(RecursiveEnum.FromAcyclic(acyclic).Acyclic.Items[0].Span[1] == 4);
FixedArray4<uint> pod = new uint[] { 1, 2, 3, 4 };
Check(RecursiveEnum.FromPod(pod).Pod.Items[3] == 4);
FixedArray32<ReadOnlyMemory<LargeRecursiveEnum>> large = default;
large[31] = new[] { LargeRecursiveEnum.FromEmpty() };
Check(LargeRecursiveEnumView.Create(LargeRecursiveEnum.FromChildren(large)).ChildrenItems!.Value[31][0].Tag == 0);

for (int i = 0; i < 1000; ++i) _ = RecursiveEnum.FromDirect(direct).Direct.Items;
before = GC.GetAllocatedBytesForCurrentThread();
for (int i = 0; i < 1000; ++i)
    Check(RecursiveEnum.FromDirect(direct).Direct.Items[0].Length == 1);
Check(GC.GetAllocatedBytesForCurrentThread() == before);
Console.WriteLine("Recursive native storage passed.");
