using System;
using System.Linq;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Text;

namespace Intrlay.Generator;

/// <summary>Expands resolved wire schemas during compilation and IDE analysis.</summary>
[Generator(LanguageNames.CSharp)]
public sealed class BindingGenerator : IIncrementalGenerator
{
    private static readonly DiagnosticDescriptor InvalidSchema = new(
        "ILAY001", "Invalid Intrlay schema", "Cannot generate Intrlay bindings: {0}",
        "Intrlay", DiagnosticSeverity.Error, isEnabledByDefault: true);

    public void Initialize(IncrementalGeneratorInitializationContext context)
    {
        var schemas = context.SyntaxProvider.ForAttributeWithMetadataName(
            "Intrlay.GenerateBindingsAttribute",
            static (node, _) => node is ClassDeclarationSyntax,
            static (input, _) => Schema.Read((INamedTypeSymbol)input.TargetSymbol));

        context.RegisterSourceOutput(schemas.Combine(context.CompilationProvider), static (output, input) =>
        {
            var (schema, compilation) = input;
            try
            {
                ValidateLanguage(schema, (CSharpCompilation)compilation);
                var source = new Emitter(schema).Generate();
                output.AddSource(schema.HintName, SourceText.From(source, Encoding.UTF8));
            }
            catch (Exception error) when (error is ArgumentException or InvalidOperationException)
            {
                output.ReportDiagnostic(Diagnostic.Create(InvalidSchema, Location.None, error.Message));
            }
        });
    }

    private static void ValidateLanguage(Schema schema, CSharpCompilation compilation)
    {
        if (schema.CSharpLanguageVersion < 14)
            throw new ArgumentException("C# bindings require language version 14 or later.");
        var selected = compilation.LanguageVersion;
        var display = LanguageVersionFacts.ToDisplayString(selected);
        if (int.TryParse(display.Split('.')[0], out int version) && version < schema.CSharpLanguageVersion)
            throw new ArgumentException($"Bindings require C# {schema.CSharpLanguageVersion}, but the compilation uses C# {display}.");
        if (schema.CSharpLanguageVersion >= 15 && schema.Nodes.Values.Any(node => node.Native && node.Kind == "Enum" && !node.ValueEnum)
            && (compilation.GetTypeByMetadataName("System.Runtime.CompilerServices.UnionAttribute") is null
                || compilation.GetTypeByMetadataName("System.Runtime.CompilerServices.IUnion") is null))
            throw new ArgumentException("C# union bindings require the .NET 11 UnionAttribute and IUnion types.");
    }
}
