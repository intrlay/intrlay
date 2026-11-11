//! Expand inline accessor paths without changing the canonical encoding graph.

use intrlay_schema::{ApiGeneration, Exposure};

use crate::{GenerateError, Visibility};

use super::{Field, Id, Kind, TypeGraph, documentation::Contexts, identifier};

/// API spelling and semantic provenance follow separate paths. The context uses
/// wildcard array indices so shared descriptions are emitted once per group.
struct Path {
    name: String,
    source: String,
    context: String,
}

pub(super) struct Flattened {
    pub fields: Vec<Field>,
    pub contexts: Contexts,
}

pub(super) fn fields(
    plan: &TypeGraph,
    fields: &[Field],
    flatten: bool,
) -> Result<Flattened, GenerateError> {
    let mut result = Flattened {
        fields: Vec::new(),
        contexts: Contexts::default(),
    };
    for field in fields {
        if expands(
            plan,
            field.ty,
            field.options.exposure,
            field.options.explicit_exposure,
            flatten,
        ) {
            let path = Path {
                name: field
                    .explicit_name
                    .as_ref()
                    .unwrap_or(&field.source_name)
                    .clone(),
                source: field.source_path.clone(),
                context: field.source_path.clone(),
            };
            expand(
                plan,
                field,
                &path,
                field.offset,
                field.options.generation,
                &mut result,
            )?;
        } else {
            result.fields.push(field.clone());
        }
    }
    Ok(result)
}

fn expands(plan: &TypeGraph, id: Id, exposure: Exposure, explicit: bool, inherited: bool) -> bool {
    matches!(plan.nodes[id].kind, Kind::Record(_) | Kind::Array { .. })
        && (exposure == Exposure::Flatten || (inherited && !explicit))
}

fn intersect(parent: ApiGeneration, child: ApiGeneration) -> ApiGeneration {
    ApiGeneration {
        getters: parent.getters && child.getters,
        setters: parent.setters && child.setters,
        create: parent.create && child.create,
    }
}

fn expand(
    plan: &TypeGraph,
    field: &Field,
    path: &Path,
    offset: usize,
    generation: ApiGeneration,
    result: &mut Flattened,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[field.ty];
    result.contexts.insert(&path.context, &field.docs);
    result.contexts.insert(
        &format!("{} ({})", path.context, node.source_name),
        &node.docs,
    );
    match &node.kind {
        Kind::Record(fields) => {
            for child in fields {
                let mut inherited = child.clone();
                inherited.options.inherit_helpers(&field.options);
                let child = &inherited;
                let segment = child.explicit_name.as_ref().unwrap_or(&child.source_name);
                let path = Path {
                    name: if node.transparent {
                        path.name.clone()
                    } else {
                        format!("{}_{segment}", path.name)
                    },
                    source: format!("{}.{}", path.source, source_segment(child)),
                    context: format!("{}.{}", path.context, source_segment(child)),
                };
                let generation = intersect(generation, child.options.generation);
                let offset = offset + child.offset;
                if expands(
                    plan,
                    child.ty,
                    child.options.exposure,
                    child.options.explicit_exposure,
                    true,
                ) {
                    expand(plan, child, &path, offset, generation, result)?;
                } else {
                    result
                        .fields
                        .push(leaf(plan, child, &path, offset, generation)?);
                }
            }
        }
        Kind::Array { element, length } => {
            let node = &plan.nodes[*element];
            let generation = intersect(generation, node.options.generation);
            let mut child = field.clone();
            child.ty = *element;
            child.options = node.options.clone();
            child.options.inherit_helpers(&field.options);
            // The array's description belongs to the group, not to each element.
            child.docs.clear();
            for index in 0..*length {
                let path = Path {
                    name: format!("{}_{index}", path.name),
                    source: format!("{}[{index}]", path.source),
                    context: format!("{}[*]", path.context),
                };
                let offset = offset + index * node.layout.size();
                if expands(
                    plan,
                    *element,
                    node.options.exposure,
                    node.options.explicit_exposure,
                    true,
                ) {
                    expand(plan, &child, &path, offset, generation, result)?;
                } else {
                    result
                        .fields
                        .push(leaf(plan, &child, &path, offset, generation)?);
                }
            }
        }
        _ => unreachable!("only inline records and fixed arrays are expanded"),
    }
    Ok(())
}

fn leaf(
    plan: &TypeGraph,
    source: &Field,
    path: &Path,
    offset: usize,
    generation: ApiGeneration,
) -> Result<Field, GenerateError> {
    let mut field = source.clone();
    // Naming applies once to the complete path, including explicit segment names.
    // This avoids repeating policy prefixes on every nested field.
    field.source_name.clone_from(&path.name);
    field.source_path.clone_from(&path.source);
    field.flattened = true;
    field.explicit_name = None;
    field.name = identifier(&plan.names.symbol(
        &path.name,
        plan.backend.input_field_kind,
        Some(Visibility::Public),
        plan.backend.input_field_modifiers,
    ))?;
    field.property_name = identifier(&plan.names.property(
        &path.name,
        !generation.setters || !plan.native_value(field.ty),
    ))?;
    field.offset = offset;
    field.options.generation = generation;
    Ok(field)
}

fn source_segment(field: &Field) -> &str {
    field
        .source_path
        .rsplit('.')
        .next()
        .unwrap_or(&field.source_path)
}
