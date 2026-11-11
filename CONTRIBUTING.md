# Contributing to Intrlay

Intrlay welcomes bug reports, documentation improvements, and changes to its
schema frontends, generator, and runtime packages.

Start with these guides:

- [Design principles](docs/docs/contributing/design-principles.mdx): the goals and boundaries
  that guide implementation decisions.
- [Development](docs/docs/contributing/development.mdx): repository layout, toolchains,
  and checks.
- [Code style](docs/docs/contributing/code-style.mdx): conventions for handwritten and
  generated code.

## Reporting a problem

Include the source declaration, annotations, generation command, target language
and toolchain, and the smallest consumer that demonstrates the problem. Explain
the expected behavior and the observed result. For performance reports, include
the data shape, build settings, measurement method, and allocation or buffer-reuse
assumptions.

## Making a change

Keep the change focused and explain its resulting behavior. Add a regression case
for a bug or a meaningful feature, preferably in an existing fixture family.
Regenerate affected bindings and exercise them in their target language; a text
comparison alone does not establish that generated code works.

C++ frontend reviews follow the supported schema declarations described in the
[C++ guide](docs/docs/languages/cpp.mdx#export-declarations). Valid C++ syntax alone
does not make a missing extraction case a defect. Establish a practical schema
use case before expanding that boundary; prefer concrete export aliases to
source-rewriting machinery for uncommon declaration forms. Buffer safety, type
identity, and the documented encoding contract remain mandatory.

Run the relevant checks while developing and `cargo xtask check` before submitting
a change that affects schemas, generated code, or runtimes. Documentation-only
changes need working links and accurate examples; run `npm --prefix docs run check`
for the documentation site. State which checks ran and any that could not run.

Contributions are licensed under the project's existing Apache-2.0 OR MIT terms.
