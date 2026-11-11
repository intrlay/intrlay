//! Ask Clang to resolve adapter aliases, including partial specializations.
//!
//! Probes are appended to an in-memory translation unit. Function bodies are
//! never interpreted: only the compiler-selected `schema_type` enters the IR.

mod exports;

use std::{collections::BTreeMap, fmt::Write, path::Path};

use clang::{Entity, EntityKind, Index, TranslationUnit, Type as CppType, Unsaved};

use crate::FrontendError;

const PROBE_PREFIX: &str = "intrlay_frontend_adapter_query_";

/// A type expression evaluated in the namespace where it is nameable.
/// Query indices remain stable as later passes append dependent queries.
#[derive(PartialEq, Eq)]
pub struct Query {
    pub expression: String,
    namespaces: Vec<Namespace>,
}

#[derive(Clone, PartialEq, Eq)]
struct Namespace {
    name: Option<String>,
    inline: bool,
}

/// The compiler-selected type and an accessible alias for inspecting its children.
#[derive(Clone)]
pub struct ResolvedType<'tu> {
    pub native: CppType<'tu>,
    pub expression: String,
}

/// Keep Clang's actual type identity alongside each resolved schema. Its display
/// name only selects a lookup bucket: distinct local classes can share a name.
pub struct Mapping<'tu> {
    source: CppType<'tu>,
    schema: ResolvedType<'tu>,
}

/// No probes are needed when only the identity adapter template is present.
pub fn has_specializations(root: Entity<'_>) -> bool {
    root.get_children()
        .into_iter()
        .any(|entity| match entity.get_kind() {
            EntityKind::Namespace | EntityKind::LinkageSpec => has_specializations(entity),
            EntityKind::StructDecl
            | EntityKind::ClassDecl
            | EntityKind::ClassTemplatePartialSpecialization => {
                entity.get_name().as_deref() == Some("Adapter")
                    && entity.get_semantic_parent().is_some_and(|parent| {
                        parent.get_kind() == EntityKind::Namespace
                            && parent.get_name().as_deref() == Some("intrlay")
                            && parent
                                .get_semantic_parent()
                                .is_some_and(|scope| scope.get_semantic_parent().is_none())
                    })
            }
            _ => false,
        })
}

pub fn parse<'index>(
    index: &'index Index<'_>,
    path: &Path,
    arguments: &[String],
    original: &str,
    queries: &[Query],
) -> Result<TranslationUnit<'index>, FrontendError> {
    let mut source = original.to_owned();
    for (index, query) in queries.iter().enumerate() {
        source.push('\n');
        for namespace in &query.namespaces {
            let inline = if namespace.inline { "inline " } else { "" };
            let name = namespace.name.as_deref().unwrap_or_default();
            writeln!(source, "{inline}namespace {name} {{").map_err(format_error)?;
        }
        let expression = &query.expression;
        // Clang may expand a public alias into a private canonical type name.
        // C++20 [temp.spec]/6 permits that name as an explicit-instantiation
        // argument. Capture it there, then resolve AdapterSchema normally so
        // access checks inside adapter constraints and definitions still apply.
        // Keep the argument outside the helper namespace to preserve its lookup.
        writeln!(
            source,
            r#"namespace {PROBE_PREFIX}{index}_source {{
struct SourceTag {{}};
auto source_type(SourceTag);
template<class T> struct CaptureSource {{
    friend auto source_type(SourceTag) {{ return ::std::type_identity<T>{{}}; }}
}};
}}
template struct {PROBE_PREFIX}{index}_source::CaptureSource<{expression}>;
namespace {PROBE_PREFIX}{index} {{
using Source = typename decltype({PROBE_PREFIX}{index}_source::source_type({PROBE_PREFIX}{index}_source::SourceTag{{}}))::type;
using Schema = typename ::intrlay::detail::AdapterSchema<::std::remove_cv_t<Source>>::type;
static_assert(sizeof(Schema) > 0, "adapter schema_type must be a complete object type");
}}"#
        )
        .map_err(format_error)?;
        for (depth, namespace) in query.namespaces.iter().enumerate().rev() {
            source.push_str("}\n");
            if let Some(name) = &namespace.name {
                // Qualify named paths to avoid namesakes imported through an
                // anonymous namespace. Inside anonymous scopes, publish one
                // scope at a time so their identity is not lost.
                let prefix = &query.namespaces[..=depth];
                let path = if prefix.iter().all(|scope| scope.name.is_some()) {
                    let names: Vec<_> = prefix
                        .iter()
                        .filter_map(|scope| scope.name.as_deref())
                        .collect();
                    format!("::{}", names.join("::"))
                } else {
                    name.clone()
                };
                writeln!(
                    source,
                    "namespace {PROBE_PREFIX}{index} = {path}::{PROBE_PREFIX}{index};"
                )
                .map_err(format_error)?;
            }
        }
    }
    let unit = index
        .parser(path)
        .arguments(arguments)
        .unsaved(&[Unsaved::new(path, source)])
        .parse()
        .map_err(|error| FrontendError(format!("cannot resolve C++ adapters: {error:?}")))?;
    crate::check_diagnostics(&unit)?;
    Ok(unit)
}

