using System;
using System.Buffers.Binary;
using System.Text;

namespace Intrlay;

public readonly struct Optional<T>
{
    public bool HasValue { get; }
    public T Value { get; }

    public Optional(T value)
    {
        HasValue = true;
        Value = value;
    }

    public static implicit operator Optional<T>(T value) => new(value);
}

// Only explicit source Box<T> edges allocate a reference. Ordinary records and
// enum payloads remain values, including when stored in arrays.
public sealed class Box<T>
{
    public T Value { get; }

    public Box(T value)
    {
        Value = value;
    }
}

public struct Cursor
{
    public int End;

    public Cursor(int end)
    {
        End = end;
    }

    public int Reserve(int size, int alignment)
    {
        int start = checked(End + alignment - 1) & -alignment;
        End = checked(start + size);
        if (size < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(size));
        }
        return start;
    }

    /// <summary>Checks that construction wrote the previously measured extent.</summary>
    public readonly void Finish(int byteLength)
    {
        if (End != byteLength)
        {
            throw new InvalidOperationException("Input changed size between measurement and writing.");
        }
    }
}
