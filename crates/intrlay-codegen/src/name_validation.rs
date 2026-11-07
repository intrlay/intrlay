//! Detect API collisions before target compilers encounter duplicate declarations.

use std::collections::BTreeMap;

mod reference_scopes;

use intrlay_schema::Language;

use crate::{
    GenerateError,
    api::{Accessor, IndexedAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph, Variant},
    symbols,
};

pub fn validate(generation: &GenerationPlan) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let mut declarations = plan
        .backend
        .reserved_declarations
        .iter()
        .map(|name| ((*name).into(), "generated module infrastructure".into()))
        .collect();
    if plan.backend.language == Language::CSharp {
        claim(
            plan,
            &mut declarations,
            &crate::csharp::binding_name(plan),
            "generated binding class",
            true,
        )?;
    }
    if plan.backend.language == Language::Cpp {
        declarations.insert(
            "INTRLAY_MEMBER".into(),
            "generated member-access macro".into(),
        );
    }
    validate_source_names(plan, &mut declarations)?;
    if plan.backend.language == Language::CSharp {
        let lengths = plan
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Kind::Array { length, .. } if length != 0 => Some(length),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        for length in lengths {
            claim(
                plan,
                &mut declarations,
                &crate::csharp::array_name(plan, length),
                "native inline array",
                true,
            )?;
        }
    }
    let views = &generation.views.types;
    for &id in &generation.views.emitted {
        let node = &views.nodes[id];
        let name = if plan.backend.language == Language::CSharp {
            crate::csharp::view_name(views, id)
        } else {
            node.view_name.clone()
        };
        claim(plan, &mut declarations, &name, "view", true)?;
        if plan.backend.language == Language::CSharp {
            let span = crate::csharp::span_name(&name);
            claim(plan, &mut declarations, &span, "span view", true)?;
            if node.options.generation.setters {
                claim(
                    plan,
                    &mut declarations,
                    &format!("{span}ReadOnly"),
                    "read-only span view",
                    true,
                )?;
                validate_view_members(views, id, &format!("{span}ReadOnly"), true)?;
            }
            validate_view_members(views, id, &span, false)?;
        }
        if plan.backend.language == Language::Cpp
            || (plan.backend.language == Language::CSharp && node.options.generation.setters)
        {
            claim(
                plan,
                &mut declarations,
                &format!("{name}ReadOnly"),
                "read-only view",
                true,
            )?;
            if plan.backend.language == Language::CSharp {
                validate_view_members(views, id, &format!("{name}ReadOnly"), true)?;
            }
        }
        if plan.backend.language == Language::Cpp {
            claim(
                plan,
                &mut declarations,
                &format!("Basic{name}"),
                "view template",
                true,
            )?;
        }
        validate_view_members(views, id, &name, false)?;
    }
    validate_aliases(views, &mut declarations)?;
    validate_helpers(generation, &mut declarations)
}

