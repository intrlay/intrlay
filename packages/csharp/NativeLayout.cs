using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Intrlay;

/// <summary>Direct access for generated codecs with proven native/wire compatibility.</summary>
/// <remarks>Generated callers prove field offsets and representation; these helpers check
/// the current byte order, CLR size, and address alignment before exposing typed references.</remarks>
public static class NativeLayout
{
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static bool TryGetSpan<T>(ReadOnlySpan<byte> bytes, int stride, int alignment, out ReadOnlySpan<T> values)
        where T : unmanaged
    {
        if (Compatible<T>(bytes, stride, alignment))
        {
            values = MemoryMarshal.Cast<byte, T>(bytes);
            return true;
        }
        values = default;
        return false;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static bool TryGetSpan<T>(Span<byte> bytes, int stride, int alignment, out Span<T> values)
        where T : unmanaged
    {
        if (Compatible<T>(bytes, stride, alignment))
        {
            values = MemoryMarshal.Cast<byte, T>(bytes);
            return true;
        }
        values = default;
        return false;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static unsafe bool Compatible<T>(ReadOnlySpan<byte> bytes, int stride, int alignment)
        where T : unmanaged
    {
        if (!BitConverter.IsLittleEndian || Unsafe.SizeOf<T>() != stride) return false;
        // Supported native leaves require at most eight-byte alignment. A larger
        // schema alignment does not change their CLR representation. On 32-bit
        // runtimes, conservatively avoid borrowing eight-byte-aligned elements:
        // the GC only guarantees pointer-width alignment when moving byte arrays.
        int required = Math.Min(alignment, sizeof(ulong));
        if (required > IntPtr.Size) return false;
        fixed (byte* address = bytes)
            return ((nuint)address & (nuint)(required - 1)) == 0;
    }
}
