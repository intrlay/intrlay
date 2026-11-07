using System;
using System.Buffers;
using IntrlayGenerated.ConstructionExtents;
using IntrlayGenerated.Regression;
using ExtentStringsViewReadOnly = IntrlayGenerated.ConstructionExtents.SequenceOfStringViewReadOnly;
using ExtentStringsSpanViewReadOnly = IntrlayGenerated.ConstructionExtents.SequenceOfStringSpanViewReadOnly;

internal static class ConstructionExtents
{
    public static void Run()
    {
        Action<ReadOnlyMemory<string>, byte[]>[] constructors =
        {
            (input, _) => { ExtentStringsView.Create(input); },
            (input, bytes) => { ExtentStringsView.CreateInPlace(bytes.AsMemory(3, 24), input); },
            (input, _) => { ExtentStringsSpanView.Create(input); },
            (input, bytes) => { ExtentStringsSpanView.CreateInPlace(bytes.AsSpan(3, 24), input); },
            (input, _) => { ExtentStringsViewReadOnly.Create(input); },
            (input, bytes) => { ExtentStringsViewReadOnly.CreateInPlace(bytes.AsMemory(3, 24), input); },
            (input, _) => { ExtentStringsSpanViewReadOnly.Create(input); },
            (input, bytes) => { ExtentStringsSpanViewReadOnly.CreateInPlace(bytes.AsSpan(3, 24), input); },
        };
        foreach (var construct in constructors)
        {
            foreach (bool grow in new[] { false, true })
            {
                using var source = new ChangingStrings(grow);
                var bytes = new byte[30];
                bytes.AsSpan().Fill(0xA5);
                bool rejected = false;
                try { construct(source.Source, bytes); }
                catch (InvalidOperationException) when (!grow) { rejected = true; }
                catch (ArgumentException) when (grow) { rejected = true; }
                if (!rejected) throw new Exception("construction published a different write extent");
                if (bytes[2] != 0xA5 || bytes[27] != 0xA5) throw new Exception("construction escaped destination slice");
            }
        }

        ReadOnlyMemory<string> stable = new[] { "Z" };
        var value = ExtentStringsView.Create(stable);
        if (value.Bytes.Length != 17 || value[0] != "Z") throw new Exception("stable dynamic construction");
        var storage = new byte[24];
        var reused = ExtentStringsSpanView.CreateInPlace(storage.AsSpan(3), stable);
        if (reused.Bytes.Length != 17 || reused[0] != "Z") throw new Exception("stable reused construction");

        // Fixed native layouts retain their copy path and exact destination slice.
        storage.AsSpan().Fill(0xA5);
        var fixedValue = ScalarView.CreateInPlace(storage.AsMemory(3, 4), 1.25f);
        if (fixedValue.Value != 1.25f || storage[2] != 0xA5 || storage[7] != 0xA5)
            throw new Exception("fixed native construction");
        Console.WriteLine("C#: dynamic factories check their completed extent; fixed native copies are preserved");
    }

    private sealed class ChangingStrings(bool grow) : MemoryManager<string>
    {
        private readonly string[] shortItems = { "Z" };
        private readonly string[] longItems = { "ABCD" };
        private int calls;

        public ReadOnlyMemory<string> Source => CreateMemory(1);
        public override Span<string> GetSpan() => (++calls == 1) == grow ? shortItems : longItems;
        public override MemoryHandle Pin(int elementIndex = 0) => throw new NotSupportedException();
        public override void Unpin() { }
        protected override void Dispose(bool disposing) { }
    }
}