fn format_error(error: std::fmt::Error) -> FrontendError {
    FrontendError(error.to_string())
}

pub fn resolved<'tu>(
    root: Entity<'tu>,
    queries: &[Query],
) -> Result<BTreeMap<String, Vec<Mapping<'tu>>>, FrontendError> {
    let mut probes = BTreeMap::new();
    collect_probes(root, &mut probes)?;
    let mut result: BTreeMap<String, Vec<Mapping<'tu>>> = BTreeMap::new();
    for index in 0..queries.len() {
        let probe = probes
            .get(&index)
            .ok_or_else(|| FrontendError("missing adapter probe namespace".into()))?;
        let aliases: BTreeMap<_, _> = probe
            .get_children()
            .into_iter()
            .filter_map(|entity| {
                Some((
                    entity.get_name()?,
                    entity.get_typedef_underlying_type()?.get_canonical_type(),
                ))
            })
            .collect();
        let source = *aliases
            .get("Source")
            .ok_or_else(|| FrontendError("missing adapter source type".into()))?;
        let schema = *aliases
            .get("Schema")
            .ok_or_else(|| FrontendError("missing adapter schema_type".into()))?;
        result
            .entry(source.get_display_name())
            .or_default()
            .push(Mapping {
                source,
                schema: ResolvedType {
                    native: schema,
                    expression: format!("::{PROBE_PREFIX}{index}::Schema"),
                },
            });
    }
    Ok(result)
}

fn collect_probes<'tu>(
    parent: Entity<'tu>,
    probes: &mut BTreeMap<usize, Entity<'tu>>,
) -> Result<(), FrontendError> {
    for entity in parent.get_children() {
        if entity.get_kind() != EntityKind::Namespace {
            continue;
        }
        if let Some(index) = entity
            .get_name()
            .and_then(|name| name.strip_prefix(PROBE_PREFIX)?.parse::<usize>().ok())
        {
            if probes.insert(index, entity).is_some() {
                return Err(FrontendError("conflicting adapter probe namespace".into()));
            }
        } else {
            collect_probes(entity, probes)?;
        }
    }
    Ok(())
}

/// A single pass's resolved types and newly discovered concrete instantiations.
#[derive(Default)]
pub struct Adapters<'tu> {
    enabled: bool,
    resolved: BTreeMap<String, Vec<Mapping<'tu>>>,
    pub pending: Vec<(CppType<'tu>, Query)>,
}

impl<'tu> Adapters<'tu> {
    pub fn new(enabled: bool, resolved: BTreeMap<String, Vec<Mapping<'tu>>>) -> Self {
        Self {
            enabled,
            resolved,
            pending: Vec::new(),
        }
    }

    pub fn resolve(&mut self, native: CppType<'tu>, expression: &str) -> Option<ResolvedType<'tu>> {
        self.query(
            native,
            Query {
                expression: expression.to_owned(),
                namespaces: Vec::new(),
            },
        )
    }

    pub fn resolve_export(
        &mut self,
        native: CppType<'tu>,
        entity: Entity<'tu>,
    ) -> Result<Option<ResolvedType<'tu>>, FrontendError> {
        // The supported export forms are the same with or without adapters.
        let query = exports::query(entity)?;
        if !self.enabled {
            // Export spellings are only compiled when adapter probes exist.
            return Ok(Some(ResolvedType {
                native: native.get_canonical_type(),
                expression: query.expression,
            }));
        }
        Ok(self.query(native, query))
    }

    fn resolved_type(&self, native: CppType<'tu>) -> Option<ResolvedType<'tu>> {
        let native = native.get_canonical_type();
        self.resolved
            .get(&native.get_display_name())
            .and_then(|bucket| bucket.iter().find(|mapping| mapping.source == native))
            .map(|mapping| mapping.schema.clone())
    }

    fn query(&mut self, native: CppType<'tu>, query: Query) -> Option<ResolvedType<'tu>> {
        let native = native.get_canonical_type();
        if !self.enabled {
            return Some(ResolvedType {
                native,
                expression: query.expression,
            });
        }
        let schema = self.resolved_type(native);
        if schema.is_none() && !self.pending.iter().any(|(source, _)| *source == native) {
            self.pending.push((native, query));
        }
        schema
    }
}
