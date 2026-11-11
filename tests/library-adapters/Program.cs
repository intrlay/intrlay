using System;
using System.IO;
using IntrlayGenerated.LibraryAdapters;

static class Program
{
    static void Check(bool value) { if (!value) throw new Exception("library adapter mismatch"); }

    static void Main()
    {
        var bytes = File.ReadAllBytes("target/library-adapters/sample.bin");
        var view = new SampleView(bytes);
        Check(view.Bounds.Min.Y == -2 && view.Bounds.Max.Z == 6);
        Check(view.Points[1].X == 10 && view.Matrix.YAxis.Z == 6);
        Check(view.Mask.X && !view.Mask.Y && view.Mask.Z);
        Check(view.Labels[1] == "two 🦀");
        var input = new SampleInput(
            new BoundsInput(new Vec3AInput(-1, -2, -3), new Vec3AInput(4, 5, 6)),
            new[] { new Vec3AInput(7, 8, 9), new Vec3AInput(10, 11, 12) },
            new Mat3AInput(new Vec3AInput(1, 2, 3), new Vec3AInput(4, 5, 6), new Vec3AInput(7, 8, 9)),
            new BVec3AInput(true, false, true), new[] { "one", "two 🦀" });
        Check(SampleView.Create(input).Bytes.Span.SequenceEqual(bytes));
    }
}
