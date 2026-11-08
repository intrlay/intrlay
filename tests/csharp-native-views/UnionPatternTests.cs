#if INTRLAY_CSHARP15
using System;
using System.Runtime.CompilerServices;
using NativeViews.Generated;

internal static class UnionPatternTests
{
    public static void Run()
    {
        ProtocolNamesInput value = new ProtocolNamesValueInput(31);
        ProtocolNamesInput hasValue = new ProtocolNamesHasValueInput(37);
        ProtocolNamesInput getValue = new ProtocolNamesTryGetValueInput(41);
        ProtocolNamesInput create = new ProtocolNamesCreateInput(43);
        Check(Read(value) == 31 && Read(hasValue) == 37 && Read(getValue) == 41 && Read(create) == 43,
            "protocol names coexist with schema variants");
        Check(value.Value.Field0 == 31 && hasValue.HasValue.Field0 == 37,
            "existing C# 14 payload accessors remain available");
        Check(Read(default) == 0, "tag zero remains a valid case");
        Check(SparseUnionInput.FromEmpty() is SparseUnionEmptyInput, "empty case is a value");
        Check(SparseUnionInput.FromEmpty() is not null, "empty case is not absence");
        SparseUnionInput invalid = default;
        Check(invalid is null, "default sparse tag has no declared case");
        SparseUnionInput? absent = null;
        Check(absent is null, "nullable union absence");
        SparseUnionInput? present = new SparseUnionValueInput(47);
        Check(present is SparseUnionValueInput { Field0: 47 }, "nullable implicit union conversion");

        NestedInput nested = new NestedTextInput("retained");
        var payload = PayloadInput.FromNested(new[] { NestedInput.FromNumber(5), nested });
        Check(payload is PayloadNestedInput { Items: var items }
            && items[1] is NestedTextInput { Field0: "retained" }, "nested managed payload patterns");
        var oldPath = ProtocolNamesInput.FromValue(31);
        Check(ProtocolNamesView.Create(in oldPath).Bytes.Span.SequenceEqual(
            ProtocolNamesView.Create(in value).Bytes.Span), "conversion retains the existing wire layout");

        for (int index = 0; index < 20000; ++index) _ = Read(value);
        long before = GC.GetAllocatedBytesForCurrentThread();
        uint sum = 0;
        for (int index = 0; index < 20000; ++index)
        {
            ProtocolNamesInput input = new ProtocolNamesValueInput((uint)index);
            sum += Read(input);
        }
        Check(GC.GetAllocatedBytesForCurrentThread() == before, "union construction and typed matching do not box");
        Check(sum == 199990000, "pattern loop results");
        Console.WriteLine("C# 15: union conversions, exhaustive patterns, defaults and allocation-free access passed.");
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static uint Read(in ProtocolNamesInput value) => value switch
    {
        ProtocolNamesValueInput item => item.Field0,
        ProtocolNamesHasValueInput item => item.Field0,
        ProtocolNamesTryGetValueInput item => item.Field0,
#if !INTRLAY_INCOMPLETE_UNION
        ProtocolNamesCreateInput item => item.Field0,
#endif
    };

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception(message);
    }
}
#endif
