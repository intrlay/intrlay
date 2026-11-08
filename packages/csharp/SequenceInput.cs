using System;
using System.Runtime.CompilerServices;

namespace Intrlay;

/// <summary>A sequence source whose initialization strategy is part of its type.</summary>
/// <typeparam name="T">The exact generated element input type.</typeparam>
/// <remarks>
/// Length and contents must stay stable throughout measurement and writing.
/// A value source's Length must equal its Span.Length. A zero source's span is ignored.
/// </remarks>
public interface ISequenceSource<T>
{
    static abstract bool IsZeroed { get; }
    int Length { get; }
    ReadOnlySpan<T> Span { get; }
}

/// <summary>A fixed-array source; the schema supplies the extent for zero initialization.</summary>
/// <typeparam name="T">The exact generated element input type.</typeparam>
public interface IFixedArraySource<T>
{
    static abstract bool IsZeroed { get; }
    ReadOnlySpan<T> Span { get; }
}

/// <summary>Borrowed memory for a generic sequence or fixed-array input.</summary>
/// <typeparam name="T">The exact generated element input type.</typeparam>
public readonly struct SequenceInput<T> : ISequenceSource<T>, IFixedArraySource<T>
{
    private readonly ReadOnlyMemory<T> _values;

    private SequenceInput(ReadOnlyMemory<T> values) => _values = values;

    public static bool IsZeroed => false;
    public int Length => _values.Length;
    public ReadOnlySpan<T> Span => _values.Span;
    public ReadOnlyMemory<T> Memory => _values;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static SequenceInput<T> From(ReadOnlyMemory<T> values) => new(values);

    public static implicit operator SequenceInput<T>(T[] values) => From(values);
    public static implicit operator SequenceInput<T>(Memory<T> values) => From(values);
    public static implicit operator SequenceInput<T>(ReadOnlyMemory<T> values) => From(values);
}

/// <summary>Stack-backed or borrowed elements for a scoped generic input.</summary>
/// <typeparam name="T">The exact generated element input type.</typeparam>
public readonly ref struct SequenceSpanInput<T> : ISequenceSource<T>, IFixedArraySource<T>
{
    private readonly ReadOnlySpan<T> _values;

    private SequenceSpanInput(ReadOnlySpan<T> values) => _values = values;

    public static bool IsZeroed => false;
    public int Length => _values.Length;
    public ReadOnlySpan<T> Span => _values;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static SequenceSpanInput<T> From(ReadOnlySpan<T> values) => new(values);

    public static implicit operator SequenceSpanInput<T>(T[] values) => From(values);
    public static implicit operator SequenceSpanInput<T>(Span<T> values) => From(values);
    public static implicit operator SequenceSpanInput<T>(ReadOnlySpan<T> values) => From(values);
}

/// <summary>Reserves and clears a sequence without allocating source elements.</summary>
/// <typeparam name="T">The zero-initializable element input type.</typeparam>
public readonly struct ZeroedSequence<T> : ISequenceSource<T>
{
    public ZeroedSequence(int length)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(length);
        Length = length;
    }

    public static bool IsZeroed => true;
    public int Length { get; }
    public ReadOnlySpan<T> Span => default;
}

/// <summary>Clears a fixed array using the extent declared by its schema.</summary>
/// <typeparam name="T">The zero-initializable element input type.</typeparam>
public readonly struct ZeroedArray<T> : IFixedArraySource<T>
{
    public static bool IsZeroed => true;
    public ReadOnlySpan<T> Span => default;
}
