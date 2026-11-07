using System;
using System.Runtime.CompilerServices;

namespace Intrlay;

/// <summary>Static specialization of a wire element's stride and access.</summary>
/// <typeparam name="T">The value or value-type view returned without boxing.</typeparam>
public interface IElementAccess<T>
{
    static abstract int Stride { get; }
    static abstract T Read(Memory<byte> bytes, int offset);
}

/// <summary>Static access to an element of immutable storage.</summary>
public interface IReadOnlyElementAccess<T>
{
    static abstract int Stride { get; }
    static abstract T Read(ReadOnlyMemory<byte> bytes, int offset);
}

/// <summary>Contiguous element headers with compile-time specialized access.</summary>
/// <typeparam name="T">The element value or generated view struct.</typeparam>
/// <typeparam name="TAccess">A struct implementing static stride and read operations.</typeparam>
public readonly struct SequenceView<T, TAccess> where TAccess : struct, IElementAccess<T>
{
    private readonly Memory<byte> _bytes;
    private readonly int _offset;
    public int Count { get; }

    public SequenceView(Memory<byte> bytes, int offset, int count)
    {
        _bytes = bytes;
        _offset = offset;
        Count = count;
    }

    public T this[int index]
    {
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        get => TAccess.Read(_bytes, _offset + index * TAccess.Stride);
    }
}

/// <summary>Read-only sequence using static dispatch without delegates or boxing.</summary>
public readonly struct ReadOnlySequenceView<T, TAccess> where TAccess : struct, IReadOnlyElementAccess<T>
{
    private readonly ReadOnlyMemory<byte> _bytes;
    private readonly int _offset;
    public int Count { get; }

    public ReadOnlySequenceView(ReadOnlyMemory<byte> bytes, int offset, int count)
    {
        _bytes = bytes;
        _offset = offset;
        Count = count;
    }

    public T this[int index]
    {
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        get => TAccess.Read(_bytes, _offset + index * TAccess.Stride);
    }
}
