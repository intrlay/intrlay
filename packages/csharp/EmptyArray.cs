using System;

namespace Intrlay;

/// <summary>Native storage for a zero-length fixed array, without an allocated array.</summary>
/// <remarks>Its CLR size is not its zero-byte wire size, so it never qualifies for POD copying.</remarks>
public readonly struct EmptyArray<T>
{
    public int Length => 0;

    public EmptyArray(ReadOnlySpan<T> values)
    {
        if (!values.IsEmpty) throw new ArgumentException("Expected an empty fixed array.", nameof(values));
    }

    public static implicit operator EmptyArray<T>(T[] values) => new(values);
    public static implicit operator EmptyArray<T>(ReadOnlyMemory<T> values) => new(values.Span);
    public static implicit operator ReadOnlySpan<T>(EmptyArray<T> value) => ReadOnlySpan<T>.Empty;
}
