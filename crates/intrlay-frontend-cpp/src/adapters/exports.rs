//! Name ordinary namespace declarations and concrete export aliases.
//!
//! Adapter probes refer to names that remain usable after the source is parsed.
//! Out-of-line definitions and template specializations use an annotated alias;
//! the frontend does not reconstruct their declarations or macro expansions.

use clang::{Entity, EntityKind};

use super::{Namespace, Query};
use crate::FrontendError;

pub(super) fn query(entity: Entity<'_>) -> Result<Query, FrontendError> {
    let lexical = lookup_scope(entity.get_lexical_parent());
    let semantic = lookup_scope(entity.get_semantic_parent());
    if lexical != semantic || entity.get_template().is_some() {
        return Err(FrontendError(format!(
            "export {} through an annotated namespace-scope type alias; out-of-line declarations and template specializations are not direct export roots",
            entity.get_name().unwrap_or_default()
        )));
    }

    let keyword = match entity.get_kind() {
        EntityKind::StructDecl => "struct ",
        EntityKind::ClassDecl => "class ",
        EntityKind::UnionDecl => "union ",
        EntityKind::EnumDecl => "enum ",
        _ => "",
    };
    let name = entity
        .get_name()
        .ok_or_else(|| FrontendError("export must have a C++ type name".into()))?;
    let mut namespaces = Vec::new();
    let mut declaration = entity;
    while let Some(parent) = declaration.get_lexical_parent() {
        if parent.get_kind() == EntityKind::Namespace {
            namespaces.push(Namespace {
                name: parent.get_name(),
                inline: parent.is_inline_namespace(),
            });
        }
        declaration = parent;
    }
    namespaces.reverse();

    // Direct members take precedence over names imported through anonymous
    // namespaces. Anonymous scopes themselves require their lexical context.
    let mut prefix = String::new();
    if namespaces.iter().all(|scope| scope.name.is_some()) {
        prefix.push_str("::");
        for scope in &namespaces {
            if let Some(name) = &scope.name {
                prefix.push_str(name);
                prefix.push_str("::");
            }
        }
    }
    Ok(Query {
        expression: format!("{keyword}{prefix}{name}"),
        namespaces,
    })
}

fn lookup_scope(mut parent: Option<Entity<'_>>) -> Option<Entity<'_>> {
    // Linkage specifications do not introduce a C++ name-lookup scope.
    while parent.is_some_and(|entity| entity.get_kind() == EntityKind::LinkageSpec) {
        parent = parent.and_then(|entity| entity.get_lexical_parent());
    }
    parent
}
