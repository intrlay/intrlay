using System;
using System.Text;
using Microsoft.CodeAnalysis;
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

        context.RegisterSourceOutput(schemas, static (output, schema) =>
        {
            try
            {
                var source = new Emitter(schema).Generate();
                output.AddSource(schema.HintName, SourceText.From(source, Encoding.UTF8));
            }
            catch (Exception error) when (error is ArgumentException or InvalidOperationException)
            {
                output.ReportDiagnostic(Diagnostic.Create(InvalidSchema, Location.None, error.Message));
            }
        });
    }
}