/// Helpers must use the emitter's final spelling and its actual declaration
/// scope. Source names may be distinct even when their helper names collide.
fn validate_helpers(
    generation: &GenerationPlan,
    declarations: &mut BTreeMap<String, String>,
) -> Result<(), GenerateError> {
    let plan = &generation.encoding.types;
    let views = &generation.views.types;
    if plan.backend.language == Language::Python {
        for (alias, _) in crate::python::private_aliases(generation)? {
            claim(plan, declarations, &alias, "private-name reference", true)?;
        }
    }
    match plan.backend.language {
        Language::Rust => {
            for node in &plan.nodes {
                if let Kind::Enum(variants) = &node.kind
                    && variants.iter().any(|variant| !variant.fields.is_empty())
                {
                    claim(
                        plan,
                        declarations,
                        &format!("{}VariantView", node.name),
                        "variant view",
                        true,
                    )?;
                }
            }
        }
        Language::Cpp => {
            let mut helpers = BTreeMap::new();
            helpers.insert("members".into(), "native member accessor namespace".into());
            for (id, node) in views.nodes.iter().enumerate() {
                if !matches!(node.kind, Kind::Scalar(_) | Kind::Unit | Kind::String) {
                    claim(
                        plan,
                        &mut helpers,
                        &crate::cpp::reflection::name(views, id),
                        "wire descriptor",
                        true,
                    )?;
                }
            }
        }
        language => {
            // C# codecs belong to the binding class; the other backends place
            // them beside source/view declarations in the module.
            let mut binding_members = BTreeMap::new();
            let helpers = if language == Language::CSharp {
                binding_members.insert(
                    crate::csharp::binding_name(plan),
                    "enclosing binding class".into(),
                );
                &mut binding_members
            } else {
                declarations
            };
            for &id in &generation.encoding.encoders {
                for operation in ["measure", "write"] {
                    if operation == "write" || plan.has_payload(id) {
                        claim(
                            plan,
                            helpers,
                            &symbols::codec_function(plan, operation, id)?,
                            "codec helper",
                            true,
                        )?;
                    }
                }
            }
            if language == Language::CSharp {
                for id in 0..plan.nodes.len() {
                    if plan.csharp_native_layout(id) {
                        claim(
                            plan,
                            helpers,
                            &symbols::codec_function(plan, "read_native", id)?,
                            "native reader",
                            true,
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_view_members(
    plan: &TypeGraph,
    id: Id,
    name: &str,
    read_only: bool,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    let mut members = reserved_members(plan, id)?;
    if node.options.generation.setters
        && (!plan.has_payload(id) || plan.backend.language == Language::Rust)
        && !read_only
    {
        claim(
            plan,
            &mut members,
            &symbols::runtime_symbol(plan, "replace")?,
            "in-place value replacement",
            false,
        )?;
    }
    if matches!(node.kind, Kind::Sequence(_))
        && (!plan.backend.language.has_value_types() || plan.backend.language == Language::CSharp)
    {
        for key in ["element_offset", "element_count"] {
            claim(
                plan,
                &mut members,
                &symbols::runtime_symbol(plan, key)?,
                "cached collection layout",
                false,
            )?;
        }
    }
    if matches!(plan.backend.language, Language::CSharp | Language::Dart) {
        members.insert(name.into(), "enclosing view type".into());
    }
    if plan.backend.language == Language::Cpp
        && !matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. })
    {
        claim(
            plan,
            &mut members,
            &format!("Basic{name}"),
            "enclosing view type",
            true,
        )?;
        claim(plan, &mut members, "Byte", "view template parameter", true)?;
        claim(
            plan,
            &mut members,
            &symbols::runtime_symbol(plan, "metadata")?,
            "view metadata alias",
            true,
        )?;
    }
    match &node.kind {
        Kind::Record(fields) => validate_fields(plan, node, fields, None, &mut members, read_only)?,
        Kind::Enum(variants) => {
            validate_enum_fields(plan, node, variants, &mut members, read_only)?;
        }
        Kind::Sequence(element) | Kind::Array { element, .. }
            if plan.backend.language == Language::CSharp
                && (node.options.generation.create || plan.csharp_native_layout(*element)) =>
        {
            validate_collection_input(plan, *element)?;
        }
        Kind::Sequence(element) | Kind::Array { element, .. }
            if !plan.backend.language.has_value_types()
                && plan.folded_field(*element).is_none() =>
        {
            if let Kind::Record(fields) = &plan.nodes[*element].kind {
                validate_indexed_fields(plan, node, &plan.nodes[*element], fields, &mut members)?;
            }
        }
        Kind::Scalar(_) | Kind::Unit | Kind::String | Kind::Optional { .. } | Kind::Boxed(_)
            if !plan.backend.language.has_value_types() =>
        {
            validate_root_value_members(plan, id, &mut members)?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_root_value_members(
    plan: &TypeGraph,
    id: Id,
    members: &mut BTreeMap<String, String>,
) -> Result<(), GenerateError> {
    for (name, role) in Accessor::root(plan, id)?.declarations(false) {
        let role = match role {
            "field accessor" => "value accessor",
            "field setter" => "value setter method",
            other => other,
        };
        claim(plan, members, name, role, false)?;
    }
    Ok(())
}

fn validate_enum_fields(
    plan: &TypeGraph,
    node: &Node,
    variants: &[Variant],
    members: &mut BTreeMap<String, String>,
    read_only: bool,
) -> Result<(), GenerateError> {
    if plan.backend.language != Language::Rust && node.options.generation.getters {
        claim(
            plan,
            members,
            &symbols::runtime_symbol(plan, "tag")?,
            "enum tag accessor",
            false,
        )?;
    }
    for variant in variants {
        let mut input = BTreeMap::new();
        if matches!(
            plan.backend.language,
            Language::TypeScript | Language::Python
        ) {
            input.insert("kind".into(), "variant discriminant".into());
        }
        for field in &variant.fields {
            claim(plan, &mut input, &field.name, "payload field", false)?;
        }
        validate_fields(
            plan,
            node,
            &variant.fields,
            Some(variant),
            members,
            read_only,
        )?;
    }
    Ok(())
}

fn validate_indexed_fields(
    plan: &TypeGraph,
    node: &Node,
    element: &Node,
    fields: &[Field],
    members: &mut BTreeMap<String, String>,
) -> Result<(), GenerateError> {
    for key in ["get", "set", "length"] {
        members.insert(
            symbols::runtime_symbol(plan, key)?,
            "collection infrastructure".into(),
        );
    }
    for field in fields {
        for (name, role) in IndexedAccessors::new(plan, node, element, field).declarations() {
            claim(plan, members, name, role, false)?;
        }
    }
    Ok(())
}

// Only names appearing in this collection's construction signatures share its
// nested ElementAccess scope. Other source types with that name remain usable.
fn validate_collection_input(plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    match node.kind {
        Kind::Record(_) | Kind::Enum(_) if node.name == "ElementAccess" => {
            Err(GenerateError(
                "generated name ElementAccess conflicts between collection infrastructure and native input type; assign an explicit target name".into(),
            ))
        }
        Kind::Optional { value, .. }
        | Kind::Boxed(value)
        | Kind::Sequence(value)
        | Kind::Array { element: value, .. } => validate_collection_input(plan, value),
        _ => Ok(()),
    }
}

fn validate_aliases(
    plan: &TypeGraph,
    declarations: &mut BTreeMap<String, String>,
) -> Result<(), GenerateError> {
    for (alias, id) in &plan.exports {
        let declared = matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_));
        if !declared || *alias != plan.nodes[*id].name {
            claim(plan, declarations, alias, "export alias", true)?;
        }
        if *alias != plan.nodes[*id].name {
            claim(
                plan,
                declarations,
                &format!("{alias}View"),
                "export view alias",
                true,
            )?;
            if plan.backend.language == Language::CSharp {
                let name = format!("{alias}View");
                let span = crate::csharp::span_name(&name);
                claim(plan, declarations, &span, "export span view alias", true)?;
                let read_only = !plan.nodes[*id].options.generation.setters;
                validate_view_members(plan, *id, &name, read_only)?;
                validate_view_members(plan, *id, &span, read_only)?;
            }
        }
    }
    Ok(())
}

fn validate_source_names(
    plan: &TypeGraph,
    declarations: &mut BTreeMap<String, String>,
) -> Result<(), GenerateError> {
    for node in &plan.nodes {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            validate_native_type_name(plan, &node.name)?;
            claim(plan, declarations, &node.name, "source value", true)?;
        }
        if let Kind::Record(fields) = &node.kind
            && plan.folded_node(node).is_none()
        {
            validate_native_fields(plan, &node.name, fields)?;
        }
        if let Kind::Enum(variants) = &node.kind {
            let mut names = BTreeMap::new();
            if plan.backend.language == Language::CSharp {
                names.extend(inherited_members(plan));
                names.insert(
                    symbols::runtime_symbol(plan, "tag")?,
                    "enum discriminant".into(),
                );
                names.insert(node.name.clone(), "enclosing enum type".into());
                for variant in variants {
                    claim(
                        plan,
                        &mut names,
                        &plan.names.method(&format!("from_{}", variant.name), true),
                        "variant factory",
                        false,
                    )?;
                }
            }
            for variant in variants {
                claim(plan, &mut names, &variant.name, "enum variant", true)?;
                if variant.synthetic_payload
                    && matches!(plan.backend.language, Language::Cpp | Language::Rust)
                {
                    continue;
                }
                let payload = format!("{}{}", node.name, variant.name);
                validate_native_type_name(plan, &payload)?;
                validate_native_fields(plan, &payload, &variant.fields)?;
                if matches!(
                    plan.backend.language,
                    Language::Python | Language::Dart | Language::CSharp | Language::Cpp
                ) {
                    claim(plan, declarations, &payload, "variant payload", true)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_native_type_name(plan: &TypeGraph, name: &str) -> Result<(), GenerateError> {
    if plan.backend.language == Language::CSharp
        && matches!(name, "var" | "record" | "scoped" | "required" | "file")
    {
        return Err(GenerateError(format!(
            "{name} is an unsupported C# native type name in generated declarations; assign an explicit target name"
        )));
    }
    Ok(())
}

fn validate_native_fields(
    plan: &TypeGraph,
    enclosing_type: &str,
    fields: &[Field],
) -> Result<(), GenerateError> {
    if !matches!(
        plan.backend.language,
        Language::CSharp | Language::Dart | Language::Cpp
    ) {
        return Ok(());
    }
    let mut members = inherited_members(plan);
    for reference in reference_scopes::native_fields(plan, fields) {
        claim(
            plan,
            &mut members,
            &reference,
            "referenced native type",
            false,
        )?;
    }
    if matches!(plan.backend.language, Language::CSharp | Language::Dart) {
        members.insert(enclosing_type.to_owned(), "enclosing native type".into());
    }
    if plan.backend.language == Language::Cpp {
        members.insert(
            "INTRLAY_MEMBER".into(),
            "generated member-access macro".into(),
        );
    }
    let mut parameters = BTreeMap::new();
    for field in fields {
        claim(plan, &mut members, &field.name, "native field", false)?;
        if plan.backend.language == Language::Dart {
            let parameter = crate::dart::constructor_parameter(&field.name);
            if parameter.is_empty() {
                return Err(GenerateError(format!(
                    "private Dart field {} has no public constructor parameter name; assign an explicit target name",
                    field.name
                )));
            }
            claim(
                plan,
                &mut parameters,
                parameter,
                "constructor parameter",
                false,
            )?;
        }
    }
    Ok(())
}

fn validate_fields(
    plan: &TypeGraph,
    node: &Node,
    fields: &[Field],
    variant: Option<&crate::plan::Variant>,
    members: &mut BTreeMap<String, String>,
    read_only: bool,
) -> Result<(), GenerateError> {
    let mut inputs = BTreeMap::new();
    for field in fields {
        claim(plan, &mut inputs, &field.name, "input field", false)?;
        for (name, role) in Accessor::field(plan, node, field, variant).declarations(read_only) {
            claim(plan, members, name, role, false)?;
        }
    }
    Ok(())
}

fn claim(
    plan: &TypeGraph,
    names: &mut BTreeMap<String, String>,
    name: &str,
    role: &str,
    type_name: bool,
) -> Result<(), GenerateError> {
    // Dart's accessor keywords are also valid ordinary member names, including
    // the default root setter method `set`.
    let permits_keyword = !type_name
        && (plan.backend.language == Language::TypeScript
            || (plan.backend.language == Language::Dart && matches!(name, "get" | "set")));
    let intrinsic_type = type_name
        && plan.backend.language == Language::CSharp
        && crate::csharp::INTRINSIC_KEYWORDS.contains(&name);
    let runtime_macro =
        plan.backend.language == Language::Cpp && crate::cpp::OBJECT_MACROS.contains(&name);
    if intrinsic_type
        || runtime_macro
        || (!permits_keyword
            && plan
                .backend
                .keywords
                .split_whitespace()
                .any(|keyword| name == keyword))
    {
        return Err(GenerateError(format!(
            "{name} is a reserved {:?} identifier; assign an explicit target name",
            plan.backend.language
        )));
    }
    if let Some(previous) = names.insert(name.into(), role.into()) {
        return Err(GenerateError(format!(
            "generated name {name} conflicts between {previous} and {role}; assign an explicit target name"
        )));
    }
    Ok(())
}

fn reserved_members(plan: &TypeGraph, id: Id) -> Result<BTreeMap<String, String>, GenerateError> {
    let node = &plan.nodes[id];
    if plan.backend.language == Language::Cpp {
        return cpp_infrastructure(plan, node);
    }
    if plan.backend.language == Language::TypeScript {
        return typescript_infrastructure(plan, node);
    }
    let mut members = inherited_members(plan);
    for key in [
        "bytes",
        "byte_size",
        "alignment",
        "create",
        "create_in_place",
        "compute_byte_length",
        "storage",
        "offset",
        "buffer",
    ] {
        let name = symbols::runtime_symbol(plan, key)?;
        // Some policies deliberately reuse the same spelling across static
        // and instance scopes; reserve it once against user-defined members.
        members.insert(name, "view infrastructure".into());
    }
    if plan.backend.language == Language::CSharp {
        for key in ["as_span", "to_value", "try_get_native_span"] {
            members.insert(
                symbols::runtime_symbol(plan, key)?,
                "native view access".into(),
            );
        }
    }
    for special in match plan.backend.language {
        Language::Rust => &[
            "as_bytes",
            "as_view",
            "as_view_mut",
            "from_bytes_unchecked",
            "from_parts_unchecked",
            "into_buffer",
            "compute_layout",
            "LAYOUT",
        ][..],
        // These names belong to the imported runtime View, independently of
        // the naming policy used for generated declarations.
        Language::Python => &[
            "__init__",
            "__slots__",
            "_storage",
            "_offset",
            "buffer",
            "_intrlay",
            "_builtins",
            "_typing",
            "_abc",
        ][..],
        Language::TypeScript => &["constructor"][..],
        Language::Cpp => &["INTRLAY_MEMBER"][..],
        _ => &[],
    } {
        members.insert((*special).into(), "view infrastructure".into());
    }
    for reference in reference_scopes::view(plan, id)? {
        claim(
            plan,
            &mut members,
            &reference,
            "referenced type or codec",
            false,
        )?;
    }
    Ok(members)
}

// C++ fields, methods, aliases, and static members share a class scope. Check
// actual declarations rather than silently merging their resolved spellings.
fn cpp_infrastructure(
    plan: &TypeGraph,
    node: &Node,
) -> Result<BTreeMap<String, String>, GenerateError> {
    let mut members = BTreeMap::new();
    let collection = matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. });
    if collection && !crate::cpp::collections::has_custom_names(plan)? {
        return Ok(members);
    }
    let mut keys = vec!["bytes", "byte_size", "alignment"];
    if collection {
        keys.extend(["storage", "metadata", "size_property", "get", "set"]);
        for name in ["Descriptor", "Byte", "Constructible", "extent"] {
            claim(plan, &mut members, name, "collection infrastructure", true)?;
        }
        claim(
            plan,
            &mut members,
            &crate::cpp::collections::facade_name(plan),
            "enclosing collection type",
            true,
        )?;
    } else {
        keys.extend(["buffer", "offset"]);
    }
    if collection || node.options.generation.create {
        keys.extend(["create", "create_in_place", "compute_byte_length"]);
    }
    for key in keys {
        claim(
            plan,
            &mut members,
            &symbols::runtime_symbol(plan, key)?,
            "view infrastructure",
            false,
        )?;
    }
    claim(
        plan,
        &mut members,
        "INTRLAY_MEMBER",
        "generated member-access macro",
        true,
    )?;
    Ok(members)
}

// JavaScript classes have distinct static and instance scopes. Validate each
// before merging reservations for schema-defined instance members.
fn typescript_infrastructure(
    plan: &TypeGraph,
    node: &Node,
) -> Result<BTreeMap<String, String>, GenerateError> {
    let mut instances = BTreeMap::from([("constructor".into(), "view constructor".into())]);
    for key in ["bytes", "storage", "offset"] {
        claim(
            plan,
            &mut instances,
            &symbols::runtime_symbol(plan, key)?,
            "view instance infrastructure",
            false,
        )?;
    }
    let mut statics = BTreeMap::new();
    let mut keys = vec!["byte_size", "alignment"];
    if node.options.generation.create {
        keys.extend(["create", "create_in_place", "compute_byte_length"]);
    }
    for key in keys {
        claim(
            plan,
            &mut statics,
            &symbols::runtime_symbol(plan, key)?,
            "view static infrastructure",
            false,
        )?;
    }
    instances.extend(statics);
    Ok(instances)
}

fn inherited_members(plan: &TypeGraph) -> BTreeMap<String, String> {
    let names: &[&str] = match plan.backend.language {
        // Prefixes used inside generated class bodies cannot be shadowed by
        // members. `convert` is used only by module-level codecs.
        Language::Dart => &[
            "hashCode",
            "runtimeType",
            "toString",
            "noSuchMethod",
            "core",
            "typed",
            "intrlay",
        ],
        Language::CSharp => &[
            "Equals",
            "Finalize",
            "GetHashCode",
            "GetType",
            "MemberwiseClone",
            "ReferenceEquals",
            "ToString",
        ],
        Language::Python => &[
            "__new__",
            "__class__",
            "__dict__",
            "__weakref__",
            "__getattribute__",
            "__getattr__",
            "__setattr__",
            "__delattr__",
            "__init_subclass__",
            "__class_getitem__",
            "__del__",
        ],
        _ => &[],
    };
    names
        .iter()
        .map(|name| ((*name).into(), "inherited object member".into()))
        .collect()
}
