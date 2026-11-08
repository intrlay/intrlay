using System;

namespace Intrlay;

/// <summary>A generated source that encodes the same wire schema as its canonical input.</summary>
/// <typeparam name="TInput">The canonical input type, used to prevent mixing different schemas.</typeparam>
/// <remarks>
/// Writers call this interface through constrained generic parameters, without boxing.
/// Sources must remain stable across both passes and must not overlap the destination.
/// Methods append external payloads to the cursor; the enclosing writer reserves the header.
/// </remarks>
public interface IEncodeInput<TInput>
{
    void Measure(ref Cursor cursor);
    void Write(Span<byte> buffer, int offset, ref Cursor cursor);
}
