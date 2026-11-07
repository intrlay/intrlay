using System;
using System.Buffers.Binary;
using System.Text;
using System.Runtime.CompilerServices;

namespace Intrlay;

/// <summary>Fixed-layout operations used by generated accessors.</summary>
public static class Reader
{
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static int Dereference(ReadOnlySpan<byte> bytes, int offset) =>
        offset + (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.Slice(offset));

    public static string StringAt(ReadOnlySpan<byte> bytes, int offset)
    {
        int start = Dereference(bytes, offset);
        int length = (int)BinaryPrimitives.ReadUInt32LittleEndian(bytes.Slice(offset + 4));
        return Encoding.UTF8.GetString(bytes.Slice(start, length));
    }
}
