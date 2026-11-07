using System;
using System.Collections.Generic;

namespace Intrlay.Generator;

/// <summary>Allocates internal identifiers without hiding names visible in the generated scope.</summary>
internal sealed class NameScope(IEnumerable<string> reserved)
{
    private readonly HashSet<string> names = new(reserved, StringComparer.Ordinal);

    public string Allocate(string stem)
    {
        var name = stem;
        for (int suffix = 2; !names.Add(name); ++suffix) name = stem + suffix;
        return name;
    }
}
