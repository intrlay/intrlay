using System;
using System.Buffers;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Threading;

namespace Intrlay;

/// <summary>Retains a native collection owner and exact slice without copying elements.</summary>
/// <remarks>Intrlay has no char primitive, so generated collections never use string-backed memory.</remarks>
public readonly struct MemorySlot
{
    private readonly object? _owner;
    private readonly int _start;
    private readonly int _length;

    private MemorySlot(object? owner, int start, int length)
    {
        _owner = owner;
        _start = start;
        _length = length;
    }

    /// <summary>Retains the original slice. Arrays and default manager windows require no allocation.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static MemorySlot From<T>(ReadOnlyMemory<T> memory)
    {
        if (memory.Equals(default(ReadOnlyMemory<T>))) return default;
        // A manager can expose an array without transferring ownership to it.
        if (MemoryMarshal.TryGetMemoryManager<T, MemoryManager<T>>(memory, out var manager, out int start, out int length))
            return FromManager(memory, manager, start, length);
        if (MemoryMarshal.TryGetArray(memory, out var array))
            return new(array.Array, array.Offset, array.Count);
        throw new NotSupportedException("Intrlay collection memory must be backed by an array or MemoryManager.");
    }

    /// <summary>Restores the slice with the element type supplied by its generated owner.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public ReadOnlyMemory<T> As<T>()
    {
        if (_owner is null) return default;
        if (_owner is T[] array) return new(array, _start, _length);
        if (_owner is MemoryManager<T> manager) return manager.Memory.Slice(_start, _length);
        return (ReadOnlyMemory<T>)_owner;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static MemorySlot FromManager<T>(ReadOnlyMemory<T> memory, MemoryManager<T> manager, int start, int length)
    {
        // The reported start is relative to GetSpan, not the virtual Memory
        // property. An override may expose a different or changing window.
        if (ManagerWindows<T>.IsDefault(manager.GetType()))
            return new(manager, start, length);
        return new(memory, 0, 0);
    }

    private static class ManagerWindows<T>
    {
        // Weak keys allow collectible manager types to unload. Reflection occurs
        // once per concrete type, never while reading a retained payload.
        private static readonly ConditionalWeakTable<Type, Policy> Policies = new();
        private static Policy? _lastPolicy;

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        internal static bool IsDefault(Type type)
        {
            // Repeated inputs normally use the same concrete owner type. Avoid
            // a weak-table lookup for each value while preserving mixed owners.
            var policy = Volatile.Read(ref _lastPolicy);
            return policy is not null && policy.Type == type ? policy.IsDefault : Resolve(type);
        }

        private static bool Resolve(Type type)
        {
            var policy = Policies.GetValue(type, static key => new(key));
            // Keep collectible types exclusively in the weak table.
            if (!type.IsCollectible) Volatile.Write(ref _lastPolicy, policy);
            return policy.IsDefault;
        }

        private sealed class Policy
        {
            internal readonly Type Type;
            internal readonly bool IsDefault;

            internal Policy(Type type)
            {
                Type = type;
                // Enumerating also handles types hiding Memory with an unrelated
                // property; GetProperty by name would throw for that valid case.
                foreach (var property in type.GetProperties())
                {
                    if (property.Name != nameof(MemoryManager<T>.Memory)) continue;
                    if (property.GetMethod?.DeclaringType != typeof(MemoryManager<T>))
                    {
                        IsDefault = false;
                        return;
                    }
                    IsDefault = true;
                }
            }
        }
    }
}
