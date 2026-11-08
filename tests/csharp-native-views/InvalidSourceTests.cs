#if INTRLAY_TEST_INVALID_SOURCES
using System;
using Intrlay;
using NativeViews.Generated;

static class InvalidSourceTests
{
    public static void Reject()
    {
        // A source must preserve the schema's exact element type.
        _ = FlatPacketInput.FromSources(0, new ZeroedSequence<uint>(1),
            new ZeroedSequence<float>(0), "", new ZeroedArray<uint>(), false);
        _ = FlatPacketInput.FromSources(0, new ZeroedSequence<VertexInput>(1),
            new ZeroedSequence<float>(0), "", new ZeroedArray<int>(), false);

        // Matching generic contracts, not structural similarity, identify a schema.
        var vertex = VertexInput.FromSources(0, new ZeroedArray<float>(), new ZeroedArray<float>());
        _ = FlatPacketView.Create(in vertex);

        // Strings have external payloads and cannot be replaced with zeroed headers.
        _ = PayloadInput.FromData(default, new ZeroedArray<string>(), default);

        // Heap-compatible inputs cannot retain ref struct sources.
        Span<float> values = stackalloc float[3];
        _ = FlatPacketInput.FromSources(0, new ZeroedSequence<VertexInput>(0),
            SequenceSpanInput<float>.From(values), "", new ZeroedArray<uint>(), false);
    }
}
#endif
