using System;
using IntrlayGenerated.Regression;
using IntrlayGenerated.Alpha;
using IntrlayGenerated.Beta;
using IntrlayGenerated.Arrays;
using IntrlayGenerated.Contracts;
using IntrlayGenerated.TaggedDefaults;

ConstructionExtents.Run();

var a = AlphaView.Create(new Alpha(new uint[] { 1, 2 }));
var b = BetaView.Create(new Beta(new uint[] { 3, 4 }));
if (a.Values[1] != 2 || b.Values[0] != 3) throw new Exception("module coexistence");
ReadOnlyMemory<byte> bytes = new byte[] { 7, 0, 0, 0 };
if (new ReadOnlyViewReadOnly(bytes).Value != 7) throw new Exception("read-only memory");
if (new AlphaViewReadOnly(a.Bytes).Values[1] != 2) throw new Exception("read-only descendants");
if (ScalarView.Create(1.5f).Value != 1.5f) throw new Exception("scalar export");
var vectors = ProjectionsView.Create(new Projections(new Vector[] { new(1, 2, 3) }));
if (vectors.Vectors.Count != 1 || vectors.Vectors[0].Z != 3) throw new Exception("typed record collection");
vectors.Vectors[0].SetY(9);
if (vectors.Vectors[0].Y != 9) throw new Exception("borrowed mutation");
if (new ProjectionsViewReadOnly(vectors.Bytes).Vectors[0].Y != 9) throw new Exception("read-only collection");
var arrays = FixedArraysView.Create(new FixedArrays(
    new float[] { 1, 2, 3 }, new float[] { 4, 5, 6, 7 },
    new IntrlayGenerated.Arrays.FixedArray3<float>[] { new float[] { 8, 9, 10 }, new float[] { 11, 12, 13 } },
    new ReadOnlyMemory<float>(new float[] { 14, 15, 16 }), null, new float[] { 17, 18, 19 }));
var three = arrays.Three;
var readOnly = arrays.ReadOnly;
if (three.Count != 3 || arrays.Four.Count != 4 || arrays.Matrix.Count != 2 || arrays.Matrix[1][2] != 13)
    throw new Exception("fixed extents and nested arrays");
three.Set(1, 20);
if (arrays.Three[1] != 20 || readOnly[0] != 17 || !arrays.Present.HasValue || arrays.Present.GetValueOrDefault()[2] != 16 || arrays.Absent.HasValue)
    throw new Exception("fixed views and optional states");
readOnly.Set(0, 18);
if (arrays.ReadOnly[0] != 18) throw new Exception("native accessor generation");
Console.WriteLine("C#: independent modules, read-only buffers, scalar exports, and structural collections passed");

var contracts = new ContractsView(System.IO.File.ReadAllBytes("target/regressions/contracts.bin"));
if (contracts.Identifier.Field0 != 9 || contracts.Identifiers[1].Field0 != 11) throw new Exception("value-type wrappers");
if (IdentifierView.ByteSize != 16) throw new Exception("transparent alignment");
contracts.Identifier.SetField0(19);
contracts.Editable.SetValue(30);
contracts.WriteOnly.SetValue(31);
if (contracts.Identifier.Field0 != 19 || contracts.Editable.Value != 30) throw new Exception("nested writes");
contracts.ReadOnly.SetValue(32);
contracts.ReadOnlyNested!.Value[0].SetValue(33);
contracts.Fixed.SetValue(34);
if (contracts.WriteOnly.Value != 31 || contracts.ReadOnly.Value != 32 || contracts.Fixed.Value != 34)
    throw new Exception("native accessor generation");
Console.WriteLine("C#: value-type wrappers and complete native accessors passed");

// default(T) is legal CLR input, but it has no wire variant for these enums.
static void RejectUnknownTag(Action action)
{
    try { action(); }
    catch (ArgumentException error) when (error.ParamName == "value") { return; }
    throw new Exception("unmatched native enum tag was accepted");
}

var tagBuffer = new byte[128];
RejectUnknownTag(() => NonZeroTagView.Create(default));
RejectUnknownTag(() => NonZeroTagView.CreateInPlace(tagBuffer, default));
RejectUnknownTag(() => { NonZeroTagSpanView.CreateInPlace(tagBuffer.AsSpan(), default); });
RejectUnknownTag(() => NonZeroPayloadTagView.ComputeByteLength(default));
RejectUnknownTag(() => NonZeroPayloadTagView.Create(default));
RejectUnknownTag(() => NonZeroPayloadTagView.CreateInPlace(tagBuffer, default));
RejectUnknownTag(() => TaggedItemsView.Create(new TaggedItems(new NonZeroTag[] { default })));

var validTag = NonZeroTagView.CreateInPlace(tagBuffer, NonZeroTag.FromSecond());
if (validTag.Tag != 2) throw new Exception("valid enum after rejected construction");
if (RestrictedView.Create(default).Tag != 0) throw new Exception("declared zero-tag variant must remain valid");
Console.WriteLine("C#: default enum tags without a declared variant are rejected");
